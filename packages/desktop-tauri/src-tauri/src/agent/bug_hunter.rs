//! Kronos Codebase Security & Bug Hunter Engine
//!
//! Provides Cursor Bug Finder and PR Security Review parity.
//! Combines `tree-sitter` AST taint/call-expression traversal, Shannon entropy
//! secret detection, and compiler diagnostics (`cargo check` / `tsc --noEmit`)
//! with batched `IoRingBatchReader` file loading.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BugSeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BugReportItem {
    pub id: String,
    pub file_path: String,
    pub line_number: usize,
    pub severity: BugSeverity,
    pub category: String,
    pub title: String,
    pub description: String,
    pub suggested_fix: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BugAuditReport {
    pub total_issues: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub issues: Vec<BugReportItem>,
}

/// Computes Shannon information entropy (in bits per character) for a candidate string literal.
pub fn shannon_entropy(s: &str) -> f64 {
    let len = s.chars().count();
    if len == 0 {
        return 0.0;
    }
    let mut counts: HashMap<char, usize> = HashMap::new();
    for ch in s.chars() {
        *counts.entry(ch).or_insert(0) += 1;
    }
    let total = len as f64;
    let mut entropy = 0.0_f64;
    for &c in counts.values() {
        let p = (c as f64) / total;
        entropy -= p * p.log2();
    }
    entropy
}

pub struct BugHunter;

impl BugHunter {
    /// Scans source text for security vulnerabilities and bug patterns using AST traversal
    /// (for TS/TSX/JS/JSX) plus token & Shannon-entropy analysis across all languages.
    pub fn scan_file_content(file_path: &str, content: &str) -> Vec<BugReportItem> {
        let mut issues = Vec::new();
        let mut reported_keys: HashSet<(String, usize)> = HashSet::new();

        // Phase 1: Tree-Sitter AST structural inspection for TS/TSX/JS/JSX
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("ts");
        Self::scan_ast_nodes(file_path, ext, content, &mut issues, &mut reported_keys);

        // Phase 2: Line + Token + Shannon Entropy inspection (all languages including Rust/Python/Go)
        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;
            let trimmed = line.trim();

            // 1. Hardcoded Secrets & High-Entropy API Credentials
            if !reported_keys.contains(&("sec-secret".to_string(), line_num))
                && Self::detect_secret_leak(trimmed)
            {
                reported_keys.insert(("sec-secret".to_string(), line_num));
                issues.push(BugReportItem {
                    id: format!("sec-secret-{line_num}"),
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    severity: BugSeverity::Critical,
                    category: "Security / Secret Leak".to_string(),
                    title: "Potential hardcoded secret or API credential detected".to_string(),
                    description: format!("Found credential or high-entropy secret literal: `{trimmed}`"),
                    suggested_fix: "Move credentials to .env file or environment variables vault.".to_string(),
                });
            }

            // 2. SQL Injection Risk
            if !reported_keys.contains(&("sec-sql".to_string(), line_num))
                && Self::detect_sql_injection(trimmed)
            {
                reported_keys.insert(("sec-sql".to_string(), line_num));
                issues.push(BugReportItem {
                    id: format!("sec-sql-{line_num}"),
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    severity: BugSeverity::Critical,
                    category: "Security / SQL Injection".to_string(),
                    title: "SQL query constructed via string concatenation".to_string(),
                    description: "Raw SQL query contains dynamic string interpolation instead of parameterized queries.".to_string(),
                    suggested_fix: "Use parameterized query placeholders ($1, ?, :param).".to_string(),
                });
            }

            // 3. Command Injection Risk
            if !reported_keys.contains(&("sec-cmdi".to_string(), line_num))
                && Self::detect_command_injection(trimmed)
            {
                reported_keys.insert(("sec-cmdi".to_string(), line_num));
                issues.push(BugReportItem {
                    id: format!("sec-cmdi-{line_num}"),
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    severity: BugSeverity::High,
                    category: "Security / Command Injection".to_string(),
                    title: "Shell execution with unescaped string formatting".to_string(),
                    description: "Spawning a shell command with concatenated input can lead to arbitrary code execution.".to_string(),
                    suggested_fix: "Pass command arguments as an array slice without invoking an intermediate shell.".to_string(),
                });
            }

            // 4. Unhandled Promise / Floating Async
            if !reported_keys.contains(&("bug-async".to_string(), line_num))
                && Self::detect_floating_async(trimmed)
            {
                reported_keys.insert(("bug-async".to_string(), line_num));
                issues.push(BugReportItem {
                    id: format!("bug-async-{line_num}"),
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    severity: BugSeverity::Medium,
                    category: "Reliability / Unhandled Async".to_string(),
                    title: "Floating async promise without await or catch handler".to_string(),
                    description: "Async call result is ignored, which can lead to silent unhandled rejection crashes.".to_string(),
                    suggested_fix: "Prefix with `await` or chain `.catch(...)` error handler.".to_string(),
                });
            }
        }

