use crate::agent::types::{AgentTimelineItem, StoredAgentRecord};
use crate::kernel::config_store::atomic_write;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordStore {
    format: u32,
    records: HashMap<String, StoredAgentRecord>,
    #[serde(rename = "timelineGenerations")]
    timeline_generations: HashMap<String, u64>,
}

struct Timeline {
    items: Vec<AgentTimelineItem>,
    // None means a legacy JSON snapshot (or a missing history), not a journal yet.
    journal_len: Option<u64>,
    append_error: Option<String>,
}

pub struct AgentStorage {
    home_dir: PathBuf,
    records: RwLock<RecordStore>,
    // Metadata writers share one file, but readers/timeline operations never
    // acquire this commit lock or wait for its disk I/O.
    record_commit: Mutex<()>,
    // Only the short lookup/insert holds this map lock. Disk I/O owns one agent lock.
    timelines: RwLock<HashMap<String, Arc<Mutex<Option<Timeline>>>>>,
}

impl AgentStorage {
    pub fn new(home_dir: impl Into<PathBuf>) -> Self {
        Self::try_new(home_dir)
            .expect("Failed to load agent records; original store was not modified")
    }

    pub fn try_new(home_dir: impl Into<PathBuf>) -> io::Result<Self> {
        let home_dir = home_dir.into();
        let records = Self::load_records(&home_dir)?;
        Ok(Self {
            home_dir,
            records: RwLock::new(records),
            record_commit: Mutex::new(()),
            timelines: RwLock::new(HashMap::new()),
        })
    }

    pub fn save_record(&self, record: StoredAgentRecord) -> io::Result<()> {
        let agent = self.agent_lock(&record.id);
        let _agent = agent.lock().unwrap_or_else(|e| e.into_inner());
        let _commit = self.record_commit.lock().unwrap_or_else(|e| e.into_inner());
        let id = record.id.clone();
        let (bytes, record) = {
            let mut store = self.records.write().unwrap_or_else(|e| e.into_inner());
            let previous = store.records.insert(id.clone(), record);
            let bytes = serde_json::to_vec_pretty(&*store);
            // Restore the visible snapshot before releasing the lock for I/O.
            let Some(record) = store.records.remove(&id) else {
                return Err(io::Error::new(io::ErrorKind::NotFound, "Record missing from store"));
            };
            if let Some(previous) = previous {
                store.records.insert(id.clone(), previous);
            }
            (bytes, record)
        };
        self.persist_record_bytes(&bytes.map_err(invalid_data)?)?;
        self.records.write().unwrap_or_else(|e| e.into_inner()).records.insert(id, record);
        Ok(())
    }

    pub fn get_record(&self, id: &str) -> Option<StoredAgentRecord> {
        self.records.read().unwrap_or_else(|e| e.into_inner()).records.get(id).cloned()
    }

    pub fn list_records(&self) -> Vec<StoredAgentRecord> {
        let mut list: Vec<_> = self
            .records
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .records
            .values()
            .cloned()
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub fn delete_record(&self, id: &str) -> io::Result<bool> {
        let agent = self.agent_lock(id);
        let mut timeline = agent.lock().unwrap_or_else(|e| e.into_inner());
        let _commit = self.record_commit.lock().unwrap_or_else(|e| e.into_inner());
        let (bytes, generation, next) = {
            let mut store = self.records.write().unwrap_or_else(|e| e.into_inner());
            if !store.records.contains_key(id) {
                return Ok(false);
            }
            let old_generation = store.timeline_generations.get(id).copied();
            let generation = old_generation.unwrap_or(0);
            let next = generation.checked_add(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Timeline generation exhausted")
            })?;
            let Some(record) = store.records.remove(id) else {
                return Ok(false);
            };
            store.timeline_generations.insert(id.to_owned(), next);
            let bytes = serde_json::to_vec_pretty(&*store);
            store.records.insert(id.to_owned(), record);
            match old_generation {
                Some(value) => {
                    store.timeline_generations.insert(id.to_owned(), value);
                }
                None => {
                    store.timeline_generations.remove(id);
                }
            }
            (bytes, generation, next)
        };
        self.persist_record_bytes(&bytes.map_err(invalid_data)?)?;
        {
            let mut store = self.records.write().unwrap_or_else(|e| e.into_inner());
            store.records.remove(id);
            store.timeline_generations.insert(id.to_owned(), next);
        }
        // Removal and generation advance share one commit; cleanup cannot revive
        // obsolete history. Keep the same agent lock object to fence queued users.
        *timeline = None;
        self.remove_obsolete_history(id, generation);
        Ok(true)
    }

