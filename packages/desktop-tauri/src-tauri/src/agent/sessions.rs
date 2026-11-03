use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

use crate::agent::registry::resolve_command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProviderSessionDescriptor {
    pub provider_id: String,
    pub provider_label: String,
    pub provider_handle_id: String,
    pub cwd: String,
    pub title: Option<String>,
    pub first_prompt_preview: Option<String>,
    pub last_prompt_preview: Option<String>,
    pub last_activity_at: String,
}

/// Discovers sessions across specified providers or all supported providers.
pub async fn discover_recent_provider_sessions(
    providers: &[String],
    limit: usize,
    filter_cwd: Option<&str>,
    query: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let mut all_sessions = Vec::new();
    let query_lower = query.map(|q| q.trim().to_lowercase());

    for provider in providers {
        let p_lower = provider.to_lowercase();
        let sessions = match p_lower.as_str() {
            "opencode" => discover_opencode_sessions(limit, filter_cwd).await,
            "codex" => discover_codex_sessions(limit, filter_cwd).await,
            "pi" => discover_pi_sessions(limit, filter_cwd).await,
            "omp" => discover_omp_sessions(limit, filter_cwd).await,
            "claude" => discover_claude_sessions(limit, filter_cwd).await,
            _ => Vec::new(),
        };

        for s in sessions {
            // Apply query filter if present
            if let Some(q) = &query_lower {
                if !q.is_empty() {
                    let matches_title = s.title.as_deref().map(|t| t.to_lowercase().contains(q)).unwrap_or(false);
                    let matches_preview = s.first_prompt_preview.as_deref().map(|p| p.to_lowercase().contains(q)).unwrap_or(false);
                    let matches_id = s.provider_handle_id.to_lowercase().contains(q);
                    if !matches_title && !matches_preview && !matches_id {
                        continue;
                    }
                }
            }
            all_sessions.push(s);
        }
    }

    // Sort by last_activity_at descending
    all_sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    if all_sessions.len() > limit {
        all_sessions.truncate(limit);
    }

    all_sessions
}

/// Discovers OpenCode sessions via `opencode session list --format json`.
pub async fn discover_opencode_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let bin_path = match resolve_command("opencode") {
        Some(p) => p,
        None => return Vec::new(),
    };

    let max_count = (limit * 2).max(20).to_string();
    let mut cmd = tokio::process::Command::new(&bin_path);
    cmd.args(["session", "list", "--format", "json", "-n", &max_count]);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());

    let output = match cmd.output().await {
        Ok(out) if out.status.success() => out.stdout,
        _ => return Vec::new(),
    };

    let items: Vec<serde_json::Value> = match serde_json::from_slice(&output) {
        Ok(arr) => arr,
        _ => return Vec::new(),
    };

    let mut descriptors = Vec::new();
    for item in items {
        let id = match item.get("id").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let title = item.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
        let cwd = item.get("directory")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| filter_cwd.unwrap_or("."))
            .to_string();

        if let Some(target_cwd) = filter_cwd {
            if !paths_match(&cwd, target_cwd) {
                continue;
            }
        }

        let last_activity_at = if let Some(updated_ms) = item.get("updated").and_then(|v| v.as_i64()) {
            format_epoch_millis_iso(updated_ms)
        } else {
            crate::agent::types::now_iso()
        };

        descriptors.push(RecentProviderSessionDescriptor {
            provider_id: "opencode".to_string(),
            provider_label: "OpenCode".to_string(),
            provider_handle_id: id,
            cwd,
            title: title.clone(),
            first_prompt_preview: title,
            last_prompt_preview: None,
            last_activity_at,
        });
    }

    descriptors
}

/// Discovers Codex sessions from `~/.codex/session_index.jsonl`.
pub async fn discover_codex_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let index_file = home.join(".codex").join("session_index.jsonl");
    let file = match File::open(&index_file) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    for line in reader.lines().flatten() {
        if !line.trim().is_empty() {
            lines.push(line);
        }
    }

    // Newest sessions are typically at the end of the append log
    lines.reverse();

    let default_cwd = filter_cwd.unwrap_or_else(|| home.to_str().unwrap_or(".")).to_string();
    let mut descriptors = Vec::new();

    for line in lines {
        if descriptors.len() >= limit {
            break;
        }
        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let id = match obj.get("id").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };

        let title = obj.get("thread_name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let updated_at = obj.get("updated_at")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| "")
            .to_string();

        let last_activity_at = if !updated_at.is_empty() {
            updated_at
        } else {
            crate::agent::types::now_iso()
        };

        descriptors.push(RecentProviderSessionDescriptor {
            provider_id: "codex".to_string(),
            provider_label: "Codex".to_string(),
            provider_handle_id: id,
            cwd: default_cwd.clone(),
            title: title.clone(),
            first_prompt_preview: title,
            last_prompt_preview: None,
            last_activity_at,
        });
    }

    descriptors
}

