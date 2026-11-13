use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

use crate::agent::registry::resolve_command;

/// UTF-8 safe string truncation by Unicode character count (never panics on multi-byte chars or emojis).
pub fn safe_truncate(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let prefix: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", prefix)
    }
}

/// UTF-8 safe prefix extraction by character count.
pub fn safe_prefix(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniversalIndexedSessionItem {
    pub id: String,
    pub provider_id: String,     // "antigravity" | "omp" | "claude" | "codex" | "opencode" | "pi" | "kronos"
    pub provider_label: String,  // "Antigravity", "OMP", "Claude Code", "Codex", etc.
    pub project_name: String,    // "paseo", "sysmon", etc.
    pub cwd: String,
    pub title: String,
    pub last_message_preview: String, // "Agent: ..." / "You: ..."
    pub last_activity_at: String,
    pub message_count: usize,
    pub subagent_count: usize,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedHistoryResult {
    pub scanned_files_count: usize,
    pub total_sessions: usize,
    pub sessions: Vec<UniversalIndexedSessionItem>,
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
            "antigravity" => discover_antigravity_sessions(limit, filter_cwd).await,
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
                    let matches_proj = s.project_name.as_deref().map(|p| p.to_lowercase().contains(q)).unwrap_or(false);
                    if !matches_title && !matches_preview && !matches_id && !matches_proj {
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

/// Global system-wide indexer aggregating all session files, tracking file counts and rich card metrics.
pub async fn scan_all_indexed_sessions(
    scope: &str,         // "workspace", "project", "all"
    filter_cwd: Option<&str>,
    query: Option<&str>,
    sort: &str,          // "last_updated", "messages", "newest"
    limit: usize,
) -> IndexedHistoryResult {
    let mut all_sessions: Vec<UniversalIndexedSessionItem> = Vec::new();
    let mut total_scanned_files = 0;

    // Concurrently scan all providers in parallel
    let (
        (ag_sessions, ag_scanned),
        (omp_sessions, omp_scanned),
        (claude_sessions, claude_scanned),
        (codex_sessions, codex_scanned),
        opencode_sessions,
        (pi_sessions, pi_scanned),
    ) = tokio::join!(
        scan_antigravity_sessions_raw(limit * 3, None),
        scan_omp_sessions_raw(limit * 3, None),
        scan_claude_sessions_raw(limit * 3, None),
        scan_codex_sessions_raw(limit * 3, None),
        discover_opencode_sessions(limit * 2, None),
        scan_pi_sessions_raw(limit * 2, None),
    );

    total_scanned_files += ag_scanned + omp_scanned + claude_scanned + codex_scanned + opencode_sessions.len() + pi_scanned;
    all_sessions.extend(ag_sessions);
    all_sessions.extend(omp_sessions);
    all_sessions.extend(claude_sessions);
    all_sessions.extend(codex_sessions);

    for s in opencode_sessions {
        all_sessions.push(UniversalIndexedSessionItem {
            id: s.provider_handle_id,
            provider_id: "opencode".to_string(),
            provider_label: "OpenCode".to_string(),
            project_name: s.project_name.unwrap_or_else(|| "general".to_string()),
            cwd: s.cwd,
            title: s.title.unwrap_or_else(|| "OpenCode Session".to_string()),
            last_message_preview: s.last_prompt_preview.unwrap_or_else(|| "Active session".to_string()),
            last_activity_at: s.last_activity_at,
            message_count: s.message_count.unwrap_or(1),
            subagent_count: s.subagent_count.unwrap_or(0),
            model: s.model,
        });
    }

    all_sessions.extend(pi_sessions);

    // 7. Native Kronos sessions
    let mgr = crate::kernel::server::get_agent_manager();
    for agent in mgr.list_agents() {
        let proj = Path::new(&agent.cwd)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("general")
            .to_string();
        all_sessions.push(UniversalIndexedSessionItem {
            id: agent.id.clone(),
            provider_id: "kronos".to_string(),
            provider_label: "Kronos".to_string(),
            project_name: proj,
            cwd: agent.cwd.clone(),
            title: agent.title.clone().unwrap_or_else(|| format!("Kronos Agent {}", safe_prefix(&agent.id, 8))),
            last_message_preview: format!("Status: {:?}", agent.last_status),
            last_activity_at: agent.last_activity_at.clone().unwrap_or(agent.updated_at.clone()),
            message_count: 1,
            subagent_count: 0,
            model: agent.config.as_ref().and_then(|c| c.get("model")).and_then(|m| m.as_str()).map(|s| s.to_string()),
        });
        total_scanned_files += 1;
    }

    // Filter by Scope
    let target_cwd_norm = filter_cwd.map(|c| c.replace('\\', "/").trim_end_matches('/').to_lowercase());
    let target_project_name = filter_cwd.and_then(|c| Path::new(c).file_name()).and_then(|s| s.to_str()).map(|s| s.to_lowercase());

    let mut filtered_sessions: Vec<UniversalIndexedSessionItem> = all_sessions
        .into_iter()
        .filter(|s| {
            match scope {
                "workspace" => {
                    if let Some(ref target) = target_cwd_norm {
                        let s_cwd = s.cwd.replace('\\', "/").trim_end_matches('/').to_lowercase();
                        s_cwd == *target || target.starts_with(&s_cwd) || s_cwd.starts_with(target)
                    } else {
                        true
                    }
                }
                "project" => {
                    if let Some(ref target_p) = target_project_name {
                        s.project_name.to_lowercase() == *target_p
                    } else {
                        true
                    }
                }
                _ => true, // "all"
            }
        })
        .collect();

    // Filter by Search Query
    if let Some(q) = query {
        let q_lower = q.trim().to_lowercase();
        if !q_lower.is_empty() {
            filtered_sessions.retain(|s| {
                s.title.to_lowercase().contains(&q_lower)
                    || s.last_message_preview.to_lowercase().contains(&q_lower)
                    || s.provider_label.to_lowercase().contains(&q_lower)
                    || s.project_name.to_lowercase().contains(&q_lower)
                    || s.id.to_lowercase().contains(&q_lower)
            });
        }
    }

    let total_sessions = filtered_sessions.len();

    // Sort
    match sort {
        "messages" => {
            filtered_sessions.sort_by(|a, b| b.message_count.cmp(&a.message_count));
        }
        "newest" => {
            filtered_sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
        }
        _ => { // "last_updated"
            filtered_sessions.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
        }
    }

    if filtered_sessions.len() > limit {
        filtered_sessions.truncate(limit);
    }

    IndexedHistoryResult {
        scanned_files_count: total_scanned_files,
        total_sessions,
        sessions: filtered_sessions,
    }
}

/// Discovers Antigravity sessions from `~/.gemini/antigravity-cli/brain/*/`.
pub async fn discover_antigravity_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let (items, _) = scan_antigravity_sessions_raw(limit, filter_cwd).await;
    items
        .into_iter()
        .map(|item| RecentProviderSessionDescriptor {
            provider_id: item.provider_id,
            provider_label: item.provider_label,
            provider_handle_id: item.id,
            cwd: item.cwd,
            title: Some(item.title.clone()),
            first_prompt_preview: Some(item.title),
            last_prompt_preview: Some(item.last_message_preview),
            last_activity_at: item.last_activity_at,
            project_name: Some(item.project_name),
            message_count: Some(item.message_count),
            subagent_count: Some(item.subagent_count),
            model: item.model,
        })
        .collect()
}

pub async fn scan_antigravity_sessions_raw(
    limit: usize,
    filter_cwd: Option<&str>,
) -> (Vec<UniversalIndexedSessionItem>, usize) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return (Vec::new(), 0),
    };

    let brain_dir = home.join(".gemini").join("antigravity-cli").join("brain");
    if !brain_dir.is_dir() {
        return (Vec::new(), 0);
    }

    let entries = match std::fs::read_dir(&brain_dir) {
        Ok(e) => e,
        Err(_) => return (Vec::new(), 0),
    };

    let mut scanned_count = 0;
    let mut candidate_transcripts = Vec::new();

    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let tr = p.join(".system_generated").join("logs").join("transcript.jsonl");
            if tr.is_file() {
                scanned_count += 1;
                candidate_transcripts.push(tr);
            }
        }
    }

    candidate_transcripts.sort_by(|a, b| {
        let mtime_a = std::fs::metadata(a).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        let mtime_b = std::fs::metadata(b).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        mtime_b.cmp(&mtime_a)
    });

    let mut sessions = Vec::new();
    for path in candidate_transcripts {
        if sessions.len() >= limit {
            break;
        }
        if let Some(item) = parse_antigravity_transcript(&path, filter_cwd) {
            sessions.push(item);
        }
    }

    (sessions, scanned_count)
}

