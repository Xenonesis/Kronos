//! Stateless single-turn LLM runner for Kronos IDE micro-actions:
//! - Inline Ctrl+K code editing
//! - Debounced ghost-text completion
//! - AI semantic commit message generation from real git diff
//! - Parallel Multi-Model Comparison Arena benchmarking

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;

use crate::agent::providers::lmstudio::LmStudioClient;
use crate::agent::providers::ollama::OllamaClient;
use crate::agent::registry::resolve_command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableModelOption {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickLlmResponse {
    pub provider: String,
    pub model: String,
    pub text: String,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tokens_per_second: f64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArenaCandidateInput {
    pub provider: String,
    pub model: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArenaCandidateResult {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub display_name: String,
    pub response: String,
    pub latency_ms: u64,
    pub tokens_per_second: f64,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub error: Option<String>,
}

/// Lists actually installed / running models across Ollama, LM Studio, and CLI providers.
pub async fn list_available_models() -> Vec<AvailableModelOption> {
    let mut options = Vec::new();

    let ollama = OllamaClient::default_local();
    if ollama.check_health().await {
        if let Ok(models) = ollama.list_models().await {
            for m in models.into_iter().take(6) {
                options.push(AvailableModelOption {
                    id: format!("ollama:{m}"),
                    provider: "ollama".to_string(),
                    model: m.clone(),
                    display_name: format!("Ollama ({m})"),
                });
            }
        }
    }

    let lmstudio = LmStudioClient::default_local();
    if lmstudio.check_health().await {
        if let Ok(models) = lmstudio.list_models().await {
            for m in models.into_iter().take(4) {
                options.push(AvailableModelOption {
                    id: format!("lmstudio:{m}"),
                    provider: "lmstudio".to_string(),
                    model: m.clone(),
                    display_name: format!("LM Studio ({m})"),
                });
            }
        }
    }

    for (provider, bin, default_model, label) in [
        ("claude", "claude", "claude-sonnet-4-5", "Claude Code CLI"),
        ("codex", "codex", "codex-default", "Codex CLI"),
        ("opencode", "opencode", "opencode-default", "OpenCode CLI"),
        ("pi", "pi", "pi-default", "Pi Coding CLI"),
        ("omp", "omp", "omp-default", "OMP CLI"),
    ] {
        if resolve_command(bin).is_some() {
            options.push(AvailableModelOption {
                id: format!("{provider}:{default_model}"),
                provider: provider.to_string(),
                model: default_model.to_string(),
                display_name: label.to_string(),
            });
        }
    }

    options
}

async fn pick_default_provider(
    preferred_provider: Option<&str>,
    preferred_model: Option<&str>,
) -> Result<(String, Option<String>), String> {
    if let Some(prov) = preferred_provider.filter(|s| !s.trim().is_empty()) {
        return Ok((prov.trim().to_string(), preferred_model.map(str::to_string)));
    }

    let ollama = OllamaClient::default_local();
    if ollama.check_health().await {
        let model = ollama
            .list_models()
            .await
            .ok()
            .and_then(|list| list.into_iter().next())
            .or_else(|| preferred_model.map(str::to_string));
        if model.is_some() {
            return Ok(("ollama".to_string(), model));
        }
    }

    let lmstudio = LmStudioClient::default_local();
    if lmstudio.check_health().await {
        let model = lmstudio
            .list_models()
            .await
            .ok()
            .and_then(|list| list.into_iter().next())
            .or_else(|| preferred_model.map(str::to_string));
        return Ok(("lmstudio".to_string(), model));
    }

    for cli in ["claude", "codex", "opencode", "pi", "omp"] {
        if resolve_command(cli).is_some() {
            return Ok((cli.to_string(), preferred_model.map(str::to_string)));
        }
    }

    Err("No running local LLM (Ollama / LM Studio) or CLI provider (claude / codex / opencode) was found.".to_string())
}

fn strip_code_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(after_open) = trimmed.strip_prefix("```") {
        let after_lang = after_open
            .split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or(after_open);
        if let Some(inner) = after_lang.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn estimate_tokens(text: &str) -> u64 {
    ((text.len() as f64) / 4.0).ceil().max(1.0) as u64
}

fn build_response(
    provider: &str,
    model: &str,
    text: String,
    started: Instant,
    input_tokens: u64,
    output_tokens: u64,
    cost_usd: f64,
) -> QuickLlmResponse {
    let latency_ms = started.elapsed().as_millis().max(1) as u64;
    let secs = (latency_ms as f64) / 1000.0;
    let tokens_per_second = if secs > 0.0 {
        ((output_tokens as f64) / secs * 10.0).round() / 10.0
    } else {
        0.0
    };
    QuickLlmResponse {
        provider: provider.to_string(),
        model: model.to_string(),
        text,
        latency_ms,
        input_tokens,
        output_tokens,
        tokens_per_second,
        cost_usd,
    }
}

async fn run_ollama(
    model: Option<&str>,
    system: &str,
    prompt: &str,
) -> Result<QuickLlmResponse, String> {
    let started = Instant::now();
    let endpoint =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let clean_model = model
        .map(|m| m.strip_prefix("ollama/").unwrap_or(m))
        .filter(|m| !m.is_empty())
        .unwrap_or("llama3")
        .to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed creating HTTP client: {e}"))?;

    let body = serde_json::json!({
        "model": clean_model,
        "stream": false,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt }
        ]
    });

    let resp = client
        .post(format!("{endpoint}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Ollama returned HTTP {}", resp.status()));
    }
    let val: Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid Ollama response: {e}"))?;
    let text = val
        .pointer("/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let inp = val
        .get("prompt_eval_count")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| estimate_tokens(prompt));
    let out = val
        .get("eval_count")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| estimate_tokens(&text));
    Ok(build_response("ollama", &clean_model, text, started, inp, out, 0.0))
}

