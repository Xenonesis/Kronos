use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ControlInput {
    pub run_id: String,
    pub surface_generation: u64,
    pub action: ControlAction,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    Main,
    Secondary,
    Guest,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum ControlAction {
    Create { role: Role },
    Reload { role: Role },
    Destroy { role: Role },
    // Internally tagged unit variants ignore extra fields in Serde; empty structs reject them.
    StopListener {},
    StartListener {},
    OpenGate { enabled: bool },
    AckGate { enabled: bool },
    WriterGate { enabled: bool },
    Observe {},
    Exit {},
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderObservation {
    pub provider_pids: Vec<u32>,
    pub provider_starts: u64,
}

#[derive(Default)]
pub(crate) struct ProviderMeasurements {
    observation: ProviderObservation,
    invalid: bool,
}

impl ProviderMeasurements {
    // Historical resumed turn leaders, not a claim that these PIDs are still alive.
    pub(crate) fn record(&mut self, pid: Option<u32>) {
        if self.invalid {
            return;
        }
        let Some(starts) = self
            .observation
            .provider_starts
            .checked_add(1)
            .filter(|value| *value <= 9_007_199_254_740_991)
        else {
            self.invalid = true;
            return;
        };
        self.observation.provider_starts = starts;
        let Some(pid) = pid.filter(|pid| *pid != 0) else {
            self.invalid = true;
            return;
        };
        if self.observation.provider_pids.contains(&pid) {
            return;
        }
        if self.observation.provider_pids.len() == 256 {
            self.invalid = true;
            return;
        }
        self.observation.provider_pids.push(pid);
    }

    pub(crate) fn snapshot(&self) -> Result<ProviderObservation, String> {
        if self.invalid {
            return Err("Incomplete native provider measurements".into());
        }
        Ok(self.observation.clone())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeAcceptanceObservation {
    pub run_id: String,
    pub case_id: super::AcceptanceCase,
    pub role: String,
    pub incarnation: u64,
    #[serde(flatten)]
    pub session: crate::local_transport::SessionObservation,
    #[serde(flatten)]
    pub providers: ProviderObservation,
    pub closed_sessions: Vec<crate::local_transport::SessionObservation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReportInput {
    pub run_id: String,
    pub surface_generation: u64,
    pub report: serde_json::Value,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ControlEffect {
    Gate,
    Window,
    Listener,
    Renderer,
    Observe,
    Exit,
}

pub(crate) fn control_effect(action: &ControlAction) -> ControlEffect {
    match action {
        ControlAction::OpenGate { .. } | ControlAction::WriterGate { .. } => ControlEffect::Gate,
        ControlAction::Create { .. }
        | ControlAction::Reload { .. }
        | ControlAction::Destroy { .. } => ControlEffect::Window,
        ControlAction::StopListener {} | ControlAction::StartListener {} => ControlEffect::Listener,
        ControlAction::AckGate { .. } => ControlEffect::Renderer,
        ControlAction::Observe {} => ControlEffect::Observe,
        ControlAction::Exit {} => ControlEffect::Exit,
    }
}

/// Dispatch a closed scenario command at the renderer's fixed entry. Dispatch is not
/// proof the gate engaged; the renderer's own report is the delivery evidence.
pub(crate) fn acceptance_ack_gate_script(enabled: bool) -> String {
    format!("void window.__KRONOS_IPC_ACCEPTANCE_START__({{type:\"ack-gate\",enabled:{enabled}}})")
}

/// Window label is also the native surface identity, so each role owns one label.
pub(crate) fn surface_label(role: &Role) -> &'static str {
    match role {
        Role::Main => "main",
        Role::Secondary => "secondary",
        Role::Guest => "guest",
    }
}

fn role_profile(role: &Role) -> usize {
    match role {
        Role::Main => 0,
        Role::Secondary => 1,
        Role::Guest => 2,
    }
}

fn register_page_load<'a>(
    builder: tauri::WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle>,
    app: &tauri::AppHandle,
    label: &str,
) -> tauri::WebviewWindowBuilder<'a, tauri::Wry, tauri::AppHandle> {
    let navigation = app
        .state::<std::sync::Arc<crate::local_transport::IpcState>>()
        .inner()
        .clone();
    let label = label.to_owned();
    builder.on_navigation(move |url| navigation.navigate(&label, url)).on_page_load(|window, payload| {
        let state = window.state::<std::sync::Arc<crate::local_transport::IpcState>>();
        match payload.event() {
            tauri::webview::PageLoadEvent::Started => state.register_window(&window),
            tauri::webview::PageLoadEvent::Finished => {
                let dispatched = state.acceptance_bootstrap(&window).and_then(|bootstrap| {
                    // Eval targets the current document; old generations cannot authorize later controls.
                    window.eval(&format!("void window.__KRONOS_IPC_ACCEPTANCE_START__({bootstrap}).then(()=>window.__KRONOS_IPC_ACCEPTANCE_START__({{type:\"run\"}})).catch(()=>console.error('[acceptance] Scenario failed'))"))
                        .map_err(|error| error.to_string())
                });
                if let Err(error) = dispatched { crate::eprintln!("[acceptance] Bootstrap dispatch failed: {error}"); }
            }
        }
    })
}

/// Initial authority cannot come from a control whose caller does not yet exist.
pub(crate) fn start(app: &tauri::AppHandle, preflight: bool) -> Result<(), String> {
    if !preflight {
        create_surface(app, &Role::Main)?;
    }
    Ok(())
}

fn create_surface(app: &tauri::AppHandle, role: &Role) -> Result<(), String> {
    let label = surface_label(role);
    if app.get_webview_window(label).is_some() {
        return Err("Acceptance surface already exists".into());
    }
    let _window = match role {
        // The guest is deliberately an unregistered, untrusted origin with no bridge.
        Role::Guest => {
            let url = url::Url::parse("about:blank").map_err(|error| error.to_string())?;
            let mut builder =
                tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::External(url))
                    .title("Kronos Acceptance Guest")
                    .visible(false)
                    .focused(false)
                    .focusable(false)
                    .inner_size(800.0, 600.0);
            if let Some(directory) = crate::desktop_acceptance::window_data_directory(2) {
                builder = builder.data_directory(directory);
            }
            builder.build().map_err(|error| error.to_string())?
        }
        _ => {
            let bridge = crate::desktop_acceptance::bridge_script()
                .ok_or("Acceptance bridge script unavailable")?;
            let builder = crate::create_desktop_window(
                app,
                label,
                bridge,
                crate::desktop_acceptance::window_data_directory(role_profile(role)),
            )
            // Trusted surfaces only: the owned scenario is the fixed renderer entry.
            .initialization_script(crate::desktop_acceptance::scenario_source())
            .focused(false)
            .focusable(false);
            register_page_load(builder, app, label)
                .build()
                .map_err(|error| error.to_string())?
        }
    };
    Ok(())
}

fn reload_surface(app: &tauri::AppHandle, role: &Role) -> Result<(), String> {
    app.get_webview_window(surface_label(role))
        .ok_or("Acceptance surface is missing")?
        .reload()
        .map_err(|error| error.to_string())
}

fn destroy_surface(app: &tauri::AppHandle, role: &Role) -> Result<(), String> {
    // `destroy` bypasses the hide-on-close interception and releases the surface.
    app.get_webview_window(surface_label(role))
        .ok_or("Acceptance surface is missing")?
        .destroy()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn ipc_acceptance_control(
    input: ControlInput,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, std::sync::Arc<crate::local_transport::IpcState>>,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    match control_effect(&input.action) {
        ControlEffect::Gate => {
            let (kind, enabled) = match &input.action {
                ControlAction::OpenGate { enabled } => {
                    (crate::local_transport::AcceptanceGateKind::Open, *enabled)
                }
                ControlAction::WriterGate { enabled } => (
                    crate::local_transport::AcceptanceGateKind::WriterError,
                    *enabled,
                ),
                _ => return Err("Unknown acceptance gate".into()),
            };
            state.acceptance_gate_for(
                &window,
                &input.run_id,
                input.surface_generation,
                kind,
                enabled,
            )?;
            Ok(serde_json::json!({ "applied": true }))
        }
        ControlEffect::Window => {
            state.acceptance_scope(&window, &input.run_id, input.surface_generation)?;
            let guest_unregistered = match &input.action {
                ControlAction::Create { role } => {
                    create_surface(&app, role)?;
                    if matches!(role, Role::Guest) {
                        let guest = app
                            .get_webview_window("guest")
                            .ok_or("Acceptance guest surface missing")?;
                        if state.authorize_window(&guest).is_ok() {
                            return Err("Acceptance guest acquired native authority".into());
                        }
                        true
                    } else {
                        false
                    }
                }
                ControlAction::Reload { role } => {
                    reload_surface(&app, role)?;
                    false
                }
                ControlAction::Destroy { role } => {
                    destroy_surface(&app, role)?;
                    false
                }
                _ => return Err("Unknown acceptance window action".into()),
            };
            Ok(serde_json::json!({ "applied": true, "guestUnregistered": guest_unregistered }))
        }
        ControlEffect::Listener => {
            state.acceptance_scope(&window, &input.run_id, input.surface_generation)?;
            // Never starts an engine; only an already-ready owned core can toggle its listener.
            crate::daemon::acceptance_set_network(matches!(
                &input.action,
                ControlAction::StartListener {}
            ))
            .await?;
            Ok(serde_json::json!({ "applied": true }))
        }
        ControlEffect::Renderer => {
            state.acceptance_scope(&window, &input.run_id, input.surface_generation)?;
            let enabled = match &input.action {
                ControlAction::AckGate { enabled } => *enabled,
                _ => return Err("Unknown acceptance renderer command".into()),
            };
            window
                .eval(&acceptance_ack_gate_script(enabled))
                .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({ "dispatched": true }))
        }
        ControlEffect::Observe => Ok(serde_json::to_value(state.acceptance_observe(
            &window,
            &input.run_id,
            input.surface_generation,
        )?)
        .map_err(|error| error.to_string())?),
        ControlEffect::Exit => {
            state.acceptance_scope(&window, &input.run_id, input.surface_generation)?;
            app.exit(0);
            Ok(serde_json::json!({ "exiting": true }))
        }
    }
}

#[tauri::command]
pub(crate) async fn ipc_acceptance_report(
    input: ReportInput,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, std::sync::Arc<crate::local_transport::IpcState>>,
) -> Result<serde_json::Value, String> {
    state.inner().acceptance_report_result(
        &window,
        &input.run_id,
        input.surface_generation,
        &input.report,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acceptance_provider_measurements_are_bounded() {
        let mut measurements = ProviderMeasurements::default();
        assert_eq!(measurements.snapshot().unwrap().provider_starts, 0);
        measurements.record(Some(41));
        measurements.record(Some(41));
        let observation = measurements.snapshot().unwrap();
        assert_eq!(
            observation.provider_starts, 2,
            "resumed provider starts were not counted"
        );
        assert_eq!(observation.provider_pids, vec![41]);
        for pid in 42..=296 {
            measurements.record(Some(pid));
        }
        assert_eq!(measurements.snapshot().unwrap().provider_pids.len(), 256);
        measurements.record(Some(297));
        assert!(
            measurements.snapshot().is_err(),
            "overflow silently truncated native evidence"
        );
        for pid in [None, Some(0)] {
            let mut invalid = ProviderMeasurements::default();
            invalid.record(pid);
            assert!(
                invalid.snapshot().is_err(),
                "missing native PID became successful evidence"
            );
        }
        let mut overflow = ProviderMeasurements::default();
        overflow.observation.provider_starts = (1u64 << 53) - 1;
        overflow.record(Some(41));
        assert!(
            overflow.snapshot().is_err(),
            "unsafe JSON integer became successful evidence"
        );
    }

    #[test]
    fn test_acceptance_control_effects_are_closed() {
        use super::ControlAction::*;
        for action in [
            OpenGate { enabled: true },
            OpenGate { enabled: false },
            WriterGate { enabled: true },
        ] {
            assert_eq!(control_effect(&action), ControlEffect::Gate);
        }
        assert_eq!(control_effect(&Observe {}), ControlEffect::Observe);
        assert_eq!(control_effect(&Exit {}), ControlEffect::Exit);
        for action in [StopListener {}, StartListener {}] {
            assert_eq!(control_effect(&action), ControlEffect::Listener);
        }
        for action in [
            Create { role: Role::Main },
            Create { role: Role::Guest },
            Reload {
                role: Role::Secondary,
            },
            Destroy { role: Role::Guest },
        ] {
            assert_eq!(control_effect(&action), ControlEffect::Window);
        }
        assert_eq!(
            control_effect(&AckGate { enabled: true }),
            ControlEffect::Renderer
        );
    }

    #[test]
    fn test_acceptance_ack_gate_script_targets_fixed_scenario_entry() {
        let enabled = acceptance_ack_gate_script(true);
        assert!(enabled.contains("__KRONOS_IPC_ACCEPTANCE_START__"));
        assert!(enabled.contains(r#"{type:"ack-gate",enabled:true}"#));
        assert!(
            !enabled.contains('\n'),
            "acceptance eval must stay one line"
        );
        let disabled = acceptance_ack_gate_script(false);
        assert!(disabled.contains("enabled:false"));
        assert_ne!(enabled, disabled);
    }

    #[test]
    fn test_acceptance_surface_labels_are_distinct() {
        let labels = [
            surface_label(&Role::Main),
            surface_label(&Role::Secondary),
            surface_label(&Role::Guest),
        ];
        assert_eq!(labels, ["main", "secondary", "guest"]);
        assert_eq!(role_profile(&Role::Main), 0);
        assert_eq!(role_profile(&Role::Secondary), 1);
        assert_eq!(role_profile(&Role::Guest), 2);
    }

    #[test]
    fn test_acceptance_report_input_is_closed() {
        let valid = serde_json::json!({"runId":"0123456789abcdef0123456789abcdef","surfaceGeneration":3,"report":{"outcome":"pass"}});
        let parsed: ReportInput = serde_json::from_value(valid.clone()).unwrap();
        assert_eq!(parsed.surface_generation, 3);
        for invalid in [
            serde_json::json!({"runId":"0123456789abcdef0123456789abcdef","surfaceGeneration":3}),
            serde_json::json!({"runId":"0123456789abcdef0123456789abcdef","surfaceGeneration":-1,"report":{}}),
            serde_json::json!({"runId":"0123456789abcdef0123456789abcdef","surfaceGeneration":3,"report":{},"role":"main"}),
            serde_json::json!({"runId":"0123456789abcdef0123456789abcdef","surfaceGeneration":3,"report":{},"control":true}),
        ] {
            assert!(
                serde_json::from_value::<ReportInput>(invalid).is_err(),
                "report input accepted an unknown or missing field"
            );
        }
    }

    #[test]
    fn test_acceptance_control_contract_is_closed() {
        let roles = ["main", "secondary", "guest"];
        let mut actions = vec![];
        for kind in ["create", "reload", "destroy"] {
            for role in roles {
                actions.push(serde_json::json!({"type":kind,"role":role}));
            }
        }
        for kind in ["stop-listener", "start-listener", "observe", "exit"] {
            actions.push(serde_json::json!({"type":kind}));
        }
        for kind in ["open-gate", "ack-gate", "writer-gate"] {
            for enabled in [false, true] {
                actions.push(serde_json::json!({"type":kind,"enabled":enabled}));
            }
        }
        for action in actions {
            let input = serde_json::json!({"runId":"0123456789abcdef0123456789abcdef", "surfaceGeneration":1, "action":action});
            let parsed: ControlInput = serde_json::from_value(input.clone()).unwrap();
            assert_eq!(parsed.run_id, "0123456789abcdef0123456789abcdef");
            assert_eq!(parsed.surface_generation, 1);
            assert_eq!(
                serde_json::to_value(parsed.action).unwrap(),
                input["action"]
            );
        }
        for action in [
            serde_json::json!({"type":"execute", "command":"anything"}),
            serde_json::json!({"type":"create", "role":"unknown"}),
            serde_json::json!({"type":"create", "role":"guest", "url":"https://example.invalid"}),
            serde_json::json!({"type":"reload", "role":"main", "script":"anything"}),
            serde_json::json!({"type":"writer-gate", "enabled":true, "label":"other"}),
            serde_json::json!({"type":"open-gate", "enabled":"true"}),
            serde_json::json!({"type":"exit", "executable":"anything"}),
            serde_json::json!({"type":"observe", "role":"secondary"}),
            serde_json::json!({"type":"start-listener", "port":6767}),
        ] {
            let input = serde_json::json!({"runId":"0123456789abcdef0123456789abcdef", "surfaceGeneration":1, "action":action});
            assert!(
                serde_json::from_value::<ControlInput>(input).is_err(),
                "unrecognized private control input accepted"
            );
        }
        let valid = serde_json::json!({"runId":"0123456789abcdef0123456789abcdef", "surfaceGeneration":1, "action":{"type":"observe"}});
        let mut extra = valid.clone();
        extra["role"] = serde_json::json!("main");
        assert!(serde_json::from_value::<ControlInput>(extra).is_err());
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove("surfaceGeneration");
        assert!(serde_json::from_value::<ControlInput>(missing).is_err());
        let mut negative = valid;
        negative["surfaceGeneration"] = serde_json::json!(-1);
        assert!(serde_json::from_value::<ControlInput>(negative).is_err());
    }
}