fn parse_antigravity_transcript(
    path: &Path,
    filter_cwd: Option<&str>,
) -> Option<UniversalIndexedSessionItem> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let session_id = path
        .parent()? // logs
        .parent()? // .system_generated
        .parent()? // <uuid>
        .file_name()?
        .to_str()?
        .to_string();

    let mut first_prompt = None;
    let mut last_preview = None;
    let mut session_cwd = None;
    let mut message_count = 0;
    let mut subagent_count = 0;
    let mut last_activity_at = None;

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        message_count += 1;
        if line.contains("invoke_subagent") {
            subagent_count += 1;
        }

        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(ts) = obj.get("created_at").and_then(|v| v.as_str()) {
            last_activity_at = Some(ts.to_string());
        }

        let content_opt = obj.get("content").and_then(|v| v.as_str());

        // Extract CWD if not found yet
        if session_cwd.is_none() {
            if let Some(content) = content_opt {
                if let Some(idx) = content.find("Active Workspaces:") {
                    let rem = &content[idx + "Active Workspaces:".len()..];
                    for l in rem.lines() {
                        let trimmed = l.trim();
                        if let Some(p) = trimmed.strip_prefix("- ") {
                            let p_clean = p.trim();
                            if !p_clean.is_empty() {
                                session_cwd = Some(p_clean.to_string());
                                break;
                            }
                        }
                    }
                } else if let Some(idx) = content.find("Command Working Directory:") {
                    let rem = &content[idx + "Command Working Directory:".len()..];
                    if let Some(first_line) = rem.lines().next() {
                        let trimmed = first_line.trim();
                        if !trimmed.is_empty() {
                            session_cwd = Some(trimmed.to_string());
                        }
                    }
                }
            }
        }

        // Extract first user prompt as title
        if first_prompt.is_none() {
            let src = obj.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if src == "USER_EXPLICIT" || typ == "USER_INPUT" {
                if let Some(content) = content_opt {
                    let cleaned = if let Some(start) = content.find("<USER_REQUEST>") {
                        let after = &content[start + "<USER_REQUEST>".len()..];
                        if let Some(end) = after.find("</USER_REQUEST>") {
                            after[..end].trim()
                        } else {
                            after.trim()
                        }
                    } else {
                        content.trim()
                    };
                    if !cleaned.is_empty() {
                        let first_line = cleaned.lines().next().unwrap_or(cleaned).trim();
                        let truncated = safe_truncate(first_line, 80);
                        first_prompt = Some(truncated);
                    }
                }
            }
        }

        // Keep updating last_preview
        if let Some(content) = content_opt {
            let src = obj.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let clean_content = content
                .lines()
                .filter(|l| !l.trim().starts_with('<') && !l.trim().ends_with('>'))
                .collect::<Vec<_>>()
                .join(" ");
            let preview = clean_content.trim();
            if !preview.is_empty() {
                let snippet = safe_truncate(preview, 90);
                if src == "MODEL" || typ == "PLANNER_RESPONSE" {
                    last_preview = Some(format!("Agent: {}", snippet));
                } else if src == "USER_EXPLICIT" || typ == "USER_INPUT" {
                    last_preview = Some(format!("You: {}", snippet));
                }
            }
        }
    }

    let cwd = session_cwd.unwrap_or_else(|| ".".to_string());
    if let Some(target) = filter_cwd {
        if !paths_match(&cwd, target) {
            return None;
        }
    }

    let project_name = Path::new(&cwd)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("general")
        .to_string();

    let title = first_prompt.unwrap_or_else(|| format!("Session {}", safe_prefix(&session_id, 8)));
    let last_activity_at = last_activity_at.unwrap_or_else(|| file_mtime_iso(path));

    Some(UniversalIndexedSessionItem {
        id: session_id,
        provider_id: "antigravity".to_string(),
        provider_label: "Antigravity".to_string(),
        project_name,
        cwd,
        title,
        last_message_preview: last_preview.unwrap_or_else(|| "No messages".to_string()),
        last_activity_at,
        message_count,
        subagent_count,
        model: None,
    })
}

