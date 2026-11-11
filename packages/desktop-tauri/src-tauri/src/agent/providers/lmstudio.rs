use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LmStudioModelEntry {
    pub id: String,
    pub object: Option<String>,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LmStudioModelsResponse {
    pub object: Option<String>,
    pub data: Vec<LmStudioModelEntry>,
}

pub struct LmStudioClient {
    base_url: String,
    http_client: reqwest::Client,
}

impl Default for LmStudioClient {
    fn default() -> Self {
        Self::default_local()
    }
}

impl LmStudioClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            base_url: base_url.into(),
            http_client,
        }
    }

    pub fn default_local() -> Self {
        Self::new("http://127.0.0.1:1234")
    }

    /// Checks if LM Studio server is running locally and responding.
    pub async fn check_health(&self) -> bool {
        let url = format!("{}/v1/models", self.base_url);
        self.http_client
            .get(&url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Fetches all models loaded or configured in LM Studio.
    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/v1/models", self.base_url);
        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to reach LM Studio at {url}: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("LM Studio returned HTTP {}", resp.status()));
        }

        let body: LmStudioModelsResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse LM Studio models response: {e}"))?;

        Ok(body.data.into_iter().map(|m| m.id).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lmstudio_models() {
        let raw_json = r#"{
            "object": "list",
            "data": [
                {
                    "id": "qwen2.5-coder-7b-instruct",
                    "object": "model",
                    "owned_by": "lmstudio"
                },
                {
                    "id": "meta-llama-3.1-8b-instruct",
                    "object": "model",
                    "owned_by": "meta"
                }
            ]
        }"#;

        let parsed: LmStudioModelsResponse = serde_json::from_str(raw_json).expect("valid json");
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].id, "qwen2.5-coder-7b-instruct");
        assert_eq!(parsed.data[1].id, "meta-llama-3.1-8b-instruct");
    }
}
