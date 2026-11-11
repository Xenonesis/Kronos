//! Kronos Custom Agent Personas & Extended Modes
//!
//! Provides targeted role directives for specialized engineering personas:
//! - Architect (System specs, modular design, non-destructive planning)
//! - Debugger (Root cause analysis, stack trace isolation, fix verification)
//! - Security Auditor (OWASP, input sanitization, memory safety)
//! - Code Reviewer (Clean architecture, performance, test coverage)

pub fn get_persona_instructions(persona_id: &str) -> Option<&'static str> {
    match persona_id.trim().to_lowercase().as_str() {
        "architect" | "plan" => Some(
            "### 🏛️ Role: System Architect\nFocus on high-level architecture, system specifications, API design, and modular planning. Outline step-by-step tasks clearly before any implementation.",
        ),
        "debugger" | "debug" => Some(
            "### 🐛 Role: Senior Debugging Specialist\nDiagnose errors methodically from root causes. Inspect stack traces, write minimal reproducing test cases, and verify fixes without introducing regressions.",
        ),
        "security" | "auditor" => Some(
            "### 🛡️ Role: Principal Security Auditor\nAudit all code for injection vulnerabilities, unauthorized access, sensitive data leakage, input validation, and boundary safety.",
        ),
        "reviewer" | "review" => Some(
            "### 🔍 Role: Senior Code Reviewer\nReview changes for algorithmic performance, idiomatic clean code, error handling completeness, documentation, and maintainability.",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_instructions() {
        assert!(get_persona_instructions("architect").is_some());
        assert!(get_persona_instructions("DEBUGGER").is_some());
        assert!(get_persona_instructions("security").is_some());
        assert!(get_persona_instructions("reviewer").is_some());
        assert!(get_persona_instructions("unknown_persona").is_none());
    }
}
