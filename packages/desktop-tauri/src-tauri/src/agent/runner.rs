use crate::agent::registry::resolve_command;
use crate::agent::types::AgentStatus;
use crate::kernel::server::{agent_record_to_snapshot, get_agent_manager, get_provider_push_tx};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Broadcasts a timeline replacement event to notify the frontend to refresh the agent's timeline.
pub fn broadcast_timeline_replacement(agent_id: &str) {
    let tx = get_provider_push_tx();
    let epoch = format!(
        "epoch-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let msg = serde_json::json!({
        "type": "session",
        "message": {
            "type": "agent.timeline.replacement",
            "payload": {
                "agentId": agent_id,
                "epoch": epoch
            }
        }
    });
    let _ = tx.send(msg.to_string());
}

/// Broadcasts an agent_update event with the latest snapshot of the agent.
pub fn broadcast_agent_status_update(agent_id: &str) {
    let mgr = get_agent_manager();
    if let Some(agent) = mgr.get_agent(agent_id) {
        let snapshot = agent_record_to_snapshot(&agent);
        let tx = get_provider_push_tx();
        let msg = serde_json::json!({
            "type": "session",
            "message": {
                "type": "agent_update",
                "payload": {
                    "kind": "upsert",
                    "agent": snapshot
                }
            }
        });
        let _ = tx.send(msg.to_string());
    }
}

/// Broadcasts a live agent_stream event.
pub fn broadcast_agent_stream_event(agent_id: &str, provider: &str, item: &serde_json::Value) {
    let tx = get_provider_push_tx();
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let timestamp = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());

    let msg = serde_json::json!({
        "type": "session",
        "message": {
            "type": "agent_stream",
            "payload": {
                "agentId": agent_id,
                "timestamp": timestamp,
                "event": {
                    "type": "timeline",
                    "provider": provider,
                    "item": item
                }
            }
        }
    });
    let _ = tx.send(msg.to_string());
}

/// Asynchronously runs a turn for an agent given its ID and user prompt.
pub async fn run_agent_turn(agent_id: String, prompt: String) {
    let mgr = get_agent_manager();
    let agent_opt = mgr.get_agent(&agent_id);
    let agent = match agent_opt {
        Some(a) => a,
        None => {
            eprintln!("[Runner] Agent {agent_id} not found for turn execution");
            return;
        }
    };

    let provider = agent.provider.clone();
    let model = agent.config.as_ref().and_then(|c| {
        c.get("model")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
    });
    let cwd = if agent.cwd.is_empty() || agent.cwd == "." {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(&agent.cwd)
    };

    // Immediately notify UI that agent is running
    let _ = mgr.set_agent_status(&agent_id, AgentStatus::Running);
    broadcast_agent_status_update(&agent_id);
    broadcast_timeline_replacement(&agent_id);

    let result = execute_provider_turn(&agent_id, &provider, model.as_deref(), &cwd, &prompt).await;

    match result {
        Ok((text, thinking)) => {
            if let Some(thk) = thinking {
                if !thk.trim().is_empty() {
                    let _ = mgr.append_thinking(&agent_id, &thk);
                }
            }
            if !text.trim().is_empty() {
                let _ = mgr.append_assistant_message(&agent_id, &text);
            } else {
                let _ = mgr.append_assistant_message(&agent_id, "Done.");
            }
            let _ = mgr.set_agent_status(&agent_id, AgentStatus::Idle);
        }
        Err(err) => {
            let _ = mgr.append_error_notice(&agent_id, &err);
            let _ = mgr.set_agent_status(&agent_id, AgentStatus::Idle);
        }
    }

    // Notify UI that turn has completed
    broadcast_agent_status_update(&agent_id);
    broadcast_timeline_replacement(&agent_id);
}