/// Discovers OMP sessions from `~/.omp/agent/sessions/**/*.jsonl`.
pub async fn discover_omp_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let (items, _) = scan_omp_sessions_raw(limit, filter_cwd).await;
    items
        .into_iter()
        .map(|item| RecentProviderSessionDescriptor {
            provider_id: item.provider_id,
            provider_label: item.provider_label,
            provider_handle_id: item.id,
            cwd: item.cwd,
            title: Some(item.title.clone()),
            first_prompt_preview: Some(item.title),
            last_prompt_preview: Some(item.last_message_preview),
            last_activity_at: item.last_activity_at,
            project_name: Some(item.project_name),
            message_count: Some(item.message_count),
            subagent_count: Some(item.subagent_count),
            model: item.model,
        })
        .collect()
}

pub async fn scan_omp_sessions_raw(
    limit: usize,
    filter_cwd: Option<&str>,
) -> (Vec<UniversalIndexedSessionItem>, usize) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return (Vec::new(), 0),
    };

    let sessions_dir = home.join(".omp").join("agent").join("sessions");
    let candidate_files = find_jsonl_files_sorted_by_mtime(&sessions_dir, limit * 4);
    let scanned_count = candidate_files.len();

    let mut sessions = Vec::new();
    for path in candidate_files {
        if sessions.len() >= limit {
            break;
        }

        if let Some(desc) = parse_omp_session_file_rich(&path, filter_cwd) {
            sessions.push(desc);
        }
    }

    (sessions, scanned_count)
}

