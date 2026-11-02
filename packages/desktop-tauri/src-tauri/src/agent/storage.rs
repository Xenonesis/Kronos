use crate::agent::types::{AgentTimelineItem, StoredAgentRecord};
use crate::kernel::config_store::atomic_write;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub struct AgentStorage {
    home_dir: PathBuf,
    records: Arc<RwLock<HashMap<String, StoredAgentRecord>>>,
    timelines: Arc<RwLock<HashMap<String, Vec<AgentTimelineItem>>>>,
}

impl AgentStorage {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        let home = home_dir.into();
        let records = Self::load_records(&home).unwrap_or_default();
        Self {
            home_dir: home,
            records: Arc::new(RwLock::new(records)),
            timelines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn save_record(&self, record: StoredAgentRecord) -> std::io::Result<()> {
        let mut map = self.records.write().unwrap();
        map.insert(record.id.clone(), record);
        Self::persist_records_atomic(&self.home_dir, &map)
    }

    pub fn get_record(&self, id: &str) -> Option<StoredAgentRecord> {
        self.records.read().unwrap().get(id).cloned()
    }

    pub fn list_records(&self) -> Vec<StoredAgentRecord> {
        let mut list: Vec<StoredAgentRecord> = self.records.read().unwrap().values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub fn delete_record(&self, id: &str) -> std::io::Result<bool> {
        let mut map = self.records.write().unwrap();
        if map.remove(id).is_some() {
            Self::persist_records_atomic(&self.home_dir, &map)?;
            let timeline_file = self.home_dir.join("agents").join("timelines").join(format!("{id}.json"));
            if timeline_file.exists() {
                let _ = fs::remove_file(timeline_file);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn append_timeline_item(&self, agent_id: &str, item: AgentTimelineItem) -> std::io::Result<()> {
        let mut map = self.timelines.write().unwrap();
        let timeline = map.entry(agent_id.to_string()).or_insert_with(|| {
            Self::load_timeline(&self.home_dir, agent_id).unwrap_or_default()
        });
        timeline.push(item);
        Self::persist_timeline_atomic(&self.home_dir, agent_id, timeline)
    }

    pub fn get_timeline(&self, agent_id: &str) -> Vec<AgentTimelineItem> {
        let mut map = self.timelines.write().unwrap();
        map.entry(agent_id.to_string())
            .or_insert_with(|| Self::load_timeline(&self.home_dir, agent_id).unwrap_or_default())
            .clone()
    }

    fn load_records(home: &Path) -> Option<HashMap<String, StoredAgentRecord>> {
        let file = home.join("agents").join("records.json");
        let content = fs::read_to_string(file).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn persist_records_atomic(home: &Path, records: &HashMap<String, StoredAgentRecord>) -> std::io::Result<()> {
        let target = home.join("agents").join("records.json");
        let json = serde_json::to_string_pretty(records)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&target, json.as_bytes())
    }

    fn load_timeline(home: &Path, agent_id: &str) -> Option<Vec<AgentTimelineItem>> {
        let file = home.join("agents").join("timelines").join(format!("{agent_id}.json"));
        let content = fs::read_to_string(file).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn persist_timeline_atomic(home: &Path, agent_id: &str, items: &[AgentTimelineItem]) -> std::io::Result<()> {
        let target = home.join("agents").join("timelines").join(format!("{agent_id}.json"));
        let json = serde_json::to_string_pretty(items)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&target, json.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::AgentStatus;
    use tempfile::tempdir;

    #[test]
    fn test_agent_storage_lifecycle() {
        let dir = tempdir().unwrap();
        let storage = AgentStorage::new(dir.path());

        let record = StoredAgentRecord {
            id: "agent-1".to_string(),
            provider: "claude".to_string(),
            cwd: "/tmp/project".to_string(),
            workspace_id: Some("wks-1".to_string()),
            created_at: "2026-11-02T01:00:00Z".to_string(),
            updated_at: "2026-11-02T01:00:00Z".to_string(),
            last_activity_at: None,
            title: Some("Implement Tests".to_string()),
            labels: HashMap::new(),
            last_status: AgentStatus::Idle,
            last_error: None,
            config: None,
            runtime_info: None,
        };

        storage.save_record(record.clone()).expect("Save record");
        assert_eq!(storage.list_records().len(), 1);
        assert_eq!(storage.get_record("agent-1"), Some(record));

        storage
            .append_timeline_item(
                "agent-1",
                AgentTimelineItem::UserMessage {
                    id: "msg-1".to_string(),
                    text: "Hello".to_string(),
                    timestamp: "2026-11-02T01:00:01Z".to_string(),
                },
            )
            .expect("Append message");

        let timeline = storage.get_timeline("agent-1");
        assert_eq!(timeline.len(), 1);

        // Verify persistence from disk reload
        let reloaded = AgentStorage::new(dir.path());
        assert_eq!(reloaded.list_records().len(), 1);
        assert_eq!(reloaded.get_timeline("agent-1").len(), 1);

        assert!(storage.delete_record("agent-1").unwrap());
        assert_eq!(storage.list_records().len(), 0);
    }
}