/// Executes the turn against the specific provider CLI.
async fn execute_provider_turn(
    agent_id: &str,
    provider: &str,
    model: Option<&str>,
    cwd: &Path,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    match provider {
        "omp" => run_omp_turn(agent_id, model, cwd, prompt).await,
        "pi" => run_pi_turn(agent_id, model, cwd, prompt).await,
        "codex" => run_codex_turn(agent_id, model, cwd, prompt).await,
        "opencode" => run_opencode_turn(agent_id, model, cwd, prompt).await,
        "claude" => run_claude_turn(agent_id, model, cwd, prompt).await,
        other => {
            // Check if binary named `other` exists, else return error
            if let Some(_) = resolve_command(other) {
                run_generic_turn(agent_id, other, cwd, prompt).await
            } else {
                Err(format!(
                    "Provider '{}' is not installed or available on this system. Please install '{}' or switch to an available provider (Codex, OpenCode, Pi, OMP).",
                    other, other
                ))
            }
        }
    }
}

/// Run turn with OMP (`omp --mode json -p "<prompt>"`)
async fn run_omp_turn(
    agent_id: &str,
    model: Option<&str>,
    cwd: &Path,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let bin = resolve_command("omp").ok_or_else(|| {
        "OMP executable not found on system PATH. Please ensure 'omp' is installed.".to_string()
    })?;

    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    cmd.arg("--mode").arg("json");
    cmd.arg("-p").arg(prompt);
    if let Some(m) = model {
        if !m.is_empty() {
            cmd.arg("--model").arg(m);
        }
    }
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn omp: {e}"))?;
    let stdout = child.stdout.take().ok_or("Failed to capture omp stdout")?;
    let mut reader = BufReader::new(stdout).lines();

    let mut accumulated_text = String::new();
    let mut accumulated_thinking = String::new();
    let mut last_error: Option<String> = None;

    // Timeout: 300 seconds (5 minutes)
    let timeout = Duration::from_secs(300);
    let start = tokio::time::Instant::now();

    while let Ok(Some(line)) = reader.next_line().await {
        if start.elapsed() > timeout {
            let _ = child.kill().await;
            return Err("OMP turn timed out after 5 minutes.".to_string());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
                match msg_type {
                    "message_update" => {
                        if let Some(evt) = val.get("assistantMessageEvent") {
                            let evt_type = evt.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if evt_type == "text_delta" {
                                if let Some(delta) = evt.get("delta").and_then(|v| v.as_str()) {
                                    accumulated_text.push_str(delta);
                                    let stream_item = serde_json::json!({
                                        "type": "assistant_message",
                                        "text": accumulated_text
                                    });
                                    broadcast_agent_stream_event(agent_id, "omp", &stream_item);
                                }
                            } else if evt_type == "thinking_delta" {
                                if let Some(delta) = evt.get("delta").and_then(|v| v.as_str()) {
                                    accumulated_thinking.push_str(delta);
                                    let stream_item = serde_json::json!({
                                        "type": "reasoning",
                                        "text": accumulated_thinking
                                    });
                                    broadcast_agent_stream_event(agent_id, "omp", &stream_item);
                                }
                            }
                        }
                    }
                    "message_end" => {
                        if let Some(msg) = val.get("message") {
                            if msg.get("role").and_then(|v| v.as_str()) == Some("assistant") {
                                if let Some(contents) = msg.get("content").and_then(|v| v.as_array()) {
                                    for item in contents {
                                        if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                                            if let Some(txt) = item.get("text").and_then(|v| v.as_str()) {
                                                if accumulated_text.is_empty() {
                                                    accumulated_text = txt.to_string();
                                                }
                                            }
                                        } else if item.get("type").and_then(|v| v.as_str()) == Some("thinking") {
                                            if let Some(thk) = item.get("thinking").and_then(|v| v.as_str()) {
                                                if accumulated_thinking.is_empty() {
                                                    accumulated_thinking = thk.to_string();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "error" => {
                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                            last_error = Some(msg.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = child.wait().await;

    if accumulated_text.is_empty() && last_error.is_some() {
        return Err(last_error.unwrap());
    }

    let thinking_opt = if accumulated_thinking.is_empty() {
        None
    } else {
        Some(accumulated_thinking)
    };

    Ok((accumulated_text, thinking_opt))
}

/// Run turn with Pi (`pi --mode json -p "<prompt>"`)
async fn run_pi_turn(
    agent_id: &str,
    model: Option<&str>,
    cwd: &Path,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let bin = resolve_command("pi").ok_or_else(|| {
        "Pi executable not found on system PATH. Please ensure 'pi' is installed.".to_string()
    })?;

    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    cmd.arg("--mode").arg("json");
    cmd.arg("-p");
    if let Some(m) = model {
        if !m.is_empty() {
            cmd.arg("--model").arg(m);
        }
    }
    cmd.arg(prompt);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn pi: {e}"))?;
    let stdout = child.stdout.take().ok_or("Failed to capture pi stdout")?;
    let mut reader = BufReader::new(stdout).lines();

    let mut accumulated_text = String::new();
    let mut accumulated_thinking = String::new();
    let mut last_error: Option<String> = None;

    let timeout = Duration::from_secs(300);
    let start = tokio::time::Instant::now();

    while let Ok(Some(line)) = reader.next_line().await {
        if start.elapsed() > timeout {
            let _ = child.kill().await;
            return Err("Pi turn timed out after 5 minutes.".to_string());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
                match msg_type {
                    "message_update" => {
                        if let Some(evt) = val.get("assistantMessageEvent") {
                            let evt_type = evt.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if evt_type == "text_delta" {
                                if let Some(delta) = evt.get("delta").and_then(|v| v.as_str()) {
                                    accumulated_text.push_str(delta);
                                    let stream_item = serde_json::json!({
                                        "type": "assistant_message",
                                        "text": accumulated_text
                                    });
                                    broadcast_agent_stream_event(agent_id, "pi", &stream_item);
                                }
                            } else if evt_type == "thinking_delta" {
                                if let Some(delta) = evt.get("delta").and_then(|v| v.as_str()) {
                                    accumulated_thinking.push_str(delta);
                                    let stream_item = serde_json::json!({
                                        "type": "reasoning",
                                        "text": accumulated_thinking
                                    });
                                    broadcast_agent_stream_event(agent_id, "pi", &stream_item);
                                }
                            }
                        }
                    }
                    "message_end" => {
                        if let Some(msg) = val.get("message") {
                            if msg.get("role").and_then(|v| v.as_str()) == Some("assistant") {
                                if let Some(contents) = msg.get("content").and_then(|v| v.as_array()) {
                                    for item in contents {
                                        if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                                            if let Some(txt) = item.get("text").and_then(|v| v.as_str()) {
                                                if accumulated_text.is_empty() {
                                                    accumulated_text = txt.to_string();
                                                }
                                            }
                                        } else if item.get("type").and_then(|v| v.as_str()) == Some("thinking") {
                                            if let Some(thk) = item.get("thinking").and_then(|v| v.as_str()) {
                                                if accumulated_thinking.is_empty() {
                                                    accumulated_thinking = thk.to_string();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "error" => {
                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                            last_error = Some(msg.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = child.wait().await;

    if accumulated_text.is_empty() && last_error.is_some() {
        return Err(last_error.unwrap());
    }

    let thinking_opt = if accumulated_thinking.is_empty() {
        None
    } else {
        Some(accumulated_thinking)
    };

    Ok((accumulated_text, thinking_opt))
}

/// Run turn with OpenCode (`opencode run --format json "<prompt>"`)
async fn run_opencode_turn(
    agent_id: &str,
    model: Option<&str>,
    cwd: &Path,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let bin = resolve_command("opencode").ok_or_else(|| {
        "OpenCode executable not found on system PATH. Please ensure 'opencode' is installed.".to_string()
    })?;

    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    cmd.arg("run").arg("--format").arg("json");
    if let Some(m) = model {
        if !m.is_empty() {
            cmd.arg("-m").arg(m);
        }
    }
    cmd.arg("--dir").arg(cwd);
    cmd.arg(prompt);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn opencode: {e}"))?;
    let stdout = child.stdout.take().ok_or("Failed to capture opencode stdout")?;
    let mut reader = BufReader::new(stdout).lines();

    let mut accumulated_text = String::new();
    let mut last_error: Option<String> = None;

    let timeout = Duration::from_secs(300);
    let start = tokio::time::Instant::now();

    while let Ok(Some(line)) = reader.next_line().await {
        if start.elapsed() > timeout {
            let _ = child.kill().await;
            return Err("OpenCode turn timed out after 5 minutes.".to_string());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
                match msg_type {
                    "text" => {
                        if let Some(part) = val.get("part") {
                            if let Some(txt) = part.get("text").and_then(|v| v.as_str()) {
                                accumulated_text.push_str(txt);
                                let stream_item = serde_json::json!({
                                    "type": "assistant_message",
                                    "text": accumulated_text
                                });
                                broadcast_agent_stream_event(agent_id, "opencode", &stream_item);
                            }
                        }
                    }
                    "error" => {
                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                            last_error = Some(msg.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = child.wait().await;

    if accumulated_text.is_empty() && last_error.is_some() {
        return Err(last_error.unwrap());
    }

    Ok((accumulated_text, None))
}

/// Run turn with Codex (`codex exec --json "<prompt>"`)
async fn run_codex_turn(
    agent_id: &str,
    model: Option<&str>,
    cwd: &Path,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let bin = resolve_command("codex").ok_or_else(|| {
        "Codex executable not found on system PATH. Please ensure OpenAI Codex is installed.".to_string()
    })?;

    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    cmd.arg("exec").arg("--json");
    if let Some(m) = model {
        if !m.is_empty() {
            cmd.arg("-m").arg(m);
        }
    }
    cmd.arg("-C").arg(cwd);
    cmd.arg(prompt);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn codex: {e}"))?;
    let stdout = child.stdout.take().ok_or("Failed to capture codex stdout")?;
    let mut reader = BufReader::new(stdout).lines();

    let mut accumulated_text = String::new();
    let mut last_error: Option<String> = None;

    let timeout = Duration::from_secs(300);
    let start = tokio::time::Instant::now();

    while let Ok(Some(line)) = reader.next_line().await {
        if start.elapsed() > timeout {
            let _ = child.kill().await;
            return Err("Codex turn timed out after 5 minutes.".to_string());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
                match msg_type {
                    "item.created" => {
                        if let Some(item) = val.get("item") {
                            if item.get("type").and_then(|v| v.as_str()) == Some("agent_message") {
                                if let Some(txt) = item.get("text").and_then(|v| v.as_str()) {
                                    accumulated_text = txt.to_string();
                                    let stream_item = serde_json::json!({
                                        "type": "assistant_message",
                                        "text": accumulated_text
                                    });
                                    broadcast_agent_stream_event(agent_id, "codex", &stream_item);
                                }
                            }
                        }
                    }
                    "error" => {
                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                            last_error = Some(msg.to_string());
                        }
                    }
                    "turn.failed" => {
                        if let Some(err) = val.get("error") {
                            if let Some(msg) = err.get("message").and_then(|v| v.as_str()) {
                                last_error = Some(msg.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _ = child.wait().await;

    if accumulated_text.is_empty() && last_error.is_some() {
        return Err(last_error.unwrap());
    }

    Ok((accumulated_text, None))
}

/// Run turn with Claude CLI (`claude -p "<prompt>"`)
async fn run_claude_turn(
    agent_id: &str,
    _model: Option<&str>,
    cwd: &Path,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let bin = resolve_command("claude").ok_or_else(|| {
        "Claude Code CLI not found on system PATH. Please ensure 'claude' is installed.".to_string()
    })?;

    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    cmd.arg("-p").arg(prompt);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().await.map_err(|e| format!("Failed to run claude: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Claude error: {err}"));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let stream_item = serde_json::json!({
        "type": "assistant_message",
        "text": text
    });
    broadcast_agent_stream_event(agent_id, "claude", &stream_item);

    Ok((text, None))
}

/// Generic CLI fallback
async fn run_generic_turn(
    _agent_id: &str,
    cmd_name: &str,
    cwd: &Path,
    prompt: &str,
) -> Result<(String, Option<String>), String> {
    let bin = resolve_command(cmd_name).ok_or_else(|| {
        format!("Command '{cmd_name}' not found on system PATH.")
    })?;

    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    cmd.arg(prompt);
    cmd.current_dir(cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().await.map_err(|e| format!("Failed to run {cmd_name}: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    Ok((text, None))
}
