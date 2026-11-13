use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DaemonConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub mcp_enabled: bool,
    #[serde(default = "default_false")]
    pub inject_into_agents: bool,
    #[serde(default = "default_false")]
    pub relay_enabled: bool,
    #[serde(default = "default_relay_endpoint")]
    pub relay_endpoint: String,
    #[serde(default)]
    pub password_hash: Option<String>,
    #[serde(default = "default_false")]
    pub auto_archive_after_merge: bool,
    #[serde(default = "default_false")]
    pub enable_terminal_agent_hooks: bool,
    #[serde(default = "default_empty_object")]
    pub providers_config: Value,
    #[serde(default)]
    pub custom_settings: HashMap<String, Value>,
}

fn default_empty_object() -> Value {
    serde_json::json!({})
}

fn default_release_channel() -> String {
    "stable".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopNotificationSettings {
    #[serde(default = "default_true")]
    pub play_sound: bool,
}

impl Default for DesktopNotificationSettings {
    fn default() -> Self {
        Self { play_sound: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopDaemonSettings {
    #[serde(default = "default_true")]
    pub manage_built_in_daemon: bool,
    #[serde(default = "default_false")]
    pub keep_running_after_quit: bool,
}

impl Default for DesktopDaemonSettings {
    fn default() -> Self {
        Self {
            manage_built_in_daemon: true,
            keep_running_after_quit: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsConfig {
    #[serde(default = "default_release_channel")]
    pub release_channel: String,
    #[serde(default)]
    pub notifications: DesktopNotificationSettings,
    #[serde(default)]
    pub daemon: DesktopDaemonSettings,
}

impl Default for DesktopSettingsConfig {
    fn default() -> Self {
        Self {
            release_channel: default_release_channel(),
            notifications: DesktopNotificationSettings::default(),
            daemon: DesktopDaemonSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLabelRecord {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLabelRemoval {
    pub name: String,
    pub seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersistedLabelCatalog {
    #[serde(default = "default_label_generation")]
    pub generation: String,
    #[serde(default)]
    pub head_seq: u64,
    #[serde(default)]
    pub labels: Vec<WorkspaceLabelRecord>,
    #[serde(default)]
    pub removals: Vec<WorkspaceLabelRemoval>,
    #[serde(default)]
    pub assignments: HashMap<String, Vec<String>>,
}

fn default_label_generation() -> String {
    "kronos-labels-v1".to_string()
}

impl Default for PersistedLabelCatalog {
    fn default() -> Self {
        Self {
            generation: default_label_generation(),
            head_seq: 0,
            labels: Vec::new(),
            removals: Vec::new(),
            assignments: HashMap::new(),
        }
    }
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
            inject_into_agents: default_false(),
            relay_enabled: default_false(),
            relay_endpoint: default_relay_endpoint(),
            password_hash: None,
            auto_archive_after_merge: default_false(),
            enable_terminal_agent_hooks: default_false(),
            providers_config: default_empty_object(),
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
        self.config.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn update_config<F>(&self, update_fn: F) -> std::io::Result<DaemonConfig>
    where
        F: FnOnce(&mut DaemonConfig),
    {
        let mut cfg = self.config.write().unwrap_or_else(|e| e.into_inner());
        update_fn(&mut cfg);
        Self::save_config_atomic(&self.home_dir, &cfg)?;
        Ok(cfg.clone())
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn get_custom_setting<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let cfg = self.config.read().unwrap_or_else(|e| e.into_inner());
        cfg.custom_settings
            .get(key)
            .and_then(|val| serde_json::from_value::<T>(val.clone()).ok())
    }

    pub fn set_custom_setting<T: Serialize>(&self, key: &str, val: &T) -> std::io::Result<()> {
        let serialized = serde_json::to_value(val)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.update_config(|cfg| {
            cfg.custom_settings.insert(key.to_string(), serialized);
        })?;
        Ok(())
    }

    pub fn get_desktop_settings(&self) -> DesktopSettingsConfig {
        self.get_custom_setting::<DesktopSettingsConfig>("desktopSettings")
            .unwrap_or_default()
    }

    pub fn patch_desktop_settings(&self, patch: Value) -> std::io::Result<DesktopSettingsConfig> {
        let mut current = self.get_desktop_settings();
        if let Some(channel) = patch.get("releaseChannel").and_then(|v| v.as_str()) {
            if channel == "stable" || channel == "beta" {
                current.release_channel = channel.to_string();
            }
        }
        if let Some(notif) = patch.get("notifications") {
            if let Some(play_sound) = notif.get("playSound").and_then(|v| v.as_bool()) {
                current.notifications.play_sound = play_sound;
            }
        }
        if let Some(daemon) = patch.get("daemon") {
            if let Some(manage) = daemon.get("manageBuiltInDaemon").and_then(|v| v.as_bool()) {
                current.daemon.manage_built_in_daemon = manage;
            }
            if let Some(keep) = daemon.get("keepRunningAfterQuit").and_then(|v| v.as_bool()) {
                current.daemon.keep_running_after_quit = keep;
            }
        }
        self.set_custom_setting("desktopSettings", &current)?;
        Ok(current)
    }

    pub fn get_labels_catalog(&self) -> PersistedLabelCatalog {
        let file = self.home_dir.join("labels.json");
        fs::read_to_string(file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_labels_catalog(&self, catalog: &PersistedLabelCatalog) -> std::io::Result<()> {
        let target = self.home_dir.join("labels.json");
        let json = serde_json::to_string_pretty(catalog)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(&target, json.as_bytes())
    }

    pub fn set_workspace_label_assignment(
        &self,
        workspace_id: &str,
        label: WorkspaceLabelRecord,
        assigned: bool,
    ) -> std::io::Result<(PersistedLabelCatalog, Vec<String>)> {
        let mut catalog = self.get_labels_catalog();
        if !catalog.labels.iter().any(|l| l.name == label.name) {
            catalog.labels.push(label.clone());
            catalog.head_seq += 1;
            catalog.removals.retain(|r| r.name != label.name);
        } else if let Some(existing) = catalog.labels.iter_mut().find(|l| l.name == label.name) {
            if existing.color != label.color {
                existing.color = label.color.clone();
                catalog.head_seq += 1;
            }
        }
        let entry = catalog
            .assignments
            .entry(workspace_id.to_string())
            .or_default();
        if assigned {
            if !entry.contains(&label.name) {
                entry.push(label.name.clone());
            }
        } else {
            entry.retain(|name| name != &label.name);
        }
        let ws_labels = entry.clone();
        self.save_labels_catalog(&catalog)?;
        Ok((catalog, ws_labels))
    }

    pub fn update_label_definition(
        &self,
        name: &str,
        new_name: Option<&str>,
        color: Option<&str>,
    ) -> std::io::Result<Option<(WorkspaceLabelRecord, usize, PersistedLabelCatalog)>> {
        let mut catalog = self.get_labels_catalog();
        let Some(pos) = catalog.labels.iter().position(|l| l.name == name) else {
            return Ok(None);
        };
        let target_name = new_name.unwrap_or(name).to_string();
        if target_name != name {
            catalog.labels[pos].name = target_name.clone();
        }
        if let Some(c) = color {
            catalog.labels[pos].color = c.to_string();
        }
        let updated = catalog.labels[pos].clone();
        let mut affected = 0usize;
        if target_name != name {
            for list in catalog.assignments.values_mut() {
                let mut changed = false;
                for item in list.iter_mut() {
                    if item == name {
                        *item = target_name.clone();
                        changed = true;
                    }
                }
                if changed {
                    affected += 1;
                }
            }
        } else {
            affected = catalog
                .assignments
                .values()
                .filter(|list| list.iter().any(|item| item == name))
                .count();
        }
        catalog.head_seq += 1;
        self.save_labels_catalog(&catalog)?;
        Ok(Some((updated, affected, catalog)))
    }

    pub fn inspect_label_delete(&self, name: &str) -> usize {
        let catalog = self.get_labels_catalog();
        catalog
            .assignments
            .values()
            .filter(|list| list.iter().any(|item| item == name))
            .count()
    }

    pub fn delete_label_definition(
        &self,
        name: &str,
    ) -> std::io::Result<(usize, PersistedLabelCatalog)> {
        let mut catalog = self.get_labels_catalog();
        let before = catalog.labels.len();
        catalog.labels.retain(|l| l.name != name);
        let mut affected = 0usize;
        for list in catalog.assignments.values_mut() {
            let prev = list.len();
            list.retain(|item| item != name);
            if list.len() != prev {
                affected += 1;
            }
        }
        if catalog.labels.len() != before || affected > 0 {
            catalog.head_seq += 1;
            let seq = catalog.head_seq;
            catalog.removals.push(WorkspaceLabelRemoval {
                name: name.to_string(),
                seq,
            });
            self.save_labels_catalog(&catalog)?;
        }
        Ok((affected, catalog))
    }

    pub fn add_project(&self, project: ProjectRecord) -> std::io::Result<()> {
        let mut map = self.projects.write().unwrap_or_else(|e| e.into_inner());
        map.insert(project.id.clone(), project);
        Self::save_projects_atomic(&self.home_dir, &map)?;
        Ok(())
    }

    pub fn list_projects(&self) -> Vec<ProjectRecord> {
        self.projects.read().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
    }

    pub fn get_project(&self, id: &str) -> Option<ProjectRecord> {
        self.projects.read().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
    }
    pub fn remove_project(&self, id: &str) -> std::io::Result<Option<ProjectRecord>> {
        let mut map = self.projects.write().unwrap_or_else(|e| e.into_inner());
        let removed = map.remove(id);
        Self::save_projects_atomic(&self.home_dir, &map)?;
        Ok(removed)
    }

    pub fn add_workspace(&self, workspace: WorkspaceRecord) -> std::io::Result<()> {
        let mut map = self.workspaces.write().unwrap_or_else(|e| e.into_inner());
        map.insert(workspace.id.clone(), workspace);
        Self::save_workspaces_atomic(&self.home_dir, &map)?;
        Ok(())
    }

    pub fn get_workspace(&self, id: &str) -> Option<WorkspaceRecord> {
        self.workspaces.read().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
    }
    pub fn list_workspaces(&self) -> Vec<WorkspaceRecord> {
        self.workspaces.read().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
    }

    pub fn remove_workspaces_for_project(
        &self,
        project_id: &str,
    ) -> std::io::Result<Vec<WorkspaceRecord>> {
        let mut map = self.workspaces.write().unwrap_or_else(|e| e.into_inner());
        let mut removed = Vec::new();
        map.retain(|_, ws| {
            if ws.project_id == project_id {
                removed.push(ws.clone());
                false
            } else {
                true
            }
        });
        if !removed.is_empty() {
            Self::save_workspaces_atomic(&self.home_dir, &map)?;
        }
        Ok(removed)
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
static CONFIG_STORE: LazyLock<DaemonConfigStore> = LazyLock::new(|| {
    let home = crate::get_data_dir();
    DaemonConfigStore::new(home)
});

pub fn get_config_store() -> &'static DaemonConfigStore {
    &CONFIG_STORE
}

pub fn atomic_write(target_path: &Path, content: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    if let Some(parent) = target_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let name = target_path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Atomic write requires a file name",
        )
    })?;
    let (temp_path, mut file) = loop {
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let path = target_path.with_file_name(temp_name);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => break (path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let written = file.write_all(content).and_then(|()| file.sync_all());
    // Windows replacement requires releasing our temporary-file handle first.
    drop(file);
    let committed = written.and_then(|()| fs::rename(&temp_path, target_path));
    if committed.is_err() {
        if let Err(cleanup) = fs::remove_file(&temp_path) {
            if cleanup.kind() != std::io::ErrorKind::NotFound {
                eprintln!(
                    "Atomic write cleanup failed for {}: {cleanup}",
                    temp_path.display()
                );
            }
        }
    }
    // File contents are synced before replacement. This does not promise a
    // cross-file transaction or parent-directory crash durability on all hosts.
    committed
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn atomic_write_replaces_content_and_cleans_failed_temporary_files() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("store.json");
        atomic_write(&target, b"old").unwrap();
        atomic_write(&target, b"new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        let blocked = dir.path().join("blocked.json");
        fs::create_dir(&blocked).unwrap();
        assert!(atomic_write(&blocked, b"never committed").is_err());
        let mut names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                std::ffi::OsString::from("blocked.json"),
                std::ffi::OsString::from("store.json")
            ]
        );
    }

    #[test]
    fn concurrent_atomic_writes_use_distinct_temporary_files() {
        let dir = tempdir().unwrap();
        let target = std::sync::Arc::new(dir.path().join("store.json"));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let writers: Vec<_> = (0..4)
            .map(|value| {
                let target = target.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let content = vec![value; 64 * 1024];
                    barrier.wait();
                    for _ in 0..8 {
                        atomic_write(&target, &content).unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let content = fs::read(&*target).unwrap();
        assert_eq!(content.len(), 64 * 1024);
        assert!(content.iter().all(|&value| value == content[0]));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

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

    #[test]
    fn test_desktop_settings_and_labels_persist_across_reloads() {
        let dir = tempdir().unwrap();
        let store = DaemonConfigStore::new(dir.path());
        let patched = store
            .patch_desktop_settings(serde_json::json!({
                "releaseChannel": "beta",
                "notifications": { "playSound": false },
                "daemon": { "keepRunningAfterQuit": true }
            }))
            .expect("patch settings");
        assert_eq!(patched.release_channel, "beta");
        assert!(!patched.notifications.play_sound);
        assert!(patched.daemon.keep_running_after_quit);

        store
            .set_workspace_label_assignment(
                "wks_1",
                WorkspaceLabelRecord {
                    name: "urgent".to_string(),
                    color: "red".to_string(),
                },
                true,
            )
            .expect("assign label");

        let reloaded = DaemonConfigStore::new(dir.path());
        let settings = reloaded.get_desktop_settings();
        assert_eq!(settings.release_channel, "beta");
        assert!(!settings.notifications.play_sound);
        assert!(settings.daemon.keep_running_after_quit);

        let catalog = reloaded.get_labels_catalog();
        assert_eq!(catalog.labels.len(), 1);
        assert_eq!(catalog.labels[0].name, "urgent");
        assert_eq!(
            catalog.assignments.get("wks_1").cloned().unwrap_or_default(),
            vec!["urgent".to_string()]
        );
    }
}
