//! Kronos Command Safety Guardrails
//!
//! Evaluates CLI commands and scripts against safety heuristics to protect
//! user filesystems, databases, and git histories from destructive AI actions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandRiskLevel {
    Safe,
    Caution,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandRiskAssessment {
    pub risk_level: CommandRiskLevel,
    pub reason: Option<String>,
    pub matched_pattern: Option<String>,
}

impl CommandRiskAssessment {
    pub fn safe() -> Self {
        Self {
            risk_level: CommandRiskLevel::Safe,
            reason: None,
            matched_pattern: None,
        }
    }

    pub fn caution(reason: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            risk_level: CommandRiskLevel::Caution,
            reason: Some(reason.into()),
            matched_pattern: Some(pattern.into()),
        }
    }

    pub fn destructive(reason: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            risk_level: CommandRiskLevel::Destructive,
            reason: Some(reason.into()),
            matched_pattern: Some(pattern.into()),
        }
    }

    pub fn is_destructive(&self) -> bool {
        self.risk_level == CommandRiskLevel::Destructive
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BudgetAssessment {
    pub is_exceeded: bool,
    pub current_tokens: u64,
    pub max_tokens: Option<u64>,
    pub current_cost_usd: f64,
    pub max_cost_usd: Option<f64>,
    pub message: Option<String>,
}

impl BudgetAssessment {
    pub fn within_budget(current_tokens: u64, current_cost_usd: f64) -> Self {
        Self {
            is_exceeded: false,
            current_tokens,
            max_tokens: None,
            current_cost_usd,
            max_cost_usd: None,
            message: None,
        }
    }

    pub fn exceeded(
        current_tokens: u64,
        max_tokens: Option<u64>,
        current_cost_usd: f64,
        max_cost_usd: Option<f64>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            is_exceeded: true,
            current_tokens,
            max_tokens,
            current_cost_usd,
            max_cost_usd,
            message: Some(message.into()),
        }
    }
}

/// Evaluates current session token & cost consumption against hard budget ceilings
pub fn evaluate_session_budget(
    current_tokens: u64,
    max_tokens: Option<u64>,
    current_cost_usd: f64,
    max_cost_usd: Option<f64>,
) -> BudgetAssessment {
    if let Some(max_t) = max_tokens {
        if max_t > 0 && current_tokens >= max_t {
            return BudgetAssessment::exceeded(
                current_tokens,
                Some(max_t),
                current_cost_usd,
                max_cost_usd,
                format!(
                    "Session token budget limit of {} tokens reached (accumulated: {} tokens).",
                    max_t, current_tokens
                ),
            );
        }
    }

    if let Some(max_c) = max_cost_usd {
        if max_c > 0.0 && current_cost_usd >= max_c {
            return BudgetAssessment::exceeded(
                current_tokens,
                max_tokens,
                current_cost_usd,
                Some(max_c),
                format!(
                    "Session cost budget limit of ${:.2} reached (accumulated: ${:.2}).",
                    max_c, current_cost_usd
                ),
            );
        }
    }

    BudgetAssessment::within_budget(current_tokens, current_cost_usd)
}

/// Splits a shell command line into pipeline stages (`;`, `|`, `&&`, `||`) while respecting
/// single and double quotes. Also returns whether each separator was a pipe (`|`).
fn split_shell_pipeline(command: &str) -> Vec<(String, bool)> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut preceded_by_pipe = false;
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            current.push(ch);
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            current.push(ch);
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            current.push(ch);
            continue;
        }

        if ch == ';' || ch == '\n' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                segments.push((trimmed, preceded_by_pipe));
            }
            current.clear();
            preceded_by_pipe = false;
        } else if ch == '&' && chars.peek() == Some(&'&') {
            chars.next();
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                segments.push((trimmed, preceded_by_pipe));
            }
            current.clear();
            preceded_by_pipe = false;
        } else if ch == '|' {
            if chars.peek() == Some(&'|') {
                chars.next();
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push((trimmed, preceded_by_pipe));
                }
                current.clear();
                preceded_by_pipe = false;
            } else {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push((trimmed, preceded_by_pipe));
                }
                current.clear();
                preceded_by_pipe = true;
            }
        } else {
            current.push(ch);
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        segments.push((trimmed, preceded_by_pipe));
    }
    segments
}

