use super::local_transport::*;
use crate::kernel::session::SessionFrame;
use std::sync::{Arc, Mutex};

type Events = Arc<Mutex<Vec<EmbeddedTransportEvent>>>;

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_result_binds_current_native_and_renderer() {
    use crate::desktop_acceptance::{commands::ProviderObservation, AcceptanceCase};
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let run = "0123456789abcdef0123456789abcdef";
    let case = AcceptanceCase::NativeSmoke;
    let report = serde_json::json!({
        "outcome":"pass", "visibleState":"sidebar-visible", "lastReceivedSequence":2,
        "sessionId":"observed", "generation":"one", "serverInfoReceived":true,
        "requestCompleted":true
    });
    let (session, _server, _receiver) = registry
        .open(&owner, "observed", "one", Arc::new(|_| Ok(())))
        .unwrap();
    session.acceptance_bind_epoch(7).unwrap();
    session.opened().unwrap();
    session.deliver(SessionFrame::Text("reply".into())).unwrap();
    session.ack(2).unwrap();
    let result = registry
        .acceptance_result(
            &owner,
            run,
            1,
            &case,
            ProviderObservation::default(),
            &report,
        )
        .expect("Current caller report and actual session must aggregate");
    assert_eq!(result["schemaVersion"], 1);
    assert_eq!(result["runId"], run);
    assert_eq!(result["caseId"], "native-smoke");
    assert!(
        result.get("outcome").is_none(),
        "Native writer must not certify pass"
    );
    assert_eq!(result["native"]["engineEpoch"], 7);
    assert_eq!(result["native"]["lastAckSequence"], 2);
    assert_eq!(result["renderer"]["lastReceivedSequence"], 2);
    for side in ["native", "renderer"] {
        assert_eq!(result[side]["runId"], run);
        assert_eq!(result[side]["caseId"], "native-smoke");
        assert_eq!(result[side]["role"], "main");
        assert_eq!(result[side]["incarnation"], 1);
        assert_eq!(result[side]["sessionId"], "observed");
        assert_eq!(result[side]["generation"], "one");
    }
    let mut aggregate = |report: &serde_json::Value| {
        registry.acceptance_result(
            &owner,
            run,
            1,
            &case,
            ProviderObservation::default(),
            report,
        )
    };
    assert!(
        aggregate(&serde_json::json!({"outcome":"pass"})).is_err(),
        "Missing session accepted"
    );
    let mut spoofed = report.clone();
    spoofed["role"] = serde_json::json!("secondary");
    assert!(
        aggregate(&spoofed).is_err(),
        "Renderer supplied native identity"
    );
    assert!(registry
        .acceptance_result(
            &owner,
            run,
            2,
            &case,
            ProviderObservation::default(),
            &report
        )
        .is_err());
    let secondary = registry
        .register_surface("secondary", "http://tauri.localhost/")
        .unwrap();
    assert!(registry
        .acceptance_result(
            &secondary,
            run,
            2,
            &case,
            ProviderObservation::default(),
            &report
        )
        .is_err());
    registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    assert!(
        registry
            .acceptance_result(
                &owner,
                run,
                1,
                &case,
                ProviderObservation::default(),
                &report
            )
            .is_err(),
        "Stale document aggregated successor result"
    );
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_epoch_follows_core_not_network() {
    let first = crate::kernel::server::SessionContext::initialize().unwrap();
    let epoch = first.acceptance_epoch;
    assert!(epoch > 0, "core epoch was not assigned");
    first.network_failed("owned fixture listener failure".into());
    assert_eq!(first.acceptance_epoch, epoch);
    first.stop();
    let second = crate::kernel::server::SessionContext::initialize().unwrap();
    assert!(
        second.acceptance_epoch > epoch,
        "new core reused previous epoch"
    );
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_observation_is_session_bound() {
    use crate::desktop_acceptance::commands::ProviderObservation;
    use crate::desktop_acceptance::AcceptanceCase;
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let run = "0123456789abcdef0123456789abcdef";
    let case = AcceptanceCase::NativeSmoke;
    assert!(registry
        .acceptance_observation(&owner, run, &case, ProviderObservation::default())
        .is_err());
    let (session, _server, _receiver) = registry
        .open(&owner, "observed", "one", Arc::new(|_| Ok(())))
        .unwrap();
    assert!(
        registry
            .acceptance_observation(&owner, run, &case, ProviderObservation::default())
            .is_err(),
        "unbound session produced core evidence"
    );
    let context = crate::kernel::server::SessionContext::initialize().unwrap();
    session
        .acceptance_bind_epoch(context.acceptance_epoch)
        .unwrap();
    session.opened().unwrap();
    session.send(Some("request".into()), None).unwrap();
    session.deliver(SessionFrame::Text("reply".into())).unwrap();
    session.ack(2).unwrap();
    let observed = registry
        .acceptance_observation(
            &owner,
            run,
            &case,
            ProviderObservation {
                provider_pids: vec![41],
                provider_starts: 1,
            },
        )
        .expect("actual session counters were not observable");
    let value = serde_json::to_value(observed).unwrap();
    assert_eq!(value["runId"], run);
    assert_eq!(value["caseId"], "native-smoke");
    assert_eq!(value["role"], "main");
    assert_eq!(value["incarnation"], 1);
    assert_eq!(value["engineEpoch"], context.acceptance_epoch);
    assert_eq!(value["opened"], true);
    assert_eq!(value["sessionId"], "observed");
    assert_eq!(value["generation"], "one");
    assert_eq!(value["lastSentSequence"], 2);
    assert_eq!(value["lastAckSequence"], 2);
    assert_eq!(value["sessionQueuePeakFrames"], 1);
    assert_eq!(value["sessionQueuePeakDecodedBytes"], 7);
    assert_eq!(value["channelPeakEvents"], 1);
    assert!(value["channelPeakSerializedBytes"].as_u64().unwrap() > 0);
    assert_eq!(value["providerPids"], serde_json::json!([41]));
    assert_eq!(value["providerStarts"], 1);
    assert!(
        session
            .acceptance_bind_epoch(context.acceptance_epoch + 1)
            .is_err(),
        "session relabelled with another engine epoch"
    );
    assert_eq!(
        session.acceptance_snapshot().engine_epoch,
        Some(context.acceptance_epoch)
    );
    let secondary = registry
        .register_surface("secondary", "http://tauri.localhost/")
        .unwrap();
    assert!(registry
        .acceptance_observation(&secondary, run, &case, ProviderObservation::default())
        .is_err());
    let guest = registry
        .register_surface("guest", "http://tauri.localhost/")
        .unwrap();
    assert!(registry
        .acceptance_observation(&guest, run, &case, ProviderObservation::default())
        .is_err());
    let (_other, _server2, _receiver2) = registry
        .open(&owner, "ambiguous", "two", Arc::new(|_| Ok(())))
        .unwrap();
    assert!(
        registry
            .acceptance_observation(&owner, run, &case, ProviderObservation::default())
            .is_err(),
        "ambiguous sessions silently selected"
    );
    registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    assert!(
        registry
            .acceptance_observation(&owner, run, &case, ProviderObservation::default())
            .is_err(),
        "stale owner observed successor"
    );
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_controls_use_actual_caller() {
    let mut registry = Registry::new(None);
    let main = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let secondary = registry
        .register_surface("secondary", "http://tauri.localhost/")
        .unwrap();
    assert!(registry.caller("guest", "http://tauri.localhost/").is_err());
    assert!(
        registry
            .acceptance_authorize(&main, "run", "other-run", 1)
            .is_err(),
        "cross-run control accepted"
    );
    assert!(
        registry
            .acceptance_authorize(&main, "run", "run", 2)
            .is_err(),
        "other native incarnation accepted"
    );
    assert!(registry
        .acceptance_authorize(&main, "run", "run", 1)
        .is_ok());
    assert!(registry
        .acceptance_authorize(&secondary, "run", "run", 2)
        .is_ok());
    let guest = registry
        .register_surface("guest", "http://tauri.localhost/")
        .unwrap();
    assert!(
        registry
            .acceptance_authorize(&guest, "run", "run", 3)
            .is_err(),
        "registered guest acquired private control"
    );
    registry.destroy_surface("main");
    assert!(
        registry
            .acceptance_authorize(&main, "run", "run", 1)
            .is_err(),
        "stale page controlled successor"
    );
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_trust_reload_holds_initial_open_until_scenario_release() {
    use crate::desktop_acceptance::AcceptanceCase;
    assert!(crate::local_transport::acceptance_initial_open_gate(Some(
        &AcceptanceCase::TrustReload
    )));
    assert!(!crate::local_transport::acceptance_initial_open_gate(Some(
        &AcceptanceCase::NativeSmoke
    )));
    assert!(!crate::local_transport::acceptance_initial_open_gate(None));
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_bootstrap_binds_native_surface() {
    use crate::desktop_acceptance::AcceptanceCase;
    let mut registry = Registry::new(None);
    let main = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let case = AcceptanceCase::NativeSmoke;
    assert_eq!(
        registry.acceptance_bootstrap(&main, "run", &case).unwrap(),
        serde_json::json!({
            "type": "bootstrap", "runId": "run", "caseId": "native-smoke", "role": "main", "surfaceGeneration": 1
        })
    );
    let secondary = registry
        .register_surface("secondary", "http://tauri.localhost/")
        .unwrap();
    assert_eq!(
        registry
            .acceptance_bootstrap(&secondary, "run", &case)
            .unwrap()["surfaceGeneration"],
        2
    );
    let mut trust_registry = Registry::new(None);
    trust_registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let trust_secondary = trust_registry
        .register_surface("secondary", "http://tauri.localhost/")
        .unwrap();
    let trust = AcceptanceCase::TrustReload;
    assert_eq!(
        trust_registry
            .acceptance_bootstrap(&trust_secondary, "run", &trust)
            .unwrap()["primaryGeneration"],
        1
    );
    let guest = registry
        .register_surface("guest", "http://tauri.localhost/")
        .unwrap();
    assert!(
        registry.acceptance_bootstrap(&guest, "run", &case).is_err(),
        "Guest acquired bootstrap authority"
    );
    registry.destroy_surface("main");
    assert!(
        registry.acceptance_bootstrap(&main, "run", &case).is_err(),
        "Destroyed page bootstrapped"
    );
    let successor = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    assert!(
        registry.acceptance_bootstrap(&main, "run", &case).is_err(),
        "Stale page acquired successor generation"
    );
    let next = registry
        .acceptance_bootstrap(&successor, "run", &case)
        .unwrap();
    assert_eq!(next["surfaceGeneration"], 4);
    assert!(registry
        .acceptance_authorize(&successor, "run", "run", 1)
        .is_err());
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_reports_cannot_impersonate_window() {
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    assert!(
        registry
            .acceptance_stamp_report(
                &owner,
                &serde_json::json!({"role":"secondary", "outcome":"pass"})
            )
            .is_err(),
        "renderer supplied native role"
    );
    assert!(
        registry
            .acceptance_stamp_report(
                &owner,
                &serde_json::json!({"action":"stop-network", "outcome":"pass"})
            )
            .is_err(),
        "report smuggled control"
    );
    assert!(registry
        .acceptance_stamp_report(
            &owner,
            &serde_json::json!({"visibleState":"x".repeat(16384), "outcome":"pass"})
        )
        .is_err());
    let report = registry.acceptance_stamp_report(&owner, &serde_json::json!({"visibleState":"ready", "outcome":"pass", "lastReceivedSequence":2})).unwrap();
    assert_eq!(report["role"], "main");
    assert_eq!(report["incarnation"], 1);
    registry.destroy_surface("main");
    assert!(registry
        .acceptance_stamp_report(&owner, &serde_json::json!({"outcome":"pass"}))
        .is_err());
}

#[cfg(feature = "ipc-acceptance")]
#[tokio::test]
async fn test_acceptance_open_gate_releases_only_current_owner() {
    let state = Arc::new(IpcState::new(None));
    let (owner, session) = {
        let mut registry = state.registry.lock().unwrap();
        let owner = registry
            .register_surface("main", "http://tauri.localhost/")
            .unwrap();
        registry
            .acceptance_set_gate(&owner, AcceptanceGateKind::Open, true)
            .unwrap();
        let (session, _, _) = registry
            .open(&owner, "held", "old", Arc::new(|_| Ok(())))
            .unwrap();
        (owner, session)
    };
    let waiting = state.acceptance_wait_open(&owner, &session);
    tokio::pin!(waiting);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut waiting)
            .await
            .is_err(),
        "open bypassed held owner gate"
    );
    let (next_owner, next_session) = {
        let mut registry = state.registry.lock().unwrap();
        let next_owner = registry
            .register_surface("main", "http://tauri.localhost/")
            .unwrap();
        assert!(
            registry
                .acceptance_set_gate(&owner, AcceptanceGateKind::Open, false)
                .is_err(),
            "old document released successor gate"
        );
        let (session, _, _) = registry
            .open(&next_owner, "new", "next", Arc::new(|_| Ok(())))
            .unwrap();
        (next_owner, session)
    };
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), &mut waiting)
            .await
            .unwrap()
            .is_err(),
        "detached open resumed successfully"
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        state.acceptance_wait_open(&next_owner, &next_session),
    )
    .await
    .unwrap()
    .unwrap();
    state
        .registry
        .lock()
        .unwrap()
        .acceptance_set_gate(&next_owner, AcceptanceGateKind::Open, true)
        .unwrap();
    let released = state.acceptance_wait_open(&next_owner, &next_session);
    tokio::pin!(released);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut released)
            .await
            .is_err()
    );
    state
        .registry
        .lock()
        .unwrap()
        .acceptance_set_gate(&next_owner, AcceptanceGateKind::Open, false)
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), &mut released)
        .await
        .unwrap()
        .unwrap();
    next_session.opened().unwrap();
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_writer_gate_is_owner_scoped_at_evaluator() {
    let state = Arc::new(IpcState::new(None));
    let owner = state
        .registry
        .lock()
        .unwrap()
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    state
        .registry
        .lock()
        .unwrap()
        .register_surface("secondary", "http://tauri.localhost/")
        .unwrap();
    let first = tauri::ipc::Channel::<EmbeddedTransportEvent>::new(|_| Ok(()));
    let first_id = first.id();
    let _first_output = state.scoped_channel("main", first).unwrap();
    let second = tauri::ipc::Channel::<EmbeddedTransportEvent>::new(|_| Ok(()));
    let second_id = second.id();
    let _second_output = state.scoped_channel("secondary", second).unwrap();
    state
        .registry
        .lock()
        .unwrap()
        .acceptance_set_gate(&owner, AcceptanceGateKind::WriterError, true)
        .unwrap();
    let body = tauri::ipc::InvokeResponseBody::Json("{}".into());
    let mut evaluated = false;
    assert!(state.intercept("main", first_id, 0, &body, |_| {
        evaluated = true;
        Ok(())
    }));
    assert!(
        !evaluated,
        "owned writer-error gate evaluated a real channel frame"
    );
    assert!(state.intercept("secondary", second_id, 0, &body, |_| {
        evaluated = true;
        Ok(())
    }));
    assert!(evaluated, "main gate leaked into secondary evaluator");
    state
        .registry
        .lock()
        .unwrap()
        .acceptance_set_gate(&owner, AcceptanceGateKind::WriterError, false)
        .unwrap();
    let recovered = tauri::ipc::Channel::<EmbeddedTransportEvent>::new(|_| Ok(()));
    let recovered_id = recovered.id();
    let _recovered_output = state.scoped_channel("main", recovered).unwrap();
    let mut recovered_evaluated = false;
    assert!(state.intercept("main", recovered_id, 0, &body, |_| {
        recovered_evaluated = true;
        Ok(())
    }));
    assert!(
        recovered_evaluated,
        "released writer gate blocked a fresh route for current owner"
    );
    state
        .registry
        .lock()
        .unwrap()
        .acceptance_set_gate(&owner, AcceptanceGateKind::WriterError, true)
        .unwrap();
    state
        .registry
        .lock()
        .unwrap()
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    assert!(state
        .registry
        .lock()
        .unwrap()
        .acceptance_set_gate(&owner, AcceptanceGateKind::WriterError, false)
        .is_err());
    let next = tauri::ipc::Channel::<EmbeddedTransportEvent>::new(|_| Ok(()));
    let next_id = next.id();
    let _next_output = state.scoped_channel("main", next).unwrap();
    let mut next_evaluated = false;
    assert!(state.intercept("main", next_id, 0, &body, |_| {
        next_evaluated = true;
        Ok(())
    }));
    assert!(next_evaluated, "stale writer gate affected successor");
}