/// Discovers Pi sessions from `~/.pi/agent/sessions/**/*.jsonl`.
pub async fn discover_pi_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let sessions_dir = home.join(".pi").join("agent").join("sessions");
    let candidate_files = find_jsonl_files_sorted_by_mtime(&sessions_dir, limit * 3);

    let mut descriptors = Vec::new();
    for path in candidate_files {
        if descriptors.len() >= limit {
            break;
        }

        if let Some(desc) = parse_pi_session_file(&path, filter_cwd) {
            descriptors.push(desc);
        }
    }

    descriptors
}

/// Discovers OMP sessions from `~/.omp/agent/sessions/**/*.jsonl`.
pub async fn discover_omp_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let sessions_dir = home.join(".omp").join("agent").join("sessions");
    let candidate_files = find_jsonl_files_sorted_by_mtime(&sessions_dir, limit * 3);

    let mut descriptors = Vec::new();
    for path in candidate_files {
        if descriptors.len() >= limit {
            break;
        }

        if let Some(desc) = parse_omp_session_file(&path, filter_cwd) {
            descriptors.push(desc);
        }
    }

    descriptors
}

/// Discovers Claude sessions from `~/.claude/projects/**/*.jsonl`.
pub async fn discover_claude_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let projects_dir = home.join(".claude").join("projects");
    let candidate_files = find_jsonl_files_sorted_by_mtime(&projects_dir, limit * 3);

    let mut descriptors = Vec::new();
    for path in candidate_files {
        if descriptors.len() >= limit {
            break;
        }

        if let Some(desc) = parse_claude_session_file(&path, filter_cwd) {
            descriptors.push(desc);
        }
    }

    descriptors
}