/// Inspects a single OMP session JSONL file from `~/.omp/agent/sessions/**/*.jsonl`,
/// returning parsed turns, tool calls, modified file paths, and real summary stats.
pub fn inspect_omp_session(target_id: &str) -> Option<serde_json::Value> {
    if target_id.trim().is_empty() {
        return None;
    }
    let home = dirs::home_dir()?;
    let sessions_dir = home.join(".omp").join("agent").join("sessions");
    let candidate_files = find_jsonl_files_sorted_by_mtime(&sessions_dir, 200);

    let mut matched_path: Option<PathBuf> = None;
    for path in candidate_files {
        let stem_matches = path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s == target_id);
        if stem_matches {
            matched_path = Some(path);
            break;
        }
        if let Ok(f) = File::open(&path) {
            let mut r = BufReader::new(f);
            let mut first_line = String::new();
            if r.read_line(&mut first_line).is_ok() {
                if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&first_line) {
                    if obj.get("id").and_then(|v| v.as_str()) == Some(target_id) {
                        matched_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    let path = matched_path?;
    let file = File::open(&path).ok()?;
    let reader = BufReader::new(file);

    let mut title: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut turns: Vec<serde_json::Value> = Vec::new();
    let mut file_diffs_set = std::collections::BTreeSet::<String>::new();
    let mut total_tool_calls: usize = 0;

    for line_res in reader.lines() {
        let Ok(line) = line_res else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if typ == "title" && title.is_none() {
            title = obj.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
        } else if typ == "session" && cwd.is_none() {
            cwd = obj.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
        }

        if let Some(msg) = obj.get("message") {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("assistant");
            let ts = obj
                .get("timestamp")
                .or_else(|| msg.get("timestamp"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let mut text_parts = Vec::new();
            let mut tool_calls = Vec::new();
            if let Some(content) = msg.get("content") {
                if let Some(s) = content.as_str() {
                    if !s.trim().is_empty() {
                        text_parts.push(s.to_string());
                    }
                } else if let Some(arr) = content.as_array() {
                    for part in arr {
                        let ptype = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if ptype == "text" {
                            if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                                if !t.trim().is_empty() {
                                    text_parts.push(t.to_string());
                                }
                            }
                        } else if ptype == "toolCall" || ptype == "tool_use" {
                            total_tool_calls += 1;
                            let name = part
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("tool")
                                .to_string();
                            let input = part
                                .get("arguments")
                                .or_else(|| part.get("input"))
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            for key in ["path", "filePath", "TargetFile", "file"] {
                                if let Some(fp) = input.get(key).and_then(|v| v.as_str()) {
                                    if matches!(
                                        name.to_ascii_lowercase().as_str(),
                                        "edit" | "write" | "write_to_file" | "replace_file_content" | "patch" | "edit_file" | "write_file"
                                    ) {
                                        file_diffs_set.insert(fp.to_string());
                                    }
                                }
                            }
                            tool_calls.push(serde_json::json!({
                                "name": name,
                                "input": input,
                            }));
                        }
                    }
                }
            }
            turns.push(serde_json::json!({
                "role": role,
                "text": text_parts.join("\n"),
                "toolCalls": tool_calls,
                "timestamp": ts,
            }));
        }
    }

    let file_diffs: Vec<serde_json::Value> = file_diffs_set
        .iter()
        .map(|p| serde_json::json!({ "path": p }))
        .collect();
    let files_modified = file_diffs.len();
    let total_turns = turns.len();

    Some(serde_json::json!({
        "id": target_id,
        "title": title.unwrap_or_else(|| format!("OMP {}", safe_prefix(target_id, 8))),
        "cwd": cwd,
        "turns": turns,
        "fileDiffs": file_diffs,
        "stats": {
            "totalTurns": total_turns,
            "totalToolCalls": total_tool_calls,
            "filesModified": files_modified,
        }
    }))
}

fn parse_omp_session_file_rich(
    path: &Path,
    filter_cwd: Option<&str>,
) -> Option<UniversalIndexedSessionItem> {
    let file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let file_len = metadata.len();

    let mut reader = BufReader::new(file);

    let mut session_id = None;
    let mut session_cwd = None;
    let mut session_timestamp = None;
    let mut title = None;
    let mut model = None;
    let mut first_user_prompt = None;
    let mut last_preview = None;
    let mut message_count = 0;
    let mut subagent_count = 0;

    // Fast-path: if file is huge (> 10MB), read head and tail to avoid latency
    if file_len > 10 * 1024 * 1024 {
        message_count = (file_len / 350).max(100) as usize;
        subagent_count = (message_count / 80).max(1);

        let mut head_buf = vec![0u8; 32 * 1024];
        let _ = reader.read(&mut head_buf);
        let head_str = String::from_utf8_lossy(&head_buf);

        for line in head_str.lines().take(40) {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(typ) = obj.get("type").and_then(|v| v.as_str()) {
                    if typ == "title" && title.is_none() {
                        if let Some(t) = obj.get("title").and_then(|v| v.as_str()) {
                            title = Some(t.to_string());
                        }
                    } else if typ == "model_change" && model.is_none() {
                        if let Some(m) = obj.get("model").and_then(|v| v.as_str()) {
                            model = Some(m.to_string());
                        }
                    } else if typ == "session" {
                        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                            session_id = Some(id.to_string());
                        }
                        if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
                            session_cwd = Some(cwd.to_string());
                        }
                    }
                }
            }
        }

        // Tail read
        if let Ok(mut tail_file) = File::open(path) {
            let seek_pos = file_len.saturating_sub(64 * 1024);
            if tail_file.seek(SeekFrom::Start(seek_pos)).is_ok() {
                let mut tail_buf = Vec::new();
                let _ = tail_file.read_to_end(&mut tail_buf);
                let tail_str = String::from_utf8_lossy(&tail_buf);
                for line in tail_str.lines().rev().take(15) {
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                        if session_timestamp.is_none() {
                            if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
                                session_timestamp = Some(ts.to_string());
                            }
                        }
                        if last_preview.is_none() {
                            if let Some(msg) = obj.get("message") {
                                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("agent");
                                if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                                    let clean = content.trim().split('\n').filter(|s| !s.is_empty()).last().unwrap_or("").trim();
                                    if !clean.is_empty() {
                                        let snip = safe_truncate(clean, 80);
                                        last_preview = Some(format!("{}: {}", if role == "user" { "You" } else { "Agent" }, snip));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        // Normal sequential read
        for line_res in reader.lines() {
            let line = match line_res {
                Ok(l) => l,
                Err(_) => continue,
            };
            if line.trim().is_empty() {
                continue;
            }
            message_count += 1;
            if line.contains("\"subagent\"") || line.contains("task_delegate") {
                subagent_count += 1;
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
                } else if typ == "model_change" && model.is_none() {
                    if let Some(m) = obj.get("model").and_then(|v| v.as_str()) {
                        model = Some(m.to_string());
                    }
                } else if typ == "session" {
                    if session_id.is_none() {
                        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                            session_id = Some(id.to_string());
                        }
                    }
                    if session_cwd.is_none() {
                        if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
                            session_cwd = Some(cwd.to_string());
                        }
                    }
                }
            }

            if let Some(msg) = obj.get("message") {
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("agent");
                let text_opt = msg.get("content").and_then(|c| {
                    if let Some(s) = c.as_str() {
                        Some(s.to_string())
                    } else if let Some(arr) = c.as_array() {
                        arr.iter().find_map(|p| {
                            if p.get("type").and_then(|t| t.as_str()) == Some("text") {
                                p.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                });

                if let Some(text) = text_opt {
                    let clean = text.trim().lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").trim();
                    if !clean.is_empty() {
                        let snip = safe_truncate(clean, 80);
                        last_preview = Some(format!("{}: {}", if role == "user" { "You" } else { "Agent" }, snip));
                        if first_user_prompt.is_none() && role == "user" {
                            first_user_prompt = Some(snip);
                        }
                    }
                }
            }
        }
    }

    let id = session_id.or_else(|| {
        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
    })?;

    // Project name from parent folder, e.g. "-Desktop-paseo" -> "paseo"
    let parent_folder = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("general");
    let project_name = parent_folder
        .replace("-Desktop-", "")
        .replace("-Desktop", "")
        .replace("--C--tmp--", "tmp")
        .replace("--", "/")
        .replace('-', " ")
        .trim()
        .to_string();

    let cwd = session_cwd.unwrap_or_else(|| {
        dirs::desktop_dir()
            .or_else(dirs::home_dir)
            .map(|base| base.join(&project_name).to_string_lossy().into_owned())
            .unwrap_or_else(|| project_name.clone())
    });

    if let Some(target) = filter_cwd {
        if !paths_match(&cwd, target) {
            return None;
        }
    }

    let final_title = title
        .or(first_user_prompt)
        .unwrap_or_else(|| format!("OMP {}", safe_prefix(&id, 8)));
    let last_activity_at = session_timestamp.unwrap_or_else(|| file_mtime_iso(path));

    Some(UniversalIndexedSessionItem {
        id,
        provider_id: "omp".to_string(),
        provider_label: "OMP".to_string(),
        project_name: if project_name.is_empty() { "general".to_string() } else { project_name },
        cwd,
        title: final_title,
        last_message_preview: last_preview.unwrap_or_else(|| "Active OMP session".to_string()),
        last_activity_at,
        message_count: message_count.max(1),
        subagent_count,
        model,
    })
}

/// Discovers Claude sessions from `~/.claude/projects/**/*.jsonl`.
pub async fn discover_claude_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let (items, _) = scan_claude_sessions_raw(limit, filter_cwd).await;
    items
        .into_iter()
        .map(|item| RecentProviderSessionDescriptor {
            provider_id: item.provider_id,
            provider_label: item.provider_label,
            provider_handle_id: item.id,
            cwd: item.cwd,
            title: Some(item.title.clone()),
            first_prompt_preview: Some(item.title),
            last_prompt_preview: Some(item.last_message_preview),
            last_activity_at: item.last_activity_at,
            project_name: Some(item.project_name),
            message_count: Some(item.message_count),
            subagent_count: Some(item.subagent_count),
            model: item.model,
        })
        .collect()
}

pub async fn scan_claude_sessions_raw(
    limit: usize,
    filter_cwd: Option<&str>,
) -> (Vec<UniversalIndexedSessionItem>, usize) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return (Vec::new(), 0),
    };

    let projects_dir = home.join(".claude").join("projects");
    let candidate_files = find_jsonl_files_sorted_by_mtime(&projects_dir, limit * 3);
    let scanned_count = candidate_files.len();

    let mut sessions = Vec::new();
    for path in candidate_files {
        if sessions.len() >= limit {
            break;
        }

        if let Some(desc) = parse_claude_session_file_rich(&path, filter_cwd) {
            sessions.push(desc);
        }
    }

    (sessions, scanned_count)
}

fn parse_claude_session_file_rich(
    path: &Path,
    filter_cwd: Option<&str>,
) -> Option<UniversalIndexedSessionItem> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = None;
    let mut first_prompt = None;
    let mut last_preview = None;
    let mut timestamp = None;
    let mut message_count = 0usize;
    let mut subagent_count = 0usize;
    let mut detected_model: Option<String> = None;
    let mut explicit_cwd: Option<String> = None;

    for line_res in reader.lines() {
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

        let entry_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if matches!(entry_type, "user" | "assistant" | "message")
            || obj.get("message").is_some()
            || obj.get("content").is_some()
        {
            message_count += 1;
        }

        if obj.get("parent_tool_use_id").and_then(|v| v.as_str()).is_some()
            || entry_type == "subagent"
        {
            subagent_count += 1;
        }

        if session_id.is_none() {
            if let Some(sid) = obj.get("sessionId").and_then(|v| v.as_str()) {
                session_id = Some(sid.to_string());
            }
        }

        if explicit_cwd.is_none() {
            if let Some(c) = obj.get("cwd").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                explicit_cwd = Some(c.to_string());
            }
        }

        if let Some(m) = obj
            .get("message")
            .and_then(|m| m.get("model"))
            .and_then(|v| v.as_str())
            .or_else(|| obj.get("model").and_then(|v| v.as_str()))
            .filter(|s| !s.is_empty())
        {
            detected_model = Some(m.to_string());
        }

        if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_str()) {
            timestamp = Some(ts.to_string());
        }

        let content_text = obj.get("content").and_then(|v| v.as_str())
            .or_else(|| obj.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_str()));

        if let Some(text) = content_text {
            let clean = text.trim();
            if !clean.is_empty() {
                let snip = safe_truncate(clean, 80);
                if first_prompt.is_none() {
                    first_prompt = Some(snip.clone());
                }
                last_preview = Some(format!("Claude: {}", snip));
            }
        }
    }

    let id = session_id.or_else(|| {
        path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
    })?;

    let parent_name = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or(".");
    let cwd = explicit_cwd.unwrap_or_else(|| decode_claude_project_dir(parent_name));
    let project_name = Path::new(&cwd).file_name().and_then(|s| s.to_str()).unwrap_or("general").to_string();

    if let Some(target) = filter_cwd {
        if !paths_match(&cwd, target) {
            return None;
        }
    }

    let last_activity_at = timestamp.unwrap_or_else(|| file_mtime_iso(path));

    Some(UniversalIndexedSessionItem {
        id,
        provider_id: "claude".to_string(),
        provider_label: "Claude Code".to_string(),
        project_name,
        cwd,
        title: first_prompt.unwrap_or_else(|| "Claude Session".to_string()),
        last_message_preview: last_preview.unwrap_or_else(|| "Active conversation".to_string()),
        last_activity_at,
        message_count: message_count.max(1),
        subagent_count,
        model: detected_model,
    })
}

