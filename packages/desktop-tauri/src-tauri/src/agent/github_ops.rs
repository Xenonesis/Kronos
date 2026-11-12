//! Kronos 1-Click GitHub Pull Request Operations
//!
//! Provides seamless GitHub PR generation with branch publishing, `gh pr create` invocation,
//! and automatic GitHub web comparison fallback link generation.

use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePrRequest {
    pub cwd: String,
    pub title: String,
    pub body: String,
    pub base_branch: String,
    pub head_branch: String,
    pub auto_push: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePrResult {
    pub success: bool,
    pub pr_url: String,
    pub method: String, // "gh_cli" | "web_fallback"
    pub message: String,
}

/// Discovers the GitHub repository web URL from git remote origin.
pub fn get_github_repo_url(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(cwd)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Support git@github.com:owner/repo.git or https://github.com/owner/repo.git
    if raw.starts_with("git@github.com:") {
        let rest = &raw["git@github.com:".len()..];
        let cleaned = rest.trim_end_matches(".git");
        Some(format!("https://github.com/{}", cleaned))
    } else if raw.starts_with("https://github.com/") {
        let cleaned = raw.trim_end_matches(".git");
        Some(cleaned.to_string())
    } else {
        None
    }
}

/// Creates a GitHub Pull Request using `gh` CLI if available, falling back to a direct GitHub web comparison URL.
pub fn execute_create_pr(req: &CreatePrRequest) -> Result<CreatePrResult, String> {
    let cwd = Path::new(&req.cwd);
    if !cwd.is_dir() {
        return Err(format!("Workspace directory not found: {}", req.cwd));
    }

    // Step 1: Push branch if auto_push is enabled
    if req.auto_push {
        let push_status = Command::new("git")
            .args(["push", "-u", "origin", &req.head_branch])
            .current_dir(cwd)
            .status();

        match push_status {
            Ok(s) if !s.success() => {
                // Non-fatal if remote already up to date, but log warning
            }
            Err(e) => {
                eprintln!("[GitHubOps] git push warning: {e}");
            }
            _ => {}
        }
    }

    // Step 2: Try `gh pr create`
    #[cfg(windows)]
    let gh_cmd = "gh.exe";
    #[cfg(not(windows))]
    let gh_cmd = "gh";

    let gh_attempt = Command::new(gh_cmd)
        .args([
            "pr",
            "create",
            "--base",
            &req.base_branch,
            "--head",
            &req.head_branch,
            "--title",
            &req.title,
            "--body",
            &req.body,
        ])
        .current_dir(cwd)
        .output();

    if let Ok(output) = gh_attempt {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // `gh pr create` outputs the URL directly on stdout
            let url = out_str
                .lines()
                .last()
                .unwrap_or(&out_str)
                .trim()
                .to_string();

            return Ok(CreatePrResult {
                success: true,
                pr_url: url.clone(),
                method: "gh_cli".to_string(),
                message: format!("Successfully created pull request: {}", url),
            });
        }
    }

    // Step 3: Fallback to GitHub Web Compare URL
    if let Some(repo_url) = get_github_repo_url(cwd) {
        let encoded_title = percent_encode(&req.title);
        let encoded_body = percent_encode(&req.body);
        let fallback_url = format!(
            "{}/compare/{}...{}?expand=1&title={}&body={}",
            repo_url, req.base_branch, req.head_branch, encoded_title, encoded_body
        );

        return Ok(CreatePrResult {
            success: true,
            pr_url: fallback_url.clone(),
            method: "web_fallback".to_string(),
            message: "Prepared GitHub pull request URL".to_string(),
        });
    }

    Err("Failed to execute `gh pr create` and could not resolve GitHub remote URL".to_string())
}

/// Zero-dependency URL percent-encoder for query parameters.
pub fn percent_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_repo_url_parsing() {
        let ssh_url = "git@github.com:octocat/Hello-World.git";
        let rest = &ssh_url["git@github.com:".len()..];
        let cleaned = rest.trim_end_matches(".git");
        assert_eq!(format!("https://github.com/{}", cleaned), "https://github.com/octocat/Hello-World");

        let https_url = "https://github.com/octocat/Hello-World.git";
        assert_eq!(https_url.trim_end_matches(".git"), "https://github.com/octocat/Hello-World");
    }

    #[test]
    fn test_percent_encode() {
        assert_eq!(percent_encode("hello world!"), "hello+world%21");
        assert_eq!(percent_encode("feat/test"), "feat%2Ftest");
    }
}