#[tokio::test]
async fn test_managed_status_never_exports_credentials() {
    if !crate::daemon::tests::lifecycle_child(
        "local_transport_test::test_managed_status_never_exports_credentials",
    )
    .await
    {
        return;
    }
    let status: serde_json::Value = serde_json::from_str(&crate::get_daemon_status_json()).unwrap();
    assert!(
        status.get("localCredential").is_none(),
        "status exported credential field"
    );
}

#[test]
fn test_managed_transport_setting_and_identity() {
    for (engine, setting, expected) in [
        ("rust", None, "websocket"),
        ("rust", Some("ipc"), "ipc"),
        ("rust", Some("websocket"), "websocket"),
        ("node", Some("ipc"), "websocket"),
    ] {
        assert_eq!(managed_transport(engine, setting).unwrap(), expected);
    }
    for setting in ["", "auto", "IPC"] {
        assert!(managed_transport("rust", Some(setting)).is_err());
    }
    assert!(credential_matches(
        "managed",
        "managed",
        "ipc://embedded",
        0,
        "ipc",
        false
    ));
    assert!(!credential_matches(
        "managed",
        "other",
        "ipc://embedded",
        0,
        "ipc",
        false
    ));
    assert!(!credential_matches(
        "managed",
        "managed",
        "ipc://embedded",
        0,
        "ipc",
        true
    ));
    assert!(!credential_matches(
        "managed",
        "managed",
        "ipc://embedded",
        0,
        "websocket",
        false
    ));
    assert!(!credential_matches(
        "managed",
        "other",
        "localhost:19117",
        19117,
        "websocket",
        false
    ));
}

