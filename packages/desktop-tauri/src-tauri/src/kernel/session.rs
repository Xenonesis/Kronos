use super::server::{get_crdt_engine, get_provider_push_tx, get_watcher_engine, SessionContext};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, watch, OwnedSemaphorePermit, Semaphore};

pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const QUEUE_MAX_FRAMES: usize = 128;
pub const QUEUE_MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionFrame {
    Text(String),
    Binary(Vec<u8>),
    Close { code: u16, reason: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionProfile {
    Full,
    LegacyLine,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SessionError {
    Closed,
    FrameTooLarge,
    Overflow,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SessionError {}

#[cfg(feature = "ipc-acceptance")]
#[derive(Default)]
struct QueueMetrics {
    frames: usize,
    bytes: usize,
    peak_frames: usize,
    peak_bytes: usize,
}

#[cfg(feature = "ipc-acceptance")]
struct QueueLease {
    metrics: Arc<Mutex<QueueMetrics>>,
    bytes: usize,
}
#[cfg(feature = "ipc-acceptance")]
impl QueueLease {
    fn new(metrics: Arc<Mutex<QueueMetrics>>, bytes: usize) -> Self {
        {
            let mut current = metrics.lock().unwrap();
            current.frames += 1;
            current.bytes += bytes;
            current.peak_frames = current.peak_frames.max(current.frames);
            current.peak_bytes = current.peak_bytes.max(current.bytes);
        }
        Self { metrics, bytes }
    }
}
#[cfg(feature = "ipc-acceptance")]
impl Drop for QueueLease {
    fn drop(&mut self) {
        let mut current = self.metrics.lock().unwrap();
        current.frames -= 1;
        current.bytes -= self.bytes;
    }
}

struct QueuedFrame {
    frame: SessionFrame,
    _bytes: OwnedSemaphorePermit,
    #[cfg(feature = "ipc-acceptance")]
    _measurement: QueueLease,
}
#[derive(Clone)]
struct TerminalFrame {
    frame: SessionFrame,
    discard_pending: bool,
}

#[derive(Clone)]
pub struct FrameSender {
    messages: mpsc::Sender<QueuedFrame>,
    bytes: Arc<Semaphore>,
    terminal: watch::Sender<Option<TerminalFrame>>,
    gate: Arc<Mutex<()>>,
    #[cfg(feature = "ipc-acceptance")]
    metrics: Arc<Mutex<QueueMetrics>>,
}

pub struct FrameReceiver {
    messages: mpsc::Receiver<QueuedFrame>,
    terminal: watch::Receiver<Option<TerminalFrame>>,
    done: bool,
}

impl FrameSender {
    #[cfg(feature = "ipc-acceptance")]
    pub(crate) fn acceptance_queue_stats(&self) -> (usize, usize, usize, usize) {
        let current = self.metrics.lock().unwrap();
        (
            current.frames,
            current.bytes,
            current.peak_frames,
            current.peak_bytes,
        )
    }

    fn finish(&self, frame: SessionFrame, discard_pending: bool) {
        self.terminal.send_if_modified(|current| {
            if current.is_some() {
                return false;
            }
            *current = Some(TerminalFrame {
                frame,
                discard_pending,
            });
            true
        });
    }

    /// Recovery close cannot be blocked behind a full data queue.
    pub fn terminate(&self, code: u16, reason: &str) {
        let _guard = self.gate.lock().unwrap();
        self.finish(
            SessionFrame::Close {
                code,
                reason: reason.to_owned(),
            },
            true,
        );
    }

    pub fn try_send(&self, frame: SessionFrame) -> Result<(), SessionError> {
        let _guard = self.gate.lock().unwrap();
        if self.terminal.borrow().is_some() || self.messages.is_closed() {
            return Err(SessionError::Closed);
        }
        let size = match &frame {
            SessionFrame::Text(text) => text.len(),
            SessionFrame::Binary(bytes) => bytes.len(),
            SessionFrame::Close { .. } => {
                self.finish(frame, false);
                return Ok(());
            }
        };
        if size > MAX_FRAME_BYTES {
            self.finish(
                SessionFrame::Close {
                    code: 1009,
                    reason: "Frame too large".into(),
                },
                true,
            );
            return Err(SessionError::FrameTooLarge);
        }
        let permit = match self.bytes.clone().try_acquire_many_owned(size as u32) {
            Ok(permit) => permit,
            Err(_) => {
                self.finish(
                    SessionFrame::Close {
                        code: 1013,
                        reason: "Stream overflow; reconnect and resynchronize".into(),
                    },
                    true,
                );
                return Err(SessionError::Overflow);
            }
        };
        // Reserve admission before measuring; a rejected 129th frame is not a queue high-water mark.
        match self.messages.try_reserve() {
            Ok(slot) => {
                slot.send(QueuedFrame {
                    frame,
                    _bytes: permit,
                    #[cfg(feature = "ipc-acceptance")]
                    _measurement: QueueLease::new(self.metrics.clone(), size),
                });
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(SessionError::Closed),
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.finish(
                    SessionFrame::Close {
                        code: 1013,
                        reason: "Stream overflow; reconnect and resynchronize".into(),
                    },
                    true,
                );
                Err(SessionError::Overflow)
            }
        }
    }
}

impl FrameReceiver {
    pub async fn recv(&mut self) -> Option<SessionFrame> {
        if self.done {
            return None;
        }
        loop {
            let terminal = self.terminal.borrow().clone();
            if let Some(terminal) = terminal {
                if !terminal.discard_pending {
                    if let Ok(queued) = self.messages.try_recv() {
                        return Some(queued.frame);
                    }
                }
                self.done = true;
                self.messages.close();
                while self.messages.try_recv().is_ok() {}
                return Some(terminal.frame);
            }
            tokio::select! {
                biased;
                changed = self.terminal.changed() => {
                    if changed.is_err() { return self.messages.recv().await.map(|queued| queued.frame); }
                }
                queued = self.messages.recv() => return queued.map(|queued| queued.frame),
            }
        }
    }
}

pub struct SessionIo {
    pub incoming: FrameReceiver,
    pub outgoing: FrameSender,
    pub profile: SessionProfile,
}

fn frame_queue() -> (FrameSender, FrameReceiver) {
    let (sender, receiver) = mpsc::channel(QUEUE_MAX_FRAMES);
    let (terminal_tx, terminal_rx) = watch::channel(None);
    (
        FrameSender {
            messages: sender,
            bytes: Arc::new(Semaphore::new(QUEUE_MAX_BYTES)),
            terminal: terminal_tx,
            gate: Arc::new(Mutex::new(())),
            #[cfg(feature = "ipc-acceptance")]
            metrics: Arc::new(Mutex::new(QueueMetrics::default())),
        },
        FrameReceiver {
            messages: receiver,
            terminal: terminal_rx,
            done: false,
        },
    )
}

pub fn session_pair(profile: SessionProfile) -> (SessionIo, SessionIo) {
    let (to_server, server_in) = frame_queue();
    let (to_client, client_in) = frame_queue();
    (
        SessionIo {
            incoming: server_in,
            outgoing: to_client,
            profile,
        },
        SessionIo {
            incoming: client_in,
            outgoing: to_server,
            profile,
        },
    )
}

pub(crate) async fn process_session_text(
    text: &str,
    terminals: &mut HashMap<String, u8>,
) -> Result<Option<String>, SessionError> {
    let message = serde_json::from_str::<serde_json::Value>(text).ok();
    let inner = message.as_ref().map(|value| {
        if value["type"] == "session" {
            value.get("message").unwrap_or(value)
        } else {
            value
        }
    });
    let mut subscribed_slot = None;
    if let Some(inner) = inner {
        let terminal_id = inner
            .get("terminalId")
            .or_else(|| {
                inner
                    .get("payload")
                    .and_then(|payload| payload.get("terminalId"))
            })
            .and_then(|id| id.as_str());
        if let Some(id) = terminal_id {
            match inner["type"].as_str().unwrap_or("") {
                "subscribe_terminal_request" => {
                    let slot = if let Some(slot) = terminals.get(id) {
                        *slot
                    } else {
                        let slot = (1..=u8::MAX)
                            .find(|candidate| !terminals.values().any(|value| value == candidate))
                            .ok_or(SessionError::Overflow)?;
                        terminals.insert(id.to_owned(), slot);
                        slot
                    };
                    subscribed_slot = Some(slot);
                }
                "unsubscribe_terminal_request" | "kill_terminal_request" => {
                    terminals.remove(id);
                }
                _ => {}
            }
        }
    }
    let reply = super::server::process_incoming_frame(text).await;
    if let (Some(slot), Some(reply)) = (subscribed_slot, reply.as_ref()) {
        let mut value: serde_json::Value =
            serde_json::from_str(reply).expect("serialized session reply");
        let response = if value["type"] == "session" {
            &mut value["message"]
        } else {
            &mut value
        };
        response["payload"]["slot"] = serde_json::json!(slot);
        return Ok(Some(value.to_string()));
    }
    Ok(reply)
}

pub async fn run_session(context: Arc<SessionContext>, io: SessionIo) -> Result<(), SessionError> {
    let outgoing = io.outgoing.clone();
    let result = run_session_inner(context, io).await;
    match result {
        Err(SessionError::Overflow) => {
            outgoing.terminate(1013, "Stream overflow; reconnect and resynchronize")
        }
        Err(SessionError::FrameTooLarge) => outgoing.terminate(1009, "Frame too large"),
        _ => {}
    }
    result
}

async fn run_session_inner(
    context: Arc<SessionContext>,
    mut io: SessionIo,
) -> Result<(), SessionError> {
    let mut shutdown = context.subscribe_shutdown();
    if context.is_stopped() {
        io.outgoing.try_send(SessionFrame::Close {
            code: 1000,
            reason: "Daemon stopped".into(),
        })?;
        return Ok(());
    }
    let first = tokio::select! {
        _ = shutdown.changed() => {
            io.outgoing.try_send(SessionFrame::Close { code:1000, reason:"Daemon stopped".into() })?;
            return Ok(());
        }
        first = tokio::time::timeout(Duration::from_secs(10), io.incoming.recv()) => first,
    };
    let mut announce = io.profile == SessionProfile::LegacyLine;
    let admission = match first {
        Ok(Some(SessionFrame::Text(text))) if text.len() <= 8192 => {
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(hello) => {
                    announce |= hello.get("auth").is_some()
                        || hello["capabilities"]["hello_rejection"].as_bool() == Some(true);
                    context.access.authenticate_hello(&hello).await
                }
                Err(_) => Err("password_required"),
            }
        }
        Ok(Some(frame @ SessionFrame::Close { .. })) => {
            io.outgoing.try_send(frame)?;
            return Ok(());
        }
        Ok(None) => return Ok(()),
        _ => Err("password_required"),
    };
    if let Err(reason) = admission {
        if announce {
            io.outgoing.try_send(SessionFrame::Text(serde_json::json!({"type":"hello.rejected", "reason":reason, "accepts":["password"]}).to_string()))?;
        }
        let (code, message) = match reason {
            "incompatible_protocol" => (4003, "Incompatible protocol version"),
            "incorrect_password" => (4401, "Incorrect password"),
            _ => (4401, "Password required"),
        };
        io.outgoing.try_send(SessionFrame::Close {
            code,
            reason: message.into(),
        })?;
        return Ok(());
    }
    io.outgoing.try_send(SessionFrame::Text(
        super::protocol::build_server_info_response().to_string(),
    ))?;
    let full = io.profile == SessionProfile::Full;
    let mut watcher = full.then(|| get_watcher_engine().subscribe_events());
    let mut crdt = full.then(|| get_crdt_engine().subscribe());
    let mut terminal = full.then(|| crate::native_pty::get_pty_manager().subscribe_output());
    let mut provider = full.then(|| get_provider_push_tx().subscribe());
    let mut watcher_active = full;
    let mut crdt_active = full;
    let mut terminals = HashMap::new();
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                io.outgoing.try_send(SessionFrame::Close { code:1000, reason:"Daemon stopped".into() })?;
                break;
            }
            chunk = async { terminal.as_mut().unwrap().recv().await }, if full => {
                match chunk {
                    Ok(chunk) => if let Some(slot) = terminals.get(&chunk.terminal_id) {
                        let mut frame = Vec::with_capacity(2 + chunk.data.len());
                        frame.extend_from_slice(&[1, *slot]);
                        frame.extend_from_slice(&chunk.data);
                        io.outgoing.try_send(SessionFrame::Binary(frame))?;
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return Err(SessionError::Overflow),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            update = async { provider.as_mut().unwrap().recv().await }, if full => {
                match update {
                    Ok(text) => io.outgoing.try_send(SessionFrame::Text(text))?,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!("Session provider push lagged by {} messages; continuing without dropping connection", skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            events = async { watcher.as_mut().unwrap().recv().await }, if watcher_active => {
                match events {
                    Ok(events) => io.outgoing.try_send(SessionFrame::Text(serde_json::json!({"type":"vfs/change_events", "payload":events}).to_string()))?,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return Err(SessionError::Overflow),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => watcher_active = false,
                }
            }
            event = async { crdt.as_mut().unwrap().recv().await }, if crdt_active => {
                match event {
                    Ok(event) => io.outgoing.try_send(SessionFrame::Text(serde_json::json!({"type":"crdt/update_event", "payload":{
                        "room":event.room, "update":event.update_base64, "sourceId":event.source_id,
                    }}).to_string()))?,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return Err(SessionError::Overflow),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => crdt_active = false,
                }
            }
            frame = io.incoming.recv() => match frame {
                Some(SessionFrame::Text(text)) => {
                    let reply = if full { process_session_text(&text, &mut terminals).await? }
                        else { super::server::process_incoming_frame(text.trim()).await };
                    if let Some(reply) = reply { io.outgoing.try_send(SessionFrame::Text(reply))?; }
                }
                Some(SessionFrame::Binary(bytes)) if full => {
                    if bytes.len() >= 2 && matches!(bytes[0], 2 | 3) {
                        if let Some((id, _)) = terminals.iter().find(|(_, slot)| **slot == bytes[1]) {
                            if bytes[0] == 2 {
                                if let Ok(text) = std::str::from_utf8(&bytes[2..]) { let _ = crate::native_pty::get_pty_manager().write_terminal(id, text); }
                            } else if let Ok(resize) = serde_json::from_slice::<serde_json::Value>(&bytes[2..]) {
                                let rows = resize["rows"].as_u64().unwrap_or(24) as u16;
                                let cols = resize["cols"].as_u64().unwrap_or(80) as u16;
                                let _ = crate::native_pty::get_pty_manager().resize_terminal(id, cols, rows);
                            }
                        }
                    } else if let Ok(text) = std::str::from_utf8(&bytes) {
                        if let Some(reply) = process_session_text(text, &mut terminals).await? { io.outgoing.try_send(SessionFrame::Text(reply))?; }
                    }
                }
                Some(frame @ SessionFrame::Close { .. }) => { io.outgoing.try_send(frame)?; break; }
                Some(SessionFrame::Binary(_)) => {
                    io.outgoing.try_send(SessionFrame::Close { code:1003, reason:"Legacy line RPC requires text".into() })?;
                    break;
                }
                None => break,
            }
        }
    }
    Ok(())
}
