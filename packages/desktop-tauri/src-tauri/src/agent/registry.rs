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

        // 6. Generic ACP
        map.insert(
            "acp".to_string(),
            ProviderDescriptor {
                id: "acp".to_string(),
                label: "Agent Client Protocol".to_string(),
                description: "ACP stdio agent protocol runner (Cursor, Copilot, Trae)".to_string(),
                is_installed: true,
            },
        );

        // 7. Mock
        map.insert(
            "mock".to_string(),
            ProviderDescriptor {
                id: "mock".to_string(),
                label: "Mock Agent".to_string(),
                description: "Mock agent for testing, benchmarking, and development".to_string(),
                is_installed: true,
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

pub fn is_command_available(cmd: &str) -> bool {
    let check_cmd = if cfg!(windows) { "where" } else { "which" };
    std::process::Command::new(check_cmd)
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
}