/// Tokenizes a single pipeline stage into unquoted lowercase tokens, skipping leading `sudo` or env assignments.
fn tokenize_shell_stage(stage: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;

    for ch in stage.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                cur.push(ch);
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if !cur.is_empty() {
                tokens.push(cur.to_lowercase());
                cur.clear();
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        tokens.push(cur.to_lowercase());
    }

    // Strip leading `sudo` or `KEY=VALUE` env overrides
    while let Some(first) = tokens.first() {
        if first == "sudo" || (first.contains('=') && !first.starts_with('-')) {
            tokens.remove(0);
        } else {
            break;
        }
    }
    tokens
}

/// Evaluates command string against a pure-Rust shell pipeline & flag risk analyzer.
pub fn evaluate_command_risk(command: &str) -> CommandRiskAssessment {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return CommandRiskAssessment::safe();
    }

    let lower = trimmed.to_lowercase();

    // 1. Fork bombs & severe system disruption
    if lower.contains(":(){ :|:& };:") || lower.contains("forkbomb") {
        return CommandRiskAssessment::destructive(
            "Process fork bomb denial of service",
            "forkbomb",
        );
    }

    let mut caution_result: Option<CommandRiskAssessment> = None;
    let stages = split_shell_pipeline(trimmed);

    for (stage_raw, is_piped_into) in &stages {
        let stage_lower = stage_raw.to_lowercase();
        let tokens = tokenize_shell_stage(stage_raw);
        let Some(first_raw) = tokens.first() else {
            continue;
        };
        let bin = first_raw
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(first_raw.as_str())
            .trim_end_matches(".exe");

        // Pipe-to-shell execution (`curl ... | sh`, `wget ... | bash`, `iwr ... | iex`)
        if *is_piped_into && matches!(bin, "sh" | "bash" | "zsh" | "pwsh" | "powershell" | "iex" | "invoke-expression") {
            return CommandRiskAssessment::destructive(
                "Piping stream directly into shell interpreter",
                "pipe_to_shell",
            );
        }

        // Ignore pure `echo` / `printf` unless command substitution `$(...)` or backticks are present
        if matches!(bin, "echo" | "printf" | "write-host" | "write-output")
            && !stage_raw.contains("$(")
            && !stage_raw.contains('`')
        {
            continue;
        }

        // 2. Destructive File & Disk Operations (`rm`, `Remove-Item`, `del`, `rmdir`, `format`, `mkfs`, `dd`)
        if bin == "rm" {
            let mut has_r = false;
            let mut has_f = false;
            for tok in tokens.iter().skip(1) {
                if tok == "--recursive" {
                    has_r = true;
                } else if tok == "--force" {
                    has_f = true;
                } else if tok.starts_with('-') && !tok.starts_with("--") {
                    if tok.contains('r') {
                        has_r = true;
                    }
                    if tok.contains('f') {
                        has_f = true;
                    }
                }
            }
            if has_r && has_f {
                return CommandRiskAssessment::destructive(
                    "Recursive force file deletion (rm -rf)",
                    "rm -rf",
                );
            }
        }

        if matches!(bin, "remove-item" | "ri" | "erase") {
            let has_recurse = tokens.iter().any(|t| t == "-recurse" || t == "-r");
            let has_force = tokens.iter().any(|t| t == "-force" || t == "-fo");
            if has_recurse && has_force {
                return CommandRiskAssessment::destructive(
                    "PowerShell recursive force deletion (Remove-Item -Recurse -Force)",
                    "Remove-Item -Recurse -Force",
                );
            }
        }

        if (bin == "del" || bin.starts_with("del/"))
            && (stage_lower.contains("/s") || stage_lower.contains("/q"))
        {
            return CommandRiskAssessment::destructive(
                "Windows recursive quiet file deletion (del /s /q)",
                "del /s /q",
            );
        }

        if matches!(bin, "rmdir" | "rd") && stage_lower.contains("/s") {
            return CommandRiskAssessment::destructive(
                "Windows recursive folder destruction (rmdir /s)",
                "rmdir /s",
            );
        }

        if bin == "format" && tokens.iter().skip(1).any(|t| t.contains(':')) {
            return CommandRiskAssessment::destructive("Disk formatting command", "format");
        }

        if bin.starts_with("mkfs") || (bin == "dd" && tokens.iter().any(|t| t.starts_with("if=") || t.starts_with("of=/dev/"))) {
            return CommandRiskAssessment::destructive(
                "Direct raw block device write or filesystem creation",
                "mkfs/dd",
            );
        }

        // 3. Destructive Git Operations
        if bin == "git" {
            let sub = tokens.get(1).map(|s| s.as_str()).unwrap_or("");
            if sub == "reset" && tokens.iter().any(|t| t == "--hard") {
                return CommandRiskAssessment::destructive(
                    "Permanent uncommitted and committed work destruction (git reset --hard)",
                    "git reset --hard",
                );
            }
            if sub == "clean" && tokens.iter().skip(2).any(|t| t.starts_with('-') && t.contains('f')) {
                return CommandRiskAssessment::destructive(
                    "Untracked workspace file purge (git clean -fd)",
                    "git clean -f",
                );
            }
            if sub == "push" && tokens.iter().skip(2).any(|t| t == "--force" || t == "-f" || t.starts_with("--force")) {
                return CommandRiskAssessment::destructive(
                    "Remote git history rewrite (git push --force)",
                    "git push --force",
                );
            }
            if matches!(sub, "checkout" | "restore") && tokens.iter().skip(2).any(|t| t == ".") {
                caution_result = Some(CommandRiskAssessment::caution(
                    "Reversion of all uncommitted working tree modifications",
                    "git_checkout_all",
                ));
            }
        }

        // 4. Database destructive SQL DDL actions
        if stage_lower.contains("drop database")
            || stage_lower.contains("drop table")
            || stage_lower.contains("truncate table")
        {
            return CommandRiskAssessment::destructive(
                "Destructive SQL database or table drop/truncate",
                "drop/truncate table",
            );
        }

        // 5. Caution Operations (Package publishing, ungraceful kill)
        if (matches!(bin, "npm" | "pnpm" | "yarn" | "cargo") && tokens.get(1).map(|s| s.as_str()) == Some("publish"))
            || (bin == "gem" && tokens.get(1).map(|s| s.as_str()) == Some("push"))
        {
            caution_result = Some(CommandRiskAssessment::caution(
                "Public registry package publication",
                "package_publish",
            ));
        }

        if (bin == "kill" && tokens.iter().any(|t| t == "-9" || t == "-kill" || t == "-sigkill"))
            || (bin == "taskkill" && tokens.iter().any(|t| t == "/f"))
        {
            caution_result = Some(CommandRiskAssessment::caution(
                "Forceful process termination without state flushing",
                "force_kill",
            ));
        }
    }

    caution_result.unwrap_or_else(CommandRiskAssessment::safe)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPolicy {
    pub execution_mode: String, // "guarded" | "yolo"
    pub auto_approve_read_files: bool,
    pub auto_approve_terminal_safe: bool,
    pub auto_approve_file_edits: bool,
    pub auto_approve_web_search: bool,
    pub auto_approve_mcp: bool,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            execution_mode: "guarded".to_string(),
            auto_approve_read_files: true,
            auto_approve_terminal_safe: true,
            auto_approve_file_edits: false,
            auto_approve_web_search: true,
            auto_approve_mcp: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetPolicy {
    pub max_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            max_tokens: None,
            max_cost_usd: None,
        }
    }
}

