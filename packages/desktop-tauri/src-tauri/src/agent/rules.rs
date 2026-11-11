use std::path::{Path, PathBuf};

const MAX_RULE_FILE_BYTES: u64 = 64 * 1024; // 64 KB per file
const MAX_TOTAL_RULES_BYTES: usize = 128 * 1024; // 128 KB total across all rule files

/// Discovers and aggregates workspace project rules (e.g. .cursorrules, AGENT.md, CLAUDE.md, .kronos/rules)
/// into a structured system prompt context block.
pub fn load_workspace_rules(cwd: &Path) -> Option<String> {
    if !cwd.is_dir() {
        return None;
    }

    let mut rules_sections: Vec<(String, String)> = Vec::new();
    let mut total_bytes = 0usize;

    // 1. Direct root instruction files in priority order
    let candidate_files = [
        ".cursorrules",
        ".kronosrules",
        "AGENT.md",
        "AGENTS.md",
        "CLAUDE.md",
        ".kronos/rules",
    ];

    for file_name in candidate_files {
        let file_path = cwd.join(file_name);
        if let Some(content) = read_rule_file(&file_path) {
            total_bytes += content.len();
            rules_sections.push((file_name.to_string(), content));
            if total_bytes >= MAX_TOTAL_RULES_BYTES {
                break;
            }
        }
    }

    // 2. Scan directory rules: .cursor/rules and .kronos/rules/*.md
    if total_bytes < MAX_TOTAL_RULES_BYTES {
        let rule_dirs = [cwd.join(".cursor").join("rules"), cwd.join(".kronos").join("rules")];
        for dir in rule_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                let mut dir_files: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file()
                            && p.extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| ext == "md" || ext == "mdc")
                    })
                    .collect();
                dir_files.sort();

                for path in dir_files {
                    if let Some(content) = read_rule_file(&path) {
                        let relative_label = path
                            .strip_prefix(cwd)
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_else(|_| path.file_name().unwrap_or_default().to_string_lossy().into_owned());
                        total_bytes += content.len();
                        rules_sections.push((relative_label, content));
                        if total_bytes >= MAX_TOTAL_RULES_BYTES {
                            break;
                        }
                    }
                }
            }
        }
    }

    if rules_sections.is_empty() {
        return None;
    }

    let mut output = String::with_capacity(total_bytes + 256);
    output.push_str("### Workspace Instructions & Rules\n");
    output.push_str("The repository contains explicit guidelines that you MUST follow:\n\n");

    for (label, content) in rules_sections {
        output.push_str(&format!("#### Rules from `{label}`\n"));
        output.push_str(content.trim());
        output.push_str("\n\n");
    }

    Some(output.trim_end().to_string())
}

fn read_rule_file(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }

    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() == 0 || metadata.len() > MAX_RULE_FILE_BYTES {
        return None;
    }

    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_load_workspace_rules_none_when_empty() {
        let dir = tempdir().unwrap();
        assert!(load_workspace_rules(dir.path()).is_none());
    }

    #[test]
    fn test_load_workspace_rules_single_file() {
        let dir = tempdir().unwrap();
        let cursorrules_path = dir.path().join(".cursorrules");
        let mut file = File::create(&cursorrules_path).unwrap();
        writeln!(file, "Never use any in TypeScript. Always prefer strict typing.").unwrap();

        let rules = load_workspace_rules(dir.path()).expect("Should load rules");
        assert!(rules.contains("### Workspace Instructions & Rules"));
        assert!(rules.contains("#### Rules from `.cursorrules`"));
        assert!(rules.contains("Never use any in TypeScript"));
    }

    #[test]
    fn test_load_workspace_rules_multi_files_and_dir() {
        let dir = tempdir().unwrap();
        // Root file
        let agent_path = dir.path().join("AGENT.md");
        let mut f1 = File::create(&agent_path).unwrap();
        writeln!(f1, "Follow hexagonal architecture.").unwrap();

        // Subdirectory rule
        let cursor_rules_dir = dir.path().join(".cursor").join("rules");
        std::fs::create_dir_all(&cursor_rules_dir).unwrap();
        let mdc_path = cursor_rules_dir.join("styling.mdc");
        let mut f2 = File::create(&mdc_path).unwrap();
        writeln!(f2, "Use tailwind classes only.").unwrap();

        let rules = load_workspace_rules(dir.path()).expect("Should load combined rules");
        assert!(rules.contains("#### Rules from `AGENT.md`"));
        assert!(rules.contains("Follow hexagonal architecture."));
        assert!(rules.contains("styling.mdc`"));
        assert!(rules.contains("Use tailwind classes only."));
    }
}
