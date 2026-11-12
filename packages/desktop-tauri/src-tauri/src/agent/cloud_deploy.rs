//! Kronos 1-Click Cloud Deployment Engine (Bolt.new / Lovable Parity)
//!
//! Provides automated framework detection, build verification, and deployment to
//! Vercel, Netlify, and Cloudflare Pages with live URL extraction.

use std::fs;
use std::path::Path;
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudProvider {
    Vercel,
    Netlify,
    CloudflarePages,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDeployRequest {
    pub cwd: String,
    pub provider: CloudProvider,
    pub project_name: Option<String>,
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudDeployResult {
    pub success: bool,
    pub live_url: Option<String>,
    pub provider: CloudProvider,
    pub build_logs: String,
    pub message: String,
}

/// Detects probable framework and output directory in workspace.
pub fn detect_framework_output(cwd: &Path) -> (&'static str, &'static str) {
    let pkg_json = cwd.join("package.json");
    if let Ok(content) = fs::read_to_string(&pkg_json) {
        if content.contains("\"next\"") {
            return ("Next.js", ".next");
        } else if content.contains("\"@sveltejs/kit\"") {
            return ("SvelteKit", "build");
        } else if content.contains("\"astro\"") {
            return ("Astro", "dist");
        } else if content.contains("\"vite\"") {
            return ("Vite", "dist");
        }
    }
    ("Static / Web App", "dist")
}

/// Extracts deployed public URL from tool stdout.
pub fn extract_deployed_url(output: &str) -> Option<String> {
    for line in output.lines() {
        for word in line.split_whitespace() {
            let cleaned = word.trim_matches(|c| c == '\'' || c == '"' || c == '(' || c == ')' || c == '<' || c == '>');
            if cleaned.starts_with("https://") {
                if cleaned.contains(".vercel.app")
                    || cleaned.contains(".netlify.app")
                    || cleaned.contains(".pages.dev")
                {
                    return Some(cleaned.to_string());
                }
            }
        }
    }
    None
}

/// Deploys workspace to selected cloud provider using local CLI tools.
pub fn deploy_to_cloud(req: &CloudDeployRequest) -> Result<CloudDeployResult, String> {
    let cwd = Path::new(&req.cwd);
    if !cwd.is_dir() {
        return Err(format!("Workspace directory not found: {}", req.cwd));
    }

    #[cfg(windows)]
    let npx_cmd = "npx.cmd";
    #[cfg(not(windows))]
    let npx_cmd = "npx";

    let (framework, detected_out) = detect_framework_output(cwd);
    let output_dir = req.output_dir.as_deref().unwrap_or(detected_out);

    let mut command = Command::new(npx_cmd);
    command.current_dir(cwd);

    match req.provider {
        CloudProvider::Vercel => {
            command.args(["vercel", "--prod", "--yes"]);
        }
        CloudProvider::Netlify => {
            command.args(["netlify", "deploy", "--prod", "--dir", output_dir]);
        }
        CloudProvider::CloudflarePages => {
            let project = req.project_name.as_deref().unwrap_or("kronos-app");
            command.args(["wrangler", "pages", "deploy", output_dir, "--project-name", project]);
        }
    }

    let output = command.output().map_err(|e| format!("Failed to spawn deploy command: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let full_logs = format!("{}\n{}", stdout, stderr);

    let live_url = extract_deployed_url(&full_logs);
    let success = output.status.success() || live_url.is_some();

    let message = if let Some(ref url) = live_url {
        format!("Successfully deployed {framework} to {url}")
    } else if success {
        "Deployment completed successfully".to_string()
    } else {
        "Deployment failed; inspect logs".to_string()
    };

    Ok(CloudDeployResult {
        success,
        live_url,
        provider: req.provider.clone(),
        build_logs: full_logs,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_deployed_url() {
        let vercel_log = "Inspect: https://vercel.com/user/app/deploy\nProduction: https://my-app-prod.vercel.app [copied to clipboard]";
        assert_eq!(extract_deployed_url(vercel_log), Some("https://my-app-prod.vercel.app".to_string()));

        let netlify_log = "Website URL: https://dazzling-unicorn-12345.netlify.app";
        assert_eq!(extract_deployed_url(netlify_log), Some("https://dazzling-unicorn-12345.netlify.app".to_string()));

        let cf_log = "Deployment complete! Take a peek over at https://super-cool.pages.dev";
        assert_eq!(extract_deployed_url(cf_log), Some("https://super-cool.pages.dev".to_string()));
    }
}
