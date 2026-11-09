use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcEnvelope {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub payload: Value,
}

impl RpcEnvelope {
    pub fn new(id: impl Into<String>, msg_type: impl Into<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            msg_type: msg_type.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn ok(id: impl Into<String>, msg_type: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            id: id.into(),
            msg_type: msg_type.into(),
            success: true,
            data,
            error: None,
        }
    }

    pub fn err(
        id: impl Into<String>,
        msg_type: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            msg_type: msg_type.into(),
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

pub type RpcPayload = Value;
pub type ClientRequest = RpcEnvelope;
pub type ServerResponse = RpcResponse;
pub type SessionEvent = RpcEnvelope;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum WSInboundMessage {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "hello")]
    Hello {
        #[serde(rename = "clientId", default)]
        client_id: Option<String>,
        #[serde(rename = "clientType", default)]
        client_type: Option<String>,
        #[serde(rename = "protocolVersion", default)]
        protocol_version: Option<u32>,
        #[serde(default)]
        capabilities: Option<Value>,
    },
    #[serde(rename = "session")]
    Session { message: Value },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum WSOutboundMessage {
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "session")]
    Session { message: Value },
    #[serde(rename = "hello.rejected")]
    HelloRejected {
        reason: String,
        accepts: Vec<String>,
    },
}

pub fn build_server_info_payload() -> Value {
    let server_id = crate::get_or_create_server_id();
    serde_json::json!({
        "status": "server_info",
        "protocolVersion": 1,
        "serverId": server_id,
        "hostname": "localhost",
        "version": env!("CARGO_PKG_VERSION"),
        "desktopManaged": true,
        "features": {
            "connectedHosts": false,
            "hostMultiplexing": false,
            "terminalChunkedReplay": false,
            "binaryChunkingV1": true,
            "providersSnapshot": true,
            "providersSnapshotCwd": true,
            "directorySync": true,
            "workspaceSetupRun": true,
            "workspaceSetupObservation": true,
            "creationLifecycle": true,
            "backgroundWorkspaces": true,
            "workspaceSortTimes": true,
            "workspacePinning": true,
            "workspaceLabels": true,
            "skillManagement": true,
            "projectAdd": true,
            "stableProjectIdentity": true,
            "projectGithubClone": true,
            "workspaceGithubRepositorySearch": true,
            "projectCreateDirectory": true,
            "projectList": true,
            "projectRemove": true,
            "workspaceMultiplicity": true,
            "wholeTurnTimeline": true,
            "kernelTelemetry": true,
            "kernelAst": true,
            "kernelCrdt": true,
            "kernelIoRing": true,
            "kernelShm": true,
            "kernelSpeech": true,
            "agentHistorySearch": true
        }
    })
}

pub fn wrap_session_outbound(message: Value) -> Value {
    serde_json::json!({
        "type": "session",
        "message": message
    })
}

pub fn build_server_info_response() -> Value {
    wrap_session_outbound(serde_json::json!({
        "type": "status",
        "payload": build_server_info_payload()
    }))
}
