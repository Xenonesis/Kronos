//! Kronos Autonomous Self-Healing Diagnostic Engine
//!
//! Provides post-turn verification probes (Rust `cargo check`, TypeScript `tsc`, Python syntax)
//! and formats automated repair turns to heal compiler and type errors before concluding agent execution.

use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticErrorSummary {
    pub system: String,
    pub error_count: usize,
    pub raw_output: String,
    pub primary_file: Option<String>,
}

impl DiagnosticErrorSummary {
    pub fn new(system: impl Into<String>, raw_output: impl Into<String>, error_count: usize) -> Self {
        let raw = raw_output.into();
        let primary_file = extract_primary_file(&raw);
        Self {
            system: system.into(),
            error_count,
            raw_output: raw,
            primary_file,
        }
    }

    /// Formats an actionable self-healing repair instruction prompt.
    pub fn to_repair_prompt(&self) -> String {
        format!(
            "[AUTONOMOUS SELF-HEALING DIAGNOSTIC]\n\
            The previous code changes introduced compilation/typecheck errors in the workspace.\n\n\
            Tool: {}\n\
            Errors Detected: {}\n\n\
            Diagnostic Output:\n\
            ```\n\
            {}\n\
            ```\n\n\
            Please immediately fix the root cause of these compiler/typecheck errors to restore a clean build.",
            self.system,
            self.error_count,
            self.raw_output.trim()
        )
    }
}

/// Attempts to parse the first file path mentioned in diagnostic output.
fn extract_primary_file(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        // Rust style: --> src/main.rs:12:5
        if let Some(pos) = trimmed.find("--> ") {
            let rest = &trimmed[pos + 4..];
            if let Some(colon) = rest.find(':') {
                return Some(rest[..colon].trim().to_string());
            }
        }
        // TS / GCC style: src/index.ts:14:1 - error TS2322
        if let Some(colon) = trimmed.find(':') {
            let candidate = &trimmed[..colon];
            if candidate.ends_with(".ts")
                || candidate.ends_with(".tsx")
                || candidate.ends_with(".rs")
                || candidate.ends_with(".py")
                || candidate.ends_with(".go")
            {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Runs a fast non-destructive diagnostic probe against the workspace.
/// Returns Some(DiagnosticErrorSummary) if compilation/typecheck failures exist.
pub fn run_workspace_diagnostic_probe(workspace_root: &Path) -> Option<DiagnosticErrorSummary> {
    if !workspace_root.is_dir() {
        return None;
    }

    // 1. Rust Cargo project check
    let cargo_toml = workspace_root.join("Cargo.toml");
    if cargo_toml.exists() {
        if let Ok(output) = Command::new("cargo")
            .arg("check")
            .arg("--quiet")
            .current_dir(workspace_root)
            .output()
        {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let count = stderr.matches("error[").count().max(1);
                // Truncate output to at most 3000 chars to avoid prompt bloat
                let snippet = if stderr.len() > 3000 {
                    format!("{}...\n[truncated]", &stderr[..3000])
                } else {
                    stderr.to_string()
                };
                return Some(DiagnosticErrorSummary::new("cargo check", snippet, count));
            }
        }
    }

    // 2. TypeScript project check (tsconfig.json)
    let tsconfig = workspace_root.join("tsconfig.json");
    if tsconfig.exists() {
        #[cfg(windows)]
        let cmd = "npx.cmd";
        #[cfg(not(windows))]
        let cmd = "npx";

        if let Ok(output) = Command::new(cmd)
            .arg("tsc")
            .arg("--noEmit")
            .current_dir(workspace_root)
            .output()
        {
            if !output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let full = format!("{}\n{}", stdout, stderr);
                let count = full.matches("error TS").count().max(1);
                let snippet = if full.len() > 3000 {
                    format!("{}...\n[truncated]", &full[..3000])
                } else {
                    full
                };
                return Some(DiagnosticErrorSummary::new("tsc --noEmit", snippet, count));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_primary_file_rust() {
        let rust_err = "error[E0425]: cannot find value `foo` in this scope\n  --> src/main.rs:42:13\n   |";
        assert_eq!(extract_primary_file(rust_err), Some("src/main.rs".to_string()));
    }

    #[test]
    fn test_extract_primary_file_ts() {
        let ts_err = "src/components/button.tsx:12:5 - error TS2322: Type 'string' is not assignable to type 'number'.";
        assert_eq!(extract_primary_file(ts_err), Some("src/components/button.tsx".to_string()));
    }

    #[test]
    fn test_repair_prompt_formatting() {
        let diag = DiagnosticErrorSummary::new("cargo check", "error[E0308]: mismatched types", 1);
        let prompt = diag.to_repair_prompt();
        assert!(prompt.contains("[AUTONOMOUS SELF-HEALING DIAGNOSTIC]"));
        assert!(prompt.contains("cargo check"));
        assert!(prompt.contains("mismatched types"));
    }
}