#[test]
fn test_managed_credential_target_is_current_endpoint() {
    assert!(crate::kernel::access::is_managed_target(
        "localhost:19117",
        19117
    ));
    for target in [
        "localhost:6767",
        "localhost:19118",
        "remote.example:19117",
        "ws://user@localhost:19117",
        "ws://localhost:19117/ws?other=1",
    ] {
        assert!(
            !crate::kernel::access::is_managed_target(target, 19117),
            "credential target accepted: {target}"
        );
    }
    assert!(!crate::kernel::access::is_managed_target(
        "localhost:6767",
        0
    ));
    let canonical = if cfg!(windows) {
        r"pipe://\\.\pipe\kronos-ipc"
    } else {
        "unix:///tmp/kronos-ipc.sock"
    };
    assert!(
        !crate::kernel::access::is_managed_target(canonical, 0),
        "legacy bridge cannot authenticate without its actual network listener"
    );
}

#[cfg(feature = "ipc-acceptance")]
#[test]
fn test_acceptance_channel_peaks_release_on_ack_and_close() {
    let (_, _, session, _server, _receiver, _) = fixture();
    session.opened().unwrap();
    session.deliver(SessionFrame::Text("one".into())).unwrap();
    session.deliver(SessionFrame::Text("two".into())).unwrap();
    let stats = session.acceptance_snapshot();
    assert_eq!(
        stats.channel_peak_events, 2,
        "unacknowledged events not measured"
    );
    assert_eq!(stats.channel_current_events, 2);
    assert!(stats.channel_peak_serialized_bytes > 6);
    assert_eq!(stats.last_sent_sequence, 3);
    session.ack(2).unwrap();
    let stats = session.acceptance_snapshot();
    assert_eq!(stats.last_ack_sequence, 2);
    assert_eq!(stats.channel_current_events, 1);
    assert_eq!(stats.channel_peak_events, 2);
    session.close(1013, "resynchronize");
    let stats = session.acceptance_snapshot();
    assert_eq!(stats.channel_current_events, 0);
    assert_eq!(stats.channel_current_serialized_bytes, 0);
    assert_eq!(stats.close_code, Some(1013));
    assert_eq!(stats.channel_peak_events, 2);
}

