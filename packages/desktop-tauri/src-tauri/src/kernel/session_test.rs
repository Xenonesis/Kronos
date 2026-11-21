use crate::kernel::server::{EmbeddedDaemon, SessionContext};
use crate::kernel::session::{
    run_session, session_pair, SessionError, SessionFrame, SessionIo, SessionProfile,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[cfg(feature = "ipc-acceptance")]
#[tokio::test]
async fn test_acceptance_queue_counts_admitted_leases() {
    let (mut server, client) = session_pair(SessionProfile::Full);
    client.outgoing.try_send(SessionFrame::Text("abc".into())).unwrap();
    client.outgoing.try_send(SessionFrame::Binary(vec![1; 5])).unwrap();
    assert_eq!(client.outgoing.acceptance_queue_stats(), (2, 8, 2, 8), "queue peaks were not measured");
    server.incoming.recv().await.unwrap();
    assert_eq!(client.outgoing.acceptance_queue_stats(), (1, 5, 2, 8));
    drop(server);
    assert_eq!(client.outgoing.acceptance_queue_stats(), (0, 0, 2, 8));
    let (mut server, client) = session_pair(SessionProfile::Full);
    for _ in 0..128 { client.outgoing.try_send(SessionFrame::Binary(vec![0])).unwrap(); }
    assert!(client.outgoing.try_send(SessionFrame::Binary(vec![0])).is_err());
    assert_eq!(client.outgoing.acceptance_queue_stats(), (128, 128, 128, 128), "failed admission polluted peaks");
    server.incoming.recv().await.unwrap();
    assert_eq!(client.outgoing.acceptance_queue_stats(), (0, 0, 128, 128));
}

type TestSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn read_json(socket: &mut TestSocket) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .expect("WebSocket reply deadline")
        .expect("WebSocket EOF")
        .unwrap();
    serde_json::from_str(frame.to_text().unwrap()).unwrap()
}

enum Peer {
    Channel(SessionIo),
    Network(TestSocket),
}

struct Fixture {
    peer: Option<Peer>,
    context: Arc<SessionContext>,
    daemon: Option<EmbeddedDaemon>,
    task: Option<tokio::task::JoinHandle<Result<(), SessionError>>>,
}

impl Fixture {
    async fn new(network: bool, profile: SessionProfile) -> Self {
        if network {
            let daemon = EmbeddedDaemon::start_test(0).await.unwrap();
            let (socket, _) = tokio::time::timeout(
                Duration::from_secs(3),
                connect_async(format!("ws://127.0.0.1:{}/ws", daemon.port)),
            )
            .await
            .unwrap()
            .unwrap();
            Self {
                context: daemon.context.clone(),
                daemon: Some(daemon),
                task: None,
                peer: Some(Peer::Network(socket)),
            }
        } else {
            let context = SessionContext::initialize().unwrap();
            let (server, client) = session_pair(profile);
            let task = tokio::spawn(run_session(context.clone(), server));
            Self {
                context,
                daemon: None,
                task: Some(task),
                peer: Some(Peer::Channel(client)),
            }
        }
    }

    async fn send(&mut self, frame: SessionFrame) {
        match self.peer.as_mut().unwrap() {
            Peer::Channel(io) => io.outgoing.try_send(frame).unwrap(),
            Peer::Network(socket) => {
                let frame = match frame {
                    SessionFrame::Text(text) => Message::Text(text.into()),
                    SessionFrame::Binary(bytes) => Message::Binary(bytes.into()),
                    SessionFrame::Close { .. } => Message::Close(None),
                };
                tokio::time::timeout(Duration::from_secs(3), socket.send(frame))
                    .await
                    .unwrap()
                    .unwrap();
            }
        }
    }

