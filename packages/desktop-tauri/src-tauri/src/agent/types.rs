use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Running,
    Paused,
    Closed,
    Error,
}

impl Default for AgentStatus {
    fn default() -> Self {
        AgentStatus::Closed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoredAgentRecord {
    pub id: String,
    pub provider: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub last_status: AgentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_info: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_usage: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentTimelineItem {
    UserMessage {
        id: String,
        text: String,
        timestamp: String,
    },
    AssistantMessage {
        id: String,
        text: String,
        timestamp: String,
    },
    Thinking {
        id: String,
        text: String,
        timestamp: String,
    },
    ToolCall {
        id: String,
        call_id: String,
        name: String,
        input: Value,
        timestamp: String,
    },
    ToolResult {
        id: String,
        call_id: String,
        output: Value,
        is_error: bool,
        timestamp: String,
    },
    ErrorNotice {
        id: String,
        message: String,
        timestamp: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: String,
    pub label: String,
    pub description: String,
    pub is_installed: bool,
}

/// Returns the current UTC time formatted as an RFC-3339 / ISO-8601 string with millisecond precision,
/// e.g. "2026-11-03T06:33:05.123Z".
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