fn fixture() -> (
    Registry,
    Owner,
    Arc<IpcSession>,
    crate::kernel::session::SessionIo,
    crate::kernel::session::FrameReceiver,
    Events,
) {
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let (output, events) = sink();
    let (session, server, receiver) = registry.open(&owner, "session", "one", output).unwrap();
    (registry, owner, session, server, receiver, events)
}

fn sink() -> (EventSink, Events) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let output = events.clone();
    (
        Arc::new(move |event| {
            output.lock().unwrap().push(event);
            Ok(())
        }),
        events,
    )
}

// Removing actual-owner comparison would let another registered window control this session.
#[test]
fn test_ipc_cross_owner_commands_rejected() {
    let mut registry = Registry::new(None);
    let first = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let second = registry
        .register_surface("chat", "http://tauri.localhost/chat")
        .unwrap();
    let (output, _) = sink();
    let (_session, _server, _receiver) = registry.open(&first, "session", "one", output).unwrap();
    assert!(registry.session(&first, "session", "one").is_ok());
    assert!(
        registry.session(&second, "session", "one").is_err(),
        "foreign caller obtained session control"
    );
}

// Ignoring generation lets a late close/ack touch a replacement sharing the session ID.
#[test]
fn test_ipc_stale_generation_rejected() {
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "tauri://localhost/")
        .unwrap();
    let (output, _) = sink();
    let (_session, _server, _receiver) =
        registry.open(&owner, "session", "current", output).unwrap();
    assert!(registry.session(&owner, "session", "current").is_ok());
    assert!(
        registry.session(&owner, "session", "old").is_err(),
        "old generation obtained session control"
    );
    registry
        .register_surface("main", "tauri://localhost/")
        .unwrap();
    assert!(
        registry.session(&owner, "session", "current").is_err(),
        "replaced surface retained control"
    );
}

