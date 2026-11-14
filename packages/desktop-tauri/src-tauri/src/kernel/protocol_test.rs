#[cfg(test)]
mod tests {
    use crate::kernel::generated_types::HandshakePayload;
    use crate::kernel::protocol::{RpcEnvelope, RpcPayload, RpcResponse};
    use serde_json::json;

    #[test]
    fn test_rpc_envelope_dynamic_payload_roundtrip() {
        let raw = json!({
            "id": "req-1",
            "type": "workspace.create",
            "payload": {
                "slug": "test-worktree",
                "repoRoot": "C:\\repo"
            }
        });

        let envelope: RpcEnvelope =
            serde_json::from_value(raw.clone()).expect("Failed to deserialize");
        assert_eq!(envelope.id, "req-1");
        assert_eq!(envelope.msg_type, "workspace.create");
        assert_eq!(envelope.payload["slug"], "test-worktree");

        let serialized = serde_json::to_value(&envelope).expect("Failed to serialize");
        assert_eq!(serialized, raw);
    }

    #[test]
    fn test_rpc_response_roundtrip() {
        let raw_success = json!({
            "id": "res-1",
            "type": "workspace.create",
            "success": true,
            "data": {
                "status": "ok"
            }
        });
        let resp: RpcResponse =
            serde_json::from_value(raw_success.clone()).expect("Failed to deserialize success");
        assert_eq!(resp.id, "res-1");
        assert_eq!(resp.msg_type, "workspace.create");
        assert!(resp.success);
        assert_eq!(resp.data.as_ref().unwrap()["status"], "ok");
        assert!(resp.error.is_none());

        let serialized = serde_json::to_value(&resp).expect("Failed to serialize success");
        assert_eq!(serialized, raw_success);

        let raw_err = json!({
            "id": "res-2",
            "type": "workspace.create",
            "success": false,
            "error": "Workspace already exists"
        });
        let resp_err: RpcResponse =
            serde_json::from_value(raw_err.clone()).expect("Failed to deserialize error");
        assert!(!resp_err.success);
        assert_eq!(resp_err.error.as_deref(), Some("Workspace already exists"));
        assert!(resp_err.data.is_none());

        let serialized_err = serde_json::to_value(&resp_err).expect("Failed to serialize error");
        assert_eq!(serialized_err, raw_err);
    }

    #[test]
    fn test_handshake_payload_roundtrip() {
        let payload = HandshakePayload {
            client_version: "0.14.0".into(),
            client_id: "client-abc".into(),
        };
        let val = serde_json::to_value(&payload).expect("Failed to serialize handshake payload");
        let deserialized: HandshakePayload =
            serde_json::from_value(val).expect("Failed to deserialize handshake payload");
        assert_eq!(deserialized.client_version, "0.14.0");
        assert_eq!(deserialized.client_id, "client-abc");
    }

    #[test]
    fn test_rpc_envelope_default_payload() {
        let raw = json!({
            "id": "req-2",
            "type": "ping"
        });
        let envelope: RpcEnvelope = serde_json::from_value(raw).expect("Failed to deserialize");
        assert_eq!(envelope.id, "req-2");
        assert_eq!(envelope.msg_type, "ping");
        assert_eq!(envelope.payload, serde_json::Value::Null);

        let payload: RpcPayload = json!({ "ack": true });
        assert_eq!(payload["ack"], true);
    }

    #[test]
    fn test_ws_inbound_outbound_envelopes() {
        use crate::kernel::protocol::{
            build_server_info_response, WSInboundMessage, WSOutboundMessage,
        };

        // 1. Hello inbound deserialization
        let hello_raw = json!({
            "type": "hello",
            "clientId": "cid-123",
            "clientType": "cli",
            "protocolVersion": 1
        });
        let parsed: WSInboundMessage = serde_json::from_value(hello_raw).expect("parse hello");
        match parsed {
            WSInboundMessage::Hello { client_id, client_type, protocol_version, .. } => {
                assert_eq!(client_id.as_deref(), Some("cid-123"));
                assert_eq!(client_type.as_deref(), Some("cli"));
                assert_eq!(protocol_version, Some(1));
            }
            _ => panic!("Expected Hello variant"),
        }

        // 2. Ping inbound deserialization
        let ping_raw = json!({ "type": "ping" });
        let parsed_ping: WSInboundMessage = serde_json::from_value(ping_raw).expect("parse ping");
        assert_eq!(parsed_ping, WSInboundMessage::Ping);

        // 3. Pong outbound serialization
        let pong = WSOutboundMessage::Pong;
        let pong_val = serde_json::to_value(pong).expect("serialize pong");
        assert_eq!(pong_val["type"], "pong");

        // 4. Server info outbound response structure
        let info = build_server_info_response();
        assert_eq!(info["type"], "session");
        assert_eq!(info["message"]["type"], "status");
        assert_eq!(info["message"]["payload"]["status"], "server_info");
        assert_eq!(info["message"]["payload"]["protocolVersion"], 1);
        assert!(!info["message"]["payload"]["serverId"].as_str().unwrap().is_empty());
        assert_eq!(info["message"]["payload"]["features"]["projectAdd"], true);
        assert_eq!(info["message"]["payload"]["features"]["agentProfiles"], true);
        assert_eq!(info["message"]["payload"]["features"]["agentConfigApply"], true);
    }

    #[test]
    fn test_generated_session_inbound_roundtrip() {
        use crate::kernel::generated_types::SessionInboundMessage;

        // 1. hosts.list.request
        let hosts_list_json = json!({
            "type": "hosts.list.request",
            "requestId": "req-hosts-42",
            "observe": true
        });
        let in_msg: SessionInboundMessage = serde_json::from_value(hosts_list_json.clone()).expect("Deserialize HostsListRequest");
        match in_msg {
            SessionInboundMessage::HostsListRequest { request_id, observe } => {
                assert_eq!(request_id, "req-hosts-42");
                assert_eq!(observe, Some(true));
            }
            _ => panic!("Expected HostsListRequest variant"),
        }

        // 2. loop/run
        let loop_run_json = json!({
            "type": "loop/run",
            "requestId": "req-loop-1",
            "prompt": "Fix all tests",
            "cwd": "/workspace/project",
            "archive": true
        });
        let loop_msg: SessionInboundMessage = serde_json::from_value(loop_run_json.clone()).expect("Deserialize LoopRun");
        match loop_msg {
            SessionInboundMessage::LoopRun { request_id, prompt, cwd, archive, .. } => {
                assert_eq!(request_id, "req-loop-1");
                assert_eq!(prompt, "Fix all tests");
                assert_eq!(cwd, "/workspace/project");
                assert_eq!(archive, Some(true));
            }
            _ => panic!("Expected LoopRun variant"),
        }

        // 3. Unknown fallback variant
        let unknown_json = json!({
            "type": "custom.future.unrecognized.message",
            "foo": "bar"
        });
        let unknown_msg: SessionInboundMessage = serde_json::from_value(unknown_json).expect("Deserialize Unknown");
        assert_eq!(unknown_msg, SessionInboundMessage::Unknown);
    }
}
