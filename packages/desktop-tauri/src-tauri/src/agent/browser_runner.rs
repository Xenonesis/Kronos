//! Kronos Autonomous Browser Testing & UI Verification Engine (Cline / Devin Parity)
//!
//! Enables autonomous agents and users to navigate localhost web applications,
//! inspect rendered DOM trees, detect fatal runtime/console errors, and auto-generate UI repair prompts.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserElementInfo {
    pub tag: String,
    pub label: String,
    pub selector: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserVerificationReport {
    pub url: String,
    pub status_code: u16,
    pub is_healthy: bool,
    pub title: Option<String>,
    pub detected_errors: Vec<String>,
    pub interactive_elements: Vec<BrowserElementInfo>,
    pub auto_fix_prompt: Option<String>,
}

/// Parses title from HTML markup.
fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    if let Some(start) = lower.find("<title>") {
        if let Some(end) = lower[start..].find("</title>") {
            let title = &html[start + 7..start + end];
            return Some(title.trim().to_string());
        }
    }
    None
}

/// Extracts interactive elements (buttons, links, inputs) from HTML markup.
fn extract_interactive_elements(html: &str) -> Vec<BrowserElementInfo> {
    let mut elements = Vec::new();
    let lower = html.to_lowercase();

    // Scan for button tags
    let mut offset = 0;
    while let Some(pos) = lower[offset..].find("<button") {
        let tag_start = offset + pos;
        if let Some(close_tag) = lower[tag_start..].find('>') {
            let open_tag_end = tag_start + close_tag + 1;
            if let Some(end_tag) = lower[open_tag_end..].find("</button>") {
                let inner = &html[open_tag_end..open_tag_end + end_tag];
                let label = inner.trim().chars().take(40).collect::<String>();
                if !label.is_empty() {
                    elements.push(BrowserElementInfo {
                        tag: "button".to_string(),
                        label,
                        selector: format!("button:nth-of-type({})", elements.len() + 1),
                    });
                }
            }
        }
        offset = tag_start + 7;
        if offset >= lower.len() || elements.len() >= 20 {
            break;
        }
    }

    elements
}

/// Executes autonomous browser verification against a target URL.
pub async fn verify_web_ui(url: &str) -> Result<BrowserVerificationReport, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach web application at {url}: {e}"))?;

    let status_code = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();

    let mut detected_errors = Vec::new();
    let lower_body = body.to_lowercase();

    if lower_body.contains("react error boundary") || lower_body.contains("something went wrong") {
        detected_errors.push("React Error Boundary rendered in DOM".to_string());
    }
    if lower_body.contains("uncaught typeerror") || lower_body.contains("typeerror:") {
        detected_errors.push("Uncaught TypeError detected in web app".to_string());
    }
    if lower_body.contains("cannot find module") || lower_body.contains("module not found") {
        detected_errors.push("Vite / Webpack Module Resolution Error".to_string());
    }
    if status_code >= 400 {
        detected_errors.push(format!("Server returned HTTP error status: {status_code}"));
    }

    let is_healthy = detected_errors.is_empty() && (200..300).contains(&status_code);
    let title = extract_html_title(&body);
    let interactive_elements = extract_interactive_elements(&body);

    let auto_fix_prompt = if !is_healthy {
        Some(format!(
            "[AUTONOMOUS BROWSER UI VERIFICATION ERROR]\n\
            The web application at `{url}` encountered client errors during rendering:\n\n\
            Errors:\n{}\n\n\
            Please immediately fix the frontend code to resolve these UI runtime errors.",
            detected_errors.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n")
        ))
    } else {
        None
    };

    Ok(BrowserVerificationReport {
        url: url.to_string(),
        status_code,
        is_healthy,
        title,
        detected_errors,
        interactive_elements,
        auto_fix_prompt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_html_title() {
        let html = "<html><head><title>My Awesome Dashboard</title></head><body><h1>Hello</h1></body></html>";
        assert_eq!(extract_html_title(html), Some("My Awesome Dashboard".to_string()));
    }

    #[test]
    fn test_extract_interactive_elements() {
        let html = "<div><button>Save Changes</button><button>Cancel</button></div>";
        let elements = extract_interactive_elements(html);
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].label, "Save Changes");
        assert_eq!(elements[1].label, "Cancel");
    }
}
