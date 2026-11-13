use crate::agent::types::ProviderDescriptor;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<String, ProviderDescriptor>>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut map = HashMap::new();

        // 1. Claude
        map.insert(
            "claude".to_string(),
            ProviderDescriptor {
                id: "claude".to_string(),
                label: "Claude Code".to_string(),
                description: "Anthropic Claude CLI agent with tools and sidechains".to_string(),
                is_installed: is_command_available("claude"),
            },
        );

        // 2. Codex
        map.insert(
            "codex".to_string(),
            ProviderDescriptor {
                id: "codex".to_string(),
                label: "OpenAI Codex".to_string(),
                description: "OpenAI Codex App Server agent".to_string(),
                is_installed: is_command_available("codex"),
            },
        );

        // 3. OpenCode
        map.insert(
            "opencode".to_string(),
            ProviderDescriptor {
                id: "opencode".to_string(),
                label: "OpenCode".to_string(),
                description: "Local OpenCode agent runtime over HTTP/SSE".to_string(),
                is_installed: is_command_available("opencode"),
            },
        );

        // 4. Pi
        map.insert(
            "pi".to_string(),
            ProviderDescriptor {
                id: "pi".to_string(),
                label: "Pi Agent".to_string(),
                description: "Pi CLI tool-use runtime".to_string(),
                is_installed: is_command_available("pi"),
            },
        );

        // 5. OMP
        map.insert(
            "omp".to_string(),
            ProviderDescriptor {
                id: "omp".to_string(),
                label: "Open Model Protocol".to_string(),
                description: "OMP stdio and native Model Context Protocol bridge".to_string(),
                is_installed: is_command_available("omp"),
            },
        );

        // 6. Generic ACP + configured ACP extensions
        let cfg = crate::kernel::config_store::get_config_store().get_config();
        let acp_custom = cfg
            .providers_config
            .get("acp")
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(is_command_available)
            .unwrap_or(false);
        map.insert(
            "acp".to_string(),
            ProviderDescriptor {
                id: "acp".to_string(),
                label: "Agent Client Protocol".to_string(),
                description: "ACP stdio agent protocol runner (Cursor, Copilot, Trae)".to_string(),
                is_installed: is_command_available("acp") || acp_custom,
            },
        );
        if let Some(obj) = cfg.providers_config.as_object() {
            for (prov_id, entry) in obj {
                if entry.get("extends").and_then(|v| v.as_str()) == Some("acp") && prov_id != "acp"
                {
                    let cmd_bin = entry
                        .get("command")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.as_str());
                    let available = cmd_bin.map(is_command_available).unwrap_or(false);
                    map.insert(
                        prov_id.clone(),
                        ProviderDescriptor {
                            id: prov_id.clone(),
                            label: entry
                                .get("label")
                                .and_then(|v| v.as_str())
                                .unwrap_or(prov_id)
                                .to_string(),
                            description: "Configured ACP stdio agent".to_string(),
                            is_installed: available,
                        },
                    );
                }
            }
        }

        #[cfg(test)]
        // 7. Mock (strictly for unit tests)
        map.insert(
            "mock".to_string(),
            ProviderDescriptor {
                id: "mock".to_string(),
                label: "Mock Agent".to_string(),
                description: "Mock agent for testing, benchmarking, and development".to_string(),
                is_installed: true,
            },
        );

        // 8. Ollama
        map.insert(
            "ollama".to_string(),
            ProviderDescriptor {
                id: "ollama".to_string(),
                label: "Ollama (Local)".to_string(),
                description: "Local open-weights LLMs via Ollama daemon (127.0.0.1:11434)".to_string(),
                is_installed: is_command_available("ollama") || is_local_port_open(11434),
            },
        );

        // 9. LM Studio
        map.insert(
            "lmstudio".to_string(),
            ProviderDescriptor {
                id: "lmstudio".to_string(),
                label: "LM Studio (Local)".to_string(),
                description: "Local GGUF models via LM Studio local inference server (127.0.0.1:1234)".to_string(),
                is_installed: is_command_available("lms") || is_local_port_open(1234),
            },
        );

        Self {
            providers: Arc::new(RwLock::new(map)),
        }
    }

    pub fn list(&self) -> Vec<ProviderDescriptor> {
        let map = self.providers.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<ProviderDescriptor> {
        let map = self.providers.read().unwrap();
        map.get(id).cloned()
    }

    pub fn register(&self, descriptor: ProviderDescriptor) {
        let mut map = self.providers.write().unwrap();
        map.insert(descriptor.id.clone(), descriptor);
    }
}

pub fn resolve_command(cmd: &str) -> Option<std::path::PathBuf> {
    let path_cmd = std::path::Path::new(cmd);
    if path_cmd.is_file() {
        return Some(path_cmd.to_path_buf());
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let direct = dir.join(cmd);
            if direct.is_file() {
                return Some(direct);
            }
            #[cfg(windows)]
            {
                for ext in &["exe", "cmd", "bat"] {
                    let with_ext = dir.join(format!("{}.{}", cmd, ext));
                    if with_ext.is_file() {
                        return Some(with_ext);
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        if let Some(home) = dirs::home_dir() {
            let candidate_dirs = [
                home.join(".bun").join("bin"),
                home.join("AppData").join("Local").join("Programs").join("OpenAI").join("Codex").join("bin"),
                home.join(".cargo").join("bin"),
                home.join("AppData").join("Roaming").join("npm"),
                home.join(".local").join("bin"),
            ];
            for dir in &candidate_dirs {
                let direct = dir.join(cmd);
                if direct.is_file() {
                    return Some(direct);
                }
                for ext in &["exe", "cmd", "bat"] {
                    let with_ext = dir.join(format!("{}.{}", cmd, ext));
                    if with_ext.is_file() {
                        return Some(with_ext);
                    }
                }
            }
        }
    }

    None
}

pub fn is_command_available(cmd: &str) -> bool {
    resolve_command(cmd).is_some()
}

pub fn is_local_port_open(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(40)).is_ok()
}

pub fn resolve_provider_for_model(model: &str) -> Option<&'static str> {
    let lower = model.to_lowercase();
    if lower.starts_with("ollama/") {
        Some("ollama")
    } else if lower.starts_with("lmstudio/") {
        Some("lmstudio")
    } else if lower.starts_with("opencode/") || lower.starts_with("omniroute/") {
        Some("opencode")
    } else if lower.starts_with("gpt-") || lower.starts_with("codex") {
        Some("codex")
    } else if lower.starts_with("claude") {
        Some("claude")
    } else if lower.starts_with("pi/") || lower.starts_with("pi-") {
        Some("pi")
    } else if lower.starts_with("omp/") || lower.contains('/') {
        Some("omp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_registry_builtins() {
        let registry = ProviderRegistry::new();
        let providers = registry.list();
        assert!(providers.len() >= 7);

        let claude = registry.get("claude").expect("Claude provider");
        assert_eq!(claude.id, "claude");

        let mock = registry.get("mock").expect("Mock provider");
        assert!(mock.is_installed);
    }

    #[test]
    fn test_is_command_available_resolution() {
        assert!(is_command_available("cmd") || is_command_available("sh"));
        assert!(!is_command_available("definitely_nonexistent_binary_xyz_12345"));
    }
}
