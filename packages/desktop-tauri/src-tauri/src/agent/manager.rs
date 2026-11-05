use crate::agent::registry::ProviderRegistry;
use crate::agent::storage::AgentStorage;
use crate::agent::types::{now_iso, AgentStatus, AgentTimelineItem, StoredAgentRecord};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

pub struct AgentManager {
    storage: Arc<AgentStorage>,
    registry: Arc<ProviderRegistry>,
    active_statuses: Arc<RwLock<HashMap<String, AgentStatus>>>,
}

impl AgentManager {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        let storage = Arc::new(AgentStorage::new(home_dir));
        let registry = Arc::new(ProviderRegistry::new());
        Self {
            storage,
            registry,
            active_statuses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn storage(&self) -> &Arc<AgentStorage> {
        &self.storage
    }

    pub fn registry(&self) -> &Arc<ProviderRegistry> {
        &self.registry
    }

    pub fn create_agent(
        &self,
        provider: &str,
        cwd: &str,
        title: Option<String>,
        workspace_id: Option<String>,
        config: Option<Value>,
    ) -> Result<StoredAgentRecord, String> {
        let provider_desc = self
            .registry
            .get(provider)
            .unwrap_or_else(|| crate::agent::types::ProviderDescriptor {
                id: provider.to_string(),
                label: provider.to_string(),
                description: format!("Agent provider {provider}"),
                is_installed: true,
            });

        let id = format!("ag_{}", uuid_v4_simple());
        let now = now_iso();

        let record = StoredAgentRecord {
            id: id.clone(),
            provider: provider_desc.id,
            cwd: cwd.to_string(),
            workspace_id,
            created_at: now.clone(),
            updated_at: now,
            last_activity_at: None,
            title,
            labels: HashMap::new(),
            last_status: AgentStatus::Idle,
            last_error: None,
            config,
            runtime_info: None,
        };

        self.storage
            .save_record(record.clone())
            .map_err(|e| format!("Failed to save agent record: {e}"))?;

        self.active_statuses
            .write()
            .unwrap()
            .insert(id, AgentStatus::Idle);

        Ok(record)
    }

    pub fn get_agent(&self, id: &str) -> Option<StoredAgentRecord> {
        let mut record = self.storage.get_record(id)?;
        if let Some(status) = self.active_statuses.read().unwrap().get(id) {
            record.last_status = *status;
        }
        Some(record)
    }

    pub fn list_agents(&self) -> Vec<StoredAgentRecord> {
        let mut list = self.storage.list_records();
        let statuses = self.active_statuses.read().unwrap();
        for rec in &mut list {
            if let Some(status) = statuses.get(&rec.id) {
                rec.last_status = *status;
            }
        }
        list
    }

    pub fn send_message(&self, id: &str, text: &str) -> Result<(), String> {
        let _record = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;

        let msg_id = format!("msg_{}", uuid_v4_simple());
        let item = AgentTimelineItem::UserMessage {
            id: msg_id,
            text: text.to_string(),
            timestamp: now_iso(),
        };

        self.storage
            .append_timeline_item(id, item)
            .map_err(|e| format!("Failed to append message: {e}"))?;

        self.active_statuses
            .write()
            .unwrap()
            .insert(id.to_string(), AgentStatus::Running);


        Ok(())
    }
    pub fn append_assistant_message(&self, id: &str, text: &str) -> Result<(), String> {
        let msg_id = format!("msg_{}", uuid_v4_simple());
        let item = AgentTimelineItem::AssistantMessage {
            id: msg_id,
            text: text.to_string(),
            timestamp: now_iso(),
        };
        self.storage
            .append_timeline_item(id, item)
            .map_err(|e| format!("Failed to append assistant message: {e}"))
    }

    pub fn append_thinking(&self, id: &str, text: &str) -> Result<(), String> {
        let thk_id = format!("thk_{}", uuid_v4_simple());
        let item = AgentTimelineItem::Thinking {
            id: thk_id,
            text: text.to_string(),
            timestamp: now_iso(),
        };
        self.storage
            .append_timeline_item(id, item)
            .map_err(|e| format!("Failed to append thinking: {e}"))
    }

    pub fn append_error_notice(&self, id: &str, message: &str) -> Result<(), String> {
        let err_id = format!("err_{}", uuid_v4_simple());
        let item = AgentTimelineItem::ErrorNotice {
            id: err_id,
            message: message.to_string(),
            timestamp: now_iso(),
        };
        self.storage
            .append_timeline_item(id, item)
            .map_err(|e| format!("Failed to append error notice: {e}"))
    }

    pub fn set_agent_status(&self, id: &str, status: AgentStatus) -> Result<(), String> {
        let mut record = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;
        record.last_status = status;
        record.updated_at = now_iso();
        self.active_statuses
            .write()
            .unwrap()
            .insert(id.to_string(), status);
        self.storage
            .save_record(record)
            .map_err(|e| format!("Failed to save status: {e}"))
    }

    pub fn set_agent_model(&self, id: &str, model: &str) -> Result<StoredAgentRecord, String> {
        self.set_agent_model_with_provider(id, model, None)
    }

    pub fn set_agent_model_with_provider(
        &self,
        id: &str,
        model: &str,
        provider: Option<&str>,
    ) -> Result<StoredAgentRecord, String> {
        let mut record = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;
        let mut config = record.config.unwrap_or_else(|| serde_json::json!({}));
        let resolved_provider = provider
            .map(|p| p.to_string())
            .or_else(|| crate::agent::registry::resolve_provider_for_model(model).map(|p| p.to_string()));
        if let Some(obj) = config.as_object_mut() {
            if let Some(ref p) = resolved_provider {
                record.provider = p.clone();
                obj.insert("provider".to_string(), serde_json::json!(p));
            }
            if obj.get("model").and_then(Value::as_str) != Some(model) {
                // Effort IDs are model-specific; a stale level can be invalid for the new model.
                obj.remove("thinkingOptionId");
            }
            obj.insert("model".to_string(), serde_json::json!(model));
        }
        record.config = Some(config);
        record.updated_at = now_iso();
        self.storage
            .save_record(record.clone())
            .map_err(|e| format!("Failed to save record: {e}"))?;
        Ok(record)
    }

    pub fn set_agent_thinking(
        &self,
        id: &str,
        thinking_option_id: Option<&str>,
    ) -> Result<StoredAgentRecord, String> {
        let mut record = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;
        let mut config = record.config.unwrap_or_else(|| serde_json::json!({}));
        let obj = config
            .as_object_mut()
            .ok_or_else(|| "Agent config must be an object".to_string())?;
        if let Some(option) = thinking_option_id.filter(|option| !option.trim().is_empty()) {
            obj.insert("thinkingOptionId".to_string(), serde_json::json!(option));
        } else {
            obj.remove("thinkingOptionId");
        }
        record.config = Some(config);
        record.updated_at = now_iso();
        self.storage
            .save_record(record.clone())
            .map_err(|e| format!("Failed to save thinking setting: {e}"))?;
        Ok(record)
    }

    pub fn get_timeline(&self, id: &str) -> Vec<AgentTimelineItem> {
        self.storage.get_timeline(id)
    }

    pub fn cancel_agent(&self, id: &str) -> Result<(), String> {
        let _ = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;

        self.active_statuses
            .write()
            .unwrap()
            .insert(id.to_string(), AgentStatus::Closed);

        Ok(())
    }

    pub fn delete_agent(&self, id: &str) -> Result<bool, String> {
        self.active_statuses.write().unwrap().remove(id);
        self.storage
            .delete_record(id)
            .map_err(|e| format!("Failed to delete agent: {e}"))
    }
}


fn uuid_v4_simple() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}_{:x}", now.as_nanos(), std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_thinking_setting_survives_reload_and_can_be_cleared() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());
        let agent = manager.create_agent(
            "codex", ".", None, None,
            Some(serde_json::json!({"model": "runtime-model", "thinkingOptionId": "high"})),
        ).unwrap();
        manager.set_agent_thinking(&agent.id, Some("low")).unwrap();
        let reloaded = AgentManager::new(dir.path());
        let record = reloaded.get_agent(&agent.id).unwrap();
        assert_eq!(record.config.as_ref().unwrap()["thinkingOptionId"], "low");
        assert_eq!(record.config.as_ref().unwrap()["model"], "runtime-model");
        reloaded.set_agent_thinking(&agent.id, None).unwrap();
        let cleared = AgentManager::new(dir.path()).get_agent(&agent.id).unwrap();
        assert!(cleared.config.as_ref().unwrap().get("thinkingOptionId").is_none());
        assert_eq!(cleared.config.as_ref().unwrap()["model"], "runtime-model");
        assert!(manager.set_agent_thinking("missing-thinking-agent", Some("high")).is_err());
    }

    #[test]
    fn test_thinking_choice_is_not_carried_to_a_different_model() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());
        let agent = manager.create_agent(
            "codex", ".", None, None,
            Some(serde_json::json!({"model": "first", "thinkingOptionId": "xhigh"})),
        ).unwrap();
        let unchanged = manager.set_agent_model(&agent.id, "first").unwrap();
        assert_eq!(unchanged.config.as_ref().unwrap()["thinkingOptionId"], "xhigh");
        manager.set_agent_model(&agent.id, "second").unwrap();
        let reloaded = AgentManager::new(dir.path()).get_agent(&agent.id).unwrap();
        assert_eq!(reloaded.config.as_ref().unwrap()["model"], "second");
        assert!(reloaded.config.as_ref().unwrap().get("thinkingOptionId").is_none());
    }

    #[test]
    fn test_agent_manager_crud_and_timeline() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());

        let created = manager
            .create_agent(
                "mock",
                "/tmp/test-project",
                Some("Test Agent".to_string()),
                Some("wks-1".to_string()),
                None,
            )
            .expect("Create agent");

        assert_eq!(created.provider, "mock");
        assert_eq!(created.cwd, "/tmp/test-project");
        assert_eq!(created.last_status, AgentStatus::Idle);

        let retrieved = manager.get_agent(&created.id).expect("Get agent");
        assert_eq!(retrieved.title, Some("Test Agent".to_string()));

        manager
            .send_message(&created.id, "Run test command")
            .expect("Send message");

        let timeline = manager.get_timeline(&created.id);
        assert_eq!(timeline.len(), 1);

        let updated = manager.get_agent(&created.id).unwrap();
        assert_eq!(updated.last_status, AgentStatus::Running);
        manager
            .append_thinking(&created.id, "Analyzing project structure...")
            .expect("Append thinking");
        manager
            .append_assistant_message(&created.id, "Hello! I can help you with this project.")
            .expect("Append assistant message");
        manager
            .append_error_notice(&created.id, "Minor warning: cache cold")
            .expect("Append error notice");

        let timeline = manager.get_timeline(&created.id);
        assert_eq!(timeline.len(), 4);

        let updated_record = manager
            .set_agent_model(&created.id, "gpt-6-luna")
            .expect("Set agent model");
        assert_eq!(
            updated_record
                .config
                .as_ref()
                .and_then(|c| c.get("model"))
                .and_then(|v| v.as_str()),
            Some("gpt-6-luna")
        );

        manager.cancel_agent(&created.id).expect("Cancel agent");
        let cancelled = manager.get_agent(&created.id).unwrap();
        assert_eq!(cancelled.last_status, AgentStatus::Closed);

        assert!(manager.delete_agent(&created.id).unwrap());
        assert_eq!(manager.list_agents().len(), 0);
    }
}