fn parse_pi_session_file(path: &Path, filter_cwd: Option<&str>) -> Option<RecentProviderSessionDescriptor> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = None;
    let mut session_cwd = None;
    let mut session_timestamp = None;
    let mut first_prompt = None;

    for (idx, line_res) in reader.lines().enumerate() {
        if idx > 15 {
            break;
        }
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(typ) = obj.get("type").and_then(|v| v.as_str()) {
            if typ == "session" {
                if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                    session_id = Some(id.to_string());
                }
                if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
                    session_cwd = Some(cwd.to_string());
                }
                if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
                    session_timestamp = Some(ts.to_string());
                }
            } else if typ == "message" && first_prompt.is_none() {
                if let Some(msg) = obj.get("message") {
                    if msg.get("role").and_then(|v| v.as_str()) == Some("user") {
                        if let Some(content) = msg.get("content").and_then(|v| v.as_array()) {
                            for part in content {
                                if let Some(txt) = part.get("text").and_then(|v| v.as_str()) {
                                    if !txt.trim().is_empty() {
                                        first_prompt = Some(txt.trim().to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let id = session_id.or_else(|| {
        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
    })?;

    let cwd = session_cwd.unwrap_or_else(|| ".".to_string());
    if let Some(target) = filter_cwd {
        if !paths_match(&cwd, target) {
            return None;
        }
    }

    let last_activity_at = session_timestamp.unwrap_or_else(|| file_mtime_iso(path));

    Some(RecentProviderSessionDescriptor {
        provider_id: "pi".to_string(),
        provider_label: "Pi".to_string(),
        provider_handle_id: id,
        cwd,
        title: first_prompt.clone(),
        first_prompt_preview: first_prompt,
        last_prompt_preview: None,
        last_activity_at,
    })
}

fn parse_omp_session_file(path: &Path, filter_cwd: Option<&str>) -> Option<RecentProviderSessionDescriptor> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = None;
    let mut session_cwd = None;
    let mut session_timestamp = None;
    let mut title = None;

    for (idx, line_res) in reader.lines().enumerate() {
        if idx > 15 {
            break;
        }
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(typ) = obj.get("type").and_then(|v| v.as_str()) {
            if typ == "title" && title.is_none() {
                if let Some(t) = obj.get("title").and_then(|v| v.as_str()) {
                    title = Some(t.to_string());
                }
                if session_timestamp.is_none() {
                    if let Some(up) = obj.get("updatedAt").and_then(|v| v.as_str()) {
                        session_timestamp = Some(up.to_string());
                    }
                }
            } else if typ == "session" {
                if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                    session_id = Some(id.to_string());
                }
                if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
                    session_cwd = Some(cwd.to_string());
                }
                if session_timestamp.is_none() {
                    if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
                        session_timestamp = Some(ts.to_string());
                    }
                }
                if title.is_none() {
                    if let Some(t) = obj.get("title").and_then(|v| v.as_str()) {
                        title = Some(t.to_string());
                    }
                }
            }
        }
    }

    let id = session_id.or_else(|| {
        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
    })?;

    let cwd = session_cwd.unwrap_or_else(|| ".".to_string());
    if let Some(target) = filter_cwd {
        if !paths_match(&cwd, target) {
            return None;
        }
    }

    let last_activity_at = session_timestamp.unwrap_or_else(|| file_mtime_iso(path));

    Some(RecentProviderSessionDescriptor {
        provider_id: "omp".to_string(),
        provider_label: "Oh My Pi".to_string(),
        provider_handle_id: id,
        cwd,
        title: title.clone(),
        first_prompt_preview: title,
        last_prompt_preview: None,
        last_activity_at,
    })
}

fn parse_claude_session_file(path: &Path, filter_cwd: Option<&str>) -> Option<RecentProviderSessionDescriptor> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = None;
    let mut first_prompt = None;
    let mut timestamp = None;

    for (idx, line_res) in reader.lines().enumerate() {
        if idx > 20 {
            break;
        }
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if session_id.is_none() {
            if let Some(sid) = obj.get("sessionId").and_then(|v| v.as_str()) {
                session_id = Some(sid.to_string());
            }
        }

        if timestamp.is_none() {
            if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
                timestamp = Some(ts.to_string());
            }
        }

        if first_prompt.is_none() {
            if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                first_prompt = Some(content.to_string());
            } else if let Some(msg) = obj.get("message") {
                if let Some(c) = msg.get("content").and_then(|v| v.as_str()) {
                    first_prompt = Some(c.to_string());
                }
            }
        }
    }

    let id = session_id.or_else(|| {
        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
    })?;

    // Infer cwd from parent directory (e.g. C--tmp -> C:\tmp)
    let parent_name = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or(".");
    let cwd = decode_claude_project_dir(parent_name);

    if let Some(target) = filter_cwd {
        if !paths_match(&cwd, target) {
            return None;
        }
    }

    let last_activity_at = timestamp.unwrap_or_else(|| file_mtime_iso(path));

    Some(RecentProviderSessionDescriptor {
        provider_id: "claude".to_string(),
        provider_label: "Claude".to_string(),
        provider_handle_id: id,
        cwd,
        title: first_prompt.clone(),
        first_prompt_preview: first_prompt,
        last_prompt_preview: None,
        last_activity_at,
    })
}

fn decode_claude_project_dir(encoded: &str) -> String {
    // E.g. "C--tmp" -> "C:\tmp" or "C--Users-Acer--Desktop"
    if encoded.contains("--") {
        encoded.replace("--", "\\").replace('-', "\\")
    } else {
        encoded.to_string()
    }
}

fn paths_match(a: &str, b: &str) -> bool {
    let norm_a = a.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let norm_b = b.replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm_a == norm_b
}

fn format_epoch_millis_iso(millis: i64) -> String {
    let secs = millis / 1000;
    let nsecs = ((millis % 1000) * 1_000_000) as u32;
    if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
        dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    } else {
        crate::agent::types::now_iso()
    }
}

fn file_mtime_iso(path: &Path) -> String {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(dur) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                return format_epoch_millis_iso(dur.as_millis() as i64);
            }
        }
    }
    crate::agent::types::now_iso()
}

fn find_jsonl_files_sorted_by_mtime(dir: &Path, max_files: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files, 0);

    files.sort_by(|a, b| {
        let mtime_a = std::fs::metadata(a).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        let mtime_b = std::fs::metadata(b).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        mtime_b.cmp(&mtime_a)
    });

    if files.len() > max_files {
        files.truncate(max_files);
    }

    files
}

fn collect_files_recursive(dir: &Path, acc: &mut Vec<PathBuf>, depth: usize) {
    if depth > 4 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_files_recursive(&p, acc, depth + 1);
        } else if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
            if ext.eq_ignore_ascii_case("jsonl") || ext.eq_ignore_ascii_case("json") {
                acc.push(p);
            }
        }
    }
}
