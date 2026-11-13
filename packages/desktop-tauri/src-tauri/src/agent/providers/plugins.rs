use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
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
    storage_dir: Option<PathBuf>,
}

impl Default for NativePluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePluginManager {
    pub fn new() -> Self {
        Self::with_storage_dir(Some(crate::get_data_dir()))
    }

    pub fn with_storage_dir(storage_dir: Option<PathBuf>) -> Self {
        let mut metrics = HashMap::new();
        let sources = [
            "claude", "codex", "copilot", "cursor", "grok", "kimi", "minimax", "opencode-go", "zai",
        ];

        let init_ts = crate::agent::types::now_iso();
        for s in sources {
            metrics.insert(
                s.to_string(),
                UsageMetric {
                    provider_id: s.to_string(),
                    input_tokens: 0,
                    output_tokens: 0,
                    total_cost_usd: 0.0,
                    last_updated: init_ts.clone(),
                },
            );
        }

        // Load persisted usage metrics from `<storage_dir>/usage_metrics.json` if present
        if let Some(ref dir) = storage_dir {
            let metrics_path = dir.join("usage_metrics.json");
            if let Ok(raw) = std::fs::read_to_string(&metrics_path) {
                if let Ok(saved) = serde_json::from_str::<Vec<UsageMetric>>(&raw) {
                    for m in saved {
                        metrics.insert(m.provider_id.clone(), m);
                    }
                } else if let Ok(saved_map) = serde_json::from_str::<HashMap<String, UsageMetric>>(&raw) {
                    for (k, v) in saved_map {
                        metrics.insert(k, v);
                    }
                }
            }
        }

        let active = Self::discover_active_plugins(storage_dir.as_deref());

        Self {
            usage_metrics: Arc::new(RwLock::new(metrics)),
            active_providers: Arc::new(RwLock::new(active)),
            storage_dir,
        }
    }

    fn discover_active_plugins(storage_dir: Option<&std::path::Path>) -> Vec<String> {
        let mut set: BTreeSet<String> = [
            "antigravity".to_string(),
            "hoplite".to_string(),
            "muse".to_string(),
            "worktree-include".to_string(),
        ]
        .into_iter()
        .collect();

        let mut disabled_set: BTreeSet<String> = BTreeSet::new();
        if let Some(dir) = storage_dir {
            let state_path = dir.join("plugins_state.json");
            if let Ok(raw) = std::fs::read_to_string(&state_path) {
                if let Ok(state_map) = serde_json::from_str::<HashMap<String, bool>>(&raw) {
                    for (id, enabled) in state_map {
                        if enabled {
                            set.insert(id);
                        } else {
                            disabled_set.insert(id);
                        }
                    }
                }
            }
        }

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut roots = vec![
            home.join(".paseo").join("plugins"),
            home.join(".kronos").join("plugins"),
        ];
        if let Some(dir) = storage_dir {
            roots.insert(0, dir.join("plugins"));
        }

        for root in roots {
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_dir() {
                    continue;
                }
                for manifest_name in ["plugin.json", "manifest.json", "package.json"] {
                    let mf = p.join(manifest_name);
                    if mf.is_file() {
                        if let Ok(raw) = std::fs::read_to_string(&mf) {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                                let fallback = p
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("plugin");
                                let id = val
                                    .get("id")
                                    .or_else(|| val.get("name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(fallback)
                                    .to_string();
                                let enabled = val
                                    .get("enabled")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(true);
                                if enabled && !disabled_set.contains(&id) {
                                    set.insert(id);
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }

        set.into_iter().filter(|id| !disabled_set.contains(id)).collect()
    }

    fn persist_metrics_locked(&self, map: &HashMap<String, UsageMetric>) {
        let Some(ref dir) = self.storage_dir else {
            return;
        };
        let _ = std::fs::create_dir_all(dir);
        let target = dir.join("usage_metrics.json");
        let tmp = dir.join("usage_metrics.json.tmp");
        let list: Vec<&UsageMetric> = map.values().collect();
        if let Ok(serialized) = serde_json::to_string_pretty(&list) {
            if std::fs::write(&tmp, serialized.as_bytes()).is_ok() {
                let _ = std::fs::rename(&tmp, &target);
            }
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
        entry.last_updated = crate::agent::types::now_iso();
        self.persist_metrics_locked(&map);
    }

    pub fn get_metrics(&self) -> Vec<UsageMetric> {
        let map = self.usage_metrics.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn list_active_plugins(&self) -> Vec<String> {
        self.active_providers.read().unwrap().clone()
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) {
        {
            let mut active = self.active_providers.write().unwrap();
            if enabled {
                if !active.iter().any(|p| p == plugin_id) {
                    active.push(plugin_id.to_string());
                }
            } else {
                active.retain(|p| p != plugin_id);
            }
        }
        if let Some(ref dir) = self.storage_dir {
            let _ = std::fs::create_dir_all(dir);
            let state_path = dir.join("plugins_state.json");
            let mut state_map: HashMap<String, bool> = std::fs::read_to_string(&state_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            state_map.insert(plugin_id.to_string(), enabled);
            if let Ok(raw) = serde_json::to_string_pretty(&state_map) {
                let _ = std::fs::write(&state_path, raw);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_plugin_metrics() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "kronos_plugin_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let manager = NativePluginManager::with_storage_dir(Some(tmp_dir.clone()));
        assert!(manager.list_active_plugins().len() >= 4);

        manager.record_tokens("claude", 1000, 500, 0.015);
        let metrics = manager.get_metrics();
        let claude = metrics.iter().find(|m| m.provider_id == "claude").unwrap();
        assert_eq!(claude.input_tokens, 1000);
        assert_eq!(claude.output_tokens, 500);
        assert_eq!(claude.total_cost_usd, 0.015);

        let reloaded = NativePluginManager::with_storage_dir(Some(tmp_dir.clone()));
        let claude_reloaded = reloaded
            .get_metrics()
            .into_iter()
            .find(|m| m.provider_id == "claude")
            .unwrap();
        assert_eq!(claude_reloaded.input_tokens, 1000);
        assert_eq!(claude_reloaded.output_tokens, 500);
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
