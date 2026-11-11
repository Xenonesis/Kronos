//! Kronos Two-Stage Architect vs Coder Execution Mode
//!
//! Separates high-level planning and architectural reasoning from rapid code execution:
//! - Stage 1 (Architect): High-reasoning model produces step-by-step diff plans and invariant specifications.
//! - Stage 2 (Coder): Fast execution model applies the concrete code modifications and verifies tests.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStage {
    Architect,
    Coder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectPlanSpec {
    pub task_title: String,
    pub affected_files: Vec<String>,
    pub architectural_invariants: Vec<String>,
    pub implementation_steps: Vec<String>,
}

/// Generates system prompt additions for Stage 1 (Architect)
pub fn format_architect_system_prompt() -> &'static str {
    r#"### ARCHITECT MODE: STAGE 1 (DESIGN & INVARIANT SPECIFICATION)
You are acting as the Chief Architect. Your sole responsibility in this turn is to analyze the user request, inspect the codebase structure, and output a rigorous implementation plan.
DO NOT write full source code implementations yet. Instead output:
1. Target Files: Exact file paths to create or modify.
2. Invariants: System invariants that must not be broken (type safety, backwards compatibility, styling standards).
3. Implementation Plan: Step-by-step instructions for the Stage 2 Coder model.
"#
}

/// Generates system prompt additions for Stage 2 (Coder)
pub fn format_coder_system_prompt(architect_plan: &str) -> String {
    format!(
        r#"### CODER MODE: STAGE 2 (EXECUTION & VERIFICATION)
You are acting as the Lead Implementation Engineer. Implement the exact architectural plan produced by Stage 1:
```markdown
{}
```
Follow every specification strictly. Run lint and typechecks upon completion to verify the build.
"#,
        architect_plan.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architect_prompts() {
        let arch = format_architect_system_prompt();
        assert!(arch.contains("ARCHITECT MODE"));

        let coder = format_coder_system_prompt("1. Update main.rs\n2. Run cargo test");
        assert!(coder.contains("CODER MODE"));
        assert!(coder.contains("Update main.rs"));
    }
}