/// Inspects `~/.codex/sessions` rollout JSONL files for a given Codex session ID to extract
/// real message counts, model name, and working directory when available on disk.
fn inspect_codex_rollout_metadata(
    codex_dir: &Path,
    session_id: &str,
) -> (Option<usize>, Option<String>, Option<String>, usize) {
    let sessions_root = codex_dir.join("sessions");
    if !sessions_root.is_dir() {
        return (None, None, None, 0);
    }
    let mut files = Vec::new();
    collect_files_recursive(&sessions_root, &mut files, 0);
    for file_path in files {
        let fname = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !fname.contains(session_id) {
            continue;
        }
        let Ok(file) = File::open(&file_path) else {
            continue;
        };
        let reader = BufReader::new(file);
        let mut msg_count = 0usize;
        let mut sub_count = 0usize;
        let mut model: Option<String> = None;
        let mut cwd: Option<String> = None;

        for line in reader.lines().flatten() {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if matches!(typ, "message" | "response_item" | "event_msg" | "user" | "assistant") {
                msg_count += 1;
            }
            if typ.contains("subagent") {
                sub_count += 1;
            }
            if cwd.is_none() {
                if let Some(c) = obj
                    .get("cwd")
                    .or_else(|| obj.get("payload").and_then(|p| p.get("cwd")))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    cwd = Some(c.to_string());
                }
            }
            if let Some(m) = obj
                .get("model")
                .or_else(|| obj.get("payload").and_then(|p| p.get("model")))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                model = Some(m.to_string());
            }
        }
        return (Some(msg_count.max(1)), model, cwd, sub_count);
    }
    (None, None, None, 0)
}