// Prefix/URL-only trust would authorize guest windows or lookalike origins.
#[test]
fn test_ipc_untrusted_surface_rejected() {
    let mut registry = Registry::new(Some("http://localhost:1420/".into()));
    registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    assert!(registry
        .caller("main", "http://tauri.localhost/workspace")
        .is_ok());
    for origin in [
        "https://example.com/",
        "http://tauri.localhost.evil/",
        "http://tauri.localhost:1234/",
        "http://user@tauri.localhost/",
    ] {
        assert!(registry.caller("main", origin).is_err(), "trusted {origin}");
    }
    assert!(registry
        .caller("chat-forged", "http://tauri.localhost/")
        .is_err());
    assert!(registry
        .register_surface("guest", "https://example.com/")
        .is_err());
    registry
        .register_surface("dev", "http://localhost:1420/")
        .unwrap();
    assert!(registry.caller("dev", "http://localhost:1421/").is_err());
}

#[test]
fn test_ipc_duplicate_open_rejected() {
    let (mut registry, owner, _session, _server, _receiver, _) = fixture();
    let (output, _) = sink();
    assert!(
        registry.open(&owner, "session", "two", output).is_err(),
        "duplicate replaced active session"
    );
}

#[test]
fn test_ipc_open_close_race() {
    let (mut registry, owner, session, _server, _receiver, events) = fixture();
    registry
        .close(&owner, "session", "one", 1000, "closed during open")
        .unwrap();
    assert!(
        session.opened().is_err(),
        "late open resurrected closed session"
    );
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["close"]
    );
    let (output, _) = sink();
    let (replacement, _server2, _receiver2) =
        registry.open(&owner, "session", "two", output).unwrap();
    registry.release(&session);
    assert!(Arc::ptr_eq(
        &replacement,
        &registry.session(&owner, "session", "two").unwrap()
    ));
    assert!(registry
        .close(&owner, "session", "one", 1000, "late")
        .is_err());
}

#[test]
fn test_ipc_close_emitted_once() {
    let (_, _, session, _server, _receiver, events) = fixture();
    session.opened().unwrap();
    session.close(1000, "done");
    session.close(1011, "late writer error");
    assert!(session.deliver(SessionFrame::Text("late".into())).is_err());
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["open", "close"]
    );
    assert_eq!(events[1].sequence, 2);
    assert_eq!(events[1].code, Some(1000));
}

