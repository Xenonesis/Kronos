use crate::agent::manager::ReservedTurn;
use crate::agent::registry::resolve_command;
#[cfg(test)]
use crate::agent::types::AgentStatus;
use crate::kernel::server::{
    agent_record_to_snapshot, get_agent_manager, get_plugin_manager, get_provider_push_tx,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

#[path = "turn_process.rs"]
mod turn_process;

/// Replacement notifications and authoritative reads must name the same timeline epoch.
pub fn broadcast_timeline_replacement(agent_id: &str) {
    let msg = json!({
        "type": "session",
        "message": {"type": "agent.timeline.replacement", "payload": {"agentId": agent_id, "epoch": crate::kernel::timeline::epoch(agent_id)}}
    });
    let _ = get_provider_push_tx().send(msg.to_string());
}

pub fn broadcast_agent_status_update(agent_id: &str) {
    broadcast_agent_status(get_agent_manager(), agent_id);
}

fn broadcast_agent_status(manager: &crate::agent::manager::AgentManager, agent_id: &str) {
    if let Some(agent) = manager.get_agent(agent_id) {
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
    cleanup_confirmed: bool,
    usage: Option<Value>,
}

impl Default for ProviderOutput {
    fn default() -> Self {
        Self {
            identity: {
                static NEXT_TURN: AtomicU64 = AtomicU64::new(0);
                format!(
                    "turn-{}-{}",
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos(),
                    NEXT_TURN.fetch_add(1, Ordering::Relaxed)
                )
            },
            rows: Vec::new(),
            message_index: 0,
            claude_message: String::new(),
            error: None,
            cleanup_confirmed: true,
            usage: None,
        }
    }
}

impl ProviderOutput {
    pub fn has_assistant_message(&self) -> bool {
        self.rows
            .iter()
            .any(|row| row.kind == OutputKind::Assistant && !row.text.trim().is_empty())
    }

    fn update(
        &mut self,
        key: String,
        kind: OutputKind,
        text: &str,
        snapshot: bool,
        live: &mut Vec<Value>,
    ) {
        let index = self
            .rows
            .iter()
            .position(|row| row.key == key && row.kind == kind)
            .unwrap_or_else(|| {
                self.rows.push(OutputRow {
                    key,
                    kind,
                    text: String::new(),
                });
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
                        self.update(
                            format!("{}:{index}", self.message_index),
                            kind,
                            delta,
                            false,
                            &mut live,
                        );
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
                    let key = part["id"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("part:{}", self.rows.len()));
                    let kind = if event_type == "reasoning" {
                        OutputKind::Reasoning
                    } else {
                        OutputKind::Assistant
                    };
                    self.update(key, kind, text, true, &mut live);
                }
            }
            // codex exec's JSONL lifecycle carries full item snapshots, not text deltas.
            // Schema: codex-rs/exec/src/exec_events.rs, ThreadItemDetails::{AgentMessage,Reasoning}.
            "codex"
                if matches!(
                    event_type,
                    "item.created" | "item.started" | "item.updated" | "item.completed"
                ) =>
            {
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
                            self.claude_message = partial["message"]["id"]
                                .as_str()
                                .unwrap_or("message")
                                .to_string();
                        }
                        Some("content_block_start") | Some("content_block_delta") => {
                            let is_start = partial["type"].as_str() == Some("content_block_start");
                            let block = if is_start {
                                &partial["content_block"]
                            } else {
                                &partial["delta"]
                            };
                            let (kind, field) = match block["type"].as_str() {
                                Some("text") | Some("text_delta") => {
                                    (OutputKind::Assistant, "text")
                                }
                                Some("thinking") | Some("thinking_delta") => {
                                    (OutputKind::Reasoning, "thinking")
                                }
                                _ => return live,
                            };
                            if let Some(text) = block[field].as_str() {
                                let index = partial["index"].as_u64().unwrap_or(0);
                                self.update(
                                    format!("{}:{index}", self.claude_message),
                                    kind,
                                    text,
                                    is_start,
                                    &mut live,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                "assistant" if event["parent_tool_use_id"].is_null() => {
                    if event["error"].is_string() {
                        self.error = Some(
                            event["error"]
                                .as_str()
                                .unwrap_or("Claude API error")
                                .to_string(),
                        );
                    } else if let Some(contents) = event["message"]["content"].as_array() {
                        let key = event["message"]["id"].as_str().unwrap_or("message");
                        // With partial messages enabled, Claude can send one finalized
                        // block per assistant frame rather than the entire message array.
                        if contents.len() == 1 {
                            let block = &contents[0];
                            let value = match block["type"].as_str() {
                                Some("text") => {
                                    Some((OutputKind::Assistant, block["text"].as_str()))
                                }
                                Some("thinking") => {
                                    Some((OutputKind::Reasoning, block["thinking"].as_str()))
                                }
                                _ => None,
                            };
                            if let Some((kind, Some(text))) = value {
                                let prefix = format!("{key}:");
                                let existing = self
                                    .rows
                                    .iter()
                                    .rev()
                                    .find(|row| {
                                        row.kind == kind
                                            && row.key.starts_with(&prefix)
                                            && text.starts_with(&row.text)
                                    })
                                    .map(|row| row.key.clone());
                                let block_key = existing
                                    .unwrap_or_else(|| format!("{key}:final:{}", self.rows.len()));
                                self.update(block_key, kind, text, true, &mut live);
                            }
                        } else {
                            self.content(key, contents, &mut live);
                        }
                    }
                }
                "result" if event["is_error"].as_bool() == Some(true) => {
                    self.error = Some(
                        event["result"]
                            .as_str()
                            .unwrap_or("Claude turn failed")
                            .to_string(),
                    );
                }
                _ => {}
            },
            _ => {}
        }
        if event_type == "error" || event_type == "turn.failed" {
            let message = event["message"]
                .as_str()
                .or_else(|| event["error"]["message"].as_str())
                .or_else(|| event["error"]["data"]["message"].as_str());
            if let Some(message) = message {
                let is_cancelled_prompt = message.contains("Cancelled a request for input")
                    || message.contains("this run cannot answer");
                if !is_cancelled_prompt {
                    self.error = Some(message.to_string());
                }
            }
        }
        if let Some(stats) = event.get("stats") {
            let mut usage = self.usage.clone().unwrap_or_else(|| json!({}));
            if let Some(ctx) = stats.get("contextUsage") {
                if let Some(tokens) = ctx.get("tokens").and_then(|t| t.as_u64()) {
                    usage["contextWindowUsedTokens"] = json!(tokens);
                }
                if let Some(cw) = ctx.get("contextWindow").and_then(|w| w.as_u64()) {
                    usage["contextWindowMaxTokens"] = json!(cw);
                }
            }
            if let Some(cost) = stats.get("cost").and_then(|c| c.as_f64()) {
                usage["totalCostUsd"] = json!(cost);
            }
            if let Some(tokens) = stats.get("tokens") {
                if let Some(out) = tokens.get("output").and_then(|o| o.as_u64()) {
                    usage["outputTokens"] = json!(out);
                }
                if let Some(inp) = tokens.get("input").and_then(|i| i.as_u64()) {
                    usage["inputTokens"] = json!(inp);
                }
            }
            if !usage.as_object().map_or(true, |o| o.is_empty()) {
                self.usage = Some(usage);
            }
        }
        if let Some(model_usage) = event.get("modelUsage").and_then(|m| m.as_object()) {
            for (_mid, info) in model_usage {
                if let Some(cw) = info.get("contextWindow").and_then(|w| w.as_u64()) {
                    if cw > 0 {
                        let usage = self.usage.get_or_insert_with(|| json!({}));
                        usage["contextWindowMaxTokens"] = json!(cw);
                    }
                }
            }
        }
        if let Some(usage_val) = event.get("usage").or_else(|| event.pointer("/event/usage")) {
            let inp = usage_val
                .get("input_tokens")
                .or_else(|| usage_val.get("input"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let out = usage_val
                .get("output_tokens")
                .or_else(|| usage_val.get("output"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let cache_read = usage_val
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let cache_create = usage_val
                .get("cache_creation_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let total = inp + out + cache_read + cache_create;
            if inp > 0 || out > 0 || total > 0 {
                let usage = self.usage.get_or_insert_with(|| json!({}));
                if inp > 0 {
                    usage["inputTokens"] = json!(inp);
                }
                if out > 0 {
                    usage["outputTokens"] = json!(out);
                }
                if total > 0 {
                    usage["contextWindowUsedTokens"] = json!(total);
                }
            }
        }
        if let Some(token_usage) = event.get("tokenUsage").or_else(|| event.get("token_usage")) {
            let usage = self.usage.get_or_insert_with(|| json!({}));
            if let Some(cw) = token_usage
                .get("model_context_window")
                .or_else(|| token_usage.get("modelContextWindow"))
                .and_then(|v| v.as_u64())
            {
                if cw > 0 {
                    usage["contextWindowMaxTokens"] = json!(cw);
                }
            }
            if let Some(last) = token_usage.get("last") {
                if let Some(tot) = last
                    .get("total_tokens")
                    .or_else(|| last.get("totalTokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage["contextWindowUsedTokens"] = json!(tot);
                }
                if let Some(inp) = last
                    .get("input_tokens")
                    .or_else(|| last.get("inputTokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage["inputTokens"] = json!(inp);
                }
                if let Some(out) = last
                    .get("output_tokens")
                    .or_else(|| last.get("outputTokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage["outputTokens"] = json!(out);
                }
            }
        }
        live
    }
}

/// Start each invocation with the persisted thinking choice, rather than acknowledging
/// a control change without passing it to the provider.
fn provider_arguments(
    provider: &str,
    model: Option<&str>,
    thinking: Option<&str>,
    cwd: &Path,
    prompt: &str,
    handle: Option<&str>,
    session_dir: Option<&Path>,
) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = match provider {
        "omp" | "pi" => vec!["--mode".into(), "json".into(), "-p".into()],
        "opencode" => vec![
            "run".into(),
            "--auto".into(),
            "--format".into(),
            "json".into(),
            "--thinking".into(),
        ],
        "codex" if handle.is_some() => vec!["exec".into(), "resume".into(), "--json".into()],
        "codex" => vec![
            "exec".into(),
            "--json".into(),
            "-C".into(),
            cwd.to_string_lossy().into_owned(),
        ],
        "claude" => vec![
            "-p".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
            "--include-partial-messages".into(),
        ],
        _ => {
            return Err(format!(
                "Provider '{provider}' has no native safe session/resume adapter"
            ))
        }
    };
    if let Some(handle) = handle {
        if handle.trim().is_empty() || handle.starts_with('-') {
            return Err(
                "Provider session handle must be an explicit non-option session identity".into(),
            );
        }
        match provider {
            "claude" | "omp" => args.extend(["--resume".into(), handle.into()]),
            "opencode" | "pi" => args.extend(["--session".into(), handle.into()]),
            "codex" => {} // Positional identity follows options, before the prompt.
            _ => unreachable!(),
        }
    } else if matches!(provider, "omp" | "pi") {
        let directory = session_dir.ok_or("OMP/Pi require an isolated session directory")?;
        args.extend([
            "--session-dir".into(),
            directory.to_string_lossy().into_owned(),
        ]);
    }
    if let Some(model) = model.filter(|model| {
        !model.is_empty() && matches!(provider, "omp" | "pi" | "opencode" | "codex" | "claude")
    }) {
        args.extend([
            if provider == "opencode" || provider == "codex" {
                "-m"
            } else {
                "--model"
            }
            .into(),
            model.into(),
        ]);
    }
    if let Some(thinking) = thinking.filter(|thinking| !thinking.is_empty()) {
        match provider {
            "omp" | "pi" => args.extend(["--thinking".into(), thinking.into()]),
            "opencode" if matches!(thinking, "max" | "minimal" | "high") => {
                args.extend(["--variant".into(), thinking.into()])
            }
            "codex" => args.extend([
                "-c".into(),
                format!("model_reasoning_effort={}", json!(thinking)),
            ]),
            "claude" => args.extend(["--effort".into(), thinking.into()]),
            _ => {}
        }
    }
    if provider == "codex" {
        if let Some(handle) = handle {
            args.push(handle.into());
        }
    }
    args.push("--".into());
    args.push(prompt.to_string());
    Ok(args)
}

fn persist_output(
    manager: &crate::agent::manager::AgentManager,
    agent_id: &str,
    output: &ProviderOutput,
) -> Result<(), String> {
    // Preserve provider order rather than collapsing all reasoning ahead of all answers.
    for row in output.rows.iter().filter(|row| !row.text.trim().is_empty()) {
        match row.kind {
            OutputKind::Reasoning => manager.append_thinking(agent_id, &row.text)?,
            OutputKind::Assistant => manager.append_assistant_message(agent_id, &row.text)?,
        }
    }
    Ok(())
}

fn provider_handle(
    agent: &crate::agent::types::StoredAgentRecord,
) -> Result<Option<String>, String> {
    if let Some(runtime) = &agent.runtime_info {
        if runtime["provider"].as_str() != Some(agent.provider.as_str()) {
            return Err(
                "Saved provider session belongs to a different provider; select a fresh agent"
                    .into(),
            );
        }
        if let Some(handle) = runtime["providerHandleId"]
            .as_str()
            .filter(|handle| !handle.is_empty())
        {
            return Ok(Some(handle.to_owned()));
        }
        if runtime["continuityRequired"].as_bool() == Some(true) {
            return Err("Previous provider turn did not expose a durable session identity. Import its explicit session handle or create a new agent; automatic fresh-session continuation is disabled.".into());
        }
    }
    Ok(agent
        .config
        .as_ref()
        .and_then(|config| config["providerHandleId"].as_str())
        .filter(|handle| !handle.is_empty())
        .map(str::to_owned))
}

fn observed_provider_handle<'a>(provider: &str, event: &'a Value) -> Option<&'a str> {
    let kind = event["type"].as_str()?;
    let handle = match provider {
        "codex" if kind == "thread.started" => event["thread_id"].as_str(),
        "claude"
            if event["parent_tool_use_id"].is_null()
                && (kind == "result" || (kind == "system" && event["subtype"] == "init")) =>
        {
            event["session_id"].as_str()
        }
        "opencode"
            if matches!(
                kind,
                "session"
                    | "session_start"
                    | "message"
                    | "step_start"
                    | "step_finish"
                    | "text"
                    | "reasoning"
                    | "tool_use"
                    | "error"
            ) =>
        {
            event["sessionID"]
                .as_str()
                .or_else(|| event["sessionId"].as_str())
                .or_else(|| event["session_id"].as_str())
                .or_else(|| event["id"].as_str())
                .or_else(|| event["part"]["sessionID"].as_str())
                .or_else(|| event["part"]["sessionId"].as_str())
        }
        _ => None,
    }?;
    (!handle.is_empty()).then_some(handle)
}

fn ingest_provider_event(
    turn: &mut ReservedTurn,
    provider: &str,
    handle: &mut Option<String>,
    output: &mut ProviderOutput,
    event: &Value,
) -> Result<(), String> {
    if let Some(observed) = observed_provider_handle(provider, event) {
        if handle.as_deref().is_some_and(|saved| saved != observed) {
            eprintln!(
                "[Runner] Provider {provider} session handle updated: was {:?}, now {}",
                handle.as_deref(),
                observed
            );
            let _ = turn.persist_provider_handle(observed);
            *handle = Some(observed.to_owned());
        } else if handle.is_none() {
            turn.persist_provider_handle(observed)?;
            *handle = Some(observed.to_owned());
        }
    }
    for item in output.ingest(provider, event) {
        let _ = get_provider_push_tx()
            .send(stream_message(&turn.agent.id, provider, &output.identity, &item).to_string());
    }
    Ok(())
}

/// The directory is private to this agent. Never search a user's global latest session.
fn isolated_session_file(directory: &Path) -> Result<Option<String>, String> {
    let mut found = None;
    for entry in std::fs::read_dir(directory)
        .map_err(|e| format!("Cannot inspect isolated provider sessions: {e}"))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl")
            || !path.is_file()
        {
            continue;
        }
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut reader = std::io::BufReader::new(file);
        // OMP may write a title record before its session header. Bound all header reads.
        let mut bytes = Vec::new();
        use std::io::Read;
        reader
            .by_ref()
            .take(32768)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if !bytes.split(|byte| *byte == b'\n').take(8).any(|line| {
            serde_json::from_slice::<Value>(line)
                .ok()
                .is_some_and(|entry| {
                    entry["type"] == "session"
                        && entry["id"].as_str().is_some_and(|id| !id.is_empty())
                })
        }) {
            continue;
        }
        if found.is_some() {
            return Err("Multiple root sessions appeared in the isolated provider directory; refusing ambiguous continuation".into());
        }
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(|e| format!("Cannot resolve provider session path: {e}"))?
                .join(path)
        };
        found = Some(absolute.to_string_lossy().into_owned());
    }
    Ok(found)
}

pub async fn run_agent_turn(turn: ReservedTurn) {
    run_reserved_turn(turn, None).await;
}

// Command injection is restricted to module tests; the lifecycle always uses the supplied manager.
async fn run_reserved_turn(mut turn: ReservedTurn, command: Option<Command>) {
    let agent_id = turn.agent.id.clone();
    let provider = turn.agent.provider.clone();
    let manager = turn.manager().clone();
    broadcast_agent_status(&manager, &agent_id);
    broadcast_timeline_replacement(&agent_id);
    let turn_id = turn.token.clone();
    let _ = get_provider_push_tx().send(
        json!({
            "type": "session", "message": {"type": "agent_stream", "payload": {
                "agentId": agent_id, "timestamp": crate::agent::types::now_iso(),
                "event": {"type": "turn_started", "provider": provider, "turnId": turn_id}
            }}
        })
        .to_string(),
    );
    let result = execute_provider_turn(&mut turn, command).await;
    let (mut error, cleanup_confirmed, output_usage) = match result {
        Ok(ref output) => {
            let mut err = persist_output(&manager, &agent_id, output)
                .err()
                .or_else(|| output.error.clone());
            if output.has_assistant_message() {
                if let Some(ref e) = err {
                    if e.contains("Cancelled a request for input")
                        || e.contains("this run cannot answer")
                    {
                        err = None;
                    }
                }
            }
            (err, output.cleanup_confirmed, output.usage.clone())
        }
        Err(error) => (Some(error), true, None),
    };
    if let Some(error) = &error {
        if let Err(persist) = manager.append_error_notice(&agent_id, error) {
            eprintln!("[Runner] Failed to persist turn error: {persist}");
        }
    }
    if cleanup_confirmed {
        if let Err(completion) = turn.finish(error.as_deref()) {
            error = Some(completion);
        }
    } else {
        let message = error
            .as_deref()
            .unwrap_or("Provider cleanup could not be confirmed");
        turn.retain_cleanup_failure(message);
    }

    if let Some(ref usage) = output_usage {
        if let Some(mut agent) = manager.get_agent(&agent_id) {
            agent.last_usage = Some(usage.clone());
            let _ = manager.storage().save_record(agent);
        }
        let inp = usage
            .get("inputTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let out = usage
            .get("outputTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cost = usage
            .get("totalCostUsd")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        get_plugin_manager().record_tokens(&provider, inp, out, cost);
    }

    let terminal_event = if turn.is_cancelled() && cleanup_confirmed {
        json!({"type": "turn_canceled", "provider": provider, "turnId": turn_id, "reason": "Cancellation requested"})
    } else if let Some(error) = error {
        json!({"type": "turn_failed", "provider": provider, "turnId": turn_id, "error": error})
    } else {
        let mut event = json!({"type": "turn_completed", "provider": provider, "turnId": turn_id});
        if let Some(ref usage) = output_usage {
            event["usage"] = usage.clone();
        }
        event
    };
    let _ = get_provider_push_tx().send(json!({
        "type": "session", "message": {"type": "agent_stream", "payload": {
            "agentId": agent_id, "timestamp": crate::agent::types::now_iso(), "event": terminal_event
        }}
    }).to_string());
    broadcast_agent_status(&manager, &agent_id);
    broadcast_timeline_replacement(&agent_id);
}

async fn cleanup_failed_provider_start(
    turn: &mut ReservedTurn,
    tree: &mut turn_process::ProcessTree,
    child: &mut tokio::process::Child,
    prior: &crate::agent::types::StoredAgentRecord,
    error: String,
) -> ProviderOutput {
    let mut output = ProviderOutput::default();
    output.error = Some(error);
    if let Err(cleanup) = tree.cleanup(child).await {
        output.cleanup_confirmed = false;
        output.error = Some(format!(
            "{}; {cleanup}; agent remains reserved",
            output.error.as_deref().unwrap_or("Provider setup failed")
        ));
    } else {
        #[cfg(windows)]
        if tree.was_suspended() {
            if let Err(restore) = turn.restore_unstarted_provider(prior) {
                output.error = Some(format!(
                    "{}; {restore}",
                    output.error.as_deref().unwrap_or("Provider setup failed")
                ));
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (turn, prior);
    output
}

async fn execute_local_http_provider_turn(
    turn: &mut ReservedTurn,
    provider: &str,
    model: Option<&str>,
    prompt: &str,
    system_prompt: Option<&str>,
) -> Result<ProviderOutput, String> {
    use futures_util::StreamExt;
    let mut output = ProviderOutput::default();
    output.identity = turn.token.clone();
    output.cleanup_confirmed = true;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let clean_model = model
        .map(|m| {
            m.strip_prefix("ollama/")
                .or_else(|| m.strip_prefix("lmstudio/"))
                .unwrap_or(m)
        })
        .unwrap_or("llama3");

    let mut messages = Vec::new();
    if let Some(sys) = system_prompt.filter(|s| !s.trim().is_empty()) {
        messages.push(serde_json::json!({
            "role": "system",
            "content": sys
        }));
    }
    messages.push(serde_json::json!({
        "role": "user",
        "content": prompt
    }));

    if provider == "ollama" {
        let endpoint =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
        let url = format!("{endpoint}/api/chat");
        let body = serde_json::json!({
            "model": clean_model,
            "messages": messages,
            "stream": true
        });

        let resp = client.post(&url).json(&body).send().await.map_err(|e| {
            format!("Failed to connect to Ollama at {url}: {e}. Ensure Ollama is running.")
        })?;

        if !resp.status().is_success() {
            return Err(format!(
                "Ollama returned HTTP error status {}",
                resp.status()
            ));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            if turn.is_cancelled() {
                break;
            }
            let bytes = chunk_result.map_err(|e| format!("Error streaming from Ollama: {e}"))?;
            let text = String::from_utf8_lossy(&bytes);
            buffer.push_str(&text);

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer.drain(..=newline_pos);
                if line.is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                    if val.get("done").and_then(|v| v.as_bool()) == Some(true) {
                        let inp = val.get("prompt_eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        let out = val.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
                        output.usage = Some(serde_json::json!({
                            "inputTokens": inp,
                            "outputTokens": out,
                            "totalCostUsd": 0.0
                        }));
                    }
                    if let Some(msg) = val.get("message") {
                        if let Some(thinking) = msg.get("thinking").and_then(|v| v.as_str()) {
                            let mut live = Vec::new();
                            output.update(
                                "local_thinking".to_string(),
                                OutputKind::Reasoning,
                                thinking,
                                false,
                                &mut live,
                            );
                            for item in live {
                                let _ = get_provider_push_tx().send(
                                    stream_message(
                                        &turn.agent.id,
                                        provider,
                                        &output.identity,
                                        &item,
                                    )
                                    .to_string(),
                                );
                            }
                        }
                        if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                            let mut live = Vec::new();
                            output.update(
                                "local_answer".to_string(),
                                OutputKind::Assistant,
                                content,
                                false,
                                &mut live,
                            );
                            for item in live {
                                let _ = get_provider_push_tx().send(
                                    stream_message(
                                        &turn.agent.id,
                                        provider,
                                        &output.identity,
                                        &item,
                                    )
                                    .to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
    } else if provider == "lmstudio" {
        let endpoint =
            std::env::var("LMSTUDIO_HOST").unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());
        let url = format!("{endpoint}/v1/chat/completions");
        let body = serde_json::json!({
            "model": clean_model,
            "messages": messages,
            "stream": true
        });

        let resp = client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to connect to LM Studio at {url}: {e}. Ensure LM Studio local server is started."))?;

        if !resp.status().is_success() {
            return Err(format!(
                "LM Studio returned HTTP error status {}",
                resp.status()
            ));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            if turn.is_cancelled() {
                break;
            }
            let bytes = chunk_result.map_err(|e| format!("Error streaming from LM Studio: {e}"))?;
            let text = String::from_utf8_lossy(&bytes);
            buffer.push_str(&text);

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer.drain(..=newline_pos);
                if line.is_empty() || line.starts_with(':') {
                    continue;
                }
                let data = line.strip_prefix("data: ").unwrap_or(&line).trim();
                if data == "[DONE]" {
                    break;
                }
                if let Ok(val) = serde_json::from_str::<Value>(data) {
                    if let Some(usage) = val.get("usage") {
                        let inp = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let out = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        output.usage = Some(serde_json::json!({
                            "inputTokens": inp,
                            "outputTokens": out,
                            "totalCostUsd": 0.0
                        }));
                    }
                    if let Some(choices) = val.get("choices").and_then(|c| c.as_array()) {
                        for choice in choices {
                            if let Some(content) =
                                choice.pointer("/delta/content").and_then(|c| c.as_str())
                            {
                                let mut live = Vec::new();
                                output.update(
                                    "local_answer".to_string(),
                                    OutputKind::Assistant,
                                    content,
                                    false,
                                    &mut live,
                                );
                                for item in live {
                                    let _ = get_provider_push_tx().send(
                                        stream_message(
                                            &turn.agent.id,
                                            provider,
                                            &output.identity,
                                            &item,
                                        )
                                        .to_string(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(output)
}

async fn execute_provider_turn(
    turn: &mut ReservedTurn,
    command: Option<Command>,
) -> Result<ProviderOutput, String> {
    if turn.is_cancelled() {
        return Ok(ProviderOutput::default());
    }
    let agent = turn.agent.clone();
    let provider = agent.provider.as_str();
    let model = agent
        .config
        .as_ref()
        .and_then(|config| config["model"].as_str());
    let thinking = agent
        .config
        .as_ref()
        .and_then(|config| config["thinkingOptionId"].as_str());
    let cwd = if agent.cwd.is_empty() || agent.cwd == "." {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(&agent.cwd)
    };

    // Automatic shadow git checkpoint before turn execution
    let _ = crate::kernel::git::GitEngine::create_checkpoint(&cwd, &turn.token).await;

    // Automatic Project Rules and High-Altitude Repo Map auto-ingestion
    let is_initial_turn = turn
        .manager()
        .storage()
        .with_timeline(&agent.id, |items| {
            !items.iter().any(|item| {
                matches!(
                    item,
                    crate::agent::types::AgentTimelineItem::AssistantMessage { .. }
                )
            })
        })
        .unwrap_or(true);

    let mut context_prefix = String::new();
    let persona_id = agent
        .config
        .as_ref()
        .and_then(|config| config["modeId"].as_str().or_else(|| config["persona"].as_str()));
    if let Some(pid) = persona_id {
        if let Some(persona_prompt) = crate::agent::personas::get_persona_instructions(pid) {
            context_prefix.push_str(persona_prompt);
            context_prefix.push_str("\n\n");
        }
    }
    if let Some(rules) = crate::agent::rules::load_workspace_rules(&cwd) {
        context_prefix.push_str(&rules);
        context_prefix.push_str("\n\n");
    }
    if is_initial_turn {
        if let Some(map) = crate::kernel::repo_map::generate_repo_map(&cwd) {
            context_prefix.push_str(&map);
            context_prefix.push_str("\n\n");
        }
    }
    let system_context = (!context_prefix.is_empty()).then(|| context_prefix.trim().to_string());

    if provider == "ollama" || provider == "lmstudio" {
        let _ = turn.mark_provider_started();
        let prompt = turn.prompt.clone();
        return execute_local_http_provider_turn(turn, provider, model, &prompt, system_context.as_deref()).await;
    }
    let mut handle = provider_handle(&agent)?;
    if handle.is_none() && agent.runtime_info.is_none() {
        let prior_conversation = turn
            .manager()
            .storage()
            .with_timeline(&agent.id, |items| {
                items
                    .iter()
                    .filter(|item| {
                        matches!(
                            item,
                            crate::agent::types::AgentTimelineItem::UserMessage { .. }
                        )
                    })
                    .take(2)
                    .count()
                    > 1
                    || items.iter().any(|item| {
                        matches!(
                            item,
                            crate::agent::types::AgentTimelineItem::AssistantMessage { .. }
                                | crate::agent::types::AgentTimelineItem::Thinking { .. }
                        )
                    })
            })
            .map_err(|e| format!("Cannot check existing provider continuity: {e}"))?;
        if prior_conversation {
            return Err("Existing agent transcript has no durable provider session identity. Import its explicit session handle or create a new agent; refusing to continue in a fresh conversation.".into());
        }
    }
    let session_dir = turn
        .manager()
        .provider_session_dir(&agent.id)
        .join(provider)
        .join(&turn.token);
    if matches!(provider, "omp" | "pi") {
        if let Some(path) = &handle {
            if !Path::new(path).is_absolute() || !Path::new(path).is_file() {
                return Err(format!(
                    "{provider} resume requires an existing absolute session file: {path}"
                ));
            }
        } else {
            std::fs::create_dir_all(&session_dir)
                .map_err(|e| format!("Failed to create provider session directory: {e}"))?;
        }
    }
    let enriched_prompt = if let Some(ref sys) = system_context {
        format!("{sys}\n\n{}", turn.prompt)
    } else {
        turn.prompt.clone()
    };
    let args = provider_arguments(
        provider,
        model,
        thinking,
        &cwd,
        &enriched_prompt,
        handle.as_deref(),
        Some(&session_dir),
    )?;
    let mut cmd = match command {
        Some(mut command) => {
            command.args(args);
            command
        }
        None => {
            let bin = resolve_command(provider).ok_or_else(|| {
                format!("Provider '{provider}' is not installed or available on this system.")
            })?;
            let mut command = Command::new(bin);
            command.args(args);
            command
        }
    };
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let (mut tree, mut child) = turn_process::ProcessTree::spawn(&mut cmd)
        .map_err(|error| format!("{provider}: {error}"))?;
    if let Err(error) = turn
        .mark_provider_started()
        .and_then(|_| tree.attach(&child))
    {
        return Ok(
            cleanup_failed_provider_start(&mut *turn, &mut tree, &mut child, &agent, error).await,
        );
    }
    let stdout = child
        .stdout
        .take()
        .expect("Provider stdout configured as piped");
    let stderr = child
        .stderr
        .take()
        .expect("Provider stderr configured as piped");
    let stderr_task = tokio::spawn(drain_stderr(stderr));
    let mut output = ProviderOutput::default();
    output.identity = turn.token.clone();
    let mut reader = BufReader::new(stdout).lines();
    let deadline = tokio::time::sleep(Duration::from_secs(300));
    tokio::pin!(deadline);
    let mut session_poll = tokio::time::interval(Duration::from_millis(100));
    let mut exited = None;
    let mut stdout_closed = false;
    loop {
        // Cancellation wins ties with output/exit and remains authoritative through cleanup.
        tokio::select! {
            biased;
            _ = turn.cancellation.changed() => { break; }
            _ = &mut deadline => {
                output.error = Some(format!("{provider} turn timed out after 5 minutes."));
                break;
            }
            line = reader.next_line(), if !stdout_closed => match line {
                Ok(Some(line)) => {
                    if let Ok(event) = serde_json::from_str::<Value>(&line) {
                        if let Err(error) = ingest_provider_event(turn, provider, &mut handle, &mut output, &event) {
                            output.error = Some(error);
                            break;
                        }
                    }
                }
                Ok(None) => { stdout_closed = true; if exited.is_some() { break; } }
                Err(error) => { output.error = Some(format!("Failed reading {provider} output: {error}")); break; }
            },
            status = child.wait(), if exited.is_none() => {
                match status {
                    Ok(status) => {
                        exited = Some(status);
                        break;
                    }
                    Err(error) => { output.error = Some(format!("Failed waiting for {provider}: {error}")); break; }
                }
            }
            _ = session_poll.tick(), if handle.is_none() && matches!(provider, "omp" | "pi") => {
                match isolated_session_file(&session_dir) {
                    Ok(Some(path)) => {
                        if let Err(error) = turn.persist_provider_handle(&path) { output.error = Some(error); break; }
                        handle = Some(path);
                    }
                    Ok(None) => {}
                    Err(error) => { output.error = Some(error); break; }
                }
            }
        }
    }
    if let Err(error) = tree.cleanup(&mut child).await {
        output.cleanup_confirmed = false;
        output.error = Some(format!(
            "{error}; agent remains reserved because process-tree cleanup was not confirmed"
        ));
    }
    // Drain bytes already buffered before cancellation, but never wait on inherited pipe handles.
    if !stdout_closed {
        let drain = tokio::time::timeout(Duration::from_millis(1500), async {
            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(event) = serde_json::from_str::<Value>(&line) {
                    if let Err(error) =
                        ingest_provider_event(turn, provider, &mut handle, &mut output, &event)
                    {
                        output.error = Some(error);
                        break;
                    }
                }
            }
        })
        .await;
        if drain.is_err()
            && output.error.is_none()
            && !turn.is_cancelled()
            && output.rows.is_empty()
        {
            output.error = Some("Provider stdout did not close after process-tree cleanup".into());
        }
    }
    let stderr = finish_stderr(stderr_task).await;
    if let Some(status) = exited {
        if status.success() {
            if output.has_assistant_message() {
                if let Some(ref err) = output.error {
                    if err.contains("Cancelled a request for input")
                        || err.contains("this run cannot answer")
                    {
                        output.error = None;
                    }
                }
            }
        } else if output.error.is_none() && !turn.is_cancelled() {
            output.error = Some(format!(
                "{provider} exited with {status}: {}",
                String::from_utf8_lossy(&stderr).trim()
            ));
        }
    }
    if handle.is_none() && matches!(provider, "omp" | "pi") {
        match isolated_session_file(&session_dir) {
            Ok(Some(path)) => {
                if let Err(error) = turn.persist_provider_handle(&path) {
                    output.error = Some(error);
                } else {
                    handle = Some(path);
                }
            }
            Ok(None) => {}
            Err(error) => output.error = Some(error),
        }
    }
    if handle.is_none() && output.error.is_none() && !turn.is_cancelled() {
        output.error = Some(format!("{provider} did not expose a durable session identity; cannot safely continue this agent. Import an explicit provider session or create a new agent."));
    }
    Ok(output)
}

async fn drain_stderr(mut stderr: impl tokio::io::AsyncRead + Unpin) -> Vec<u8> {
    let mut retained = Vec::new();
    let mut buffer = [0u8; 4096];
    while let Ok(count) = stderr.read(&mut buffer).await {
        if count == 0 {
            break;
        }
        let keep = count.min(16384usize.saturating_sub(retained.len()));
        retained.extend_from_slice(&buffer[..keep]);
    }
    retained
}

async fn finish_stderr(mut task: tokio::task::JoinHandle<Vec<u8>>) -> Vec<u8> {
    match tokio::time::timeout(Duration::from_millis(500), &mut task).await {
        Ok(Ok(bytes)) => bytes,
        _ => {
            task.abort();
            let _ = task.await;
            Vec::new()
        }
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