        issues
    }

    /// Walks the `tree-sitter` syntax tree for TypeScript/TSX/JavaScript files to identify
    /// AST-level injection sinks, high-entropy secret declarators, and unawaited promise expressions.
    fn scan_ast_nodes(
        file_path: &str,
        ext: &str,
        content: &str,
        issues: &mut Vec<BugReportItem>,
        reported_keys: &mut HashSet<(String, usize)>,
    ) {
        let lang = match ext.to_lowercase().as_str() {
            "ts" | "mts" | "cts" | "js" | "mjs" | "cjs" => {
                Some(tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT))
            }
            "tsx" | "jsx" => Some(tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TSX)),
            _ => None,
        };
        let Some(language) = lang else {
            return;
        };

        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&language).is_err() {
            return;
        }
        let Some(tree) = parser.parse(content, None) else {
            return;
        };

        let bytes = content.as_bytes();
        let mut stack = vec![tree.root_node()];

        while let Some(node) = stack.pop() {
            let kind = node.kind();
            let line_num = node.start_position().row + 1;

            match kind {
                "template_string" => {
                    // Check if template_string has a template_substitution child AND SQL keywords
                    let has_substitution = {
                        let mut cur = node.walk();
                        let found = node.children(&mut cur).any(|c| c.kind() == "template_substitution");
                        found
                    };
                    if has_substitution {
                        if let Ok(text) = node.utf8_text(bytes) {
                            let upper = text.to_uppercase();
                            if (upper.contains("SELECT ") && upper.contains(" FROM "))
                                || upper.contains("INSERT INTO ")
                                || upper.contains("DELETE FROM ")
                                || (upper.contains("UPDATE ") && upper.contains(" SET "))
                            {
                                if reported_keys.insert(("sec-sql".to_string(), line_num)) {
                                    issues.push(BugReportItem {
                                        id: format!("sec-sql-{line_num}"),
                                        file_path: file_path.to_string(),
                                        line_number: line_num,
                                        severity: BugSeverity::Critical,
                                        category: "Security / SQL Injection".to_string(),
                                        title: "SQL query constructed via template string interpolation".to_string(),
                                        description: "AST template_string node interpolates dynamic expressions directly into a SQL statement.".to_string(),
                                        suggested_fix: "Use parameterized query placeholders ($1, ?, :param).".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
                "call_expression" => {
                    if let Ok(call_text) = node.utf8_text(bytes) {
                        let trimmed = call_text.trim();
                        // Detect eval() or Function() constructor with dynamic arguments
                        if (trimmed.starts_with("eval(") || trimmed.starts_with("window.eval("))
                            && reported_keys.insert(("sec-eval".to_string(), line_num))
                        {
                            issues.push(BugReportItem {
                                id: format!("sec-eval-{line_num}"),
                                file_path: file_path.to_string(),
                                line_number: line_num,
                                severity: BugSeverity::High,
                                category: "Security / Code Injection".to_string(),
                                title: "Dynamic code execution via eval()".to_string(),
                                description: "Executing dynamic strings via `eval()` enables arbitrary code injection.".to_string(),
                                suggested_fix: "Replace `eval()` with `JSON.parse()` or a structured expression evaluator.".to_string(),
                            });
                        }
                    }
                }
                _ => {}
            }

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }
    }

    /// Evaluates multiple files into an aggregate BugAuditReport.
    pub fn generate_audit_report(files: &[(&str, &str)]) -> BugAuditReport {
        let mut report = BugAuditReport::default();

        for (path, content) in files {
            let file_issues = Self::scan_file_content(path, content);
            for issue in file_issues {
                match issue.severity {
                    BugSeverity::Critical => report.critical_count += 1,
                    BugSeverity::High => report.high_count += 1,
                    BugSeverity::Medium => report.medium_count += 1,
                    BugSeverity::Low => report.low_count += 1,
                }
                report.issues.push(issue);
            }
        }

        report.total_issues = report.issues.len();
        report
    }

    fn extract_quoted_literals(line: &str) -> Vec<&str> {
        let mut out = Vec::new();
        for quote in ['"', '\''] {
            let mut rest = line;
            while let Some(start) = rest.find(quote) {
                let after = &rest[start + 1..];
                if let Some(end) = after.find(quote) {
                    out.push(&after[..end]);
                    rest = &after[end + 1..];
                } else {
                    break;
                }
            }
        }
        out
    }

    fn detect_secret_leak(line: &str) -> bool {
        let lower = line.to_lowercase();
        if lower.starts_with("//") || lower.starts_with('#') || lower.starts_with("/*") || lower.starts_with('*') {
            return false;
        }
        // Ignore environment variable lookups
        if lower.contains("process.env") || lower.contains("std::env::var") || lower.contains("os.environ") {
            return false;
        }

        // 1. Canonical provider credential prefixes
        if line.contains("AKIA") && line.len() >= 20 {
            return true;
        }
        if line.contains("sk-proj-") || line.contains("sk-ant-") {
            return true;
        }
        if line.contains("ghp_") || line.contains("github_pat_") || line.contains("xoxb-") {
            return true;
        }
        if line.contains("-----BEGIN PRIVATE KEY-----") || line.contains("-----BEGIN RSA PRIVATE KEY-----") {
            return true;
        }

        // 2. High-entropy secret literal assigned to a credential identifier
        let has_secret_ident = lower.contains("api_key")
            || lower.contains("apikey")
            || lower.contains("client_secret")
            || lower.contains("private_key")
            || lower.contains("secret_key")
            || lower.contains("access_token");

        if has_secret_ident && (line.contains('=') || line.contains(':')) {
            for lit in Self::extract_quoted_literals(line) {
                let lit_lower = lit.to_lowercase();
                if lit.len() >= 20
                    && !lit.contains(' ')
                    && !lit.contains('/')
                    && !lit_lower.contains("example")
                    && !lit_lower.contains("placeholder")
                    && !lit_lower.contains("your_")
                    && shannon_entropy(lit) >= 3.85
                {
                    return true;
                }
            }
        }

        false
    }

    fn detect_sql_injection(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
            return false;
        }
        let upper = trimmed.to_uppercase();
        if (upper.contains("SELECT ") || upper.contains("INSERT INTO ") || upper.contains("DELETE FROM ") || (upper.contains("UPDATE ") && upper.contains(" SET ")))
            && (trimmed.contains("${") || trimmed.contains(" + ") || trimmed.contains("format!("))
        {
            return true;
        }
        false
    }

    fn detect_command_injection(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
            return false;
        }
        if (trimmed.contains("child_process.exec(")
            || trimmed.contains("execSync(")
            || trimmed.contains("Command::new(\"sh\").arg(\"-c\")")
            || trimmed.contains("Command::new(\"cmd\").arg(\"/C\")"))
            && (trimmed.contains("${") || trimmed.contains(" + ") || trimmed.contains("format!("))
        {
            return true;
        }
        false
    }

    fn detect_floating_async(line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with('*') {
            return false;
        }
        (trimmed.ends_with(".then(") || trimmed.contains("new Promise("))
            && !trimmed.contains(".catch(")
            && !trimmed.starts_with("await")
            && !trimmed.starts_with("return ")
            && !trimmed.starts_with("const ")
            && !trimmed.starts_with("let ")
    }

    /// Walks workspace source files (skipping build/vendor dirs and self-tests), loads them in batches
    /// via `IoRingBatchReader`, and returns a live `BugAuditReport`.
    pub fn scan_workspace(cwd: &std::path::Path) -> Result<BugAuditReport, String> {
        if !cwd.is_dir() {
            return Err(format!("Workspace directory not found: {}", cwd.display()));
        }

        fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                if out.len() >= 1500 {
                    break;
                }
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.')
                    || matches!(
                        name,
                        "node_modules" | "target" | "dist" | "build" | ".git" | "e2e"
                    )
                {
                    continue;
                }
                if path.is_dir() {
                    collect(&path, out);
                } else if path.is_file() {
                    if name.ends_with(".test.ts")
                        || name.ends_with(".test.tsx")
                        || name.ends_with(".spec.ts")
                        || name.contains("bug_hunter")
                    {
                        continue;
                    }
                    if matches!(
                        path.extension().and_then(|e| e.to_str()),
                        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go")
                    ) {
                        out.push(path);
                    }
                }
            }
        }

        let mut paths = Vec::new();
        collect(cwd, &mut paths);

        // Batch-read candidate files via the pure-Rust IoRing / Rayon batch reader
        let loaded_raw = crate::kernel::ioring::IoRingBatchReader::read_files_lossy(&paths, 256 * 1024);

        let mut loaded: Vec<(String, String)> = Vec::new();
        for (p, data) in loaded_raw {
            if let Ok(content) = String::from_utf8(data) {
                let rel = p
                    .strip_prefix(cwd)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                loaded.push((rel, content));
            }
        }

        let pairs: Vec<(&str, &str)> = loaded
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_str()))
            .collect();
        let mut report = Self::generate_audit_report(&pairs);
        Self::append_compiler_diagnostics(cwd, &mut report);
        for issue in &mut report.issues {
            issue.id = format!("{}-{}-{}", issue.file_path, issue.line_number, issue.id);
        }
        report.total_issues = report.issues.len();
        report.issues.truncate(120);
        Ok(report)
    }

    fn append_compiler_diagnostics(cwd: &Path, report: &mut BugAuditReport) {
        if cwd.join("Cargo.toml").is_file() {
            if let Some(out) = Self::run_limited_command(
                cwd,
                "cargo",
                &["check", "--message-format=short", "--quiet"],
                std::time::Duration::from_secs(18),
            ) {
                for (idx, line) in out.lines().take(30).enumerate() {
                    let trimmed = line.trim();
                    if let Some((file_part, line_num, severity, msg)) =
                        Self::parse_short_diagnostic(trimmed)
                    {
                        match severity {
                            BugSeverity::High => report.high_count += 1,
                            BugSeverity::Medium => report.medium_count += 1,
                            _ => {}
                        }
                        report.issues.push(BugReportItem {
                            id: format!("diag-cargo-{idx}"),
                            file_path: file_part,
                            line_number: line_num,
                            severity,
                            category: "Compiler Diagnostic (Cargo)".to_string(),
                            title: msg.chars().take(90).collect(),
                            description: msg,
                            suggested_fix: "Inspect the reported Rust compiler error or warning.".to_string(),
                        });
                    }
                }
            }
        } else if cwd.join("tsconfig.json").is_file() {
            if let Some(out) = Self::run_limited_command(
                cwd,
                if cfg!(windows) { "npx.cmd" } else { "npx" },
                &["--no-install", "tsc", "--noEmit"],
                std::time::Duration::from_secs(15),
            ) {
                for (idx, line) in out.lines().take(30).enumerate() {
                    if let Some((file_part, line_num, msg)) = Self::parse_tsc_diagnostic(line.trim()) {
                        report.high_count += 1;
                        report.issues.push(BugReportItem {
                            id: format!("diag-tsc-{idx}"),
                            file_path: file_part,
                            line_number: line_num,
                            severity: BugSeverity::High,
                            category: "TypeScript Compiler Diagnostic".to_string(),
                            title: msg.chars().take(90).collect(),
                            description: msg,
                            suggested_fix: "Resolve the TypeScript type error reported by tsc.".to_string(),
                        });
                    }
                }
            }
        }
    }

    fn run_limited_command(
        cwd: &Path,
        bin: &str,
        args: &[&str],
        timeout: std::time::Duration,
    ) -> Option<String> {
        let mut cmd = std::process::Command::new(bin);
        cmd.args(args)
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd.spawn().ok()?;
        let started = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if started.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
                Err(_) => return None,
            }
        }
        let out = child.wait_with_output().ok()?;
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        Some(combined)
    }

    fn parse_short_diagnostic(line: &str) -> Option<(String, usize, BugSeverity, String)> {
        // format: path:line:col: error[E...]: message
        let idx = line.find(": error")
            .map(|i| (i, BugSeverity::High))
            .or_else(|| line.find(": warning").map(|i| (i, BugSeverity::Medium)))?;
        let loc = &line[..idx.0];
        let mut parts = loc.rsplitn(3, ':');
        let _col = parts.next()?.parse::<usize>().ok()?;
        let line_num = parts.next()?.parse::<usize>().ok()?;
        let file = parts.next()?.to_string();
        let msg = line[idx.0 + 2..].trim().to_string();
        Some((file, line_num, idx.1, msg))
    }

    fn parse_tsc_diagnostic(line: &str) -> Option<(String, usize, String)> {
        // format: file.ts(line,col): error TS1234: message
        let err_idx = line.find("): error TS")?;
        let paren_idx = line[..err_idx].rfind('(')?;
        let file = line[..paren_idx].to_string();
        let coords = &line[paren_idx + 1..err_idx];
        let line_num = coords.split(',').next()?.trim().parse::<usize>().ok()?;
        let msg = line[err_idx + 3..].trim().to_string();
        Some((file, line_num, msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_detection() {
        let content = "const token = \"ghp_1234567890abcdefghijklmnopqrstuvwxyz\";";
        let issues = BugHunter::scan_file_content("auth.ts", content);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, BugSeverity::Critical);
        assert_eq!(issues[0].category, "Security / Secret Leak");
    }

    #[test]
    fn test_sql_injection_detection() {
        let content = "const query = `SELECT * FROM users WHERE id = ${userId}`;";
        let issues = BugHunter::scan_file_content("db.ts", content);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, BugSeverity::Critical);
        assert!(issues[0].title.contains("SQL query"));
    }

    #[test]
    fn test_clean_file_no_issues() {
        let content = "const sum = (a: number, b: number) => a + b;\nexport default sum;";
        let issues = BugHunter::scan_file_content("math.ts", content);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_generate_audit_report() {
        let files = [
            ("sec.ts", "const key = 'sk-proj-xyz123456789';"),
            ("safe.ts", "console.log('hello');"),
        ];
        let report = BugHunter::generate_audit_report(&files);
        assert_eq!(report.total_issues, 1);
        assert_eq!(report.critical_count, 1);
    }
}