#[test]
fn test_ipc_writer_failure_surfaces() {
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let (session, _server, _receiver) = registry
        .open(
            &owner,
            "session",
            "one",
            Arc::new(|_| Err("channel writer failed".into())),
        )
        .unwrap();
    assert!(session.opened().is_err(), "channel error swallowed");
    assert!(session.send(Some("never accepted".into()), None).is_err());
}

#[test]
fn test_ipc_window_destroy_releases_session() {
    let (mut registry, owner, session, _server, _receiver, events) = fixture();
    session.opened().unwrap();
    let second = registry
        .register_surface("chat", "http://tauri.localhost/")
        .unwrap();
    let (output, _) = sink();
    let (_other, _server2, _receiver2) = registry.open(&second, "other", "two", output).unwrap();
    registry.destroy_surface("main");
    assert!(
        registry.caller("main", "http://tauri.localhost/").is_err(),
        "destroyed window remains authorized"
    );
    assert!(registry.session(&owner, "session", "one").is_err());
    assert!(session.send(Some("late".into()), None).is_err());
    assert!(registry.session(&second, "other", "two").is_ok());
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.kind == "close")
            .count(),
        1
    );
}

#[test]
fn test_ipc_queue_byte_and_frame_caps() {
    let (_, _, session, _server, _receiver, _) = fixture();
    session.opened().unwrap();
    for _ in 0..128 {
        session.send(Some(String::new()), None).unwrap();
    }
    assert!(
        session.send(Some(String::new()), None).is_err(),
        "129th queued frame accepted"
    );
    let (_, _, session, _server, _receiver, _) = fixture();
    session.opened().unwrap();
    for _ in 0..2 {
        session
            .send(Some("x".repeat(16 * 1024 * 1024)), None)
            .unwrap();
    }
    assert!(
        session.send(Some("x".into()), None).is_err(),
        "queued byte budget exceeded"
    );
}

#[test]
fn test_ipc_channel_inflight_caps() {
    let (_, _, session, _server, _receiver, _) = fixture();
    session.opened().unwrap();
    for _ in 0..64 {
        session.deliver(SessionFrame::Text(String::new())).unwrap();
    }
    assert!(
        session.deliver(SessionFrame::Text(String::new())).is_err(),
        "65th unacknowledged event accepted"
    );
    let (_, _, session, _server, _receiver, _) = fixture();
    session.opened().unwrap();
    session
        .deliver(SessionFrame::Binary(vec![7; 16 * 1024 * 1024]))
        .unwrap();
    assert!(
        session
            .deliver(SessionFrame::Binary(vec![7; 16 * 1024 * 1024]))
            .is_err(),
        "serialized channel byte budget exceeded"
    );
}

#[test]
fn test_ipc_ack_cannot_release_unsent_frames() {
    let (_, _, session, _server, _receiver, _) = fixture();
    session.opened().unwrap();
    session.deliver(SessionFrame::Text("first".into())).unwrap();
    assert!(session.ack(3).is_err(), "acknowledged unsent sequence");
    session.ack(2).unwrap();
    session.ack(1).unwrap(); // duplicate old acknowledgements release nothing
    for _ in 0..64 {
        session.deliver(SessionFrame::Text(String::new())).unwrap();
    }
    assert!(session.ack(67).is_err());
    assert!(
        session.deliver(SessionFrame::Text(String::new())).is_err(),
        "invalid ack released capacity"
    );
}

#[test]
fn test_ipc_overflow_requires_resync() {
    let (_, _, session, _server, _receiver, events) = fixture();
    session.opened().unwrap();
    for _ in 0..64 {
        session
            .deliver(SessionFrame::Text("ordered".into()))
            .unwrap();
    }
    assert!(session
        .deliver(SessionFrame::Text("not delivered".into()))
        .is_err());
    session.close(1000, "later");
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "message")
            .count(),
        64
    );
    let terminal = events.last().unwrap();
    assert_eq!(terminal.kind, "close");
    assert_eq!(terminal.code, Some(1013));
    assert!(terminal.reason.as_ref().unwrap().contains("resynchron"));
    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        (1..=66).collect::<Vec<_>>()
    );
}

#[test]
fn test_ipc_unsupported_legacy_targets_rejected() {
    assert!(validate_legacy_target(
        &serde_json::json!({"transportType":"ssh", "host":"example.com"}),
        43210
    )
    .is_err());
    assert!(validate_legacy_target(
        &serde_json::json!({"transportType":"socket", "transportPath":"/tmp/other.sock"}),
        43210
    )
    .is_err());
    assert!(validate_legacy_target(
        &serde_json::json!({"transportType":"pipe", "transportPath":r"\\.\pipe\other"}),
        43210
    )
    .is_err());
    let target = if cfg!(windows) {
        serde_json::json!({"transportType":"pipe", "transportPath":r"\\.\pipe\kronos-ipc"})
    } else {
        serde_json::json!({"transportType":"socket", "transportPath":"/tmp/kronos-ipc.sock"})
    };
    assert!(validate_legacy_target(&target, 43210).is_ok());
}

