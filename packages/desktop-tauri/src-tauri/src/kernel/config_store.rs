use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub mcp_enabled: bool,
    #[serde(default = "default_false")]
    pub relay_enabled: bool,
    #[serde(default = "default_relay_endpoint")]
    pub relay_endpoint: String,
    #[serde(default)]
    pub password_hash: Option<String>,
    #[serde(default)]
    pub custom_settings: HashMap<String, Value>,
}

fn default_port() -> u16 {
    6767
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_relay_endpoint() -> String {
    "relay.paseo.sh:443".to_string()
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            mcp_enabled: default_true(),
            relay_enabled: default_false(),
            relay_endpoint: default_relay_endpoint(),
            password_hash: None,
            custom_settings: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    #[serde(default)]
    pub default_workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub path: String,
    pub kind: String, // "directory" | "worktree"
}

pub struct DaemonConfigStore {
    home_dir: PathBuf,
    config: Arc<RwLock<DaemonConfig>>,
    projects: Arc<RwLock<HashMap<String, ProjectRecord>>>,
    workspaces: Arc<RwLock<HashMap<String, WorkspaceRecord>>>,
}

impl DaemonConfigStore {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        let home = home_dir.into();
        let config = Self::load_config(&home).unwrap_or_default();
        let projects = Self::load_projects(&home).unwrap_or_default();
        let workspaces = Self::load_workspaces(&home).unwrap_or_default();

        Self {
            home_dir: home,
            config: Arc::new(RwLock::new(config)),
            projects: Arc::new(RwLock::new(projects)),
            workspaces: Arc::new(RwLock::new(workspaces)),
        }
    }

    pub fn get_config(&self) -> DaemonConfig {
        self.config.read().unwrap().clone()
    }

    pub fn update_config<F>(&self, update_fn: F) -> std::io::Result<DaemonConfig>
    where
        F: FnOnce(&mut DaemonConfig),
    {
        let mut cfg = self.config.write().unwrap();
        update_fn(&mut cfg);
        Self::save_config_atomic(&self.home_dir, &cfg)?;
        Ok(cfg.clone())
    }

    pub fn add_project(&self, project: ProjectRecord) -> std::io::Result<()> {
        let mut map = self.projects.write().unwrap();
        map.insert(project.id.clone(), project);
        Self::save_projects_atomic(&self.home_dir, &map)?;
        Ok(())
    }

    pub fn list_projects(&self) -> Vec<ProjectRecord> {
        self.projects.read().unwrap().values().cloned().collect()
    }

    pub fn get_project(&self, id: &str) -> Option<ProjectRecord> {
        self.projects.read().unwrap().get(id).cloned()
    }

    pub fn add_workspace(&self, workspace: WorkspaceRecord) -> std::io::Result<()> {
        let mut map = self.workspaces.write().unwrap();
        map.insert(workspace.id.clone(), workspace);
        Self::save_workspaces_atomic(&self.home_dir, &map)?;
        Ok(())
    }

    pub fn get_workspace(&self, id: &str) -> Option<WorkspaceRecord> {
        self.workspaces.read().unwrap().get(id).cloned()
    }

    fn load_config(home: &Path) -> Option<DaemonConfig> {
        let file = home.join("config.json");
        let content = fs::read_to_string(file).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn load_projects(home: &Path) -> Option<HashMap<String, ProjectRecord>> {
        let file = home.join("projects").join("projects.json");
        let content = fs::read_to_string(file).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn load_workspaces(home: &Path) -> Option<HashMap<String, WorkspaceRecord>> {
        let file = home.join("projects").join("workspaces.json");
        let content = fs::read_to_string(file).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_config_atomic(home: &Path, config: &DaemonConfig) -> std::io::Result<()> {
        let target = home.join("config.json");
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&target, json.as_bytes())
    }

    fn save_projects_atomic(
        home: &Path,
        projects: &HashMap<String, ProjectRecord>,
    ) -> std::io::Result<()> {
        let target = home.join("projects").join("projects.json");
        let json = serde_json::to_string_pretty(projects)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&target, json.as_bytes())
    }

    fn save_workspaces_atomic(
        home: &Path,
        workspaces: &HashMap<String, WorkspaceRecord>,
    ) -> std::io::Result<()> {
        let target = home.join("projects").join("workspaces.json");
        let json = serde_json::to_string_pretty(workspaces)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&target, json.as_bytes())
    }
}

pub fn atomic_write(target_path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_path = target_path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&temp_path, content)?;
    fs::rename(&temp_path, target_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_daemon_config_store_lifecycle() {
        let dir = tempdir().unwrap();
        let store = DaemonConfigStore::new(dir.path());

        let cfg = store.get_config();
        assert_eq!(cfg.port, 6767);
        assert!(cfg.mcp_enabled);

        store
            .update_config(|c| {
                c.port = 6789;
                c.relay_enabled = true;
            })
            .expect("Update config");

        let reloaded = DaemonConfigStore::new(dir.path());
        let updated = reloaded.get_config();
        assert_eq!(updated.port, 6789);
        assert!(updated.relay_enabled);
    }

    #[test]
    fn test_project_and_workspace_registry() {
        let dir = tempdir().unwrap();
        let store = DaemonConfigStore::new(dir.path());

        let proj = ProjectRecord {
            id: "prj_test1".to_string(),
            name: "My Project".to_string(),
            path: "/path/to/my-project".to_string(),
            created_at: "2026-11-02T00:00:00Z".to_string(),
            default_workspace_id: Some("wks_default".to_string()),
        };

        store.add_project(proj.clone()).expect("Add project");
        let projects = store.list_projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0], proj);

        let wks = WorkspaceRecord {
            id: "wks_default".to_string(),
            project_id: "prj_test1".to_string(),
            name: "Default".to_string(),
            path: "/path/to/my-project".to_string(),
            kind: "directory".to_string(),
        };

        store.add_workspace(wks.clone()).expect("Add workspace");
        assert_eq!(store.get_workspace("wks_default"), Some(wks));
    }
}