async fn run_lmstudio(
    model: Option<&str>,
    system: &str,
    prompt: &str,
) -> Result<QuickLlmResponse, String> {
    let started = Instant::now();
    let endpoint =
        std::env::var("LMSTUDIO_HOST").unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());
    let clean_model = model
        .map(|m| m.strip_prefix("lmstudio/").unwrap_or(m))
        .filter(|m| !m.is_empty())
        .unwrap_or("local-model")
        .to_string();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed creating HTTP client: {e}"))?;

    let body = serde_json::json!({
        "model": clean_model,
        "stream": false,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt }
        ]
    });

    let resp = client
        .post(format!("{endpoint}/v1/chat/completions"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("LM Studio request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("LM Studio returned HTTP {}", resp.status()));
    }
    let val: Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid LM Studio response: {e}"))?;
    let text = val
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let inp = val
        .pointer("/usage/prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| estimate_tokens(prompt));
    let out = val
        .pointer("/usage/completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| estimate_tokens(&text));
    Ok(build_response("lmstudio", &clean_model, text, started, inp, out, 0.0))
}

fn extract_cli_text_from_jsonl(provider: &str, raw_stdout: &str) -> (String, u64, u64, f64) {
    let mut chunks = Vec::new();
    let mut inp = 0u64;
    let mut out = 0u64;
    let mut cost = 0.0f64;

    for line in raw_stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let ev_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match provider {
            "claude" => {
                if ev_type == "result" {
                    if let Some(res) = val.get("result").and_then(|v| v.as_str()) {
                        chunks = vec![res.to_string()];
                    }
                    cost = val.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(cost);
                    inp = val
                        .pointer("/usage/input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(inp);
                    out = val
                        .pointer("/usage/output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(out);
                } else if ev_type == "assistant" {
                    if let Some(arr) = val.pointer("/message/content").and_then(|c| c.as_array()) {
                        for block in arr {
                            if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                                    chunks.push(t.to_string());
                                }
                            }
                        }
                    }
                }
            }
            "codex" => {
                if ev_type == "item.completed"
                    && val.pointer("/item/type").and_then(|v| v.as_str()) == Some("agent_message")
                {
                    if let Some(t) = val.pointer("/item/text").and_then(|v| v.as_str()) {
                        chunks.push(t.to_string());
                    }
                }
            }
            "opencode" if ev_type == "text" => {
                if let Some(t) = val.pointer("/part/text").and_then(|v| v.as_str()) {
                    chunks.push(t.to_string());
                }
            }
            "omp" | "pi" if ev_type == "message_end" => {
                if let Some(arr) = val.pointer("/message/content").and_then(|c| c.as_array()) {
                    for block in arr {
                        if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                            if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                                chunks.push(t.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let text = if chunks.is_empty() {
        raw_stdout.trim().to_string()
    } else {
        chunks.join("\n")
    };
    (text, inp, out, cost)
}

async fn run_cli_provider(
    provider: &str,
    model: Option<&str>,
    system: &str,
    prompt: &str,
    cwd: Option<&Path>,
) -> Result<QuickLlmResponse, String> {
    let started = Instant::now();
    let bin_path = resolve_command(provider)
        .ok_or_else(|| format!("Provider CLI '{provider}' not found on PATH"))?;
    let combined_prompt = if system.trim().is_empty() {
        prompt.to_string()
    } else {
        format!("{system}\n\n{prompt}")
    };

    let mut args: Vec<String> = match provider {
        "claude" => vec![
            "-p".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
        ],
        "codex" => vec!["exec".into(), "--json".into()],
        "opencode" => vec!["run".into(), "--auto".into(), "--format".into(), "json".into()],
        "omp" | "pi" => vec!["--mode".into(), "json".into(), "-p".into()],
        _ => return Err(format!("Unsupported quick LLM provider '{provider}'")),
    };

    if let Some(m) = model.filter(|m| !m.is_empty() && !m.ends_with("-default")) {
        let flag = if provider == "opencode" || provider == "codex" {
            "-m"
        } else {
            "--model"
        };
        args.push(flag.into());
        args.push(m.to_string());
    }
    args.push("--".into());
    args.push(combined_prompt.clone());

    let mut cmd = Command::new(&bin_path);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd.filter(|d| d.is_dir()) {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed spawning {provider}: {e}"))?;
    let output = tokio::time::timeout(Duration::from_secs(45), child.wait_with_output())
        .await
        .map_err(|_| format!("{provider} timed out after 45s"))?
        .map_err(|e| format!("Failed reading {provider} output: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() && stdout.trim().is_empty() {
        return Err(format!("{provider} exited with error: {}", stderr.trim()));
    }

    let (text, inp, out, cost) = extract_cli_text_from_jsonl(provider, &stdout);
    let final_inp = if inp > 0 { inp } else { estimate_tokens(&combined_prompt) };
    let final_out = if out > 0 { out } else { estimate_tokens(&text) };
    let used_model = model.unwrap_or(provider);
    Ok(build_response(
        provider,
        used_model,
        text,
        started,
        final_inp,
        final_out,
        cost,
    ))
}

/// Executes a single stateless prompt turn against a specified or auto-discovered provider.
pub async fn run_single_prompt(
    preferred_provider: Option<&str>,
    preferred_model: Option<&str>,
    system: &str,
    prompt: &str,
    cwd: Option<&Path>,
) -> Result<QuickLlmResponse, String> {
    let (provider, resolved_model) =
        pick_default_provider(preferred_provider, preferred_model).await?;
    match provider.as_str() {
        "ollama" => run_ollama(resolved_model.as_deref(), system, prompt).await,
        "lmstudio" => run_lmstudio(resolved_model.as_deref(), system, prompt).await,
        other => run_cli_provider(other, resolved_model.as_deref(), system, prompt, cwd).await,
    }
}

/// Executes inline Ctrl+K code editing with real LLM output.
pub async fn run_inline_code_edit(
    filename: &str,
    selected_code: &str,
    instruction: &str,
    cwd: Option<&Path>,
    provider: Option<&str>,
    model: Option<&str>,
) -> Result<QuickLlmResponse, String> {
    let system = "You are Kronos Inline Editor. Return ONLY the transformed replacement code without any markdown explanation or surrounding prose.";
    let prompt = format!(
        "File: {filename}\nInstruction: {instruction}\n\nSelected code:\n```\n{selected_code}\n```\n\nOutput only the updated code:"
    );
    let mut res = run_single_prompt(provider, model, system, &prompt, cwd).await?;
    res.text = strip_code_fences(&res.text);
    Ok(res)
}

/// Generates a short real AI ghost-text continuation at cursor.
pub async fn run_inline_completion(
    filename: &str,
    prefix: &str,
    provider: Option<&str>,
    model: Option<&str>,
) -> Result<String, String> {
    let tail: String = prefix
        .chars()
        .rev()
        .take(900)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let system = "You are a concise code autocomplete engine. Continue ONLY what immediately follows the cursor on the next 1-3 lines. No markdown fences or explanations.";
    let prompt = format!("File: {filename}\nCode before cursor:\n{tail}\n\nCompletion:");
    let res = run_single_prompt(provider, model, system, &prompt, None).await?;
    Ok(strip_code_fences(&res.text))
}

/// Drafts an AI Conventional Commit message from the live repository git diff.
pub async fn generate_ai_semantic_commit(
    cwd: &Path,
    provider: Option<&str>,
    model: Option<&str>,
) -> Result<String, String> {
    let staged = std::process::Command::new("git")
        .args(["diff", "--cached", "--unified=2"])
        .current_dir(cwd)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let diff = if staged.trim().is_empty() {
        std::process::Command::new("git")
            .args(["diff", "--unified=2"])
            .current_dir(cwd)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default()
    } else {
        staged
    };
    if diff.trim().is_empty() {
        return Err("No modified git diff found in workspace.".to_string());
    }
    let truncated: String = diff.chars().take(6000).collect();
    let system = "Write a concise Conventional Commit message (`type(scope): subject` plus 2-3 bullet points). Output ONLY the commit message text.";
    let prompt = format!("Git diff:\n{truncated}");
    let res = run_single_prompt(provider, model, system, &prompt, Some(cwd)).await?;
    Ok(strip_code_fences(&res.text))
}

/// Benchmarks two providers/models in parallel against the same coding prompt.
pub async fn run_model_arena_comparison(
    prompt: &str,
    cwd: Option<&Path>,
    candidates: Vec<ArenaCandidateInput>,
) -> Vec<ArenaCandidateResult> {
    let system = "You are competing in the Kronos Model Comparison Arena. Provide an accurate, concise solution.";
    let futs = candidates.into_iter().enumerate().map(|(idx, cand)| async move {
        let model_ref = cand.model.as_deref();
        let display = cand
            .display_name
            .unwrap_or_else(|| format!("{}:{}", cand.provider, model_ref.unwrap_or("default")));
        let id = format!("arena-cand-{idx}-{}", cand.provider);
        match run_single_prompt(Some(&cand.provider), model_ref, system, prompt, cwd).await {
            Ok(res) => ArenaCandidateResult {
                id,
                provider: res.provider,
                model: res.model,
                display_name: display,
                response: res.text,
                latency_ms: res.latency_ms,
                tokens_per_second: res.tokens_per_second,
                total_tokens: res.input_tokens + res.output_tokens,
                cost_usd: res.cost_usd,
                error: None,
            },
            Err(err) => ArenaCandidateResult {
                id,
                provider: cand.provider,
                model: cand.model.unwrap_or_else(|| "default".to_string()),
                display_name: display,
                response: String::new(),
                latency_ms: 0,
                tokens_per_second: 0.0,
                total_tokens: 0,
                cost_usd: 0.0,
                error: Some(err),
            },
        }
    });
    futures_util::future::join_all(futs).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusProposalInput {
    pub model_name: String,
    pub solution_text: String,
}

/// Uses an active LLM provider to synthesize multiple model proposals into a unified patch/plan.
pub async fn synthesize_consensus_patch(
    task_prompt: &str,
    proposals: &[ConsensusProposalInput],
    cwd: Option<&Path>,
) -> Result<QuickLlmResponse, String> {
    if proposals.is_empty() {
        return Err("At least one candidate proposal is required for consensus synthesis.".to_string());
    }
    let mut prompt = format!("Original Task:\n{task_prompt}\n\nCandidate Proposals:\n");
    for (idx, p) in proposals.iter().enumerate() {
        let capped: String = p.solution_text.chars().take(3500).collect();
        prompt.push_str(&format!(
            "\n--- Model {}: {} ---\n{}\n",
            idx + 1,
            p.model_name,
            capped
        ));
    }
    prompt.push_str(
        "\nSynthesize the highest-confidence unified engineering plan or patch:\n\
         1. Highlight key areas of agreement across models.\n\
         2. Resolve contradictions or edge cases rigorously.\n\
         3. Produce clear, actionable final instructions.",
    );
    let system = "You are a principal software architect synthesizing multi-model engineering proposals into one cohesive, accurate execution prompt.";
    run_single_prompt(None, None, system, &prompt, cwd).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCommandSynthesis {
    pub command: String,
    pub explanation: String,
    pub risk: String,
    pub provider: String,
    pub model: String,
}

fn parse_terminal_synthesis_response(raw: &str) -> (String, String) {
    let cleaned = strip_code_fences(raw);
    if let Ok(parsed) = serde_json::from_str::<Value>(&cleaned) {
        let cmd = parsed
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let explanation = parsed
            .get("explanation")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if !cmd.is_empty() {
            return (
                cmd,
                if explanation.is_empty() {
                    "Generated shell command".to_string()
                } else {
                    explanation
                },
            );
        }
    }
    let first_line = cleaned
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string();
    (first_line, "Generated shell command".to_string())
}

/// Synthesizes a shell command from natural language via the active quick-LLM provider
/// and classifies safety risk with native Rust guardrails.
pub async fn synthesize_terminal_command(
    prompt: &str,
    cwd: Option<&Path>,
    provider: Option<&str>,
    model: Option<&str>,
) -> Result<TerminalCommandSynthesis, String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }
    let os_name = std::env::consts::OS;
    let shell_name = if cfg!(windows) { "PowerShell" } else { "POSIX sh/bash" };
    let cwd_display = cwd
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    let system = format!(
        "You are Kronos Terminal Assistant running on {os_name} ({shell_name}). \
         Convert the user request into a single accurate CLI command. \
         Respond strictly as JSON with keys \"command\" (string) and \"explanation\" (concise 1-sentence string). \
         Do not wrap in prose."
    );
    let user_prompt = format!("Working directory: {cwd_display}\nRequest: {trimmed}");
    let res = run_single_prompt(provider, model, &system, &user_prompt, cwd).await?;
    let (command, explanation) = parse_terminal_synthesis_response(&res.text);
    if command.is_empty() {
        return Err("Provider returned an empty command".to_string());
    }
    let assessment = crate::agent::guardrails::evaluate_command_risk(&command);
    let risk = match assessment.risk_level {
        crate::agent::guardrails::CommandRiskLevel::Destructive => "destructive",
        crate::agent::guardrails::CommandRiskLevel::Caution => "caution",
        crate::agent::guardrails::CommandRiskLevel::Safe => "safe",
    }
    .to_string();
    Ok(TerminalCommandSynthesis {
        command,
        explanation,
        risk,
        provider: res.provider,
        model: res.model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_terminal_synthesis_response_json_and_fences() {
        let fenced = "```json\n{\"command\":\"git status -s\",\"explanation\":\"Show short status\"}\n```";
        let (cmd, expl) = parse_terminal_synthesis_response(fenced);
        assert_eq!(cmd, "git status -s");
        assert_eq!(expl, "Show short status");

        let plain = "ls -la";
        let (cmd2, expl2) = parse_terminal_synthesis_response(plain);
        assert_eq!(cmd2, "ls -la");
        assert_eq!(expl2, "Generated shell command");
    }
}
