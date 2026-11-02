use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeEvent {
    pub event_type: String,
    pub data: Value,
}

pub struct OpenCodeClient {
    base_url: String,
    http_client: reqwest::Client,
}

impl OpenCodeClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http_client: reqwest::Client::new(),
        }
    }

    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:4096")
    }

    /// Checks if the OpenCode server is reachable and ready.
    pub async fn check_health(&self) -> bool {
        let health_url = format!("{}/health", self.base_url);
        self.http_client
            .get(&health_url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Sends a prompt turn to the OpenCode HTTP server.
    pub async fn send_prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Result<reqwest::Response, String> {
        let url = format!("{}/api/sessions/{session_id}/prompt", self.base_url);
        let body = serde_json::json!({
            "prompt": prompt
        });

        self.http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed connecting to OpenCode server: {e}"))
    }

    /// Parses an SSE (Server-Sent Events) chunk into structured OpenCode events.
    pub fn parse_sse_chunk(chunk: &str) -> Vec<OpenCodeEvent> {
        let mut events = Vec::new();
        let mut cur_event = "message".to_string();

        for line in chunk.lines() {
            let line = line.trim();
            if line.starts_with("event:") {
                cur_event = line["event:".len()..].trim().to_string();
            } else if line.starts_with("data:") {
                let data_str = line["data:".len()..].trim();
                if let Ok(data_val) = serde_json::from_str::<Value>(data_str) {
                    events.push(OpenCodeEvent {
                        event_type: cur_event.clone(),
                        data: data_val,
                    });
                }
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opencode_sse_stream_events() {
        let sse_data = "event: token\ndata: {\"text\":\"Hello \"}\n\nevent: token\ndata: {\"text\":\"world!\"}\n\n";
        let events = OpenCodeClient::parse_sse_chunk(sse_data);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "token");
        assert_eq!(events[0].data["text"], "Hello ");
        assert_eq!(events[1].data["text"], "world!");
    }
}