#[test]
fn test_ipc_channel_interceptor_scoped_and_bounded() {
    let state = Arc::new(IpcState::new(None));
    let channel = tauri::ipc::Channel::<EmbeddedTransportEvent>::new(|_| Ok(()));
    let id = channel.id();
    let output = state.scoped_channel("main", channel).unwrap();
    let body =
        tauri::ipc::InvokeResponseBody::Json(format!("{{\"text\":\"{}\"}}", "x".repeat(9000)));
    assert!(!state.intercept("guest", id, 0, &body, |_| panic!(
        "foreign channel intercepted"
    )));
    let evaluated = Arc::new(Mutex::new(Vec::new()));
    let capture = evaluated.clone();
    assert!(
        state.intercept("main", id, 7, &body, move |script| {
            capture.lock().unwrap().push(script);
            Ok(())
        }),
        "large IPC event fell through to Tauri fetch cache"
    );
    let scripts = evaluated.lock().unwrap();
    assert_eq!(scripts.len(), 1);
    assert!(scripts[0].contains("index: 7"));
    assert!(!scripts[0].contains("__TAURI_CHANNEL__|fetch"));
    drop(scripts);
    assert!(state.intercept("main", id, 8, &body, |_| Err("eval failed".into())));
    assert!(
        output(EmbeddedTransportEvent {
            session_id: "session".into(),
            generation: "one".into(),
            sequence: 1,
            kind: "open".into(),
            text: None,
            binary_base64: None,
            code: None,
            reason: None
        })
        .is_err(),
        "interceptor writer failure swallowed"
    );
    drop(output);
    assert!(!state.intercept("main", id, 9, &body, |_| panic!("dead channel retained")));
}

#[tokio::test]
async fn test_ipc_send_payload_validation() {
    let (_, _, session, mut server, _receiver, _) = fixture();
    session.opened().unwrap();
    assert!(session.send(None, None).is_err());
    assert!(session
        .send(Some("text".into()), Some("AQID".into()))
        .is_err());
    assert!(session.send(None, Some("!not base64!".into())).is_err());
    session.send(Some(String::new()), None).unwrap();
    session.send(None, Some("AQID".into())).unwrap();
    let first = tokio::time::timeout(std::time::Duration::from_secs(1), server.incoming.recv())
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(std::time::Duration::from_secs(1), server.incoming.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first, SessionFrame::Text(String::new()));
    assert_eq!(second, SessionFrame::Binary(vec![1, 2, 3]));
    assert!(session
        .send(Some("x".repeat(16 * 1024 * 1024 + 1)), None)
        .is_err());
}