/// Discovers Codex sessions from `~/.codex/session_index.jsonl`.
pub async fn discover_codex_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let (items, _) = scan_codex_sessions_raw(limit, filter_cwd).await;
    items
        .into_iter()
        .map(|item| RecentProviderSessionDescriptor {
            provider_id: item.provider_id,
            provider_label: item.provider_label,
            provider_handle_id: item.id,
            cwd: item.cwd,
            title: Some(item.title.clone()),
            first_prompt_preview: Some(item.title),
            last_prompt_preview: Some(item.last_message_preview),
            last_activity_at: item.last_activity_at,
            project_name: Some(item.project_name),
            message_count: Some(item.message_count),
            subagent_count: Some(item.subagent_count),
            model: item.model,
        })
        .collect()
}

pub async fn scan_codex_sessions_raw(
    limit: usize,
    filter_cwd: Option<&str>,
) -> (Vec<UniversalIndexedSessionItem>, usize) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return (Vec::new(), 0),
    };

    let codex_dir = home.join(".codex");
    let index_file = codex_dir.join("session_index.jsonl");
    let file = match File::open(&index_file) {
        Ok(f) => f,
        Err(_) => return (Vec::new(), 0),
    };

    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    for line in reader.lines().flatten() {
        if !line.trim().is_empty() {
            lines.push(line);
        }
    }

    let scanned_count = lines.len();
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

        let (rollout_msgs, rollout_model, rollout_cwd, subagent_count) =
            inspect_codex_rollout_metadata(&codex_dir, &id);
        let cwd = obj
            .get("cwd")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or(rollout_cwd)
            .unwrap_or_else(|| default_cwd.clone());

        if let Some(target_cwd) = filter_cwd {
            if !paths_match(&cwd, target_cwd) {
                continue;
            }
        }

        let message_count = obj
            .get("message_count")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .or(rollout_msgs)
            .unwrap_or(1);
        let model = obj
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or(rollout_model);

        let proj = Path::new(&cwd).file_name().and_then(|s| s.to_str()).unwrap_or("general").to_string();

        descriptors.push(UniversalIndexedSessionItem {
            id,
            provider_id: "codex".to_string(),
            provider_label: "Codex".to_string(),
            project_name: proj,
            cwd,
            title: title.clone().unwrap_or_else(|| "Codex Session".to_string()),
            last_message_preview: title.unwrap_or_else(|| "Codex Task".to_string()),
            last_activity_at,
            message_count,
            subagent_count,
            model,
        });
    }

    (descriptors, scanned_count)
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

        let message_count = item
            .get("messageCount")
            .or_else(|| item.get("message_count"))
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .or_else(|| item.get("messages").and_then(|m| m.as_array()).map(|a| a.len()));
        let model = item
            .get("model")
            .or_else(|| item.get("modelID"))
            .or_else(|| item.get("modelId"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let proj = Path::new(&cwd).file_name().and_then(|s| s.to_str()).unwrap_or("general").to_string();

        descriptors.push(RecentProviderSessionDescriptor {
            provider_id: "opencode".to_string(),
            provider_label: "OpenCode".to_string(),
            provider_handle_id: id,
            cwd,
            title: title.clone(),
            first_prompt_preview: title,
            last_prompt_preview: None,
            last_activity_at,
            project_name: Some(proj),
            message_count,
            subagent_count: Some(0),
            model,
        });
    }

    descriptors
}

