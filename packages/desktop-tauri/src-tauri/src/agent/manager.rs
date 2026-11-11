use crate::agent::registry::ProviderRegistry;
use crate::agent::storage::AgentStorage;
use crate::agent::types::{now_iso, AgentStatus, AgentTimelineItem, StoredAgentRecord};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

struct TurnControl {
    token: String,
    cancel: tokio::sync::watch::Sender<bool>,
    finished: tokio::sync::watch::Sender<bool>,
}

/// Admission owns the turn even before the runner is scheduled. Never clone this lease.
pub struct ReservedTurn {
    pub agent: StoredAgentRecord,
    pub(crate) prompt: String,
    pub(crate) token: String,
    pub(crate) cancellation: tokio::sync::watch::Receiver<bool>,
    manager: AgentManager,
    completed: bool,
    provider_started: bool,
}

impl ReservedTurn {
    pub fn is_cancelled(&self) -> bool {
        *self.cancellation.borrow()
    }

    pub(crate) fn manager(&self) -> &AgentManager {
        &self.manager
    }

    pub(crate) fn mark_provider_started(&mut self) -> Result<(), String> {
        let gate = self.manager.lifecycle_gate(&self.agent.id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        let mut record = self
            .manager
            .get_agent(&self.agent.id)
            .ok_or("Agent disappeared before provider start")?;
        let handle = record
            .runtime_info
            .as_ref()
            .and_then(|runtime| runtime["providerHandleId"].as_str())
            .or_else(|| {
                record
                    .config
                    .as_ref()
                    .and_then(|config| config["providerHandleId"].as_str())
            })
            .map(str::to_owned);
        record.runtime_info = Some(serde_json::json!({
            "provider": record.provider, "continuityRequired": true, "providerHandleId": handle
        }));
        self.manager
            .storage
            .save_record(record.clone())
            .map_err(|e| format!("Failed to save provider start: {e}"))?;
        self.agent = record;
        self.provider_started = true;
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn restore_unstarted_provider(
        &mut self,
        prior: &StoredAgentRecord,
    ) -> Result<(), String> {
        if !self.provider_started {
            return Ok(());
        }
        let gate = self.manager.lifecycle_gate(&self.agent.id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        let mut record = self
            .manager
            .get_agent(&self.agent.id)
            .ok_or("Agent disappeared while restoring suspended provider setup")?;
        record.runtime_info = prior.runtime_info.clone();
        self.manager
            .storage
            .save_record(record.clone())
            .map_err(|error| {
                format!(
                    "Failed to restore provider continuity after suspended setup failure: {error}"
                )
            })?;
        self.agent = record;
        self.provider_started = false;
        Ok(())
    }

    pub(crate) fn persist_provider_handle(&mut self, handle: &str) -> Result<(), String> {
        let gate = self.manager.lifecycle_gate(&self.agent.id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        let mut record = self
            .manager
            .get_agent(&self.agent.id)
            .ok_or_else(|| "Agent disappeared while saving provider session".to_string())?;
        record.runtime_info = Some(serde_json::json!({
            "provider": record.provider, "providerHandleId": handle
        }));
        self.manager
            .storage
            .save_record(record.clone())
            .map_err(|e| format!("Failed to persist provider session: {e}"))?;
        self.agent = record;
        Ok(())
    }

    pub(crate) fn retain_cleanup_failure(&mut self, error: &str) {
        let gate = self.manager.lifecycle_gate(&self.agent.id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut record) = self.manager.get_agent(&self.agent.id) {
            record.last_status = AgentStatus::Error;
            record.last_error = Some(error.to_owned());
            record.updated_at = now_iso();
            match self.manager.storage.save_record(record) {
                Ok(()) => {
                    self.manager
                        .active_statuses
                        .write()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(self.agent.id.clone(), AgentStatus::Error);
                }
                Err(persist) => eprintln!("[Runner] Failed to persist cleanup failure: {persist}"),
            }
        }
        // Never permit a new turn when the previous tree's exit is unconfirmed.
        self.completed = true;
    }

    /// Call only after the owned process tree and stream readers have been cleaned up.
    pub(crate) fn finish(&mut self, error: Option<&str>) -> Result<(), String> {
        if self.completed {
            return Ok(());
        }
        let gate = self.manager.lifecycle_gate(&self.agent.id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        if !self
            .manager
            .active_turns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&self.agent.id)
            .is_some_and(|turn| turn.token == self.token)
        {
            self.completed = true;
            return Err("Turn no longer owns this agent".into());
        }
        let status = if self.is_cancelled() {
            AgentStatus::Closed
        } else if error.is_some() {
            AgentStatus::Error
        } else {
            AgentStatus::Idle
        };
        let result = self
            .manager
            .get_agent(&self.agent.id)
            .ok_or_else(|| "Agent disappeared".to_string())
            .and_then(|mut record| {
                record.last_status = status;
                record.last_error = error.map(str::to_owned);
                record.updated_at = now_iso();
                self.manager
                    .storage
                    .save_record(record)
                    .map_err(|e| format!("Failed to save turn completion: {e}"))
            });
        if result.is_ok() {
            self.manager
                .active_statuses
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(self.agent.id.clone(), status);
        }
        if let Some(turn) = self
            .manager
            .active_turns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.agent.id)
        {
            turn.finished.send_replace(true);
        }
        self.completed = true;
        result
    }
}

impl Drop for ReservedTurn {
    fn drop(&mut self) {
        if !self.completed {
            if self.provider_started {
                self.retain_cleanup_failure(
                    "Runner dropped before provider cleanup was confirmed; agent remains reserved",
                );
            } else {
                let _ = self.finish(Some("Turn execution was dropped before completion"));
            }
        }
    }
}

struct DeletionGuard {
    pending: Arc<Mutex<HashSet<String>>>,
    id: String,
}

impl Drop for DeletionGuard {
    fn drop(&mut self) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

#[derive(Clone)]
pub struct AgentManager {
    storage: Arc<AgentStorage>,
    registry: Arc<ProviderRegistry>,
    active_statuses: Arc<RwLock<HashMap<String, AgentStatus>>>,
    active_turns: Arc<Mutex<HashMap<String, TurnControl>>>,
    lifecycle_gates: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    deleting: Arc<Mutex<HashSet<String>>>,
    home_dir: Arc<PathBuf>,
}

impl AgentManager {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        Self::try_new(home_dir).expect("Failed to initialize agent storage")
    }

    pub fn try_new(home_dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let home_dir = Arc::new(home_dir.into());
        let storage = Arc::new(AgentStorage::try_new(home_dir.as_ref().clone())?);
        let registry = Arc::new(ProviderRegistry::new());

        // Startup reconciliation: reset any agents stranded in 'Running' state across sessions or crashes.
        for mut record in storage.list_records() {
            if record.last_status == AgentStatus::Running {
                record.last_status = AgentStatus::Idle;
                record.updated_at = now_iso();
                let _ = storage.save_record(record);
            }
        }

        Ok(Self {
            storage,
            registry,
            active_statuses: Arc::new(RwLock::new(HashMap::new())),
            active_turns: Arc::new(Mutex::new(HashMap::new())),
            lifecycle_gates: Arc::new(Mutex::new(HashMap::new())),
            deleting: Arc::new(Mutex::new(HashSet::new())),
            home_dir,
        })
    }

    pub fn storage(&self) -> &Arc<AgentStorage> {
        &self.storage
    }

    pub fn registry(&self) -> &Arc<ProviderRegistry> {
        &self.registry
    }

    fn lifecycle_gate(&self, id: &str) -> Arc<Mutex<()>> {
        self.lifecycle_gates
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn ensure_inactive(&self, id: &str) -> Result<(), String> {
        if self
            .deleting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
        {
            return Err(format!("Agent {id} is being deleted"));
        }
        if self
            .active_turns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(id)
        {
            Err(format!("Agent {id} has an active turn; wait for cancellation/completion before changing it"))
        } else {
            Ok(())
        }
    }

    pub(crate) fn provider_session_dir(&self, id: &str) -> PathBuf {
        self.home_dir
            .join("agents")
            .join("provider-sessions")
            .join(id)
    }

    pub fn create_agent(
        &self,
        provider: &str,
        cwd: &str,
        title: Option<String>,
        workspace_id: Option<String>,
        config: Option<Value>,
    ) -> Result<StoredAgentRecord, String> {
        let provider_desc = self.registry.get(provider).unwrap_or_else(|| {
            crate::agent::types::ProviderDescriptor {
                id: provider.to_string(),
                label: provider.to_string(),
                description: format!("Agent provider {provider}"),
                is_installed: true,
            }
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
            runtime_info: Some(
                serde_json::json!({"provider": provider, "continuityRequired": false}),
            ),
            last_usage: None,
        };

        self.storage
            .save_record(record.clone())
            .map_err(|e| format!("Failed to save agent record: {e}"))?;

        self.active_statuses
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, AgentStatus::Idle);

        Ok(record)
    }

    pub async fn create_agent_in_worktree(
        &self,
        provider: &str,
        repo_root: &str,
        worktree_slug: &str,
        branch: Option<&str>,
        title: Option<String>,
        workspace_id: Option<String>,
        config: Option<Value>,
    ) -> Result<StoredAgentRecord, String> {
        let repo_path = std::path::Path::new(repo_root);
        let branch_name = branch.unwrap_or(worktree_slug);
        let target_path = repo_path
            .join(".kronos")
            .join("worktrees")
            .join(worktree_slug);

        if let Some(parent) = target_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Add git worktree if target doesn't exist
        if !target_path.exists() {
            crate::kernel::git::GitEngine::worktree_add(repo_path, &target_path, branch_name, true)
                .await
                .map_err(|e| format!("Failed to create git worktree: {e}"))?;
        }

        let mut record = self.create_agent(
            provider,
            &target_path.to_string_lossy(),
            title,
            workspace_id,
            config,
        )?;

        record.labels.insert(
            "worktree_path".to_string(),
            target_path.to_string_lossy().to_string(),
        );
        record
            .labels
            .insert("worktree_branch".to_string(), branch_name.to_string());
        record
            .labels
            .insert("main_repo".to_string(), repo_root.to_string());

        let _ = self.storage.save_record(record.clone());

        Ok(record)
    }

    pub async fn remove_agent_worktree(&self, agent_id: &str) -> Result<(), String> {
        let record = self
            .get_agent(agent_id)
            .ok_or_else(|| format!("Agent {agent_id} not found"))?;
        let repo_root = record
            .labels
            .get("main_repo")
            .ok_or("Agent not associated with a main repo")?;
        let worktree_path = record
            .labels
            .get("worktree_path")
            .ok_or("Agent not associated with a worktree path")?;

        crate::kernel::git::GitEngine::worktree_remove(
            std::path::Path::new(repo_root),
            std::path::Path::new(worktree_path),
        )
        .await
        .map_err(|e| format!("Failed to remove git worktree: {e}"))?;

        Ok(())
    }

    pub fn get_agent(&self, id: &str) -> Option<StoredAgentRecord> {
        let mut record = self.storage.get_record(id)?;
        if let Some(status) = self
            .active_statuses
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
        {
            record.last_status = *status;
        }
        Some(record)
    }

    pub fn list_agents(&self) -> Vec<StoredAgentRecord> {
        let mut list = self.storage.list_records();
        let statuses = self
            .active_statuses
            .read()
            .unwrap_or_else(|e| e.into_inner());
        for rec in &mut list {
            if let Some(status) = statuses.get(&rec.id) {
                rec.last_status = *status;
            }
        }
        list
    }

    pub fn send_message(&self, id: &str, text: &str) -> Result<ReservedTurn, String> {
        let gate = self.lifecycle_gate(id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        self.ensure_inactive(id)?;
        let mut record = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;
        let original = record.clone();
        let token = uuid_v4_simple();
        let (cancel, cancellation) = tokio::sync::watch::channel(false);
        let (finished, _) = tokio::sync::watch::channel(false);
        self.active_turns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id.to_string(),
                TurnControl {
                    token: token.clone(),
                    cancel,
                    finished,
                },
            );
        let effective_prompt = if text.trim_start().starts_with("/council")
            || text.trim_start().starts_with("/swarm")
        {
            let task = text
                .trim_start()
                .trim_start_matches("/council")
                .trim_start_matches("/swarm")
                .trim();
            format!(
                r#"[KRONOS COUNCIL SWARM PROTOCOL ACTIVATED]
Operate as an autonomous multi-perspective AI Council:

Phase 1: [COUNCIL: ARCHITECT]
- Inspect codebase structure and dependencies.
- Formulate a precise, step-by-step implementation blueprint.

Phase 2: [COUNCIL: IMPLEMENTER]
- Implement changes with zero placeholder code or omissions.
- Preserve conventions, comments, and architectural integrity.

Phase 3: [COUNCIL: AUDITOR]
- Verify diffs, linting, type safety, and test coverage.
- Deliver verified final solution.

TASK:
{task}"#
            )
        } else {
            text.to_string()
        };

        let mut reservation = ReservedTurn {
            agent: record.clone(),
            prompt: effective_prompt,
            token,
            cancellation,
            manager: self.clone(),
            completed: false,
            provider_started: false,
        };
        // On setup failure remove admission before returning, without invoking Drop under the gate.
        let setup = (|| {
            if record
                .title
                .as_ref()
                .is_none_or(|title| title.trim().is_empty())
            {
                if let Some(line) = text.lines().find(|line| !line.trim().is_empty()) {
                    // Match the server's provisional title: first content line, compact and bounded.
                    let mut title: String = line
                        .split_whitespace()
                        .flat_map(|word| word.chars().chain(std::iter::once(' ')))
                        .take(60)
                        .collect();
                    title.truncate(title.trim_end().len());
                    record.title = Some(title);
                    record.updated_at = now_iso();
                }
            }

            let msg_id = format!("msg_{}", uuid_v4_simple());
            let item = AgentTimelineItem::UserMessage {
                id: msg_id,
                text: text.to_string(),
                timestamp: now_iso(),
            };

            record.last_status = AgentStatus::Running;
            record.last_error = None;
            record.updated_at = now_iso();
            self.storage
                .save_record(record.clone())
                .map_err(|e| format!("Failed to save prompt admission: {e}"))?;
            if let Err(error) = self.storage.append_timeline_item(id, item) {
                return match self.storage.save_record(original) {
                Ok(()) => Err(format!("Failed to append message: {error}")),
                Err(rollback) => Err(format!("Failed to append message: {error}; failed to restore agent record: {rollback}")),
            };
            }

            self.active_statuses
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id.to_string(), AgentStatus::Running);

            Ok::<(), String>(())
        })();
        if let Err(error) = setup {
            if let Some(turn) = self
                .active_turns
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(id)
            {
                turn.finished.send_replace(true);
            }
            self.active_statuses
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .remove(id);
            reservation.completed = true;
            return Err(error);
        }
        reservation.agent = record;
        Ok(reservation)
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
        let gate = self.lifecycle_gate(id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        self.ensure_inactive(id)?;
        let mut record = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;
        record.last_status = status;
        record.updated_at = now_iso();
        self.storage
            .save_record(record)
            .map_err(|e| format!("Failed to save status: {e}"))?;
        self.active_statuses
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.to_string(), status);
        Ok(())
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
        let gate = self.lifecycle_gate(id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        self.ensure_inactive(id)?;
        let mut record = self
            .get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;
        let mut config = record.config.unwrap_or_else(|| serde_json::json!({}));
        let resolved_provider = provider.map(|p| p.to_string()).or_else(|| {
            crate::agent::registry::resolve_provider_for_model(model).map(|p| p.to_string())
        });
        if let Some(obj) = config.as_object_mut() {
            if let Some(ref p) = resolved_provider {
                if record.provider != *p {
                    record.runtime_info =
                        Some(serde_json::json!({"provider": p, "continuityRequired": false}));
                    obj.remove("providerHandleId");
                }
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
        let gate = self.lifecycle_gate(id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        self.ensure_inactive(id)?;
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
        let gate = self.lifecycle_gate(id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        self.get_agent(id)
            .ok_or_else(|| format!("Agent {id} not found"))?;
        if let Some(turn) = self
            .active_turns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
        {
            turn.cancel.send_replace(true);
        }
        Ok(())
    }

    pub async fn delete_agent_and_wait(&self, id: &str) -> Result<bool, String> {
        let (waiting, _admission) = {
            let gate = self.lifecycle_gate(id);
            let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
            if !self
                .deleting
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id.to_owned())
            {
                return Err(format!("Agent {id} is already being deleted"));
            }
            let admission = DeletionGuard {
                pending: self.deleting.clone(),
                id: id.to_owned(),
            };
            let turns = self.active_turns.lock().unwrap_or_else(|e| e.into_inner());
            let waiting = turns.get(id).map(|turn| {
                turn.cancel.send_replace(true);
                turn.finished.subscribe()
            });
            (waiting, admission)
        };
        if let Some(mut finished) = waiting {
            tokio::time::timeout(std::time::Duration::from_secs(15), async {
                while !*finished.borrow_and_update() {
                    finished
                        .changed()
                        .await
                        .map_err(|_| "Turn cleanup notification closed")?;
                }
                Ok::<(), &str>(())
            })
            .await
            .map_err(|_| "Timed out waiting for provider cleanup; agent retained".to_string())?
            .map_err(str::to_owned)?;
        }
        self.delete_agent(id)
    }

    pub fn delete_agent(&self, id: &str) -> Result<bool, String> {
        let gate = self.lifecycle_gate(id);
        let _guard = gate.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(turn) = self
            .active_turns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
        {
            turn.cancel.send_replace(true);
            return Err(
                "Cancellation requested; agent retained until provider cleanup completes".into(),
            );
        }
        let deleted = self
            .storage
            .delete_record(id)
            .map_err(|e| format!("Failed to delete agent: {e}"))?;
        self.active_statuses
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        Ok(deleted)
    }
}

pub(crate) fn uuid_v4_simple() -> String {
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
    fn test_prompt_title_persists_without_being_replaced_by_followups() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());
        let agent = manager.create_agent("omp", ".", None, None, None).unwrap();
        let mut first = manager
            .send_message(&agent.id, "\n  Fix\t the   tab title \r\nInclude tests")
            .unwrap();
        first.finish(None).unwrap();
        let reloaded = AgentManager::new(dir.path());
        assert_eq!(
            reloaded.get_agent(&agent.id).unwrap().title.as_deref(),
            Some("Fix the tab title")
        );
        let mut followup = reloaded
            .send_message(&agent.id, "A different prompt")
            .unwrap();
        followup.finish(None).unwrap();
        assert_eq!(
            reloaded.get_agent(&agent.id).unwrap().title.as_deref(),
            Some("Fix the tab title")
        );
    }

    #[test]
    fn test_prompt_title_preserves_explicit_titles_and_bounds_unicode() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());
        let named = manager
            .create_agent("omp", ".", Some("Custom title".into()), None, None)
            .unwrap();
        let mut named_turn = manager
            .send_message(&named.id, "Replace this title")
            .unwrap();
        named_turn.finish(None).unwrap();
        assert_eq!(
            manager.get_agent(&named.id).unwrap().title.as_deref(),
            Some("Custom title")
        );
        let untitled = manager.create_agent("omp", ".", None, None, None).unwrap();
        let mut blank = manager.send_message(&untitled.id, " \r\n\t ").unwrap();
        blank.finish(None).unwrap();
        assert_eq!(manager.get_agent(&untitled.id).unwrap().title, None);
        let prompt = "界".repeat(65);
        let mut titled = manager.send_message(&untitled.id, &prompt).unwrap();
        titled.finish(None).unwrap();
        assert_eq!(
            manager.get_agent(&untitled.id).unwrap().title,
            Some("界".repeat(60))
        );
    }

    #[test]
    fn test_thinking_setting_survives_reload_and_can_be_cleared() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());
        let agent = manager
            .create_agent(
                "codex",
                ".",
                None,
                None,
                Some(serde_json::json!({"model": "runtime-model", "thinkingOptionId": "high"})),
            )
            .unwrap();
        manager.set_agent_thinking(&agent.id, Some("low")).unwrap();
        let reloaded = AgentManager::new(dir.path());
        let record = reloaded.get_agent(&agent.id).unwrap();
        assert_eq!(record.config.as_ref().unwrap()["thinkingOptionId"], "low");
        assert_eq!(record.config.as_ref().unwrap()["model"], "runtime-model");
        reloaded.set_agent_thinking(&agent.id, None).unwrap();
        let cleared = AgentManager::new(dir.path()).get_agent(&agent.id).unwrap();
        assert!(cleared
            .config
            .as_ref()
            .unwrap()
            .get("thinkingOptionId")
            .is_none());
        assert_eq!(cleared.config.as_ref().unwrap()["model"], "runtime-model");
        assert!(manager
            .set_agent_thinking("missing-thinking-agent", Some("high"))
            .is_err());
    }

    #[test]
    fn test_thinking_choice_is_not_carried_to_a_different_model() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());
        let agent = manager
            .create_agent(
                "codex",
                ".",
                None,
                None,
                Some(serde_json::json!({"model": "first", "thinkingOptionId": "xhigh"})),
            )
            .unwrap();
        let unchanged = manager.set_agent_model(&agent.id, "first").unwrap();
        assert_eq!(
            unchanged.config.as_ref().unwrap()["thinkingOptionId"],
            "xhigh"
        );
        manager.set_agent_model(&agent.id, "second").unwrap();
        let reloaded = AgentManager::new(dir.path()).get_agent(&agent.id).unwrap();
        assert_eq!(reloaded.config.as_ref().unwrap()["model"], "second");
        assert!(reloaded
            .config
            .as_ref()
            .unwrap()
            .get("thinkingOptionId")
            .is_none());
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