    pub fn append_timeline_item(&self, agent_id: &str, item: AgentTimelineItem) -> io::Result<()> {
        let agent = self.agent_lock(agent_id);
        let mut cached = agent.lock().unwrap_or_else(|e| e.into_inner());
        let generation = self.generation(agent_id);
        if cached.is_none() {
            *cached = Some(self.load_timeline(agent_id, generation)?);
        }
        let Some(timeline) = cached.as_mut() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Failed to load agent timeline"));
        };
        if let Some(error) = &timeline.append_error {
            return Err(io::Error::other(error.clone()));
        }
        let target = self.journal_path(agent_id, generation);
        let mut row = serde_json::to_vec(&item).map_err(invalid_data)?;
        row.push(b'\n');
        if let Some(length) = timeline.journal_len {
            // Never create/truncate here: a missing or externally changed journal
            // requires explicit recovery, not replacement with the cached prefix.
            let mut file = OpenOptions::new().append(true).open(&target)?;
            if file.metadata()?.len() != length {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Timeline journal changed outside its owner: {}",
                        target.display()
                    ),
                ));
            }
            if let Err(error) = file.write_all(&row).and_then(|()| file.sync_all()) {
                if let Err(rollback) = file.set_len(length).and_then(|()| file.sync_all()) {
                    let detail = format!(
                        "Journal append failed ({error}); rollback failed ({rollback}); inspect {}",
                        target.display()
                    );
                    timeline.append_error = Some(detail.clone());
                    return Err(io::Error::other(detail));
                }
                return Err(error);
            }
            timeline.journal_len = Some(length + row.len() as u64);
        } else if timeline.items.is_empty() {
            atomic_write(&target, &row)?;
            timeline.journal_len = Some(row.len() as u64);
        } else {
            // The only full serialization is the legacy-to-journal cutover. Keep
            // the original JSON untouched until the new journal is committed.
            let mut bytes = Vec::new();
            for previous in &timeline.items {
                serde_json::to_writer(&mut bytes, previous).map_err(invalid_data)?;
                bytes.push(b'\n');
            }
            bytes.extend_from_slice(&row);
            atomic_write(&target, &bytes)?;
            timeline.journal_len = Some(bytes.len() as u64);
        }
        timeline.items.push(item);
        Ok(())
    }

    pub fn get_timeline(&self, agent_id: &str) -> Vec<AgentTimelineItem> {
        self.with_timeline(agent_id, <[AgentTimelineItem]>::to_vec)
            .expect("Failed to load agent timeline; original history was not modified")
    }

    /// The callback runs under this agent's lock; it must not re-enter storage for
    /// the same agent. Clone only the requested window, not the complete history.
    pub fn with_timeline<T>(
        &self,
        agent_id: &str,
        read: impl FnOnce(&[AgentTimelineItem]) -> T,
    ) -> io::Result<T> {
        let agent = self.agent_lock(agent_id);
        let mut cached = agent.lock().unwrap_or_else(|e| e.into_inner());
        if cached.is_none() {
            *cached = Some(self.load_timeline(agent_id, self.generation(agent_id))?);
        }
        let Some(timeline) = cached.as_ref() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Failed to read agent timeline"));
        };
        Ok(read(&timeline.items))
    }

    fn agent_lock(&self, agent_id: &str) -> Arc<Mutex<Option<Timeline>>> {
        if let Some(agent) = self.timelines.read().unwrap_or_else(|e| e.into_inner()).get(agent_id) {
            return agent.clone();
        }
        self.timelines
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(agent_id.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(None)))
            .clone()
    }

    fn generation(&self, agent_id: &str) -> u64 {
        self.records
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .timeline_generations
            .get(agent_id)
            .copied()
            .unwrap_or(0)
    }

    fn load_records(home: &Path) -> io::Result<RecordStore> {
        let path = home.join("agents/records.json");
        let Some(bytes) = read_optional(&path)? else {
            return Ok(RecordStore {
                format: 1,
                records: HashMap::new(),
                timeline_generations: HashMap::new(),
            });
        };
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Loaded {
            Current(RecordStore),
            Legacy(HashMap<String, StoredAgentRecord>),
        }
        let loaded: Loaded = serde_json::from_slice(&bytes).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid agent records {}: {error}", path.display()),
            )
        })?;
        match loaded {
            Loaded::Current(store) if store.format == 1 => Ok(store),
            Loaded::Current(store) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported agent records format {}", store.format),
            )),
            Loaded::Legacy(records) => Ok(RecordStore {
                format: 1,
                records,
                timeline_generations: HashMap::new(),
            }),
        }
    }

    fn persist_record_bytes(&self, bytes: &[u8]) -> io::Result<()> {
        atomic_write(&self.home_dir.join("agents/records.json"), bytes)
    }

    fn journal_path(&self, agent_id: &str, generation: u64) -> PathBuf {
        let name = if generation == 0 {
            format!("{agent_id}.jsonl")
        } else {
            format!("{agent_id}.{generation}.jsonl")
        };
        self.home_dir.join("agents/timelines").join(name)
    }

    fn load_timeline(&self, agent_id: &str, generation: u64) -> io::Result<Timeline> {
        let path = self.journal_path(agent_id, generation);
        if let Some(bytes) = read_optional(&path)? {
            let mut items = Vec::new();
            // An unterminated last record is uncommitted. Fail closed, retaining
            // every byte (including valid preceding records) for explicit repair.
            let mut offset = 0;
            for row in bytes.split_inclusive(|&byte| byte == b'\n') {
                if !row.ends_with(b"\n") {
                    return Err(io::Error::new(io::ErrorKind::InvalidData,
                        format!("Truncated timeline journal {} at byte {offset}; {} complete records preserved; repair required before appending", path.display(), items.len())));
                }
                items.push(serde_json::from_slice(&row[..row.len() - 1]).map_err(|error| {
                    io::Error::new(io::ErrorKind::InvalidData,
                        format!("Invalid timeline journal {} at byte {offset}: {error}; original bytes preserved", path.display()))
                })?);
                offset += row.len();
            }
            return Ok(Timeline {
                items,
                journal_len: Some(bytes.len() as u64),
                append_error: None,
            });
        }
        let legacy = self
            .home_dir
            .join("agents/timelines")
            .join(format!("{agent_id}.json"));
        let items = if generation == 0 {
            match read_optional(&legacy)? {
                Some(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid legacy timeline {}: {error}", legacy.display()),
                    )
                })?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };
        Ok(Timeline {
            items,
            journal_len: None,
            append_error: None,
        })
    }

    fn remove_obsolete_history(&self, agent_id: &str, generation: u64) {
        let mut paths = vec![self.journal_path(agent_id, generation)];
        if generation == 0 {
            paths.push(
                self.home_dir
                    .join("agents/timelines")
                    .join(format!("{agent_id}.json")),
            );
        }
        for path in paths {
            if let Err(error) = fs::remove_file(&path) {
                if error.kind() != io::ErrorKind::NotFound {
                    eprintln!("Agent logically deleted, but obsolete history cleanup failed for {}: {error}", path.display());
                }
            }
        }
    }
}