    async fn receive(&mut self) -> SessionFrame {
        tokio::time::timeout(Duration::from_secs(3), async {
            match self.peer.as_mut().unwrap() {
                Peer::Channel(io) => io.incoming.recv().await.expect("channel EOF"),
                Peer::Network(socket) => match socket.next().await.unwrap().unwrap() {
                    Message::Text(text) => SessionFrame::Text(text.to_string()),
                    Message::Binary(bytes) => SessionFrame::Binary(bytes.to_vec()),
                    Message::Close(Some(close)) => SessionFrame::Close {
                        code: close.code.into(),
                        reason: close.reason.to_string(),
                    },
                    frame => panic!("unexpected WebSocket frame {frame:?}"),
                },
            }
        })
        .await
        .expect("session reply deadline")
    }

    async fn json(&mut self) -> Value {
        let SessionFrame::Text(text) = self.receive().await else {
            panic!("expected text");
        };
        serde_json::from_str(&text).unwrap()
    }

    async fn admit(&mut self) {
        let hello = json!({"type":"hello", "protocolVersion":1,
            "auth":{"kind":"localCredential", "token":self.context.access.local_credential}});
        self.send(SessionFrame::Text(hello.to_string())).await;
        assert_eq!(
            self.json().await["message"]["payload"]["status"],
            "server_info"
        );
    }

    async fn close(mut self) {
        if let Some(Peer::Network(socket)) = self.peer.as_mut() {
            let _ = socket.close(None).await;
        }
        drop(self.peer.take());
        if let Some(task) = self.task.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), task)
                .await
                .expect("session teardown deadline")
                .unwrap();
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            while super::server::get_provider_push_tx().receiver_count() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("subscription teardown deadline");
        assert!(
            !self.context.is_stopped(),
            "transport teardown stopped engine"
        );
        if let Some(daemon) = self.daemon {
            daemon.stop().await;
        }
    }
}

async fn isolated(name: &str) -> bool {
    crate::daemon::tests::lifecycle_child(name).await
}

#[tokio::test]
async fn test_session_rejects_before_admission() {
    if !isolated("kernel::session_test::test_session_rejects_before_admission").await {
        return;
    }
    for network in [false, true] {
        for (hello, reason, code) in [
            (
                json!({"type":"hello", "protocolVersion":1, "capabilities":{"hello_rejection":true}}),
                "password_required",
                4401,
            ),
            (
                json!({"type":"hello", "protocolVersion":0, "capabilities":{"hello_rejection":true}}),
                "incompatible_protocol",
                4003,
            ),
        ] {
            let mut fixture = Fixture::new(network, SessionProfile::Full).await;
            assert_eq!(super::server::get_provider_push_tx().receiver_count(), 0);
            fixture.send(SessionFrame::Text(hello.to_string())).await;
            assert_eq!(fixture.json().await["reason"], reason);
            assert!(
                matches!(fixture.receive().await, SessionFrame::Close { code: actual, .. } if actual == code)
            );
            fixture.close().await;
        }
    }
}

#[tokio::test]
async fn test_session_preserves_request_id() {
    if !isolated("kernel::session_test::test_session_preserves_request_id").await {
        return;
    }
    for network in [false, true] {
        let mut fixture = Fixture::new(network, SessionProfile::Full).await;
        fixture.admit().await;
        fixture.send(SessionFrame::Text("not JSON".into())).await;
        assert!(fixture.json().await["error"]
            .as_str()
            .unwrap()
            .contains("Invalid JSON"));
        fixture
            .send(SessionFrame::Text(
                r#"{"id":"correlation-42","type":"ping","payload":{}}"#.into(),
            ))
            .await;
        let response = fixture.json().await;
        assert_eq!(response["id"], "correlation-42");
        assert_eq!(response["type"], "pong");
        assert_eq!(response["success"], true);
        fixture.close().await;
    }
}

