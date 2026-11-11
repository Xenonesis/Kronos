use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelDetails {
    pub parent_model: Option<String>,
    pub format: Option<String>,
    pub family: Option<String>,
    pub parameter_size: Option<String>,
    pub quantization_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelEntry {
    pub name: String,
    pub model: Option<String>,
    pub size: Option<u64>,
    pub details: Option<OllamaModelDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaTagsResponse {
    pub models: Vec<OllamaModelEntry>,
}

pub struct OllamaClient {
    base_url: String,
    http_client: reqwest::Client,
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::default_local()
    }
}

impl OllamaClient {
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
        Self::new("http://127.0.0.1:11434")
    }

    /// Checks if Ollama is running locally and responding.
    pub async fn check_health(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        self.http_client
            .get(&url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Fetches all models available locally in Ollama.
    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/tags", self.base_url);
        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to reach Ollama at {url}: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Ollama returned HTTP {}", resp.status()));
        }

        let body: OllamaTagsResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama tags response: {e}"))?;

        Ok(body.models.into_iter().map(|m| m.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ollama_tags() {
        let raw_json = r#"{
            "models": [
                {
                    "name": "deepseek-r1:latest",
                    "model": "deepseek-r1:latest",
                    "size": 4683075276,
                    "details": {
                        "family": "deepseek",
                        "parameter_size": "7.7B",
                        "quantization_level": "Q4_K_M"
                    }
                },
                {
                    "name": "qwen2.5-coder:7b",
                    "model": "qwen2.5-coder:7b",
                    "size": 4700000000
                }
            ]
        }"#;

        let parsed: OllamaTagsResponse = serde_json::from_str(raw_json).expect("valid json");
        assert_eq!(parsed.models.len(), 2);
        assert_eq!(parsed.models[0].name, "deepseek-r1:latest");
        assert_eq!(parsed.models[1].name, "qwen2.5-coder:7b");
    }
}
