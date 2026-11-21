use crate::kernel::session::{
    session_pair, FrameReceiver, FrameSender, SessionFrame, SessionIo, SessionProfile,
    MAX_FRAME_BYTES,
};
use base64::Engine;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, Weak};

const CHANNEL_MAX_FRAMES: usize = 64;
const CHANNEL_MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_BINARY_BASE64: usize = MAX_FRAME_BYTES.div_ceil(3) * 4;
const RESYNC: &str = "Stream overflow; reconnect and resynchronize";

pub fn managed_transport(engine: &str, setting: Option<&str>) -> Result<&'static str, String> {
    match setting {
        None | Some("websocket") => Ok("websocket"),
        Some("ipc") => Ok(if engine == "rust" { "ipc" } else { "websocket" }),
        Some(_) => Err("Unknown KRONOS_DESKTOP_LOCAL_TRANSPORT; expected ipc or websocket".into()),
    }
}

pub fn credential_matches(
    expected: &str,
    requested: &str,
    listen: &str,
    port: u16,
    transport: &str,
    node: bool,
) -> bool {
    !expected.is_empty()
        && expected == requested
        && (listen == "ipc://embedded" && !node && transport == "ipc"
            || crate::kernel::access::is_managed_target(listen, port))
}

pub fn validate_legacy_target(target: &serde_json::Value, port: u16) -> Result<(), String> {
    let kind = target["transportType"].as_str().unwrap_or("");
    let path = target["transportPath"].as_str().unwrap_or("");
    let supported = if cfg!(windows) {
        kind == "pipe"
            && (path == r"\\.\pipe\kronos-ipc" || path == format!(r"\\.\pipe\kronos-ipc-{port}"))
    } else {
        kind == "socket"
            && (path == "/tmp/kronos-ipc.sock" || path == format!("/tmp/kronos-ipc-{port}.sock"))
    };
    if supported {
        Ok(())
    } else {
        Err("Unsupported desktop transport target; SSH and external pipe/socket targets are not implemented by this bridge".into())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedTransportEvent {
    pub session_id: String,
    pub generation: String,
    pub sequence: u64,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
pub type EventSink = Arc<dyn Fn(EmbeddedTransportEvent) -> Result<(), String> + Send + Sync>;

pub struct IpcState {
    pub registry: Mutex<Registry>,
    channels: Mutex<HashMap<(String, u32), Weak<ChannelRoute>>>,
}
struct ChannelRoute {
    error: Mutex<Option<String>>,
    #[cfg(feature = "ipc-acceptance")]
    gates: Option<Arc<AcceptanceGates>>,
}
struct ScopedChannel {
    state: Weak<IpcState>,
    key: (String, u32),
    route: Arc<ChannelRoute>,
    channel: tauri::ipc::Channel<EmbeddedTransportEvent>,
}
impl Drop for ScopedChannel {
    fn drop(&mut self) {
        if let Some(state) = self.state.upgrade() {
            let mut channels = state.channels.lock().unwrap();
            if channels
                .get(&self.key)
                .is_some_and(|entry| Weak::ptr_eq(entry, &Arc::downgrade(&self.route)))
            {
                channels.remove(&self.key);
            }
        }
    }
}
impl IpcState {
    pub fn new(dev_origin: Option<String>) -> Self {
        Self {
            registry: Mutex::new(Registry::new(dev_origin)),
            channels: Mutex::new(HashMap::new()),
        }
    }
    pub fn scoped_channel(
        self: &Arc<Self>,
        label: &str,
        channel: tauri::ipc::Channel<EmbeddedTransportEvent>,
    ) -> Result<EventSink, String> {
        let key = (label.to_owned(), channel.id());
        let route = Arc::new(ChannelRoute {
            error: Mutex::new(None),
            #[cfg(feature = "ipc-acceptance")]
            gates: self
                .registry
                .lock()
                .unwrap()
                .acceptance_gates
                .get(label)
                .cloned(),
        });
        let mut channels = self.channels.lock().unwrap();
        if channels.get(&key).and_then(Weak::upgrade).is_some() {
            return Err("Channel already assigned".into());
        }
        channels.insert(key.clone(), Arc::downgrade(&route));
        let scoped = ScopedChannel {
            state: Arc::downgrade(self),
            key,
            route,
            channel,
        };
        Ok(Arc::new(move |event| {
            if let Some(error) = scoped.route.error.lock().unwrap().clone() {
                return Err(error);
            }
            scoped
                .channel
                .send(event)
                .map_err(|error| error.to_string())?;
            let result = scoped
                .route
                .error
                .lock()
                .unwrap()
                .clone()
                .map_or(Ok(()), Err);
            result
        }))
    }
    // Avoid Tauri's large-body fetch cache: closed generations cannot purge those entries.
    // Session acknowledgements bound queued direct evaluations instead.
    pub fn intercept(
        &self,
        label: &str,
        callback: u32,
        index: usize,
        body: &tauri::ipc::InvokeResponseBody,
        evaluate: impl FnOnce(String) -> Result<(), String>,
    ) -> bool {
        let route = self
            .channels
            .lock()
            .unwrap()
            .get(&(label.to_owned(), callback))
            .and_then(Weak::upgrade);
        let Some(route) = route else {
            return false;
        };
        #[cfg(feature = "ipc-acceptance")]
        if route.gates.as_ref().is_some_and(|gates| {
            gates
                .writer_error
                .load(std::sync::atomic::Ordering::Acquire)
        }) {
            *route.error.lock().unwrap() = Some("Acceptance owned channel writer gate".into());
            return true;
        }
        let result = match body {
            tauri::ipc::InvokeResponseBody::Json(json) => evaluate(format!(
                "window.__TAURI_INTERNALS__.runCallback({callback}, {{ message: {json}, index: {index} }});"
            )),
            _ => Err("Unsupported embedded channel encoding".into()),
        };
        if let Err(error) = result {
            *route.error.lock().unwrap() = Some(error);
        }
        true
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenEmbeddedInput {
    session_id: String,
    generation: String,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendEmbeddedInput {
    session_id: String,
    generation: String,
    text: Option<String>,
    binary_base64: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AckEmbeddedInput {
    session_id: String,
    generation: String,
    sequence: u64,
}

#[cfg(feature = "ipc-acceptance")]
struct AcceptanceGates {
    open: tokio::sync::watch::Sender<bool>,
    writer_error: std::sync::atomic::AtomicBool,
}

#[cfg(feature = "ipc-acceptance")]
pub(crate) enum AcceptanceGateKind {
    Open,
    WriterError,
}

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn acceptance_initial_open_gate(
    case: Option<&crate::desktop_acceptance::AcceptanceCase>,
) -> bool {
    matches!(
        case,
        Some(crate::desktop_acceptance::AcceptanceCase::TrustReload)
    )
}

impl IpcState {
    #[cfg(feature = "ipc-acceptance")]
    pub(crate) async fn acceptance_wait_open(
        &self,
        owner: &Owner,
        session: &IpcSession,
    ) -> Result<(), String> {
        let mut gate = {
            let registry = self.registry.lock().unwrap();
            registry.current_owner(owner)?;
            registry
                .acceptance_gates
                .get(&owner.label)
                .ok_or("Missing acceptance surface gate")?
                .open
                .subscribe()
        };
        loop {
            let held = *gate.borrow_and_update();
            if !held {
                break;
            }
            tokio::select! {
                changed = gate.changed() => { changed.map_err(|_| "Acceptance open gate detached")?; }
                _ = session.wait_closed() => return Err("Acceptance open detached".into()),
            }
        }
        let current = self.registry.lock().unwrap().session(
            owner,
            &session.session_id,
            &session.generation,
        )?;
        if !std::ptr::eq(current.as_ref(), session) || *session.closed.borrow() {
            return Err("Acceptance open superseded".into());
        }
        Ok(())
    }

    fn caller(&self, window: &tauri::WebviewWindow) -> Result<Owner, String> {
        let url = window.url().map_err(|error| error.to_string())?;
        self.registry
            .lock()
            .unwrap()
            .caller(window.label(), url.as_str())
    }
    pub fn authorize_window(&self, window: &tauri::WebviewWindow) -> Result<(), String> {
        self.caller(window).map(|_| ())
    }
    // Every private effect re-derives the caller and revalidates the captured generation,
    // so a deferred control cannot act on a successor document.
    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_scope(
        &self,
        window: &tauri::WebviewWindow,
        run: &str,
        generation: u64,
    ) -> Result<String, String> {
        let context =
            crate::desktop_acceptance::active().ok_or("Acceptance context unavailable")?;
        if context.run_id != run {
            return Err("Acceptance run mismatch".into());
        }
        if self.acceptance_surface_generation(window)? != generation {
            return Err("Acceptance surface generation mismatch".into());
        }
        Ok(window.label().to_owned())
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_gate_for(
        &self,
        window: &tauri::WebviewWindow,
        run: &str,
        generation: u64,
        kind: AcceptanceGateKind,
        enabled: bool,
    ) -> Result<(), String> {
        self.acceptance_scope(window, run, generation)?;
        let owner = self.caller(window)?;
        self.registry
            .lock()
            .unwrap()
            .acceptance_set_gate(&owner, kind, enabled)
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_bootstrap(
        &self,
        window: &tauri::WebviewWindow,
    ) -> Result<serde_json::Value, String> {
        let context =
            crate::desktop_acceptance::active().ok_or("Acceptance context unavailable")?;
        let url = window.url().map_err(|error| error.to_string())?;
        let registry = self.registry.lock().unwrap();
        let owner = registry.caller(window.label(), url.as_str())?;
        registry.acceptance_bootstrap(&owner, &context.run_id, &context.case_id)
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_surface_generation(
        &self,
        window: &tauri::WebviewWindow,
    ) -> Result<u64, String> {
        let context =
            crate::desktop_acceptance::active().ok_or("Acceptance context unavailable")?;
        let owner = self.caller(window)?;
        self.registry.lock().unwrap().acceptance_authorize(
            &owner,
            &context.run_id,
            &context.run_id,
            owner.incarnation,
        )?;
        Ok(owner.incarnation)
    }
    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_observe(
        &self,
        window: &tauri::WebviewWindow,
        run: &str,
        generation: u64,
    ) -> Result<crate::desktop_acceptance::commands::NativeAcceptanceObservation, String> {
        self.acceptance_scope(window, run, generation)?;
        let context =
            crate::desktop_acceptance::active().ok_or("Acceptance context unavailable")?;
        let owner = self.caller(window)?;
        let providers = crate::desktop_acceptance::provider_observations()?;
        self.registry.lock().unwrap().acceptance_observation(
            &owner,
            &context.run_id,
            &context.case_id,
            providers,
        )
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_report_result(
        &self,
        window: &tauri::WebviewWindow,
        run: &str,
        generation: u64,
        report: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let context =
            crate::desktop_acceptance::active().ok_or("Acceptance context unavailable")?;
        let owner = self.caller(window)?;
        let providers = crate::desktop_acceptance::provider_observations()?;
        // Hold registration stable through capture and persistence, including reload/destroy.
        let mut registry = self.registry.lock().unwrap();
        registry.acceptance_authorize(&owner, &context.run_id, run, generation)?;
        let mut result = registry.acceptance_result(
            &owner,
            &context.run_id,
            generation,
            &context.case_id,
            providers,
            report,
        )?;
        if owner.label == "secondary"
            && context.case_id == crate::desktop_acceptance::AcceptanceCase::TrustReload
        {
            context.write_secondary_result(&result)?;
        }
        // Only main finalizes the run; its trust result joins the exclusive secondary receipt.
        if owner.label == "main" {
            if context.case_id == crate::desktop_acceptance::AcceptanceCase::TrustReload {
                result["secondary"] = context.read_secondary_result()?;
            }
            context.write_result(&result)?;
        }
        Ok(result)
    }
    fn owned_session(
        &self,
        window: &tauri::WebviewWindow,
        id: &str,
        generation: &str,
    ) -> Result<Arc<IpcSession>, String> {
        let owner = self.caller(window)?;
        self.registry
            .lock()
            .unwrap()
            .session(&owner, id, generation)
    }
    pub fn navigate(&self, label: &str, url: &url::Url) -> bool {
        let mut registry = self.registry.lock().unwrap();
        if registry.trusted_url(url.as_str()).is_err() {
            return false;
        }
        registry.destroy_surface(label);
        true
    }
    pub fn register_window(&self, window: &tauri::WebviewWindow) {
        if let Ok(url) = window.url() {
            let _ = self
                .registry
                .lock()
                .unwrap()
                .register_surface(window.label(), url.as_str());
        }
    }
}

#[tauri::command]
pub async fn open_embedded_transport(
    input: OpenEmbeddedInput,
    on_event: tauri::ipc::Channel<EmbeddedTransportEvent>,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<IpcState>>,
) -> Result<(), String> {
    let state = state.inner().clone();
    let owner = state.caller(&window)?;
    let sink = state.scoped_channel(window.label(), on_event)?;
    let (session, server, mut receiver) =
        state
            .registry
            .lock()
            .unwrap()
            .open(&owner, &input.session_id, &input.generation, sink)?;
    let ready = tokio::select! {
        ready = crate::daemon::ensure_embedded_ready() => ready,
        _ = session.wait_closed() => Err("Session closed during engine startup".into()),
    };
    let context = match ready {
        Ok(context) => context,
        Err(error) => {
            session.close(1011, "Embedded engine unavailable");
            state.registry.lock().unwrap().release(&session);
            return Err(error);
        }
    };
    #[cfg(feature = "ipc-acceptance")]
    if let Err(error) = session.acceptance_bind_epoch(context.acceptance_epoch) {
        session.close(1011, "Acceptance core observation unavailable");
        state.registry.lock().unwrap().release(&session);
        return Err(error);
    }
    #[cfg(feature = "ipc-acceptance")]
    if let Err(error) = state.acceptance_wait_open(&owner, &session).await {
        session.close(1001, "Acceptance open detached");
        state.registry.lock().unwrap().release(&session);
        return Err(error);
    }
    if let Err(error) = session.opened() {
        state.registry.lock().unwrap().release(&session);
        return Err(error);
    }
    tauri::async_runtime::spawn(async move {
        let core = tokio::spawn(crate::kernel::session::run_session(context, server));
        loop {
            let frame = tokio::select! {
                frame = receiver.recv() => frame,
                _ = session.wait_closed() => None,
            };
            let Some(frame) = frame else {
                break;
            };
            let terminal = matches!(frame, SessionFrame::Close { .. });
            if session.deliver(frame).is_err() || terminal {
                break;
            }
        }
        session.close(1001, "Embedded session ended");
        // Session teardown never aborts an accepted business operation or the shared engine.
        let _ = core.await;
        state.registry.lock().unwrap().release(&session);
    });
    Ok(())
}

#[tauri::command]
pub async fn send_embedded_transport(
    input: SendEmbeddedInput,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<IpcState>>,
) -> Result<(), String> {
    state
        .owned_session(&window, &input.session_id, &input.generation)?
        .send(input.text, input.binary_base64)
}
#[tauri::command]
pub async fn ack_embedded_transport(
    input: AckEmbeddedInput,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<IpcState>>,
) -> Result<(), String> {
    state
        .owned_session(&window, &input.session_id, &input.generation)?
        .ack(input.sequence)
}
#[tauri::command]
pub async fn close_embedded_transport(
    input: OpenEmbeddedInput,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<IpcState>>,
) -> Result<(), String> {
    let owner = state.caller(&window)?;
    let result = state.registry.lock().unwrap().close(
        &owner,
        &input.session_id,
        &input.generation,
        1000,
        "Closed by desktop client",
    );
    result
}

#[cfg(feature = "ipc-acceptance")]
#[derive(serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RendererObservation {
    pub outcome: ObservationOutcome,
    pub visible_state: Option<String>,
    pub last_received_sequence: Option<u64>,
    pub stale_generation_rejected: Option<bool>,
    pub same_engine_epoch: Option<bool>,
    pub cross_owner_rejected: Option<bool>,
    pub guest_rejected: Option<bool>,
    pub session_id: Option<String>,
    pub generation: Option<String>,
    pub server_info_received: Option<bool>,
    pub request_completed: Option<bool>,
    pub burst_delivered: Option<bool>,
    pub resync_observed: Option<bool>,
    pub reason: Option<String>,
}

#[cfg(feature = "ipc-acceptance")]
#[derive(serde::Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ObservationOutcome {
    Pass,
    Fail,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    label: String,
    incarnation: u64,
}

#[derive(Default)]
struct Delivery {
    sequence: u64,
    acknowledged: u64,
    inflight: VecDeque<(u64, usize)>,
    bytes: usize,
    opened: bool,
    closed: bool,
    #[cfg(feature = "ipc-acceptance")]
    peak_events: usize,
    #[cfg(feature = "ipc-acceptance")]
    peak_bytes: usize,
    #[cfg(feature = "ipc-acceptance")]
    close_code: Option<u16>,
}

#[cfg(feature = "ipc-acceptance")]
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionObservation {
    pub session_id: String,
    pub generation: String,
    pub engine_epoch: Option<u64>,
    pub last_sent_sequence: u64,
    pub last_ack_sequence: u64,
    pub session_queue_peak_frames: usize,
    pub session_queue_peak_decoded_bytes: usize,
    pub channel_peak_events: usize,
    pub channel_peak_serialized_bytes: usize,
    pub channel_current_events: usize,
    pub channel_current_serialized_bytes: usize,
    pub opened: bool,
    pub close_code: Option<u16>,
}

pub struct IpcSession {
    owner: Owner,
    session_id: String,
    generation: String,
    incoming: FrameSender,
    sink: EventSink,
    delivery: Mutex<Delivery>,
    closed: tokio::sync::watch::Sender<bool>,
    #[cfg(feature = "ipc-acceptance")]
    queue_probes: [FrameSender; 2],
    #[cfg(feature = "ipc-acceptance")]
    acceptance_epoch: std::sync::OnceLock<u64>,
}

// Measure escaped JSON without allocating an oversized serialized envelope.
struct ByteCounter(usize);
impl std::io::Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > CHANNEL_MAX_BYTES - self.0 {
            return Err(std::io::Error::other("Channel envelope too large"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl IpcSession {
    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_bind_epoch(&self, epoch: u64) -> Result<(), String> {
        if epoch == 0 || epoch > 9_007_199_254_740_991 {
            return Err("Invalid acceptance core epoch".into());
        }
        self.acceptance_epoch
            .set(epoch)
            .map_err(|_| "Acceptance session core already bound".into())
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_snapshot(&self) -> SessionObservation {
        let delivery = self.delivery.lock().unwrap();
        let first = self.queue_probes[0].acceptance_queue_stats();
        let second = self.queue_probes[1].acceptance_queue_stats();
        SessionObservation {
            session_id: self.session_id.clone(),
            generation: self.generation.clone(),
            engine_epoch: self.acceptance_epoch.get().copied(),
            last_sent_sequence: delivery.sequence,
            last_ack_sequence: delivery.acknowledged,
            session_queue_peak_frames: first.2.max(second.2),
            session_queue_peak_decoded_bytes: first.3.max(second.3),
            channel_peak_events: delivery.peak_events,
            channel_peak_serialized_bytes: delivery.peak_bytes,
            channel_current_events: delivery.inflight.len(),
            channel_current_serialized_bytes: delivery.bytes,
            opened: delivery.opened,
            close_code: delivery.close_code,
        }
    }

    fn event(&self, delivery: &Delivery, kind: &str) -> EmbeddedTransportEvent {
        EmbeddedTransportEvent {
            session_id: self.session_id.clone(),
            generation: self.generation.clone(),
            sequence: delivery.sequence + 1,
            kind: kind.into(),
            text: None,
            binary_base64: None,
            code: None,
            reason: None,
        }
    }

    fn finish(&self, delivery: &mut Delivery, code: u16, reason: &str) {
        if delivery.closed {
            return;
        }
        delivery.closed = true;
        #[cfg(feature = "ipc-acceptance")]
        {
            delivery.close_code = Some(code);
        }
        self.closed.send_replace(true);
        delivery.inflight.clear();
        delivery.bytes = 0;
        self.incoming.terminate(code, reason);
        let mut event = self.event(delivery, "close");
        event.code = Some(code);
        event.reason = Some(reason.chars().take(256).collect());
        delivery.sequence = event.sequence;
        // The original writer failure is returned by its caller; close is best effort once.
        let _ = (self.sink)(event);
    }

    fn emit(&self, delivery: &mut Delivery, event: EmbeddedTransportEvent) -> Result<(), String> {
        let data = event.kind == "message";
        let mut counter = ByteCounter(0);
        let fits = serde_json::to_writer(&mut counter, &event).is_ok();
        if data
            && (!fits
                || delivery.inflight.len() >= CHANNEL_MAX_FRAMES
                || counter.0 > CHANNEL_MAX_BYTES - delivery.bytes)
        {
            self.finish(delivery, 1013, RESYNC);
            return Err(RESYNC.into());
        }
        let sequence = event.sequence;
        if let Err(error) = (self.sink)(event) {
            self.finish(delivery, 1011, "Channel writer failed; reconnect");
            return Err(error);
        }
        delivery.sequence = sequence;
        if data {
            delivery.inflight.push_back((sequence, counter.0));
            delivery.bytes += counter.0;
            #[cfg(feature = "ipc-acceptance")]
            {
                delivery.peak_events = delivery.peak_events.max(delivery.inflight.len());
                delivery.peak_bytes = delivery.peak_bytes.max(delivery.bytes);
            }
        }
        Ok(())
    }

    pub async fn wait_closed(&self) {
        let mut closed = self.closed.subscribe();
        let _ = closed.wait_for(|closed| *closed).await;
    }

    pub fn opened(&self) -> Result<(), String> {
        let mut delivery = self.delivery.lock().unwrap();
        if delivery.closed {
            return Err("Session closed during open".into());
        }
        if delivery.opened {
            return Err("Session already open".into());
        }
        let event = self.event(&delivery, "open");
        self.emit(&mut delivery, event)?;
        delivery.opened = true;
        Ok(())
    }

    pub fn send(&self, text: Option<String>, binary: Option<String>) -> Result<(), String> {
        let mut delivery = self.delivery.lock().unwrap();
        if delivery.closed || !delivery.opened {
            return Err("Session is not open".into());
        }
        let frame = match (text, binary) {
            (Some(text), None) => SessionFrame::Text(text),
            (None, Some(binary)) => {
                if binary.len() > MAX_BINARY_BASE64 {
                    self.finish(&mut delivery, 1009, "Frame too large");
                    return Err("Frame too large".into());
                }
                SessionFrame::Binary(
                    base64::engine::general_purpose::STANDARD
                        .decode(binary)
                        .map_err(|_| "Invalid binary base64")?,
                )
            }
            _ => return Err("Exactly one text or binary payload is required".into()),
        };
        if let Err(error) = self.incoming.try_send(frame) {
            let code = if error == crate::kernel::session::SessionError::FrameTooLarge {
                1009
            } else {
                1013
            };
            self.finish(
                &mut delivery,
                code,
                if code == 1009 {
                    "Frame too large"
                } else {
                    RESYNC
                },
            );
            return Err(error.to_string());
        }
        Ok(())
    }

    pub fn deliver(&self, frame: SessionFrame) -> Result<(), String> {
        let mut delivery = self.delivery.lock().unwrap();
        if delivery.closed || !delivery.opened {
            return Err("Session is not open".into());
        }
        let mut event = self.event(&delivery, "message");
        match frame {
            SessionFrame::Text(text) if text.len() <= MAX_FRAME_BYTES => event.text = Some(text),
            SessionFrame::Binary(bytes) if bytes.len() <= MAX_FRAME_BYTES => {
                event.binary_base64 = Some(base64::engine::general_purpose::STANDARD.encode(bytes));
            }
            SessionFrame::Close { code, reason } => {
                self.finish(&mut delivery, code, &reason);
                return Ok(());
            }
            _ => {
                self.finish(&mut delivery, 1009, "Frame too large");
                return Err("Frame too large".into());
            }
        }
        self.emit(&mut delivery, event)
    }

    pub fn ack(&self, sequence: u64) -> Result<(), String> {
        let mut delivery = self.delivery.lock().unwrap();
        if delivery.closed {
            return Err("Session closed".into());
        }
        if sequence > delivery.sequence {
            return Err("Acknowledgement exceeds sent sequence".into());
        }
        if sequence <= delivery.acknowledged {
            return Ok(());
        }
        delivery.acknowledged = sequence;
        while delivery
            .inflight
            .front()
            .is_some_and(|(sent, _)| *sent <= sequence)
        {
            let released = delivery.inflight.pop_front().unwrap().1;
            delivery.bytes -= released;
        }
        Ok(())
    }

    pub fn close(&self, code: u16, reason: &str) {
        self.finish(&mut self.delivery.lock().unwrap(), code, reason);
    }
}

pub struct Registry {
    surfaces: HashMap<String, (Owner, url::Url)>,
    sessions: HashMap<String, Arc<IpcSession>>,
    next_surface: u64,
    dev_origin: Option<url::Url>,
    #[cfg(feature = "ipc-acceptance")]
    acceptance_gates: HashMap<String, Arc<AcceptanceGates>>,
    #[cfg(feature = "ipc-acceptance")]
    acceptance_closed: HashMap<String, std::collections::VecDeque<(u64, SessionObservation)>>,
}
impl Registry {
    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_bootstrap(
        &self,
        owner: &Owner,
        run: &str,
        case: &crate::desktop_acceptance::AcceptanceCase,
    ) -> Result<serde_json::Value, String> {
        self.acceptance_authorize(owner, run, run, owner.incarnation)?;
        if owner.incarnation > 9_007_199_254_740_991 {
            return Err("Acceptance surface generation exceeds JavaScript safe integer".into());
        }
        let mut bootstrap = serde_json::json!({
            "type": "bootstrap", "runId": run, "caseId": case,
            "role": owner.label, "surfaceGeneration": owner.incarnation
        });
        if case == &crate::desktop_acceptance::AcceptanceCase::TrustReload
            && owner.label == "secondary"
        {
            let main = self
                .surfaces
                .get("main")
                .ok_or("Trust-reload primary surface is missing")?;
            bootstrap["primaryGeneration"] = serde_json::json!(main.0.incarnation);
        }
        Ok(bootstrap)
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_result(
        &mut self,
        owner: &Owner,
        run: &str,
        generation: u64,
        case: &crate::desktop_acceptance::AcceptanceCase,
        providers: crate::desktop_acceptance::commands::ProviderObservation,
        report: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.acceptance_authorize(owner, run, run, generation)?;
        let native = self.acceptance_observation(owner, run, case, providers)?;
        let mut renderer = self.acceptance_stamp_report(owner, report)?;
        if renderer["sessionId"] != native.session.session_id
            || renderer["generation"] != native.session.generation
        {
            return Err("Acceptance result requires the observed session".into());
        }
        renderer["runId"] = serde_json::json!(run);
        renderer["caseId"] = serde_json::json!(case);
        // Only the Node evidence join can certify pass, not a renderer outcome field.
        Ok(serde_json::json!({
            "schemaVersion": 1, "runId": run, "caseId": case,
            "native": native, "renderer": renderer
        }))
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_observation(
        &mut self,
        owner: &Owner,
        run: &str,
        case: &crate::desktop_acceptance::AcceptanceCase,
        providers: crate::desktop_acceptance::commands::ProviderObservation,
    ) -> Result<crate::desktop_acceptance::commands::NativeAcceptanceObservation, String> {
        self.acceptance_authorize(owner, run, run, owner.incarnation)?;
        // A closed session must not make a live resync ambiguous: retire it into bounded history.
        let retired = self
            .sessions
            .values()
            .filter(|session| &session.owner == owner)
            .filter(|session| session.acceptance_snapshot().close_code.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for session in &retired {
            self.record_closed(session);
            self.sessions.remove(&session.session_id);
        }
        let mut live = self
            .sessions
            .values()
            .filter(|session| &session.owner == owner)
            .cloned()
            .collect::<Vec<_>>();
        if live.len() > 1 {
            return Err("Ambiguous acceptance sessions".into());
        }
        // A writer-fault close leaves no live session, so the latest retired generation for this
        // exact surface incarnation stays attributable instead of becoming unobservable.
        let snapshot = match live.pop() {
            Some(session) => session.acceptance_snapshot(),
            None => self
                .acceptance_closed
                .get(&owner.label)
                .and_then(|history| {
                    history
                        .iter()
                        .rev()
                        .find(|(incarnation, _)| *incarnation == owner.incarnation)
                })
                .map(|(_, snapshot)| snapshot.clone())
                .ok_or("No owned acceptance session")?,
        };
        if snapshot.engine_epoch.is_none() {
            return Err("Acceptance session has no bound core".into());
        }
        Ok(
            crate::desktop_acceptance::commands::NativeAcceptanceObservation {
                run_id: run.to_owned(),
                case_id: case.clone(),
                role: owner.label.clone(),
                incarnation: owner.incarnation,
                session: snapshot,
                providers,
                closed_sessions: self
                    .acceptance_closed
                    .get(&owner.label)
                    .map(|history| {
                        history
                            .iter()
                            .filter(|(incarnation, _)| *incarnation == owner.incarnation)
                            .map(|(_, snapshot)| snapshot.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
            },
        )
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_set_gate(
        &self,
        owner: &Owner,
        kind: AcceptanceGateKind,
        enabled: bool,
    ) -> Result<(), String> {
        self.current_owner(owner)?;
        let gates = self
            .acceptance_gates
            .get(&owner.label)
            .ok_or("Missing acceptance surface gate")?;
        match kind {
            AcceptanceGateKind::Open => {
                gates.open.send_replace(enabled);
            }
            AcceptanceGateKind::WriterError => gates
                .writer_error
                .store(enabled, std::sync::atomic::Ordering::Release),
        }
        Ok(())
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_authorize(
        &self,
        owner: &Owner,
        run: &str,
        requested_run: &str,
        generation: u64,
    ) -> Result<(), String> {
        self.current_owner(owner)?;
        if run != requested_run
            || owner.incarnation != generation
            || !matches!(owner.label.as_str(), "main" | "secondary")
        {
            return Err("Acceptance caller/run incarnation mismatch or unprivileged role".into());
        }
        Ok(())
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_stamp_report(
        &self,
        owner: &Owner,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.current_owner(owner)?;
        if serde_json::to_vec(input).map_err(|e| e.to_string())?.len() > 16 * 1024 {
            return Err("Acceptance observation exceeds bound".into());
        }
        let report: RendererObservation =
            serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        match (&report.session_id, &report.generation) {
            (Some(id), Some(generation)) => {
                // The reported generation may already be retired by a close the renderer could not
                // see, so the bounded history is the second, still identity-checked source.
                if self.session(owner, id, generation).is_err()
                    && !self.acceptance_closed.get(&owner.label).is_some_and(|history| {
                        history.iter().any(|(incarnation, snapshot)| {
                            *incarnation == owner.incarnation
                                && &snapshot.session_id == id
                                && &snapshot.generation == generation
                        })
                    })
                {
                    return Err("Unknown session".into());
                }
            }
            (None, None) => {}
            _ => return Err("Incomplete report session identity".into()),
        }
        if !matches!(owner.label.as_str(), "main" | "secondary") {
            return Err("Unknown trusted acceptance role".into());
        }
        let mut stamped = serde_json::to_value(report).map_err(|e| e.to_string())?;
        stamped["role"] = serde_json::json!(owner.label);
        stamped["incarnation"] = serde_json::json!(owner.incarnation);
        Ok(stamped)
    }

    pub fn new(dev_origin: Option<String>) -> Self {
        Self {
            surfaces: HashMap::new(),
            sessions: HashMap::new(),
            next_surface: 0,
            dev_origin: dev_origin.and_then(|value| url::Url::parse(&value).ok()),
            #[cfg(feature = "ipc-acceptance")]
            acceptance_gates: HashMap::new(),
            #[cfg(feature = "ipc-acceptance")]
            acceptance_closed: HashMap::new(),
        }
    }
    fn trusted_url(&self, origin: &str) -> Result<url::Url, String> {
        let parsed = url::Url::parse(origin).map_err(|_| "Invalid surface URL")?;
        let packaged = matches!(parsed.scheme(), "tauri" | "http")
            && parsed.host_str()
                == Some(if parsed.scheme() == "tauri" {
                    "localhost"
                } else {
                    "tauri.localhost"
                })
            && parsed.port().is_none();
        let dev = matches!(parsed.scheme(), "http" | "https")
            && self
                .dev_origin
                .as_ref()
                .is_some_and(|dev| dev.origin() == parsed.origin());
        if !parsed.username().is_empty() || parsed.password().is_some() || !(packaged || dev) {
            return Err("Untrusted desktop surface".into());
        }
        Ok(parsed)
    }
    // Only native trusted-window creation/page-load paths call this, never an IPC command.
    pub fn register_surface(&mut self, label: &str, origin: &str) -> Result<Owner, String> {
        let url = self.trusted_url(origin)?;
        self.destroy_surface(label);
        self.next_surface += 1;
        let owner = Owner {
            label: label.to_owned(),
            incarnation: self.next_surface,
        };
        self.surfaces.insert(label.to_owned(), (owner.clone(), url));
        #[cfg(feature = "ipc-acceptance")]
        let active_case = crate::desktop_acceptance::active().map(|context| &context.case_id);
        #[cfg(feature = "ipc-acceptance")]
        self.acceptance_gates.insert(
            label.to_owned(),
            Arc::new(AcceptanceGates {
                open: tokio::sync::watch::channel(acceptance_initial_open_gate(active_case)).0,
                writer_error: std::sync::atomic::AtomicBool::new(false),
            }),
        );
        Ok(owner)
    }
    pub fn caller(&self, label: &str, origin: &str) -> Result<Owner, String> {
        let url = self.trusted_url(origin)?;
        let (owner, registered) = self
            .surfaces
            .get(label)
            .ok_or("Unregistered desktop surface")?;
        if url.scheme() != registered.scheme()
            || url.host_str() != registered.host_str()
            || url.port_or_known_default() != registered.port_or_known_default()
        {
            return Err("Surface origin changed".into());
        }
        Ok(owner.clone())
    }
    fn current_owner(&self, owner: &Owner) -> Result<(), String> {
        if self.surfaces.get(&owner.label).map(|surface| &surface.0) != Some(owner) {
            return Err("Stale desktop surface".into());
        }
        Ok(())
    }
    pub fn open(
        &mut self,
        owner: &Owner,
        session_id: &str,
        generation: &str,
        sink: EventSink,
    ) -> Result<(Arc<IpcSession>, SessionIo, FrameReceiver), String> {
        self.current_owner(owner)?;
        if session_id.is_empty()
            || generation.is_empty()
            || session_id.len() > 256
            || generation.len() > 256
        {
            return Err("Invalid session identity".into());
        }
        if self.sessions.contains_key(session_id) {
            return Err("Duplicate session open".into());
        }
        let (server, client) = session_pair(SessionProfile::Full);
        #[cfg(feature = "ipc-acceptance")]
        let queue_probes = [server.outgoing.clone(), client.outgoing.clone()];
        let session = Arc::new(IpcSession {
            owner: owner.clone(),
            session_id: session_id.into(),
            generation: generation.into(),
            incoming: client.outgoing,
            sink,
            delivery: Mutex::new(Delivery::default()),
            closed: tokio::sync::watch::channel(false).0,
            #[cfg(feature = "ipc-acceptance")]
            queue_probes,
            #[cfg(feature = "ipc-acceptance")]
            acceptance_epoch: std::sync::OnceLock::new(),
        });
        self.sessions.insert(session_id.into(), session.clone());
        Ok((session, server, client.incoming))
    }
    pub fn close(
        &mut self,
        owner: &Owner,
        session_id: &str,
        generation: &str,
        code: u16,
        reason: &str,
    ) -> Result<(), String> {
        let session = self.session(owner, session_id, generation)?;
        session.close(code, reason);
        self.release(&session);
        Ok(())
    }
    #[cfg(feature = "ipc-acceptance")]
    fn record_closed(&mut self, session: &Arc<IpcSession>) {
        // Bounded native history: a closed session's real peaks/close code stay observable.
        const CLOSED_HISTORY: usize = 8;
        let snapshot = session.acceptance_snapshot();
        let incarnation = session.owner.incarnation;
        let history = self
            .acceptance_closed
            .entry(session.owner.label.clone())
            .or_default();
        if history
            .iter()
            .any(|(_, entry)| entry.session_id == snapshot.session_id)
        {
            return;
        }
        history.push_back((incarnation, snapshot));
        while history.len() > CLOSED_HISTORY {
            history.pop_front();
        }
    }
    pub fn release(&mut self, session: &Arc<IpcSession>) {
        #[cfg(feature = "ipc-acceptance")]
        self.record_closed(session);
        if self
            .sessions
            .get(&session.session_id)
            .is_some_and(|current| Arc::ptr_eq(current, session))
        {
            self.sessions.remove(&session.session_id);
        }
    }
    pub fn destroy_surface(&mut self, label: &str) {
        self.surfaces.remove(label);
        #[cfg(feature = "ipc-acceptance")]
        if let Some(gates) = self.acceptance_gates.remove(label) {
            gates.open.send_replace(false);
        }
        self.sessions.retain(|_, session| {
            if session.owner.label != label {
                return true;
            }
            session.close(1001, "Desktop surface detached");
            false
        });
    }
    pub fn session(
        &self,
        owner: &Owner,
        session_id: &str,
        generation: &str,
    ) -> Result<Arc<IpcSession>, String> {
        self.current_owner(owner)?;
        let session = self.sessions.get(session_id).ok_or("Unknown session")?;
        if &session.owner != owner || session.generation != generation {
            return Err("Session owner or generation mismatch".into());
        }
        Ok(session.clone())
    }
}