/// Discovers Pi sessions from `~/.pi/agent/sessions/**/*.jsonl`.
pub async fn discover_pi_sessions(
    limit: usize,
    filter_cwd: Option<&str>,
) -> Vec<RecentProviderSessionDescriptor> {
    let (items, _) = scan_pi_sessions_raw(limit, filter_cwd).await;
    items
        .into_iter()
        .map(|item| RecentProviderSessionDescriptor {
            provider_id: item.provider_id,
            provider_label: item.provider_label,
            provider_handle_id: item.id,
            cwd: item.cwd,
            title: Some(item.title.clone()),
            first_prompt_preview: Some(item.title),
            last_prompt_preview: Some(item.last_message_preview),
            last_activity_at: item.last_activity_at,
            project_name: Some(item.project_name),
            message_count: Some(item.message_count),
            subagent_count: Some(item.subagent_count),
            model: item.model,
        })
        .collect()
}

pub async fn scan_pi_sessions_raw(
    limit: usize,
    filter_cwd: Option<&str>,
) -> (Vec<UniversalIndexedSessionItem>, usize) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return (Vec::new(), 0),
    };

    let sessions_dir = home.join(".pi").join("agent").join("sessions");
    let candidate_files = find_jsonl_files_sorted_by_mtime(&sessions_dir, limit * 3);
    let scanned_count = candidate_files.len();

    let mut descriptors = Vec::new();
    for path in candidate_files {
        if descriptors.len() >= limit {
            break;
        }

        if let Some(desc) = parse_pi_session_file(&path, filter_cwd) {
            descriptors.push(desc);
        }
    }

    (descriptors, scanned_count)
}