#[tokio::test]
async fn test_session_orders_text_and_binary() {
    if !isolated("kernel::session_test::test_session_orders_text_and_binary").await {
        return;
    }
    for network in [false, true] {
        let mut fixture = Fixture::new(network, SessionProfile::Full).await;
        fixture.admit().await;
        for (binary, id) in [(false, "first"), (true, "middle"), (false, "last")] {
            let text = json!({"id":id, "type":"ping", "payload":{}}).to_string();
            fixture
                .send(if binary {
                    SessionFrame::Binary(text.into_bytes())
                } else {
                    SessionFrame::Text(text)
                })
                .await;
        }
        for id in ["first", "middle", "last"] {
            assert_eq!(fixture.json().await["id"], id);
        }
        fixture.send(SessionFrame::Text(json!({"type":"session", "message":{
            "type":"subscribe_terminal_request", "terminalId":"fixture-binary", "requestId":"subscribe"
        }}).to_string())).await;
        assert_eq!(fixture.json().await["message"]["payload"]["slot"], 1);
        crate::native_pty::test_output_sender()
            .send(crate::native_pty::TerminalOutputChunk {
                terminal_id: "fixture-binary".into(),
                data: vec![0, 0xff, 0x80, 0x41],
            })
            .unwrap();
        assert_eq!(
            fixture.receive().await,
            SessionFrame::Binary(vec![1, 1, 0, 0xff, 0x80, 0x41])
        );
        fixture
            .send(SessionFrame::Text(r#"{"type":"ping"}"#.into()))
            .await;
        assert_eq!(fixture.json().await["type"], "pong");
        fixture.close().await;
    }
}

#[tokio::test]
async fn test_session_terminal_slot_lifecycle() {
    if !isolated("kernel::session_test::test_session_terminal_slot_lifecycle").await {
        return;
    }
    for network in [false, true] {
        let mut fixture = Fixture::new(network, SessionProfile::Full).await;
        fixture.admit().await;
        for (kind, terminal, want) in [
            ("subscribe_terminal_request", "a", Some(1)),
            ("subscribe_terminal_request", "b", Some(2)),
            ("subscribe_terminal_request", "a", Some(1)),
            ("unsubscribe_terminal_request", "a", None),
            ("subscribe_terminal_request", "c", Some(1)),
            ("subscribe_terminal_request", "b", Some(2)),
        ] {
            fixture
                .send(SessionFrame::Text(
                    json!({"type":"session", "message":{
                        "type":kind, "payload":{"terminalId":terminal}, "requestId":"slot-check"
                    }})
                    .to_string(),
                ))
                .await;
            let response = fixture.json().await;
            assert_eq!(response["message"]["payload"]["terminalId"], terminal);
            if let Some(slot) = want {
                assert_eq!(response["message"]["payload"]["slot"], slot);
            }
        }
        let output = crate::native_pty::test_output_sender();
        for (terminal_id, byte, slot) in [("b", 0x77, 2), ("c", 0x88, 1)] {
            output
                .send(crate::native_pty::TerminalOutputChunk {
                    terminal_id: terminal_id.into(),
                    data: vec![byte],
                })
                .unwrap();
            assert_eq!(
                fixture.receive().await,
                SessionFrame::Binary(vec![1, slot, byte])
            );
        }
        fixture.close().await;
    }
}

#[tokio::test]
async fn test_session_close_releases_subscriptions() {
    if !isolated("kernel::session_test::test_session_close_releases_subscriptions").await {
        return;
    }
    for network in [false, true] {
        let mut fixture = Fixture::new(network, SessionProfile::Full).await;
        fixture.admit().await;
        assert_eq!(super::server::get_provider_push_tx().receiver_count(), 1);
        fixture.close().await;
        assert_eq!(super::server::get_provider_push_tx().receiver_count(), 0);
    }
}

#[tokio::test]
async fn test_session_cancel_does_not_restart_engine() {
    if !isolated("kernel::session_test::test_session_cancel_does_not_restart_engine").await {
        return;
    }
    let context = crate::daemon::ensure_embedded_ready().await.unwrap();
    let (server, client) = session_pair(SessionProfile::Full);
    let task = tokio::spawn(run_session(context.clone(), server));
    drop(client);
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!context.is_stopped());
    let after = crate::daemon::ensure_embedded_ready().await.unwrap();
    let same = Arc::ptr_eq(&context, &after);
    crate::daemon::stop_embedded();
    assert!(same, "transport cancel replaced engine");
}

#[tokio::test]
async fn test_session_lag_signals_resync() {
    if !isolated("kernel::session_test::test_session_lag_signals_resync").await {
        return;
    }
    for network in [false, true] {
        let mut fixture = Fixture::new(network, SessionProfile::Full).await;
        fixture.admit().await;
        let sender = crate::native_pty::test_output_sender();
        for _ in 0..2050 {
            sender
                .send(crate::native_pty::TerminalOutputChunk {
                    terminal_id: "not-subscribed".into(),
                    data: vec![42],
                })
                .unwrap();
        }
        assert!(matches!(
            fixture.receive().await,
            SessionFrame::Close { code: 1013, .. }
        ));
        assert!(!fixture.context.is_stopped());
        fixture.close().await;
    }
}

#[tokio::test]
async fn test_session_legacy_profile_stays_rpc_only() {
    if !isolated("kernel::session_test::test_session_legacy_profile_stays_rpc_only").await {
        return;
    }
    let mut fixture = Fixture::new(false, SessionProfile::LegacyLine).await;
    fixture.admit().await;
    assert_eq!(super::server::get_provider_push_tx().receiver_count(), 0);
    assert!(super::server::get_provider_push_tx()
        .send("private unsolicited push".into())
        .is_err());
    fixture
        .send(SessionFrame::Text(
            r#"{"id":"legacy-rpc","type":"ping","payload":{}}"#.into(),
        ))
        .await;
    assert_eq!(fixture.json().await["id"], "legacy-rpc");
    fixture.close().await;
}

#[tokio::test]
async fn test_session_queue_limits_and_close() {
    let (mut receiver, sender) = session_pair(SessionProfile::Full);
    for _ in 0..128 {
        sender
            .outgoing
            .try_send(SessionFrame::Text("x".into()))
            .unwrap();
    }
    assert_eq!(
        receiver.incoming.recv().await,
        Some(SessionFrame::Text("x".into()))
    );
    sender
        .outgoing
        .try_send(SessionFrame::Text("replacement".into()))
        .unwrap();
    assert_eq!(
        sender
            .outgoing
            .try_send(SessionFrame::Text("overflow".into())),
        Err(SessionError::Overflow)
    );
    assert!(matches!(
        receiver.incoming.recv().await,
        Some(SessionFrame::Close { code: 1013, .. })
    ));
    assert_eq!(receiver.incoming.recv().await, None);
    assert_eq!(
        sender.outgoing.try_send(SessionFrame::Text("late".into())),
        Err(SessionError::Closed)
    );
    let (mut receiver, sender) = session_pair(SessionProfile::Full);
    for _ in 0..2 {
        sender
            .outgoing
            .try_send(SessionFrame::Binary(vec![0; 16 * 1024 * 1024]))
            .unwrap();
    }
    assert!(
        matches!(receiver.incoming.recv().await, Some(SessionFrame::Binary(bytes)) if bytes.len() == 16 * 1024 * 1024)
    );
    sender
        .outgoing
        .try_send(SessionFrame::Binary(vec![1; 16 * 1024 * 1024]))
        .unwrap();
    assert_eq!(
        sender.outgoing.try_send(SessionFrame::Binary(vec![1])),
        Err(SessionError::Overflow)
    );
    assert!(matches!(
        receiver.incoming.recv().await,
        Some(SessionFrame::Close { code: 1013, .. })
    ));
    let (mut receiver, sender) = session_pair(SessionProfile::Full);
    assert_eq!(
        sender
            .outgoing
            .try_send(SessionFrame::Text("é".repeat(8 * 1024 * 1024 + 1))),
        Err(SessionError::FrameTooLarge)
    );
    assert!(matches!(
        receiver.incoming.recv().await,
        Some(SessionFrame::Close { code: 1009, .. })
    ));
    let (mut receiver, sender) = session_pair(SessionProfile::Full);
    for frame in [
        SessionFrame::Text("first".into()),
        SessionFrame::Binary(vec![0, 255]),
    ] {
        sender.outgoing.try_send(frame.clone()).unwrap();
        assert_eq!(receiver.incoming.recv().await, Some(frame));
    }
    sender
        .outgoing
        .try_send(SessionFrame::Text("rejection".into()))
        .unwrap();
    sender
        .outgoing
        .try_send(SessionFrame::Close {
            code: 4401,
            reason: "auth".into(),
        })
        .unwrap();
    assert_eq!(
        receiver.incoming.recv().await,
        Some(SessionFrame::Text("rejection".into()))
    );
    assert!(matches!(
        receiver.incoming.recv().await,
        Some(SessionFrame::Close { code: 4401, .. })
    ));
}

#[tokio::test]
async fn test_session_terminal_slot_exhaustion() {
    if !isolated("kernel::session_test::test_session_terminal_slot_exhaustion").await {
        return;
    }
    for network in [false, true] {
        let mut fixture = Fixture::new(network, SessionProfile::Full).await;
        fixture.admit().await;
        for slot in 1..=255 {
            fixture.send(SessionFrame::Text(json!({"type":"session", "message":{
                "type":"subscribe_terminal_request", "terminalId":format!("fixture-{slot}"), "requestId":"capacity"
            }}).to_string())).await;
            assert_eq!(fixture.json().await["message"]["payload"]["slot"], slot);
        }
        fixture.send(SessionFrame::Text(json!({"type":"session", "message":{
            "type":"subscribe_terminal_request", "terminalId":"one-too-many", "requestId":"exhausted"
        }}).to_string())).await;
        assert!(matches!(
            fixture.receive().await,
            SessionFrame::Close { code: 1013, .. }
        ));
        fixture.close().await;
    }
}

#[tokio::test]
async fn test_session_crdt_events_forwarded() {
    if !isolated("kernel::session_test::test_session_crdt_events_forwarded").await {
        return;
    }
    for network in [false, true] {
        let mut fixture = Fixture::new(network, SessionProfile::Full).await;
        fixture.admit().await;
        let room = if network {
            "fixture-network-room"
        } else {
            "fixture-channel-room"
        };
        super::server::get_crdt_engine()
            .apply_text_edit(
                room,
                "insert",
                0,
                Some("fixture text"),
                None,
                Some("fixture-source".into()),
            )
            .unwrap();
        let response = fixture.json().await;
        assert_eq!(response["type"], "crdt/update_event");
        assert_eq!(response["payload"]["room"], room);
        assert_eq!(response["payload"]["sourceId"], "fixture-source");
        assert!(!response["payload"]["update"].as_str().unwrap().is_empty());
        fixture.close().await;
    }
}

// Catches the old response advertising slot 1 while routing the second terminal on slot 2.
#[tokio::test]
async fn test_websocket_terminal_slots_match_responses() {
    let name = "kernel::session_test::test_websocket_terminal_slots_match_responses";
    if !crate::daemon::tests::lifecycle_child(name).await {
        return;
    }
    let daemon = EmbeddedDaemon::start_test(0).await.unwrap();
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(3),
        connect_async(format!("ws://127.0.0.1:{}/ws", daemon.port)),
    )
    .await
    .unwrap()
    .unwrap();
    let hello = json!({"type":"hello", "protocolVersion":1,
        "auth":{"kind":"localCredential", "token":daemon.access.local_credential}});
    socket
        .send(Message::Text(hello.to_string().into()))
        .await
        .unwrap();
    assert_eq!(
        read_json(&mut socket).await["message"]["payload"]["status"],
        "server_info"
    );
    let mut slots = Vec::new();
    for (id, terminal) in [
        ("subscribe-a", "fixture-terminal-a"),
        ("subscribe-b", "fixture-terminal-b"),
    ] {
        socket
            .send(Message::Text(
                json!({"type":"session", "message":{
                    "type":"subscribe_terminal_request", "requestId":id, "terminalId":terminal
                }})
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
        let response = read_json(&mut socket).await;
        assert_eq!(response["message"]["requestId"], id);
        slots.push(response["message"]["payload"]["slot"].as_u64().unwrap());
    }
    socket.close(None).await.unwrap();
    daemon.stop().await;
    assert_eq!(
        slots,
        vec![1, 2],
        "second terminal advertised wrong binary routing slot"
    );
}
