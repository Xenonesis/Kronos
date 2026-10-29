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
}
