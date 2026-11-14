use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CiStatus {
    #[serde(default, alias = "has_failure")]
    pub has_failure: bool,
    #[serde(default, alias = "run_id")]
    pub run_id: Option<u64>,
    #[serde(default, alias = "workflow_name")]
    pub workflow_name: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRunPayload {
    #[serde(default, alias = "databaseId", alias = "id")]
    database_id: Option<u64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "headBranch")]
    head_branch: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

pub fn parse_ci_status_json(raw_json: &str) -> Result<CiStatus, String> {
    let runs: Vec<GhRunPayload> = serde_json::from_str(raw_json)
        .map_err(|e| format!("Failed to parse GitHub run list JSON: {e}"))?;

    if runs.is_empty() {
        return Ok(CiStatus {
            has_failure: false,
            run_id: None,
            workflow_name: None,
            branch: None,
            conclusion: None,
            url: None,
            message: Some("No workflow runs found for this repository.".to_string()),
        });
    }

    let run = &runs[0];
    let conclusion_str = run.conclusion.clone().unwrap_or_default();
    let status_str = run.status.clone().unwrap_or_default();
    let conclusion_lower = conclusion_str.to_lowercase();

    let is_failure = matches!(
        conclusion_lower.as_str(),
        "failure" | "timed_out" | "startup_failure" | "cancelled"
    );

    let message = if is_failure {
        let name = run.name.as_deref().unwrap_or("CI Workflow");
        let branch = run.head_branch.as_deref().unwrap_or("unknown branch");
        Some(format!(
            "Workflow '{name}' failed with conclusion '{conclusion_str}' on branch '{branch}'."
        ))
    } else if conclusion_lower == "success" {
        Some("All CI checks passed successfully.".to_string())
    } else if !status_str.is_empty() && status_str != "completed" {
        Some(format!("Workflow run is currently {status_str}."))
    } else {
        Some(format!("Workflow concluded with: {conclusion_str}"))
    };

    Ok(CiStatus {
        has_failure: is_failure,
        run_id: run.database_id,
        workflow_name: run.name.clone(),
        branch: run.head_branch.clone(),
        conclusion: run.conclusion.clone(),
        url: run.url.clone(),
        message,
    })
}

pub fn format_ci_failure_log(run_id: u64, raw_log: &str) -> String {
    let trimmed = raw_log.trim();
    let content = if trimmed.is_empty() {
        "No failed step logs available for this run."
    } else {
        trimmed
    };
    format!(
        "# CI Failure Diagnostic Log\n**Run ID:** {run_id}\n\n```text\n{content}\n```\n"
    )
}

pub fn check_ci_status(workspace_path: &Path) -> Result<CiStatus, String> {
    if !workspace_path.exists() {
        return Err(format!(
            "Workspace path does not exist: {}",
            workspace_path.display()
        ));
    }

    let output = match Command::new("gh")
        .args([
            "run",
            "list",
            "--limit",
            "1",
            "--json",
            "databaseId,status,conclusion,name,headBranch,url",
        ])
        .current_dir(workspace_path)
        .output()
    {
        Ok(out) => out,
        Err(_) => {
            // gh is not installed or failed to execute, return graceful fallback
            return Ok(CiStatus {
                has_failure: false,
                run_id: None,
                workflow_name: None,
                branch: None,
                conclusion: None,
                url: None,
                message: Some(
                    "GitHub CLI ('gh') is not installed or not found in system PATH.".to_string(),
                ),
            });
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let trimmed_err = stderr.trim();
        return Ok(CiStatus {
            has_failure: false,
            run_id: None,
            workflow_name: None,
            branch: None,
            conclusion: None,
            url: None,
            message: Some(if trimmed_err.is_empty() {
                "No CI data available or workspace not connected to a GitHub repository."
                    .to_string()
            } else {
                format!("CI status check skipped: {trimmed_err}")
            }),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ci_status_json(&stdout)
}

pub fn get_ci_failure_log(workspace_path: &Path, run_id: u64) -> Result<String, String> {
    if !workspace_path.exists() {
        return Err(format!(
            "Workspace path does not exist: {}",
            workspace_path.display()
        ));
    }
    if run_id == 0 {
        return Err("Invalid or missing CI run ID (0)".to_string());
    }

    let output = Command::new("gh")
        .args(["run", "view", &run_id.to_string(), "--log-failed"])
        .current_dir(workspace_path)
        .output()
        .map_err(|e| format!("Failed to execute 'gh run view': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("gh run view failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(format_ci_failure_log(run_id, &stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ci_status_failure() {
        let json = r#"[
            {
                "databaseId": 987654321,
                "status": "completed",
                "conclusion": "failure",
                "name": "Integration Tests",
                "headBranch": "fix/auth-flow",
                "url": "https://github.com/example/repo/actions/runs/987654321"
            }
        ]"#;

        let status = parse_ci_status_json(json).expect("should parse");
        assert!(status.has_failure);
        assert_eq!(status.run_id, Some(987654321));
        assert_eq!(status.workflow_name.as_deref(), Some("Integration Tests"));
        assert_eq!(status.branch.as_deref(), Some("fix/auth-flow"));
        assert_eq!(status.conclusion.as_deref(), Some("failure"));
        assert_eq!(
            status.url.as_deref(),
            Some("https://github.com/example/repo/actions/runs/987654321")
        );
        assert!(status.message.unwrap().contains("failed"));
    }

    #[test]
    fn test_parse_ci_status_success() {
        let json = r#"[
            {
                "databaseId": 123456,
                "status": "completed",
                "conclusion": "success",
                "name": "Build",
                "headBranch": "main",
                "url": "https://github.com/example/repo/actions/runs/123456"
            }
        ]"#;

        let status = parse_ci_status_json(json).expect("should parse");
        assert!(!status.has_failure);
        assert_eq!(status.run_id, Some(123456));
        assert_eq!(status.conclusion.as_deref(), Some("success"));
        assert!(status.message.unwrap().contains("passed"));
    }

    #[test]
    fn test_parse_ci_status_empty() {
        let json = "[]";
        let status = parse_ci_status_json(json).expect("should parse");
        assert!(!status.has_failure);
        assert_eq!(status.run_id, None);
        assert!(status.message.unwrap().contains("No workflow runs found"));
    }

    #[test]
    fn test_format_ci_failure_log() {
        let raw = "Error: Process completed with exit code 1.\n  at test_runner.rs:42";
        let formatted = format_ci_failure_log(999, raw);
        assert!(formatted.contains("Run ID:** 999"));
        assert!(formatted.contains("```text"));
        assert!(formatted.contains("test_runner.rs:42"));
    }

    #[test]
    fn test_check_ci_status_fallback_non_git_repo() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Running on an empty temporary directory with no git repository
        let res = check_ci_status(tmp.path());
        assert!(res.is_ok());
        let status = res.unwrap();
        // Since there is no git repo or remote, has_failure should be false with fallback message
        assert!(!status.has_failure);
        assert!(status.message.is_some());
    }

    #[test]
    fn test_check_ci_status_non_existent_dir() {
        let path = std::path::Path::new("C:/__non_existent_path_4829374__");
        let res = check_ci_status(path);
        assert!(res.is_err());
    }

    #[test]
    fn test_get_ci_failure_log_invalid_run_id() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let res = get_ci_failure_log(tmp.path(), 0);
        assert!(res.is_err());
    }
}
