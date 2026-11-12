use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KronosWorkspaceDoc {
    pub kind: String, // "command" | "notepad" | "playbook"
    pub name: String,
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub engine: String,
    pub raw_output: String,
}

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".next",
    ".turbo",
];

/// Scans `.kronos/commands/*.md`, `.kronos/prompts/*.md`, `.kronos/notepads/*.md`, and `.kronos/playbooks.md`.
pub fn scan_kronos_docs(cwd: &Path) -> Vec<KronosWorkspaceDoc> {
    let mut docs = Vec::new();
    let kronos_dir = cwd.join(".kronos");
    if !kronos_dir.is_dir() {
        return docs;
    }

    let playbooks_file = kronos_dir.join("playbooks.md");
    if playbooks_file.is_file() {
        if let Ok(content) = std::fs::read_to_string(&playbooks_file) {
            docs.push(KronosWorkspaceDoc {
                kind: "playbook".to_string(),
                name: "playbooks".to_string(),
                relative_path: ".kronos/playbooks.md".to_string(),
                content,
            });
        }
    }

    for (subdir, kind) in [
        ("commands", "command"),
        ("prompts", "command"),
        ("notepads", "notepad"),
    ] {
        let dir = kronos_dir.join(subdir);
        if !dir.is_dir() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Ok(content) = std::fs::read_to_string(&p) {
                        let name = p
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("doc")
                            .to_string();
                        let rel = format!(
                            ".kronos/{subdir}/{}",
                            p.file_name().and_then(|s| s.to_str()).unwrap_or("doc.md")
                        );
                        docs.push(KronosWorkspaceDoc {
                            kind: kind.to_string(),
                            name,
                            relative_path: rel,
                            content,
                        });
                    }
                }
            }
        }
    }

    docs.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    docs
}

/// Safely saves markdown documents strictly inside `<cwd>/.kronos/...`.
pub fn save_kronos_doc(cwd: &Path, relative_path: &str, content: &str) -> Result<String, String> {
    let normalized = relative_path.replace('\\', "/");
    if !normalized.starts_with(".kronos/") || normalized.contains("..") {
        return Err("Path must stay strictly within .kronos/".to_string());
    }
    let full = cwd.join(&normalized);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .kronos directory: {e}"))?;
    }
    std::fs::write(&full, content).map_err(|e| format!("Failed to write {normalized}: {e}"))?;
    Ok(normalized)
}

/// Discovers `.db`, `.sqlite`, and `.sqlite3` files inside the workspace.
pub fn discover_databases(cwd: &Path) -> Vec<String> {
    let mut found = Vec::new();
    if !cwd.is_dir() {
        return found;
    }
    collect_dbs(cwd, cwd, &mut found, 0, 5);
    found.sort();
    found
}

fn collect_dbs(root: &Path, dir: &Path, out: &mut Vec<String>, depth: usize, max_depth: usize) {
    if depth > max_depth || out.len() >= 30 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if p.is_dir() {
            if SKIP_DIRS.contains(&name_str.as_ref()) || name_str.starts_with('.') {
                continue;
            }
            collect_dbs(root, &p, out, depth + 1, max_depth);
        } else if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let lower = ext.to_lowercase();
                if matches!(lower.as_str(), "db" | "sqlite" | "sqlite3") {
                    if let Ok(rel) = p.strip_prefix(root) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
    }
}

/// Validates SQL against read-only policy.
pub fn validate_sql_safety(sql: &str, read_only: bool) -> Result<(), String> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err("SQL query cannot be empty".to_string());
    }
    if read_only {
        let upper = trimmed.to_uppercase();
        for forbidden in ["DROP ", "DELETE ", "UPDATE ", "INSERT ", "ALTER ", "TRUNCATE ", "REPLACE ", "CREATE "] {
            if upper.starts_with(forbidden) || upper.contains(&format!(";{forbidden}")) {
                return Err(format!("Blocked mutating statement '{forbidden}' in read-only mode"));
            }
        }
    }
    Ok(())
}

