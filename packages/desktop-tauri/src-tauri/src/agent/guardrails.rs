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

/// Evaluates command string against safety guardrail heuristics.
pub fn evaluate_command_risk(command: &str) -> CommandRiskAssessment {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return CommandRiskAssessment::safe();
    }

    let lower = trimmed.to_lowercase();

    // 1. Destructive File & Disk Operations
    if lower.contains("rm -rf")
        || lower.contains("rm -fr")
        || lower.contains("rm --recursive --force")
        || lower.contains("rm -r -f")
    {
        return CommandRiskAssessment::destructive(
            "Recursive force file deletion (rm -rf)",
            "rm -rf",
        );
    }

    if (lower.contains("del ") || lower.starts_with("del/"))
        && (lower.contains("/s") || lower.contains("/q"))
    {
        return CommandRiskAssessment::destructive(
            "Windows recursive quiet file deletion (del /s /q)",
            "del /s /q",
        );
    }

    if lower.contains("rmdir /s") || lower.contains("rd /s") {
        return CommandRiskAssessment::destructive(
            "Windows recursive folder destruction (rmdir /s)",
            "rmdir /s",
        );
    }

    if lower.contains("format ") && lower.contains(":") {
        return CommandRiskAssessment::destructive(
            "Disk formatting command",
            "format",
        );
    }

    if lower.contains("mkfs") || lower.contains("dd if=") {
        return CommandRiskAssessment::destructive(
            "Direct raw block device write or filesystem creation",
            "mkfs/dd",
        );
    }

    // 2. Destructive Git Operations
    if lower.contains("git reset --hard") {
        return CommandRiskAssessment::destructive(
            "Permanent uncommitted and committed work destruction (git reset --hard)",
            "git reset --hard",
        );
    }

    if lower.contains("git clean") && (lower.contains("-f") || lower.contains("-df") || lower.contains("-fd")) {
        return CommandRiskAssessment::destructive(
            "Untracked workspace file purge (git clean -fd)",
            "git clean -f",
        );
    }

    if lower.contains("git push") && (lower.contains("--force") || lower.contains("-f")) {
        return CommandRiskAssessment::destructive(
            "Remote git history rewrite (git push --force)",
            "git push --force",
        );
    }

    // 3. Database destructive actions
    if lower.contains("drop database") || lower.contains("drop table") || lower.contains("truncate table") {
        return CommandRiskAssessment::destructive(
            "Destructive SQL database or table drop/truncate",
            "drop/truncate table",
        );
    }

    // 4. Fork bombs & severe system disruption
    if lower.contains(":(){ :|:& };:") || lower.contains("forkbomb") {
        return CommandRiskAssessment::destructive(
            "Process fork bomb denial of service",
            "forkbomb",
        );
    }

    // 5. Caution Operations (Package publishing, ungraceful kill)
    if lower.contains("npm publish") || lower.contains("cargo publish") || lower.contains("gem push") {
        return CommandRiskAssessment::caution(
            "Public registry package publication",
            "package_publish",
        );
    }

    if lower.contains("kill -9") || lower.contains("taskkill /f") {
        return CommandRiskAssessment::caution(
            "Forceful process termination without state flushing",
            "force_kill",
        );
    }

    if lower.contains("git checkout .") || lower.contains("git restore .") {
        return CommandRiskAssessment::caution(
            "Reversion of all uncommitted working tree modifications",
            "git_checkout_all",
        );
    }

    CommandRiskAssessment::safe()
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
