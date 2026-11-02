use crate::agent::registry::ProviderRegistry;
use crate::agent::storage::AgentStorage;
use crate::agent::types::{AgentStatus, AgentTimelineItem, StoredAgentRecord};
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
            .ok_or_else(|| format!("Unknown agent provider: {provider}"))?;

        let id = format!("agent_{}", uuid_v4_simple());
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
        let _ = self
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

fn now_iso() -> String {
    // Basic UTC ISO-8601 formatting
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis())
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

        manager.cancel_agent(&created.id).expect("Cancel agent");
        let cancelled = manager.get_agent(&created.id).unwrap();
        assert_eq!(cancelled.last_status, AgentStatus::Closed);

        assert!(manager.delete_agent(&created.id).unwrap());
        assert_eq!(manager.list_agents().len(), 0);
    }
}