#[test]
#[cfg(feature = "ipc-acceptance")]
fn test_acceptance_observation_retains_bounded_closed_sessions() {
    use crate::desktop_acceptance::{commands::ProviderObservation, AcceptanceCase};
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let other = registry
        .register_surface("secondary", "http://tauri.localhost/")
        .unwrap();
    let run = "0123456789abcdef0123456789abcdef";
    for index in 0..10 {
        let id = format!("closed-{index}");
        let (session, _server, _receiver) = registry
            .open(&owner, &id, "one", Arc::new(|_| Ok(())))
            .unwrap();
        session.acceptance_bind_epoch(7).unwrap();
        session.opened().unwrap();
        session.close(1013, "Stream overflow; reconnect and resynchronize");
        registry.release(&session);
    }
    let (foreign, _server, _receiver) = registry
        .open(&other, "foreign", "one", Arc::new(|_| Ok(())))
        .unwrap();
    foreign.acceptance_bind_epoch(7).unwrap();
    foreign.opened().unwrap();
    foreign.close(1000, "Closed by desktop client");
    registry.release(&foreign);
    let (current, _server, _receiver) = registry
        .open(&owner, "current", "two", Arc::new(|_| Ok(())))
        .unwrap();
    current.acceptance_bind_epoch(7).unwrap();
    current.opened().unwrap();
    let observation = registry
        .acceptance_observation(
            &owner,
            run,
            &AcceptanceCase::Backpressure,
            ProviderObservation::default(),
        )
        .expect("Current owned session must still be observable");
    let encoded = serde_json::to_value(&observation).unwrap();
    let closed = encoded["closedSessions"]
        .as_array()
        .expect("Native observation must carry bounded closed-session history")
        .iter()
        .map(|entry| {
            (
                entry["sessionId"].as_str().unwrap().to_owned(),
                entry["closeCode"].as_u64().map(|code| code as u16),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(closed.len(), 8, "Closed-session history must stay bounded");
    assert_eq!(closed.first().unwrap().0, "closed-2");
    assert_eq!(closed.last().unwrap().0, "closed-9");
    assert!(closed.iter().all(|(_, code)| *code == Some(1013)));
    assert!(
        !closed.iter().any(|(id, _)| id == "foreign"),
        "Another owner's session must never enter this history"
    );
    assert_eq!(encoded["sessionId"], "current");
}

#[test]
#[cfg(feature = "ipc-acceptance")]
fn test_acceptance_observation_falls_back_to_latest_retired_session() {
    use crate::desktop_acceptance::{commands::ProviderObservation, AcceptanceCase};
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let run = "0123456789abcdef0123456789abcdef";
    let case = AcceptanceCase::Backpressure;
    // A writer fault closes the only session and its close event cannot reach the renderer.
    let (faulted, _server, _receiver) = registry
        .open(&owner, "faulted", "one", Arc::new(|_| Ok(())))
        .unwrap();
    faulted.acceptance_bind_epoch(7).unwrap();
    faulted.opened().unwrap();
    faulted.close(1011, "Channel writer failed; reconnect");
    registry.release(&faulted);
    let observed = registry
        .acceptance_observation(&owner, run, &case, ProviderObservation::default())
        .expect("A retired writer-fault session must stay observable");
    let encoded = serde_json::to_value(&observed).unwrap();
    assert_eq!(encoded["sessionId"], "faulted");
    assert_eq!(encoded["generation"], "one");
    assert_eq!(encoded["closeCode"], 1011);
    assert_eq!(encoded["engineEpoch"], 7);
    assert_eq!(encoded["closedSessions"][0]["sessionId"], "faulted");
    assert_eq!(encoded["closedSessions"][0]["closeCode"], 1011);
    // A live resync always wins over the retired generation.
    let (recovered, _server, _receiver) = registry
        .open(&owner, "recovered", "two", Arc::new(|_| Ok(())))
        .unwrap();
    recovered.acceptance_bind_epoch(7).unwrap();
    recovered.opened().unwrap();
    let live = registry
        .acceptance_observation(&owner, run, &case, ProviderObservation::default())
        .expect("A live session must stay observable");
    assert_eq!(serde_json::to_value(&live).unwrap()["sessionId"], "recovered");
    // A successor document must never inherit its predecessor's retired evidence.
    let successor = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    assert!(
        registry
            .acceptance_observation(&successor, run, &case, ProviderObservation::default())
            .is_err(),
        "successor surface observed a predecessor session"
    );
}

#[test]
#[cfg(feature = "ipc-acceptance")]
fn test_acceptance_stamp_report_accepts_retired_session() {
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let (faulted, _server, _receiver) = registry
        .open(&owner, "faulted", "one", Arc::new(|_| Ok(())))
        .unwrap();
    faulted.acceptance_bind_epoch(7).unwrap();
    faulted.opened().unwrap();
    faulted.close(1011, "Channel writer failed; reconnect");
    registry.release(&faulted);
    let stamped = registry
        .acceptance_stamp_report(
            &owner,
            &serde_json::json!({
                "outcome":"fail", "reason":"Channel writer failed",
                "sessionId":"faulted", "generation":"one"
            }),
        )
        .expect("A report naming the retired generation must still be stamped");
    assert_eq!(stamped["role"], "main");
    assert_eq!(stamped["incarnation"], 1);
    assert_eq!(stamped["sessionId"], "faulted");
    assert!(registry
        .acceptance_stamp_report(
            &owner,
            &serde_json::json!({
                "outcome":"fail", "sessionId":"unknown", "generation":"one"
            })
        )
        .is_err());
}

#[test]
#[cfg(feature = "ipc-acceptance")]
fn test_acceptance_report_carries_backpressure_delivery_claims() {
    use crate::desktop_acceptance::{commands::ProviderObservation, AcceptanceCase};
    let mut registry = Registry::new(None);
    let owner = registry
        .register_surface("main", "http://tauri.localhost/")
        .unwrap();
    let run = "0123456789abcdef0123456789abcdef";
    let (session, _server, _receiver) = registry
        .open(&owner, "observed", "one", Arc::new(|_| Ok(())))
        .unwrap();
    session.acceptance_bind_epoch(7).unwrap();
    session.opened().unwrap();
    let report = serde_json::json!({
        "outcome":"pass", "visibleState":"burst-drained", "lastReceivedSequence":1,
        "sessionId":"observed", "generation":"one", "serverInfoReceived":true,
        "requestCompleted":true, "burstDelivered":true, "resyncObserved":true
    });
    let result = registry
        .acceptance_result(
            &owner,
            run,
            1,
            &AcceptanceCase::Backpressure,
            ProviderObservation::default(),
            &report,
        )
        .expect("Backpressure delivery claims must be accepted");
    assert_eq!(result["renderer"]["burstDelivered"], true);
    assert_eq!(result["renderer"]["resyncObserved"], true);
    let mut typed = report.clone();
    typed["burstDelivered"] = serde_json::json!("yes");
    assert!(
        registry
            .acceptance_result(
                &owner,
                run,
                1,
                &AcceptanceCase::Backpressure,
                ProviderObservation::default(),
                &typed
            )
            .is_err(),
        "A non-boolean delivery claim must be rejected"
    );
}
