use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NoteRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, alias = "created_at_ms")]
    pub created_at_ms: i64,
    #[serde(default, alias = "updated_at_ms")]
    pub updated_at_ms: i64,
}

pub struct NotesStore {
    root_dir: PathBuf,
    lock: RwLock<()>,
}

static GLOBAL_NOTES_STORE: OnceLock<NotesStore> = OnceLock::new();

pub fn get_notes_store() -> &'static NotesStore {
    GLOBAL_NOTES_STORE.get_or_init(NotesStore::default_store)
}

fn sanitize_id(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if sanitized.is_empty() {
        format!("note-{}", chrono::Utc::now().timestamp_millis())
    } else {
        sanitized
    }
}

fn generate_id(title: &str) -> String {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = slug.trim_matches('-');
    let ts = chrono::Utc::now().timestamp_millis();
    if trimmed.is_empty() {
        format!("note-{}", ts)
    } else {
        let prefix: String = trimmed.chars().take(30).collect();
        format!("{}-{}", prefix.trim_matches('-'), ts)
    }
}

impl NotesStore {
    pub fn new<P: Into<PathBuf>>(root_dir: P) -> Self {
        Self {
            root_dir: root_dir.into(),
            lock: RwLock::new(()),
        }
    }

    pub fn default_store() -> Self {
        let data_dir = crate::get_data_dir().join("notes");
        Self::new(data_dir)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    fn ensure_dir(&self) -> Result<(), String> {
        if !self.root_dir.exists() {
            fs::create_dir_all(&self.root_dir)
                .map_err(|e| format!("Failed to create notes directory '{}': {e}", self.root_dir.display()))?;
        }
        Ok(())
    }

    fn note_path(&self, id: &str) -> PathBuf {
        self.root_dir.join(format!("{}.json", sanitize_id(id)))
    }

    pub fn list_notes(&self) -> Vec<NoteRecord> {
        let _guard = match self.lock.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        if !self.root_dir.exists() {
            return Vec::new();
        }

        let entries = match fs::read_dir(&self.root_dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut notes = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Ok(bytes) = fs::read(&path) {
                    if let Ok(note) = serde_json::from_slice::<NoteRecord>(&bytes) {
                        notes.push(note);
                    }
                }
            }
        }

        // Pinned notes first, then updated_at_ms desc, then created_at_ms desc, then id
        notes.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at_ms.cmp(&a.updated_at_ms))
                .then_with(|| b.created_at_ms.cmp(&a.created_at_ms))
                .then_with(|| a.id.cmp(&b.id))
        });

        notes
    }

    pub fn get_note(&self, id: &str) -> Option<NoteRecord> {
        let _guard = self.lock.read().ok()?;
        let path = self.note_path(id);
        if !path.exists() {
            return None;
        }
        let bytes = fs::read(&path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn save_note(&self, mut note: NoteRecord) -> Result<NoteRecord, String> {
        let _guard = self.lock.write().map_err(|e| e.to_string())?;
        self.ensure_dir()?;

        if note.id.trim().is_empty() {
            note.id = generate_id(&note.title);
        } else {
            note.id = sanitize_id(&note.id);
        }

        let now = chrono::Utc::now().timestamp_millis();
        if note.created_at_ms <= 0 {
            note.created_at_ms = now;
        }
        if note.updated_at_ms <= 0 {
            note.updated_at_ms = now;
        }

        let path = self.note_path(&note.id);
        let serialized = serde_json::to_vec_pretty(&note)
            .map_err(|e| format!("Failed to serialize note '{}': {e}", note.id))?;

        fs::write(&path, serialized)
            .map_err(|e| format!("Failed to write note file '{}': {e}", path.display()))?;

        Ok(note)
    }

    pub fn delete_note(&self, id: &str) -> Result<bool, String> {
        let _guard = self.lock.write().map_err(|e| e.to_string())?;
        let path = self.note_path(id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete note file '{}': {e}", path.display()))?;
        Ok(true)
    }

    pub fn search_notes(&self, query: &str) -> Vec<NoteRecord> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.list_notes();
        }
        let q = trimmed.to_lowercase();
        let all = self.list_notes();
        all.into_iter()
            .filter(|n| {
                n.title.to_lowercase().contains(&q)
                    || n.content.to_lowercase().contains(&q)
                    || n.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notes_store_crud() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = NotesStore::new(tmp.path());

        // Create
        let note = NoteRecord {
            id: "test-note-1".to_string(),
            title: "First Note".to_string(),
            content: "# Markdown Content".to_string(),
            tags: vec!["rust".to_string(), "tauri".to_string()],
            pinned: false,
            created_at_ms: 1000,
            updated_at_ms: 1000,
        };

        let saved = store.save_note(note).expect("save should succeed");
        assert_eq!(saved.id, "test-note-1");

        // Read
        let fetched = store.get_note("test-note-1").expect("should find note");
        assert_eq!(fetched.title, "First Note");
        assert_eq!(fetched.content, "# Markdown Content");
        assert_eq!(fetched.tags, vec!["rust", "tauri"]);
        assert!(!fetched.pinned);

        // Update
        let mut updated = fetched;
        updated.content = "## Updated Markdown Content".to_string();
        updated.updated_at_ms = 2000;
        store.save_note(updated).expect("update should succeed");

        let fetched_after = store.get_note("test-note-1").expect("should find note");
        assert_eq!(fetched_after.content, "## Updated Markdown Content");
        assert_eq!(fetched_after.updated_at_ms, 2000);

        // Delete
        let deleted = store.delete_note("test-note-1").expect("delete should succeed");
        assert!(deleted);
        assert!(store.get_note("test-note-1").is_none());

        // Delete non-existent
        let deleted_again = store.delete_note("test-note-1").expect("delete should succeed");
        assert!(!deleted_again);
    }

    #[test]
    fn test_notes_store_auto_id_and_timestamps() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = NotesStore::new(tmp.path());

        let note = NoteRecord {
            id: "".to_string(),
            title: "Scratchpad Note".to_string(),
            content: "Quick thought".to_string(),
            tags: vec![],
            pinned: false,
            created_at_ms: 0,
            updated_at_ms: 0,
        };

        let saved = store.save_note(note).expect("save should succeed");
        assert!(!saved.id.is_empty());
        assert!(saved.id.starts_with("scratchpad-note-"));
        assert!(saved.created_at_ms > 0);
        assert!(saved.updated_at_ms > 0);

        let fetched = store.get_note(&saved.id).expect("found by auto id");
        assert_eq!(fetched.title, "Scratchpad Note");
    }

    #[test]
    fn test_notes_store_persistence() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let store = NotesStore::new(tmp.path());
            store
                .save_note(NoteRecord {
                    id: "persistent-1".to_string(),
                    title: "P1".to_string(),
                    content: "C1".to_string(),
                    tags: vec![],
                    pinned: false,
                    created_at_ms: 100,
                    updated_at_ms: 100,
                })
                .unwrap();
            store
                .save_note(NoteRecord {
                    id: "persistent-2".to_string(),
                    title: "P2".to_string(),
                    content: "C2".to_string(),
                    tags: vec![],
                    pinned: true,
                    created_at_ms: 200,
                    updated_at_ms: 200,
                })
                .unwrap();
        }

        // New instance reading same dir
        let store_reloaded = NotesStore::new(tmp.path());
        let list = store_reloaded.list_notes();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "persistent-2"); // pinned first!
        assert_eq!(list[1].id, "persistent-1");
    }

    #[test]
    fn test_notes_store_sorting_pinned_first_then_updated_desc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = NotesStore::new(tmp.path());

        // Note 1: unpinned, updated 100
        store
            .save_note(NoteRecord {
                id: "n1".to_string(),
                title: "N1".to_string(),
                content: "".to_string(),
                tags: vec![],
                pinned: false,
                created_at_ms: 100,
                updated_at_ms: 100,
            })
            .unwrap();

        // Note 2: unpinned, updated 500
        store
            .save_note(NoteRecord {
                id: "n2".to_string(),
                title: "N2".to_string(),
                content: "".to_string(),
                tags: vec![],
                pinned: false,
                created_at_ms: 100,
                updated_at_ms: 500,
            })
            .unwrap();

        // Note 3: pinned, updated 200
        store
            .save_note(NoteRecord {
                id: "n3".to_string(),
                title: "N3".to_string(),
                content: "".to_string(),
                tags: vec![],
                pinned: true,
                created_at_ms: 100,
                updated_at_ms: 200,
            })
            .unwrap();

        // Note 4: pinned, updated 400
        store
            .save_note(NoteRecord {
                id: "n4".to_string(),
                title: "N4".to_string(),
                content: "".to_string(),
                tags: vec![],
                pinned: true,
                created_at_ms: 100,
                updated_at_ms: 400,
            })
            .unwrap();

        let list = store.list_notes();
        assert_eq!(list.len(), 4);
        // Expected order:
        // 1. n4 (pinned, updated 400)
        // 2. n3 (pinned, updated 200)
        // 3. n2 (unpinned, updated 500)
        // 4. n1 (unpinned, updated 100)
        assert_eq!(list[0].id, "n4");
        assert_eq!(list[1].id, "n3");
        assert_eq!(list[2].id, "n2");
        assert_eq!(list[3].id, "n1");
    }

    #[test]
    fn test_notes_store_search() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = NotesStore::new(tmp.path());

        store
            .save_note(NoteRecord {
                id: "rust-arch".to_string(),
                title: "Rust Architecture".to_string(),
                content: "Monocode inspired storage engine".to_string(),
                tags: vec!["kernel".to_string(), "backend".to_string()],
                pinned: false,
                created_at_ms: 100,
                updated_at_ms: 100,
            })
            .unwrap();

        store
            .save_note(NoteRecord {
                id: "frontend-ui".to_string(),
                title: "Desktop UI".to_string(),
                content: "React with tailwindcss styling".to_string(),
                tags: vec!["frontend".to_string()],
                pinned: true,
                created_at_ms: 200,
                updated_at_ms: 200,
            })
            .unwrap();

        // Search by title
        let res1 = store.search_notes("architecture");
        assert_eq!(res1.len(), 1);
        assert_eq!(res1[0].id, "rust-arch");

        // Search by content (case-insensitive)
        let res2 = store.search_notes("MONOCODE");
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].id, "rust-arch");

        // Search by tag
        let res3 = store.search_notes("frontend");
        assert_eq!(res3.len(), 1);
        assert_eq!(res3[0].id, "frontend-ui");

        // Search empty returns all
        let all = store.search_notes("  ");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "frontend-ui"); // pinned first

        // Search nonexistent
        let none = store.search_notes("nonexistent query");
        assert!(none.is_empty());
    }
}