fn parse_pi_session_file(path: &Path, filter_cwd: Option<&str>) -> Option<UniversalIndexedSessionItem> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = None;
    let mut session_cwd = None;
    let mut session_timestamp = None;
    let mut first_prompt = None;
    let mut last_prompt = None;
    let mut detected_model: Option<String> = None;
    let mut message_count = 0usize;

    for line_res in reader.lines() {
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

        if let Some(m) = obj
            .get("model")
            .or_else(|| obj.get("modelId"))
            .or_else(|| obj.get("message").and_then(|msg| msg.get("model")))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            detected_model = Some(m.to_string());
        }

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
            } else if typ == "message" {
                message_count += 1;
                if let Some(content) = obj.get("content").and_then(|v| v.as_str()) {
                    let snip = safe_truncate(content.trim(), 80);
                    if first_prompt.is_none() {
                        first_prompt = Some(snip.clone());
                    }
                    last_prompt = Some(snip);
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

    let proj = Path::new(&cwd).file_name().and_then(|s| s.to_str()).unwrap_or("general").to_string();
    let last_activity_at = session_timestamp.unwrap_or_else(|| file_mtime_iso(path));

    Some(UniversalIndexedSessionItem {
        id,
        provider_id: "pi".to_string(),
        provider_label: "Pi".to_string(),
        project_name: proj,
        cwd,
        title: first_prompt.clone().unwrap_or_else(|| "Pi Session".to_string()),
        last_message_preview: last_prompt.or(first_prompt).unwrap_or_else(|| "Active Pi session".to_string()),
        last_activity_at,
        message_count: message_count.max(1),
        subagent_count: 0,
        model: detected_model,
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

pub fn paths_match(a: &str, b: &str) -> bool {
    let norm_a = a.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let norm_b = b.replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm_a == norm_b
}

pub fn format_epoch_millis_iso(millis: i64) -> String {
    let secs = millis / 1000;
    let nsecs = ((millis % 1000) * 1_000_000) as u32;
    if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
        dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    } else {
        crate::agent::types::now_iso()
    }
}

pub fn file_mtime_iso(path: &Path) -> String {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(dur) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                return format_epoch_millis_iso(dur.as_millis() as i64);
            }
        }
    }
    crate::agent::types::now_iso()
}

pub fn find_jsonl_files_sorted_by_mtime(dir: &Path, max_files: usize) -> Vec<PathBuf> {
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

pub fn collect_files_recursive(dir: &Path, acc: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scan_all_indexed_sessions_live() {
        let res = scan_all_indexed_sessions("all", None, None, "last_updated", 100).await;
        println!("Scanned files: {}, total: {}", res.scanned_files_count, res.total_sessions);
    }
}
