use crate::agent::registry::resolve_command;
use crate::agent::types::AgentStatus;
use crate::kernel::server::{agent_record_to_snapshot, get_agent_manager, get_provider_push_tx};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

/// Replacement notifications and authoritative reads must name the same timeline epoch.
pub fn broadcast_timeline_replacement(agent_id: &str) {
    let msg = json!({
        "type": "session",
        "message": {"type": "agent.timeline.replacement", "payload": {"agentId": agent_id, "epoch": "epoch-1"}}
    });
    let _ = get_provider_push_tx().send(msg.to_string());
}

pub fn broadcast_agent_status_update(agent_id: &str) {
    if let Some(agent) = get_agent_manager().get_agent(agent_id) {
        let msg = json!({
            "type": "session",
            "message": {"type": "agent_update", "payload": {"kind": "upsert", "agent": agent_record_to_snapshot(&agent)}}
        });
        let _ = get_provider_push_tx().send(msg.to_string());
    }
}

fn stream_message(agent_id: &str, provider: &str, turn_id: &str, item: &Value) -> Value {
    // Live chunks are deltas, not committed rows. Reusing a row's seq drops later chunks
    // in the client, while incrementing it invents history positions. Completion replaces
    // these ephemeral chunks with the persisted rows and their canonical sequence numbers.
    json!({
        "type": "session",
        "message": {"type": "agent_stream", "payload": {
            "agentId": agent_id,
            "timestamp": crate::agent::types::now_iso(),
            "event": {"type": "timeline", "provider": provider, "turnId": turn_id, "item": item}
        }}
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputKind {
    Assistant,
    Reasoning,
}

struct OutputRow {
    key: String,
    kind: OutputKind,
    text: String,
}

struct ProviderOutput {
    identity: String,
    rows: Vec<OutputRow>,
    message_index: usize,
    claude_message: String,
    error: Option<String>,
}

impl Default for ProviderOutput {
    fn default() -> Self {
        Self {
            identity: {
                static NEXT_TURN: AtomicU64 = AtomicU64::new(0);
                format!(
                    "turn-{}-{}",
                    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos(),
                    NEXT_TURN.fetch_add(1, Ordering::Relaxed)
                )
            },
            rows: Vec::new(),
            message_index: 0,
            claude_message: String::new(),
            error: None,
        }
    }
}

impl ProviderOutput {
    fn update(&mut self, key: String, kind: OutputKind, text: &str, snapshot: bool, live: &mut Vec<Value>) {
        let index = self.rows.iter().position(|row| row.key == key && row.kind == kind)
            .unwrap_or_else(|| {
                self.rows.push(OutputRow { key, kind, text: String::new() });
                self.rows.len() - 1
            });
        let row = &mut self.rows[index];
        let delta = if snapshot {
            // Final events are authoritative, including when only a prefix was streamed.
            // A non-prefix correction is installed by the final timeline replacement.
            let suffix = text.strip_prefix(&row.text).unwrap_or("").to_string();
            row.text.clear();
            row.text.push_str(text);
            suffix
        } else {
            row.text.push_str(text);
            text.to_string()
        };
        if !delta.is_empty() {
            live.push(json!({
                "type": if kind == OutputKind::Reasoning { "reasoning" } else { "assistant_message" },
                "text": delta,
                "messageId": format!("{}:{}", self.identity, index)
            }));
        }
    }

    fn content(&mut self, key: &str, contents: &[Value], live: &mut Vec<Value>) {
        for (index, block) in contents.iter().enumerate() {
            let (kind, field) = match block["type"].as_str() {
                Some("text") => (OutputKind::Assistant, "text"),
                Some("thinking") => (OutputKind::Reasoning, "thinking"),
                _ => continue,
            };
            if let Some(text) = block[field].as_str() {
                self.update(format!("{key}:{index}"), kind, text, true, live);
            }
        }
    }

    fn ingest(&mut self, provider: &str, event: &Value) -> Vec<Value> {
        let mut live = Vec::new();
        let event_type = event["type"].as_str().unwrap_or("");
        match provider {
            "omp" | "pi" => match event_type {
                "message_update" => {
                    let update = &event["assistantMessageEvent"];
                    let kind = match update["type"].as_str() {
                        Some("text_delta") => Some(OutputKind::Assistant),
                        Some("thinking_delta") => Some(OutputKind::Reasoning),
                        _ => None,
                    };
                    if let (Some(kind), Some(delta)) = (kind, update["delta"].as_str()) {
                        let index = update["contentIndex"].as_u64().unwrap_or(0);
                        self.update(format!("{}:{index}", self.message_index), kind, delta, false, &mut live);
                    }
                }
                "message_end" if event["message"]["role"].as_str() == Some("assistant") => {
                    if let Some(content) = event["message"]["content"].as_array() {
                        self.content(&self.message_index.to_string(), content, &mut live);
                    }
                    self.error = event["message"]["errorMessage"].as_str().map(str::to_owned);
                    self.message_index += 1;
                }
                "auto_retry_end" if event["success"].as_bool() == Some(true) => {
                    self.error = None;
                }
                _ => {}
            },
            "opencode" if matches!(event_type, "text" | "reasoning") => {
                let part = &event["part"];
                if let Some(text) = part["text"].as_str() {
                    let key = part["id"].as_str().map(str::to_owned)
                        .unwrap_or_else(|| format!("part:{}", self.rows.len()));
                    let kind = if event_type == "reasoning" { OutputKind::Reasoning } else { OutputKind::Assistant };
                    self.update(key, kind, text, true, &mut live);
                }
            }
            // codex exec's JSONL lifecycle carries full item snapshots, not text deltas.
            // Schema: codex-rs/exec/src/exec_events.rs, ThreadItemDetails::{AgentMessage,Reasoning}.
            "codex" if matches!(event_type, "item.created" | "item.started" | "item.updated" | "item.completed") => {
                let item = &event["item"];
                let kind = match item["type"].as_str() {
                    Some("agent_message") => Some(OutputKind::Assistant),
                    Some("reasoning") => Some(OutputKind::Reasoning),
                    _ => None,
                };
                if let (Some(kind), Some(text)) = (kind, item["text"].as_str()) {
                    let key = item["id"].as_str().unwrap_or("message").to_string();
                    self.update(key, kind, text, true, &mut live);
                }
            }
            "claude" => match event_type {
                "stream_event" if event["parent_tool_use_id"].is_null() => {
                    let partial = &event["event"];
                    match partial["type"].as_str() {
                        Some("message_start") => {
                            self.claude_message = partial["message"]["id"].as_str().unwrap_or("message").to_string();
                        }
                        Some("content_block_start") | Some("content_block_delta") => {
                            let is_start = partial["type"].as_str() == Some("content_block_start");
                            let block = if is_start { &partial["content_block"] } else { &partial["delta"] };
                            let (kind, field) = match block["type"].as_str() {
                                Some("text") | Some("text_delta") => (OutputKind::Assistant, "text"),
                                Some("thinking") | Some("thinking_delta") => (OutputKind::Reasoning, "thinking"),
                                _ => return live,
                            };
                            if let Some(text) = block[field].as_str() {
                                let index = partial["index"].as_u64().unwrap_or(0);
                                self.update(format!("{}:{index}", self.claude_message), kind, text, is_start, &mut live);
                            }
                        }
                        _ => {}
                    }
                }
                "assistant" if event["parent_tool_use_id"].is_null() => {
                    if event["error"].is_string() {
                        self.error = Some(event["error"].as_str().unwrap_or("Claude API error").to_string());
                    } else if let Some(contents) = event["message"]["content"].as_array() {
                        let key = event["message"]["id"].as_str().unwrap_or("message");
                        // With partial messages enabled, Claude can send one finalized
                        // block per assistant frame rather than the entire message array.
                        if contents.len() == 1 {
                            let block = &contents[0];
                            let value = match block["type"].as_str() {
                                Some("text") => Some((OutputKind::Assistant, block["text"].as_str())),
                                Some("thinking") => Some((OutputKind::Reasoning, block["thinking"].as_str())),
                                _ => None,
                            };
                            if let Some((kind, Some(text))) = value {
                                let prefix = format!("{key}:");
                                let existing = self.rows.iter().rev().find(|row| {
                                    row.kind == kind && row.key.starts_with(&prefix) && text.starts_with(&row.text)
                                }).map(|row| row.key.clone());
                                let block_key = existing.unwrap_or_else(|| format!("{key}:final:{}", self.rows.len()));
                                self.update(block_key, kind, text, true, &mut live);
                            }
                        } else {
                            self.content(key, contents, &mut live);
                        }
                    }
                }
                "result" if event["is_error"].as_bool() == Some(true) => {
                    self.error = Some(event["result"].as_str().unwrap_or("Claude turn failed").to_string());
                }
                _ => {}
            },
            _ => {}
        }
        if event_type == "error" || event_type == "turn.failed" {
            let message = event["message"].as_str()
                .or_else(|| event["error"]["message"].as_str())
                .or_else(|| event["error"]["data"]["message"].as_str());
            if let Some(message) = message {
                self.error = Some(message.to_string());
            }
        }
        live
    }
}

/// Start each invocation with the persisted thinking choice, rather than acknowledging
/// a control change without passing it to the provider.
fn provider_arguments(provider: &str, model: Option<&str>, thinking: Option<&str>, cwd: &Path, prompt: &str) -> Vec<String> {
    let mut args: Vec<String> = match provider {
        "omp" | "pi" => vec!["--mode".into(), "json".into(), "-p".into()],
        "opencode" => vec!["run".into(), "--format".into(), "json".into(), "--thinking".into(), "--dir".into(), cwd.to_string_lossy().into_owned()],
        "codex" => vec!["exec".into(), "--json".into(), "-C".into(), cwd.to_string_lossy().into_owned()],
        "claude" => vec!["-p".into(), "--output-format".into(), "stream-json".into(), "--verbose".into(), "--include-partial-messages".into()],
        _ => Vec::new(),
    };
    if let Some(model) = model.filter(|model| !model.is_empty() && matches!(provider, "omp" | "pi" | "opencode" | "codex" | "claude")) {
        args.extend([if provider == "opencode" || provider == "codex" { "-m" } else { "--model" }.into(), model.into()]);
    }
    if let Some(thinking) = thinking.filter(|thinking| !thinking.is_empty()) {
        match provider {
            "omp" | "pi" => args.extend(["--thinking".into(), thinking.into()]),
            "opencode" if thinking != "default" => args.extend(["--variant".into(), thinking.into()]),
            "codex" => args.extend(["-c".into(), format!("model_reasoning_effort={}", json!(thinking))]),
            "claude" => args.extend(["--effort".into(), thinking.into()]),
            _ => {}
        }
    }
    args.push(prompt.to_string());
    args
}

fn persist_output(manager: &crate::agent::manager::AgentManager, agent_id: &str, output: &ProviderOutput) -> Result<(), String> {
    // Preserve provider order rather than collapsing all reasoning ahead of all answers.
    for row in output.rows.iter().filter(|row| !row.text.trim().is_empty()) {
        match row.kind {
            OutputKind::Reasoning => manager.append_thinking(agent_id, &row.text)?,
            OutputKind::Assistant => manager.append_assistant_message(agent_id, &row.text)?,
        }
    }
    Ok(())
}

pub async fn run_agent_turn(agent_id: String, prompt: String) {
    let mgr = get_agent_manager();
    let Some(agent) = mgr.get_agent(&agent_id) else {
        eprintln!("[Runner] Agent {agent_id} not found for turn execution");
        return;
    };
    let model = agent.config.as_ref().and_then(|c| c["model"].as_str());
    let thinking = agent.config.as_ref().and_then(|c| c["thinkingOptionId"].as_str());
    let cwd = if agent.cwd.is_empty() || agent.cwd == "." {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(&agent.cwd)
    };
    let _ = mgr.set_agent_status(&agent_id, AgentStatus::Running);
    broadcast_agent_status_update(&agent_id);
    broadcast_timeline_replacement(&agent_id);

    let result = execute_provider_turn(&agent_id, &agent.provider, model, thinking, &cwd, &prompt).await;
    let (error, turn_id) = match result {
        Ok(output) => (persist_output(mgr, &agent_id, &output).err().or(output.error), Some(output.identity)),
        Err(error) => (Some(error), None),
    };
    if let Some(error) = &error {
        let _ = mgr.append_error_notice(&agent_id, error);
    }
    // Finalize the client's ephemeral reasoning/answer head before authoritative
    // history replaces it, including reasoning-only and failed provider turns.
    let mut terminal_event = if let Some(error) = error {
        json!({"type": "turn_failed", "provider": agent.provider, "error": error})
    } else {
        json!({"type": "turn_completed", "provider": agent.provider})
    };
    if let Some(turn_id) = turn_id {
        terminal_event["turnId"] = json!(turn_id);
    }
    let _ = get_provider_push_tx().send(json!({
        "type": "session",
        "message": {"type": "agent_stream", "payload": {
            "agentId": agent_id,
            "timestamp": crate::agent::types::now_iso(),
            "event": terminal_event,
        }}
    }).to_string());
    let _ = mgr.set_agent_status(&agent_id, AgentStatus::Idle);
    broadcast_agent_status_update(&agent_id);
    broadcast_timeline_replacement(&agent_id);
}

async fn execute_provider_turn(agent_id: &str, provider: &str, model: Option<&str>, thinking: Option<&str>, cwd: &Path, prompt: &str) -> Result<ProviderOutput, String> {
    let bin = resolve_command(provider).ok_or_else(|| format!("Provider '{provider}' is not installed or available on this system."))?;
    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd.args(provider_arguments(provider, model, thinking, cwd, prompt));
    cmd.current_dir(cwd);
    // Non-interactive CLIs otherwise try to read inherited stdin (notably on Windows).
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn {provider}: {e}"))?;
    let stdout = child.stdout.take().ok_or("Failed to capture provider stdout")?;
    let mut stderr = child.stderr.take().ok_or("Failed to capture provider stderr")?;
    // Drain concurrently: a full stderr pipe otherwise stalls stdout indefinitely.
    let stderr_task = tokio::spawn(async move {
        let mut retained = Vec::new();
        let mut buffer = [0u8; 4096];
        while let Ok(count) = stderr.read(&mut buffer).await {
            if count == 0 { break; }
            let keep = count.min(16384usize.saturating_sub(retained.len()));
            retained.extend_from_slice(&buffer[..keep]);
        }
        retained
    });
    let mut output = ProviderOutput::default();
    let structured = matches!(provider, "omp" | "pi" | "opencode" | "codex" | "claude");
    let completion = tokio::time::timeout(Duration::from_secs(300), async {
        let mut reader = BufReader::new(stdout).lines();
        while let Some(line) = reader.next_line().await.map_err(|e| format!("Failed reading {provider} output: {e}"))? {
            if structured {
                if let Ok(event) = serde_json::from_str::<Value>(&line) {
                    for item in output.ingest(provider, &event) {
                        let msg = stream_message(agent_id, provider, &output.identity, &item);
                        let _ = get_provider_push_tx().send(msg.to_string());
                    }
                }
            } else {
                output.update("message".into(), OutputKind::Assistant, &format!("{line}\n"), false, &mut Vec::new());
            }
        }
        child.wait().await.map_err(|e| format!("Failed waiting for {provider}: {e}"))
    }).await;
    match completion {
        Ok(Ok(status)) => {
            let stderr = stderr_task.await.unwrap_or_default();
            if !status.success() && output.error.is_none() {
                let detail = String::from_utf8_lossy(&stderr);
                output.error = Some(format!("{provider} exited with {status}: {}", detail.trim()));
            }
        }
        Ok(Err(error)) => {
            let _ = child.kill().await;
            stderr_task.abort();
            output.error = Some(error);
        }
        Err(_) => {
            let _ = child.kill().await;
            stderr_task.abort();
            output.error = Some(format!("{provider} turn timed out after 5 minutes."));
        }
    }
    Ok(output)
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