fn invalid_data(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!("Failed to read {}: {error}", path.display()),
        )),
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
            last_usage: None,
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

    fn record(id: &str) -> StoredAgentRecord {
        serde_json::from_value(serde_json::json!({
            "id": id, "provider": "claude", "cwd": "/tmp/project",
            "createdAt": "2026-11-02T01:00:00Z",
            "updatedAt": "2026-11-02T01:00:00Z", "labels": {}, "lastStatus": "idle"
        }))
        .unwrap()
    }

    fn message(id: &str) -> AgentTimelineItem {
        AgentTimelineItem::UserMessage {
            id: id.into(),
            text: id.into(),
            timestamp: "2026-11-02T01:00:01Z".into(),
        }
    }

    #[test]
    fn failed_record_commits_preserve_visible_state() {
        let dir = tempdir().unwrap();
        let storage = AgentStorage::new(dir.path());
        storage.save_record(record("a")).unwrap();
        storage
            .append_timeline_item("a", message("retained"))
            .unwrap();
        let path = dir.path().join("agents/records.json");
        let original = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(storage.save_record(record("b")).is_err());
        assert!(storage.get_record("b").is_none());
        assert!(storage.delete_record("a").is_err());
        assert!(storage.get_record("a").is_some());
        assert_eq!(storage.get_timeline("a"), vec![message("retained")]);
        fs::remove_dir(&path).unwrap();
        fs::write(&path, original).unwrap();
        storage
            .append_timeline_item("a", message("after failure"))
            .unwrap();
        assert_eq!(
            AgentStorage::new(dir.path()).get_timeline("a"),
            vec![message("retained"), message("after failure")]
        );
    }

    #[test]
    fn malformed_and_unreadable_records_are_not_empty_stores() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("agents/records.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"{broken").unwrap();
        assert!(AgentStorage::try_new(dir.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{broken");
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(AgentStorage::try_new(dir.path()).is_err());
    }

    #[test]
    fn legacy_records_and_history_cut_over_without_duplicates() {
        let dir = tempdir().unwrap();
        let timeline_dir = dir.path().join("agents/timelines");
        fs::create_dir_all(&timeline_dir).unwrap();
        fs::write(
            dir.path().join("agents/records.json"),
            serde_json::to_vec(&HashMap::from([("a", record("a"))])).unwrap(),
        )
        .unwrap();
        fs::write(
            timeline_dir.join("a.json"),
            serde_json::to_vec(&vec![message("old")]).unwrap(),
        )
        .unwrap();
        let storage = AgentStorage::new(dir.path());
        storage.append_timeline_item("a", message("new")).unwrap();
        let reloaded = AgentStorage::new(dir.path());
        assert!(reloaded.get_record("a").is_some());
        assert_eq!(
            reloaded.get_timeline("a"),
            vec![message("old"), message("new")]
        );
        // Once cut over, the journal is authoritative even if a legacy backup remains.
        fs::write(timeline_dir.join("a.json"), b"broken backup").unwrap();
        reloaded
            .append_timeline_item("a", message("third"))
            .unwrap();
        assert_eq!(AgentStorage::new(dir.path()).get_timeline("a").len(), 3);
    }

    #[test]
    fn appends_preserve_journal_prefix_and_failed_writes_preserve_cache() {
        let dir = tempdir().unwrap();
        let storage = AgentStorage::new(dir.path());
        storage.append_timeline_item("a", message("first")).unwrap();
        let path = dir.path().join("agents/timelines/a.jsonl");
        let prefix = fs::read(&path).unwrap();
        storage
            .append_timeline_item("a", message("second"))
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(&prefix));
        assert_eq!(bytes.iter().filter(|&&b| b == b'\n').count(), 2);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(storage
            .append_timeline_item("a", message("failed"))
            .is_err());
        assert_eq!(
            storage.get_timeline("a"),
            vec![message("first"), message("second")]
        );
    }

    #[test]
    fn corrupt_and_truncated_history_preserves_all_source_bytes() {
        let dir = tempdir().unwrap();
        let timeline_dir = dir.path().join("agents/timelines");
        fs::create_dir_all(&timeline_dir).unwrap();
        fs::write(timeline_dir.join("legacy.json"), b"[broken").unwrap();
        let storage = AgentStorage::new(dir.path());
        assert!(storage
            .append_timeline_item("legacy", message("new"))
            .is_err());
        assert_eq!(
            fs::read(timeline_dir.join("legacy.json")).unwrap(),
            b"[broken"
        );
        assert!(!timeline_dir.join("legacy.jsonl").exists());
        for suffix in [
            b"{truncated".as_slice(),
            b"{invalid}\n".as_slice(),
            br#"{"type":"user_message","id":"tail","text":"tail","timestamp":"now"}"#.as_slice(),
        ] {
            let mut bytes = serde_json::to_vec(&message("valid")).unwrap();
            bytes.push(b'\n');
            bytes.extend_from_slice(suffix);
            fs::write(timeline_dir.join("a.jsonl"), &bytes).unwrap();
            let fresh = AgentStorage::new(dir.path());
            assert!(fresh.with_timeline("a", |rows| rows.len()).is_err());
            assert!(fresh.append_timeline_item("a", message("new")).is_err());
            assert_eq!(fs::read(timeline_dir.join("a.jsonl")).unwrap(), bytes);
        }
    }

    #[test]
    fn deletion_invalidates_cached_and_legacy_history_across_recreation() {
        let dir = tempdir().unwrap();
        let storage = AgentStorage::new(dir.path());
        storage.save_record(record("a")).unwrap();
        storage
            .append_timeline_item("a", message("deleted"))
            .unwrap();
        assert!(storage.delete_record("a").unwrap());
        assert!(storage.get_timeline("a").is_empty());
        storage.save_record(record("a")).unwrap();
        // Even an obsolete file restored after deletion cannot become current history.
        fs::write(
            dir.path().join("agents/timelines/a.json"),
            serde_json::to_vec(&vec![message("deleted")]).unwrap(),
        )
        .unwrap();
        storage
            .append_timeline_item("a", message("current"))
            .unwrap();
        let fresh = AgentStorage::new(dir.path());
        assert_eq!(fresh.get_timeline("a"), vec![message("current")]);
    }

    #[test]
    fn independent_agent_writes_do_not_wait_for_another_timeline_reader() {
        let dir = tempdir().unwrap();
        let storage = Arc::new(AgentStorage::new(dir.path()));
        storage.append_timeline_item("a", message("held")).unwrap();
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let reader_storage = storage.clone();
        let reader = std::thread::spawn(move || {
            reader_storage
                .with_timeline("a", |_| {
                    locked_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                })
                .unwrap();
        });
        locked_rx.recv().unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let writer_storage = storage.clone();
        let writer = std::thread::spawn(move || {
            done_tx
                .send(writer_storage.append_timeline_item("b", message("independent")))
                .unwrap();
        });
        let result = done_rx.recv_timeout(std::time::Duration::from_secs(5));
        release_tx.send(()).unwrap();
        reader.join().unwrap();
        writer.join().unwrap();
        result.expect("unrelated-agent append blocked").unwrap();
        assert_eq!(storage.with_timeline("b", |rows| rows.len()).unwrap(), 1);
    }

    #[test]
    fn failed_legacy_cutover_keeps_cached_and_original_history() {
        let dir = tempdir().unwrap();
        let timeline_dir = dir.path().join("agents/timelines");
        fs::create_dir_all(&timeline_dir).unwrap();
        let legacy = serde_json::to_vec(&vec![message("old")]).unwrap();
        fs::write(timeline_dir.join("a.json"), &legacy).unwrap();
        let storage = AgentStorage::new(dir.path());
        assert_eq!(storage.get_timeline("a"), vec![message("old")]);
        fs::create_dir(timeline_dir.join("a.jsonl")).unwrap();
        assert!(storage
            .append_timeline_item("a", message("failed"))
            .is_err());
        assert_eq!(storage.get_timeline("a"), vec![message("old")]);
        assert_eq!(fs::read(timeline_dir.join("a.json")).unwrap(), legacy);
        assert_eq!(fs::read_dir(&timeline_dir).unwrap().count(), 2);
    }

    #[test]
    fn concurrent_independent_agent_appends_reload_exact_histories() {
        let dir = tempdir().unwrap();
        let storage = Arc::new(AgentStorage::new(dir.path()));
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let writers: Vec<_> = (0..4)
            .map(|index| {
                let storage = storage.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let id = format!("agent-{index}");
                    barrier.wait();
                    for row in 0..8 {
                        storage
                            .append_timeline_item(&id, message(&format!("row-{row}")))
                            .unwrap();
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let reloaded = AgentStorage::new(dir.path());
        for index in 0..4 {
            let rows = reloaded.get_timeline(&format!("agent-{index}"));
            assert_eq!(rows.len(), 8);
            assert_eq!(rows.first(), Some(&message("row-0")));
            assert_eq!(rows.last(), Some(&message("row-7")));
        }
    }

    #[test]
    fn metadata_commit_does_not_block_unrelated_timeline_io() {
        let dir = tempdir().unwrap();
        let storage = Arc::new(AgentStorage::new(dir.path()));
        // Model a metadata writer waiting on disk while its dedicated commit
        // lock is held. Timeline operations must not acquire this global lock.
        let commit = storage.record_commit.lock().unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let writer_storage = storage.clone();
        let writer = std::thread::spawn(move || {
            done_tx
                .send(writer_storage.append_timeline_item("other", message("row")))
                .unwrap();
        });
        let result = done_rx.recv_timeout(std::time::Duration::from_secs(5));
        drop(commit);
        writer.join().unwrap();
        result
            .expect("timeline waited on metadata disk commit")
            .unwrap();
        assert_eq!(
            storage.with_timeline("other", |rows| rows.len()).unwrap(),
            1
        );
    }
}