        let mut turn = manager
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
        manager.cancel_agent(&created.id).expect("Cancel agent");
        assert_eq!(
            manager.get_agent(&created.id).unwrap().last_status,
            AgentStatus::Running
        );
        turn.finish(None).unwrap();

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

        let cancelled = manager.get_agent(&created.id).unwrap();
        assert_eq!(cancelled.last_status, AgentStatus::Closed);

        assert!(manager.delete_agent(&created.id).unwrap());
        assert_eq!(manager.list_agents().len(), 0);
    }

    #[tokio::test]
    async fn test_agent_manager_worktree_metadata() {
        let dir = tempdir().unwrap();
        let manager = AgentManager::new(dir.path());

        let repo_dir = dir.path().join("mock_repo");
        let _ = std::fs::create_dir_all(&repo_dir);

        let mut rec = manager
            .create_agent(
                "mock",
                &repo_dir.to_string_lossy(),
                Some("Worktree Task Agent".to_string()),
                Some("wks-wt-1".to_string()),
                None,
            )
            .unwrap();
        rec.labels.insert(
            "worktree_path".to_string(),
            repo_dir
                .join(".kronos/worktrees/feat")
                .to_string_lossy()
                .to_string(),
        );
        rec.labels
            .insert("worktree_branch".to_string(), "feat".to_string());
        rec.labels.insert(
            "main_repo".to_string(),
            repo_dir.to_string_lossy().to_string(),
        );
        manager.storage.save_record(rec.clone()).unwrap();

        let retrieved = manager.get_agent(&rec.id).unwrap();
        assert_eq!(retrieved.labels.get("worktree_branch").unwrap(), "feat");
        assert_eq!(
            retrieved.labels.get("main_repo").unwrap(),
            &repo_dir.to_string_lossy().to_string()
        );
    }
}