/// Executes a SQL query against a local SQLite database via `sqlite3` CLI or Python `sqlite3` fallback.
pub async fn run_sqlite_query(
    db_path: &PathBuf,
    sql: &str,
    read_only: bool,
) -> Result<DatabaseQueryResult, String> {
    validate_sql_safety(sql, read_only)?;

    if !db_path.is_file() {
        return Err(format!("Database file not found: {}", db_path.display()));
    }

    // Try native sqlite3 CLI first
    let mut cli = tokio::process::Command::new("sqlite3");
    if read_only {
        cli.arg("-readonly");
    }
    cli.arg("-json").arg(db_path).arg(sql);
    #[cfg(windows)]
    cli.creation_flags(0x08000000);

    if let Ok(output) = cli.output().await {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return parse_json_rows(&stdout, "sqlite3");
        }
    }

    // Fallback to Python stdlib sqlite3 (available on most dev hosts)
    let py_script = r#"
import json, sqlite3, sys
db, query = sys.argv[1], sys.argv[2]
conn = sqlite3.connect(f"file:{db}?mode=ro" if sys.argv[3] == "1" else db, uri=(sys.argv[3] == "1"))
conn.row_factory = sqlite3.Row
cur = conn.execute(query)
rows = [dict(r) for r in cur.fetchall()] if cur.description else []
print(json.dumps(rows))
"#;
    let py_bin = if cfg!(windows) { "python" } else { "python3" };
    let mut py = tokio::process::Command::new(py_bin);
    py.args([
        "-c",
        py_script,
        &db_path.to_string_lossy(),
        sql,
        if read_only { "1" } else { "0" },
    ]);
    #[cfg(windows)]
    py.creation_flags(0x08000000);

    let out = py
        .output()
        .await
        .map_err(|e| format!("Neither `sqlite3` nor `{py_bin}` available to query DB: {e}"))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "SQL query execution failed".to_string()
        } else {
            stderr
        });
    }

    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    parse_json_rows(&stdout, "python-sqlite3")
}

fn parse_json_rows(json_text: &str, engine: &str) -> Result<DatabaseQueryResult, String> {
    if json_text.is_empty() || json_text == "[]" {
        return Ok(DatabaseQueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            engine: engine.to_string(),
            raw_output: json_text.to_string(),
        });
    }

    let val: serde_json::Value =
        serde_json::from_str(json_text).map_err(|e| format!("Invalid query JSON: {e}"))?;
    let Some(arr) = val.as_array() else {
        return Ok(DatabaseQueryResult {
            columns: vec!["result".to_string()],
            rows: vec![vec![val.to_string()]],
            row_count: 1,
            engine: engine.to_string(),
            raw_output: json_text.to_string(),
        });
    };

    let mut columns = Vec::new();
    if let Some(first) = arr.first().and_then(|v| v.as_object()) {
        for key in first.keys() {
            columns.push(key.clone());
        }
    }

    let mut rows = Vec::with_capacity(arr.len());
    for item in arr {
        if let Some(obj) = item.as_object() {
            let row = columns
                .iter()
                .map(|c| match obj.get(c) {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Null) | None => "NULL".to_string(),
                    Some(other) => other.to_string(),
                })
                .collect();
            rows.push(row);
        }
    }

    Ok(DatabaseQueryResult {
        row_count: rows.len(),
        columns,
        rows,
        engine: engine.to_string(),
        raw_output: json_text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_scan_kronos_docs_blocks_traversal() {
        let dir = tempdir().unwrap();
        assert!(save_kronos_doc(dir.path(), "../evil.md", "nope").is_err());
        save_kronos_doc(dir.path(), ".kronos/playbooks.md", "# Build\nRun cargo test").unwrap();
        save_kronos_doc(
            dir.path(),
            ".kronos/notepads/api.md",
            "Always validate pagination cursors.",
        )
        .unwrap();
        save_kronos_doc(
            dir.path(),
            ".kronos/commands/review.md",
            "---\nname: review-sec\ndescription: Sec audit\n---\nAudit $DIFF",
        )
        .unwrap();

        let docs = scan_kronos_docs(dir.path());
        assert_eq!(docs.len(), 3);
        assert!(docs.iter().any(|d| d.kind == "playbook"));
        assert!(docs.iter().any(|d| d.kind == "notepad" && d.name == "api"));
        assert!(docs.iter().any(|d| d.kind == "command" && d.name == "review"));
    }

    #[test]
    fn test_sql_read_only_safety_blocks_mutations() {
        assert!(validate_sql_safety("SELECT * FROM users;", true).is_ok());
        assert!(validate_sql_safety("PRAGMA table_info(users);", true).is_ok());
        assert!(validate_sql_safety("DROP TABLE users;", true).is_err());
        assert!(validate_sql_safety("DELETE FROM users;", true).is_err());
        assert!(validate_sql_safety("DELETE FROM users;", false).is_ok());
    }
}
