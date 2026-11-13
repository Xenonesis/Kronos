//! Kronos Custom Agent Personas & Extended Modes (Roo Code / Cline Parity)
//!
//! Provides targeted role directives for specialized engineering modes:
//! - Architect (System specs, modular design, non-destructive planning)
//! - Code (Full autonomous development, feature implementation, refactoring)
//! - Ask (Read-only codebase explorer, code explanation, no file modifications)
//! - Test (Test-driven engineering, comprehensive unit tests, coverage verification)
//! - Debugger (Root cause analysis, stack trace isolation, fix verification)
//! - Security Auditor (OWASP, input sanitization, memory safety)
//! - Code Reviewer (Clean architecture, performance, test coverage)

pub fn get_persona_instructions(persona_id: &str) -> Option<&'static str> {
    match persona_id.trim().to_lowercase().as_str() {
        "architect" | "plan" => Some(
            "### 🏛️ Mode: System Architect\nFocus on high-level architecture, system specifications, API design, and modular planning. Outline step-by-step tasks clearly before any implementation. Refrain from modifying code files directly until architecture specs are complete.",
        ),
        "code" | "coder" => Some(
            "### 💻 Mode: Autonomous Software Engineer\nImplement features end-to-end, write clean idiomatic code, refactor gracefully, and verify changes directly across the codebase.",
        ),
        "ask" | "explain" => Some(
            "### ❓ Mode: Read-Only Codebase Explorer\nAnswer developer questions, explain complex architectural flows, trace dependencies, and locate symbols. Strictly do not write or modify code files or run mutating shell commands.",
        ),
        "test" | "tester" => Some(
            "### 🧪 Mode: Test-Driven Quality Engineer\nWrite comprehensive unit and integration tests, construct mock fixtures, identify boundary edge cases, and execute test suites to guarantee robust software quality.",
        ),
        "debugger" | "debug" => Some(
            "### 🐛 Mode: Senior Debugging Specialist\nDiagnose errors methodically from root causes. Inspect stack traces, write minimal reproducing test cases, and verify fixes without introducing regressions.",
        ),
        "security" | "auditor" => Some(
            "### 🛡️ Mode: Principal Security Auditor\nAudit all code for injection vulnerabilities, unauthorized access, sensitive data leakage, input validation, and boundary safety.",
        ),
        "reviewer" | "review" => Some(
            "### 🔍 Mode: Senior Code Reviewer\nReview changes for algorithmic performance, idiomatic clean code, error handling completeness, documentation, and maintainability.",
        ),
        _ => None,
    }
}

/// Builds a two-stage Architect/Coder or specialized persona system prompt block.
/// Wires `crate::agent::architect` (`ExecutionStage`, `ArchitectPlanSpec`,
/// `format_architect_system_prompt`, `format_coder_system_prompt`) into live agent execution.
pub fn build_stage_system_prompt(
    persona_id: &str,
    architect_plan: Option<&str>,
    plan_spec: Option<&crate::agent::architect::ArchitectPlanSpec>,
) -> Option<String> {
    let base = get_persona_instructions(persona_id)?;
    let normalized = persona_id.trim().to_lowercase();
    let stage = match normalized.as_str() {
        "architect" | "plan" => Some(crate::agent::architect::ExecutionStage::Architect),
        "code" | "coder" => Some(crate::agent::architect::ExecutionStage::Coder),
        _ => None,
    };

    match stage {
        Some(crate::agent::architect::ExecutionStage::Architect) => {
            let arch_block = crate::agent::architect::format_architect_system_prompt();
            Some(format!("{base}\n\n{arch_block}"))
        }
        Some(crate::agent::architect::ExecutionStage::Coder) => {
            if let Some(spec) = plan_spec {
                let formatted_spec = format!(
                    "Task: {}\nFiles: {}\nInvariants: {}\nSteps:\n{}",
                    spec.task_title,
                    spec.affected_files.join(", "),
                    spec.architectural_invariants.join("; "),
                    spec.implementation_steps.join("\n")
                );
                let coder_block =
                    crate::agent::architect::format_coder_system_prompt(&formatted_spec);
                Some(format!("{base}\n\n{coder_block}"))
            } else if let Some(plan) = architect_plan.filter(|p| !p.trim().is_empty()) {
                let coder_block = crate::agent::architect::format_coder_system_prompt(plan);
                Some(format!("{base}\n\n{coder_block}"))
            } else {
                Some(base.to_string())
            }
        }
        None => Some(base.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_instructions() {
        assert!(get_persona_instructions("architect").is_some());
        assert!(get_persona_instructions("CODE").is_some());
        assert!(get_persona_instructions("ask").is_some());
        assert!(get_persona_instructions("test").is_some());
        assert!(get_persona_instructions("DEBUGGER").is_some());
        assert!(get_persona_instructions("security").is_some());
        assert!(get_persona_instructions("reviewer").is_some());
        assert!(get_persona_instructions("unknown_persona").is_none());
    }
}
