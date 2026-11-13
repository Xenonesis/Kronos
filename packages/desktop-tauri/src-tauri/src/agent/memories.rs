//! Kronos Persistent Agent Memories System
//!
//! Preserves cross-session user preferences, architecture conventions, and learned coding habits
//! in `.kronos/memories.md` inside the project root, automatically injecting them into the agent's
//! system prompt instructions.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const KRONOS_DIR_NAME: &str = ".kronos";
pub const MEMORIES_FILE_NAME: &str = "memories.md";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub category: String,
    pub content: String,
    pub created_at: String,
}

impl MemoryEntry {
    pub fn new(category: impl Into<String>, content: impl Into<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let cat = category.into();
        let cont = content.into();
        let id = format!("mem_{}", &now[..19].replace(['-', ':', 'T'], ""));
        Self {
            id,
            category: cat,
            content: cont,
            created_at: now,
        }
    }
}

/// Resolves path to `<workspace>/.kronos/memories.md`
pub fn get_memories_file_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(KRONOS_DIR_NAME).join(MEMORIES_FILE_NAME)
}

/// Loads all persistent memories from `.kronos/memories.md` if it exists.
pub fn load_workspace_memories(workspace_root: &Path) -> Vec<MemoryEntry> {
    let file_path = get_memories_file_path(workspace_root);
    if !file_path.exists() {
        return Vec::new();
    }

    let Ok(content) = fs::read_to_string(&file_path) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    let mut current_category = "General".to_string();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            current_category = trimmed.trim_start_matches('#').trim().to_string();
            continue;
        }

        if let Some(bullet) = trimmed.strip_prefix('-').or_else(|| trimmed.strip_prefix('*')) {
            let memory_text = bullet.trim();
            if !memory_text.is_empty() {
                let mut entry = MemoryEntry::new(current_category.clone(), memory_text);
                entry.id = format!("{}_{}", entry.id, entries.len());
                entries.push(entry);
            }
        }
    }

    entries
}

/// Replaces `.kronos/memories.md` with the provided full list of memory entries.
pub fn save_workspace_memories(
    workspace_root: &Path,
    entries: &[MemoryEntry],
) -> io::Result<()> {
    let kronos_dir = workspace_root.join(KRONOS_DIR_NAME);
    if !kronos_dir.exists() {
        fs::create_dir_all(&kronos_dir)?;
    }

    let file_path = kronos_dir.join(MEMORIES_FILE_NAME);
    let mut out = String::from(
        "# Kronos Project Memories\n<!-- Persistent cross-session user preferences and project conventions -->\n",
    );

    let mut by_category: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for entry in entries {
        let cat = entry.category.trim();
        let text = entry.content.trim();
        if !text.is_empty() {
            let key = if cat.is_empty() { "General" } else { cat };
            by_category.entry(key).or_default().push(text);
        }
    }

    for (cat, items) in by_category {
        out.push_str(&format!("\n## {cat}\n"));
        for item in items {
            out.push_str(&format!("- {item}\n"));
        }
    }

    fs::write(&file_path, out)
}

/// Appends a new memory item to `.kronos/memories.md`.
pub fn append_workspace_memory(
    workspace_root: &Path,
    category: &str,
    content: &str,
) -> io::Result<()> {
    let kronos_dir = workspace_root.join(KRONOS_DIR_NAME);
    if !kronos_dir.exists() {
        fs::create_dir_all(&kronos_dir)?;
    }

    let file_path = kronos_dir.join(MEMORIES_FILE_NAME);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)?;

    // If file is new or empty, write a welcoming header
    if file.metadata()?.len() == 0 {
        writeln!(
            file,
            "# Kronos Project Memories\n<!-- Persistent cross-session user preferences and project conventions -->\n"
        )?;
    }

    writeln!(file, "\n## {}\n- {}", category, content.trim())?;
    Ok(())
}

/// Formats loaded memories into markdown for inclusion in the agent system prompt.
pub fn format_memories_for_system_prompt(memories: &[MemoryEntry]) -> Option<String> {
    if memories.is_empty() {
        return None;
    }

    let mut out = String::from("## Persistent Project & User Memories\nThe user has established the following rules across past sessions for this workspace:\n");
    for mem in memories {
        out.push_str(&format!("- [{}] {}\n", mem.category, mem.content));
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_memories_lifecycle() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();

        // 1. Initially empty
        assert!(load_workspace_memories(root).is_empty());

        // 2. Add memory
        append_workspace_memory(root, "Code Style", "Always prefer functional programming").expect("append");
        append_workspace_memory(root, "Tooling", "Use pnpm instead of npm").expect("append");

        // 3. Load back
        let loaded = load_workspace_memories(root);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].category, "Code Style");
        assert_eq!(loaded[0].content, "Always prefer functional programming");

        // 4. Prompt formatting
        let formatted = format_memories_for_system_prompt(&loaded).expect("format");
        assert!(formatted.contains("Always prefer functional programming"));
        assert!(formatted.contains("Use pnpm instead of npm"));
    }
}
