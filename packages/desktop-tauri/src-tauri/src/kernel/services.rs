use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleRecord {
    pub id: String,
    pub name: String,
    pub interval_secs: u64,
    pub prompt: String,
    pub cwd: String,
    pub enabled: bool,
    pub last_run_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HostRecord {
    pub id: String,
    pub label: String,
    pub connection_uri: String,
    pub is_online: bool,
    pub last_seen_at: Option<String>,
}

pub struct HubServicesManager {
    schedules: Arc<RwLock<HashMap<String, ScheduleRecord>>>,
    hosts: Arc<RwLock<HashMap<String, HostRecord>>>,
}

impl Default for HubServicesManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HubServicesManager {
    pub fn new() -> Self {
        Self {
            schedules: Arc::new(RwLock::new(HashMap::new())),
            hosts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn add_schedule(&self, record: ScheduleRecord) {
        let mut map = self.schedules.write().unwrap();
        map.insert(record.id.clone(), record);
    }

    pub fn list_schedules(&self) -> Vec<ScheduleRecord> {
        let map = self.schedules.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn delete_schedule(&self, id: &str) -> bool {
        let mut map = self.schedules.write().unwrap();
        map.remove(id).is_some()
    }

    pub fn add_host(&self, record: HostRecord) {
        let mut map = self.hosts.write().unwrap();
        map.insert(record.id.clone(), record);
    }

    pub fn list_hosts(&self) -> Vec<HostRecord> {
        let map = self.hosts.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn remove_host(&self, id: &str) -> bool {
        let mut map = self.hosts.write().unwrap();
        map.remove(id).is_some()
    }
}

static HUB_SERVICES: std::sync::OnceLock<HubServicesManager> = std::sync::OnceLock::new();

pub fn get_hub_services() -> &'static HubServicesManager {
    HUB_SERVICES.get_or_init(HubServicesManager::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_and_host_services() {
        let manager = HubServicesManager::new();

        let sched = ScheduleRecord {
            id: "sched_1".to_string(),
            name: "Nightly Build".to_string(),
            interval_secs: 86400,
            prompt: "Run full verification".to_string(),
            cwd: "/workspace".to_string(),
            enabled: true,
            last_run_at: None,
        };

        manager.add_schedule(sched.clone());
        assert_eq!(manager.list_schedules().len(), 1);
        assert_eq!(manager.list_schedules()[0], sched);

        let host = HostRecord {
            id: "host_1".to_string(),
            label: "Linux Server".to_string(),
            connection_uri: "tcp://192.168.1.100:6767".to_string(),
            is_online: true,
            last_seen_at: Some("2026-11-02T00:00:00Z".to_string()),
        };

        manager.add_host(host.clone());
        assert_eq!(manager.list_hosts().len(), 1);
        assert_eq!(manager.list_hosts()[0], host);

        assert!(manager.delete_schedule("sched_1"));
        assert!(manager.remove_host("host_1"));
        assert_eq!(manager.list_schedules().len(), 0);
        assert_eq!(manager.list_hosts().len(), 0);
    }
}