static GLOBAL_PERMISSION_POLICY: std::sync::OnceLock<std::sync::RwLock<PermissionPolicy>> =
    std::sync::OnceLock::new();
static PERMISSION_POLICY_CONFIGURED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static GLOBAL_BUDGET_POLICY: std::sync::OnceLock<std::sync::RwLock<BudgetPolicy>> =
    std::sync::OnceLock::new();
static AGENT_USAGE_ACCUMULATOR: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, (u64, f64)>>,
> = std::sync::OnceLock::new();

fn permission_policy_lock() -> &'static std::sync::RwLock<PermissionPolicy> {
    GLOBAL_PERMISSION_POLICY.get_or_init(|| {
        let persisted = crate::kernel::config_store::get_config_store()
            .get_custom_setting::<PermissionPolicy>("permissionPolicy");
        if let Some(policy) = persisted {
            PERMISSION_POLICY_CONFIGURED.store(true, std::sync::atomic::Ordering::Relaxed);
            std::sync::RwLock::new(policy)
        } else {
            std::sync::RwLock::new(PermissionPolicy::default())
        }
    })
}

fn budget_policy_lock() -> &'static std::sync::RwLock<BudgetPolicy> {
    GLOBAL_BUDGET_POLICY.get_or_init(|| {
        let persisted = crate::kernel::config_store::get_config_store()
            .get_custom_setting::<BudgetPolicy>("budgetPolicy")
            .unwrap_or_default();
        std::sync::RwLock::new(persisted)
    })
}

