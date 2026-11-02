use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetric {
    pub provider_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_cost_usd: f64,
    pub last_updated: String,
}

pub struct NativePluginManager {
    usage_metrics: Arc<RwLock<HashMap<String, UsageMetric>>>,
    active_providers: Arc<RwLock<Vec<String>>>,
}

impl Default for NativePluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePluginManager {
    pub fn new() -> Self {
        let mut metrics = HashMap::new();
        let sources = [
            "claude", "codex", "copilot", "cursor", "grok", "kimi", "minimax", "opencode-go", "zai",
        ];

        for s in sources {
            metrics.insert(
                s.to_string(),
                UsageMetric {
                    provider_id: s.to_string(),
                    input_tokens: 0,
                    output_tokens: 0,
                    total_cost_usd: 0.0,
                    last_updated: "2026-11-02T00:00:00Z".to_string(),
                },
            );
        }

        Self {
            usage_metrics: Arc::new(RwLock::new(metrics)),
            active_providers: Arc::new(RwLock::new(vec![
                "antigravity".to_string(),
                "hoplite".to_string(),
                "muse".to_string(),
                "worktree-include".to_string(),
            ])),
        }
    }

    pub fn record_tokens(&self, provider: &str, input: u64, output: u64, cost: f64) {
        let mut map = self.usage_metrics.write().unwrap();
        let entry = map.entry(provider.to_string()).or_insert_with(|| UsageMetric {
            provider_id: provider.to_string(),
            input_tokens: 0,
            output_tokens: 0,
            total_cost_usd: 0.0,
            last_updated: String::new(),
        });

        entry.input_tokens += input;
        entry.output_tokens += output;
        entry.total_cost_usd += cost;
    }

    pub fn get_metrics(&self) -> Vec<UsageMetric> {
        let map = self.usage_metrics.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn list_active_plugins(&self) -> Vec<String> {
        self.active_providers.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_plugin_metrics() {
        let manager = NativePluginManager::new();
        assert_eq!(manager.list_active_plugins().len(), 4);

        manager.record_tokens("claude", 1000, 500, 0.015);
        let metrics = manager.get_metrics();
        let claude = metrics.iter().find(|m| m.provider_id == "claude").unwrap();
        assert_eq!(claude.input_tokens, 1000);
        assert_eq!(claude.output_tokens, 500);
        assert_eq!(claude.total_cost_usd, 0.015);
    }
}
