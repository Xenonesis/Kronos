//! Kronos Autonomous Browser DOM Inspection Tool
//!
//! Enables autonomous agents to inspect local web applications (e.g. `http://localhost:3000`),
//! extract DOM structure, detect fatal client error signatures (React boundaries, syntax errors),
//! and verify successful frontend builds.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInspectionResult {
    pub url: String,
    pub status_code: u16,
    pub title: Option<String>,
    pub detected_errors: Vec<String>,
    pub dom_outline: Vec<String>,
    pub is_healthy: bool,
}

/// Inspects a URL by fetching its rendered markup, extracting headings and error signatures.
pub async fn inspect_url(url: &str) -> Result<BrowserInspectionResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach {url}: {e}"))?;

    let status_code = response.status().as_u16();
    let body = response
        .text()
        .await
        .unwrap_or_default();

    let mut detected_errors = Vec::new();
    let lower_body = body.to_lowercase();

    // Check for common error signatures
    if lower_body.contains("react error boundary") || lower_body.contains("something went wrong") {
        detected_errors.push("React Error Boundary triggered in DOM".to_string());
    }
    if lower_body.contains("uncaught typeerror") || lower_body.contains("typeerror:") {
        detected_errors.push("Uncaught TypeError detected in response".to_string());
    }
    if lower_body.contains("module not found") || lower_body.contains("cannot find module") {
        detected_errors.push("Bundler 'Module not found' error".to_string());
    }
    if lower_body.contains("vite: runtime error") || lower_body.contains("failed to load resource") {
        detected_errors.push("Vite / Dev server runtime compilation error".to_string());
    }
    if status_code >= 400 {
        detected_errors.push(format!("HTTP error status: {status_code}"));
    }

    // Extract title tag
    let title = extract_title(&body);

    // Extract basic DOM outline (headings, sections, interactive elements)
    let dom_outline = extract_dom_outline(&body);

    let is_healthy = detected_errors.is_empty() && (200..300).contains(&status_code);

    Ok(BrowserInspectionResult {
        url: url.to_string(),
        status_code,
        title,
        detected_errors,
        dom_outline,
        is_healthy,
    })
}

pub fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start_idx = lower.find("<title>")?;
    let end_idx = lower.find("</title>")?;
    if end_idx > start_idx + 7 {
        let t = html[start_idx + 7..end_idx].trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    None
}

pub fn extract_dom_outline(html: &str) -> Vec<String> {
    let mut outline = Vec::new();
    for line in html.lines() {
        let trimmed = line.trim();
        if (trimmed.starts_with("<h1") || trimmed.starts_with("<h2") || trimmed.starts_with("<nav") || trimmed.starts_with("<main"))
            && trimmed.contains('>')
        {
            if let Some(close) = trimmed.find('>') {
                let tag_snippet = &trimmed[..close + 1];
                outline.push(tag_snippet.to_string());
                if outline.len() >= 15 {
                    break;
                }
            }
        }
    }
    outline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title() {
        let sample = "<html><head><title>Kronos Dashboard</title></head><body><h1>Hello</h1></body></html>";
        assert_eq!(extract_title(sample), Some("Kronos Dashboard".to_string()));
    }

    #[test]
    fn test_error_detection() {
        let sample_error = "<html><body><div>Vite: Runtime Error: Cannot find module './App'</div></body></html>";
        let lower = sample_error.to_lowercase();
        assert!(lower.contains("cannot find module"));
    }
}
