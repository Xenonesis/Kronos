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