fn agent_usage_map() -> &'static std::sync::RwLock<std::collections::HashMap<String, (u64, f64)>> {
    AGENT_USAGE_ACCUMULATOR.get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()))
}

pub fn is_permission_policy_configured() -> bool {
    let _ = permission_policy_lock();
    PERMISSION_POLICY_CONFIGURED.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn get_permission_policy() -> PermissionPolicy {
    permission_policy_lock()
        .read()
        .map(|g| g.clone())
        .unwrap_or_default()
}

pub fn set_permission_policy(policy: PermissionPolicy) {
    let lock = permission_policy_lock();
    if let Ok(mut guard) = lock.write() {
        *guard = policy.clone();
    }
    PERMISSION_POLICY_CONFIGURED.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = crate::kernel::config_store::get_config_store()
        .set_custom_setting("permissionPolicy", &policy);
}

pub fn get_budget_policy() -> BudgetPolicy {
    budget_policy_lock()
        .read()
        .map(|g| g.clone())
        .unwrap_or_default()
}

pub fn set_budget_policy(policy: BudgetPolicy) {
    let lock = budget_policy_lock();
    if let Ok(mut guard) = lock.write() {
        *guard = policy.clone();
    }
    let _ = crate::kernel::config_store::get_config_store()
        .set_custom_setting("budgetPolicy", &policy);
}

pub fn record_agent_usage(agent_id: &str, tokens: u64, cost_usd: f64) {
    if let Ok(mut guard) = agent_usage_map().write() {
        let entry = guard.entry(agent_id.to_string()).or_insert((0, 0.0));
        entry.0 = entry.0.saturating_add(tokens);
        entry.1 += cost_usd.max(0.0);
    }
}

pub fn get_agent_usage_totals(agent_id: &str) -> (u64, f64) {
    agent_usage_map()
        .read()
        .ok()
        .and_then(|g| g.get(agent_id).copied())
        .unwrap_or((0, 0.0))
}

pub fn reset_agent_usage(agent_id: &str) {
    if let Ok(mut guard) = agent_usage_map().write() {
        guard.remove(agent_id);
    }
}

pub fn check_agent_budget(agent_id: &str) -> Result<(), String> {
    let policy = get_budget_policy();
    let (tokens, cost) = get_agent_usage_totals(agent_id);
    let assessment = evaluate_session_budget(tokens, policy.max_tokens, cost, policy.max_cost_usd);
    if assessment.is_exceeded {
        Err(assessment
            .message
            .unwrap_or_else(|| "Agent session budget limit exceeded.".to_string()))
    } else {
        Ok(())
    }
}

pub fn format_policy_prompt_block(policy: &PermissionPolicy) -> String {
    if policy.execution_mode == "yolo" {
        return "[EXECUTION POLICY: YOLO] User enabled autonomous YOLO mode; execute verified edits and commands directly without asking unnecessary confirmations.\n".to_string();
    }
    format!(
        "[EXECUTION POLICY: GUARDED] autoApproveReadFiles={}, autoApproveTerminalSafe={}, autoApproveFileEdits={}, autoApproveWebSearch={}, autoApproveMcp={}. Always ask before destructive shell commands or out-of-scope edits.\n",
        policy.auto_approve_read_files,
        policy.auto_approve_terminal_safe,
        policy.auto_approve_file_edits,
        policy.auto_approve_web_search,
        policy.auto_approve_mcp
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluates_safe_commands() {
        assert_eq!(evaluate_command_risk("cargo check").risk_level, CommandRiskLevel::Safe);
        assert_eq!(evaluate_command_risk("npm test").risk_level, CommandRiskLevel::Safe);
        assert_eq!(evaluate_command_risk("git status").risk_level, CommandRiskLevel::Safe);
        assert_eq!(evaluate_command_risk("dir /b").risk_level, CommandRiskLevel::Safe);
        assert_eq!(evaluate_command_risk("cat src/main.rs").risk_level, CommandRiskLevel::Safe);
    }

    #[test]
    fn test_evaluates_destructive_commands() {
        let rm_rf = evaluate_command_risk("rm -rf node_modules");
        assert_eq!(rm_rf.risk_level, CommandRiskLevel::Destructive);
        assert!(rm_rf.is_destructive());

        let del_sq = evaluate_command_risk("del /f /s /q C:\\temp\\*");
        assert_eq!(del_sq.risk_level, CommandRiskLevel::Destructive);

        let git_reset = evaluate_command_risk("git reset --hard HEAD~1");
        assert_eq!(git_reset.risk_level, CommandRiskLevel::Destructive);

        let git_push_f = evaluate_command_risk("git push origin main --force");
        assert_eq!(git_push_f.risk_level, CommandRiskLevel::Destructive);

        let drop_db = evaluate_command_risk("psql -c 'DROP DATABASE production;'");
        assert_eq!(drop_db.risk_level, CommandRiskLevel::Destructive);
    }

    #[test]
    fn test_evaluates_caution_commands() {
        let pub_npm = evaluate_command_risk("npm publish --access public");
        assert_eq!(pub_npm.risk_level, CommandRiskLevel::Caution);

        let kill = evaluate_command_risk("kill -9 1234");
        assert_eq!(kill.risk_level, CommandRiskLevel::Caution);

        let git_restore = evaluate_command_risk("git restore .");
        assert_eq!(git_restore.risk_level, CommandRiskLevel::Caution);
    }

    #[test]
    fn test_budget_evaluation() {
        // Within budget
        let ok = evaluate_session_budget(50_000, Some(100_000), 0.50, Some(2.00));
        assert!(!ok.is_exceeded);

        // Exceeds tokens
        let token_exceeded = evaluate_session_budget(120_000, Some(100_000), 0.50, Some(2.00));
        assert!(token_exceeded.is_exceeded);
        assert!(token_exceeded.message.unwrap().contains("token budget limit"));

        // Exceeds cost
        let cost_exceeded = evaluate_session_budget(50_000, Some(100_000), 2.50, Some(2.00));
        assert!(cost_exceeded.is_exceeded);
        assert!(cost_exceeded.message.unwrap().contains("cost budget limit"));
    }
}
