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
    pub dom_outline: Vec<String>,
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

/// Builds the most specific, stable CSS selector for an element from its opening tag attributes.
fn build_best_selector(tag: &str, open_tag_slice: &str, nth_index: usize) -> String {
    if let Some(id) = extract_attr(open_tag_slice, "id") {
        let clean_id = id.trim();
        if !clean_id.is_empty() && !clean_id.contains(' ') {
            return format!("#{clean_id}");
        }
    }
    if let Some(test_id) = extract_attr(open_tag_slice, "data-testid")
        .or_else(|| extract_attr(open_tag_slice, "data-test-id"))
    {
        return format!("[data-testid=\"{test_id}\"]");
    }
    if let Some(name) = extract_attr(open_tag_slice, "name") {
        return format!("{tag}[name=\"{name}\"]");
    }
    if let Some(aria) = extract_attr(open_tag_slice, "aria-label") {
        let short: String = aria.chars().take(32).collect();
        if !short.contains('"') {
            return format!("{tag}[aria-label=\"{short}\"]");
        }
    }
    if let Some(class_attr) = extract_attr(open_tag_slice, "class") {
        if let Some(first_cls) = class_attr
            .split_whitespace()
            .find(|c| !c.is_empty() && !c.contains(':') && !c.contains('['))
        {
            return format!("{tag}.{first_cls}:nth-of-type({nth_index})");
        }
    }
    format!("{tag}:nth-of-type({nth_index})")
}

/// Extracts interactive elements (`<button>`, `<a>`, `<input>`, `<select>`, `<textarea>`,
/// and elements with interactive ARIA `role` or `data-testid` attributes) from HTML markup.
pub fn extract_interactive_elements(html: &str) -> Vec<BrowserElementInfo> {
    let mut elements = Vec::new();
    let lower = html.to_lowercase();

    for (tag, close_tag) in [
        ("button", "</button>"),
        ("a", "</a>"),
        ("select", "</select>"),
        ("textarea", "</textarea>"),
    ] {
        let open_needle = format!("<{tag}");
        let mut offset = 0;
        let mut tag_count = 0usize;
        while let Some(pos) = lower[offset..].find(&open_needle) {
            let tag_start = offset + pos;
            let after_name = lower
                .as_bytes()
                .get(tag_start + open_needle.len())
                .copied()
                .unwrap_or(b' ');
            if after_name.is_ascii_alphanumeric() {
                offset = tag_start + open_needle.len();
                continue;
            }
            if let Some(close_angle) = lower[tag_start..].find('>') {
                let open_tag_end = tag_start + close_angle + 1;
                let open_tag_slice = &html[tag_start..open_tag_end];
                if let Some(end_tag) = lower[open_tag_end..].find(close_tag) {
                    let inner = &html[open_tag_end..open_tag_end + end_tag];
                    let text_clean = strip_simple_tags(inner)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .chars()
                        .take(64)
                        .collect::<String>();
                    let label = if !text_clean.is_empty() {
                        text_clean
                    } else {
                        extract_attr(open_tag_slice, "aria-label")
                            .or_else(|| extract_attr(open_tag_slice, "title"))
                            .or_else(|| extract_attr(open_tag_slice, "name"))
                            .or_else(|| extract_attr(open_tag_slice, "id"))
                            .unwrap_or_default()
                    };
                    if !label.is_empty() {
                        tag_count += 1;
                        let selector = build_best_selector(tag, open_tag_slice, tag_count);
                        elements.push(BrowserElementInfo {
                            tag: tag.to_string(),
                            label,
                            selector,
                        });
                    }
                }
            }
            offset = tag_start + open_needle.len();
            if offset >= lower.len() || elements.len() >= 35 {
                break;
            }
        }
    }

    let mut offset = 0;
    let mut input_count = 0usize;
    while let Some(pos) = lower[offset..].find("<input") {
        let tag_start = offset + pos;
        if let Some(close_angle) = lower[tag_start..].find('>') {
            let tag_slice = &html[tag_start..tag_start + close_angle + 1];
            let input_type = extract_attr(tag_slice, "type").unwrap_or_else(|| "text".to_string());
            if input_type.eq_ignore_ascii_case("hidden") {
                offset = tag_start + 6;
                continue;
            }
            let label = extract_attr(tag_slice, "placeholder")
                .or_else(|| extract_attr(tag_slice, "aria-label"))
                .or_else(|| extract_attr(tag_slice, "name"))
                .or_else(|| extract_attr(tag_slice, "id"))
                .unwrap_or_else(|| format!("{input_type} input"));
            input_count += 1;
            let selector = build_best_selector("input", tag_slice, input_count);
            elements.push(BrowserElementInfo {
                tag: "input".to_string(),
                label,
                selector,
            });
        }
        offset = tag_start + 6;
        if offset >= lower.len() || elements.len() >= 45 {
            break;
        }
    }

    // Also extract elements with role="button"|"tab"|"menuitem"|"switch" on div/span
    let mut role_offset = 0;
    let mut role_count = 0usize;
    while let Some(pos) = lower[role_offset..].find("role=") {
        let attr_pos = role_offset + pos;
        let tag_open = lower[..attr_pos].rfind('<').unwrap_or(attr_pos);
        if let Some(close_angle) = lower[attr_pos..].find('>') {
            let tag_end = attr_pos + close_angle + 1;
            let open_slice = &html[tag_open..tag_end];
            if let Some(role_val) = extract_attr(open_slice, "role") {
                let role_low = role_val.to_lowercase();
                if matches!(role_low.as_str(), "button" | "tab" | "menuitem" | "switch" | "checkbox") {
                    if let Some(label) = extract_attr(open_slice, "aria-label")
                        .or_else(|| extract_attr(open_slice, "data-testid"))
                        .or_else(|| extract_attr(open_slice, "title"))
                        .or_else(|| extract_attr(open_slice, "id"))
                    {
                        role_count += 1;
                        elements.push(BrowserElementInfo {
                            tag: format!("role:{role_low}"),
                            label,
                            selector: build_best_selector("*", open_slice, role_count),
                        });
                    }
                }
            }
            role_offset = tag_end;
        } else {
            break;
        }
        if role_offset >= lower.len() || elements.len() >= 50 {
            break;
        }
    }

    elements
}

fn strip_simple_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    let lower = tag.to_lowercase();
    let needle = format!("{attr_name}=");
    let pos = lower.find(&needle)?;
    let rest = tag[pos + needle.len()..].trim_start();
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let inner = &rest[1..];
        let end = inner.find(quote)?;
        let val = inner[..end].trim();
        if !val.is_empty() {
            return Some(val.to_string());
        }
    }
    None
}

fn find_headless_browser() -> Option<std::path::PathBuf> {
    for bin in ["msedge", "chrome", "chromium", "google-chrome", "chromium-browser"] {
        if let Some(p) = crate::agent::registry::resolve_command(bin) {
            return Some(p);
        }
    }
    #[cfg(windows)]
    {
        let candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for path in candidates {
            let p = std::path::PathBuf::from(path);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Parses Chromium/Edge stderr logs (`--enable-logging=stderr`) for real JavaScript console errors
/// and uncaught runtime exceptions.
pub fn parse_browser_console_errors(stderr_log: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for line in stderr_log.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.contains("console(") || lower.contains(":console") {
            if lower.contains("error")
                || lower.contains("uncaught")
                || lower.contains("typeerror")
                || lower.contains("referenceerror")
                || lower.contains("syntaxerror")
                || lower.contains("failed to load")
            {
                let msg = if let Some(idx) = trimmed.find("CONSOLE(") {
                    trimmed[idx..].to_string()
                } else {
                    trimmed.to_string()
                };
                if !errors.contains(&msg) {
                    errors.push(msg);
                }
            }
        } else if lower.contains("uncaught (in promise)")
            || lower.contains("uncaught typeerror")
            || lower.contains("uncaught referenceerror")
            || lower.contains("uncaught syntaxerror")
        {
            let msg = trimmed.to_string();
            if !errors.contains(&msg) {
                errors.push(msg);
            }
        }
    }
    errors
}

async fn dump_rendered_dom_and_console(url: &str) -> Option<(String, Vec<String>)> {
    let browser = find_headless_browser()?;
    let mut cmd = tokio::process::Command::new(browser);
    cmd.args([
        "--headless=new",
        "--disable-gpu",
        "--no-sandbox",
        "--enable-logging=stderr",
        "--v=0",
        "--virtual-time-budget=2500",
        "--dump-dom",
        url,
    ]);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    let output = tokio::time::timeout(Duration::from_secs(9), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let dom = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let console_errors = parse_browser_console_errors(&stderr);
    if dom.trim().is_empty() {
        None
    } else {
        Some((dom, console_errors))
    }
}

/// Executes autonomous browser verification against a target URL.
/// Uses real headless Chromium/Edge DOM rendering + JS console stderr capture when available,
/// falling back to HTTP HTML inspection otherwise.
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
    let http_body = response.text().await.unwrap_or_default();
    let mut detected_errors = Vec::new();

    let body = if (200..400).contains(&status_code) {
        if let Some((dom, console_errs)) = dump_rendered_dom_and_console(url).await {
            for err in console_errs {
                detected_errors.push(err);
            }
            dom
        } else {
            http_body
        }
    } else {
        http_body
    };

    let lower_body = body.to_lowercase();

    if lower_body.contains("react error boundary")
        || lower_body.contains("something went wrong")
        || lower_body.contains("nextjs-portal")
        || lower_body.contains("vite-error-overlay")
    {
        detected_errors.push("Framework Error Overlay / Error Boundary rendered in DOM".to_string());
    }
    if lower_body.contains("uncaught typeerror") || lower_body.contains("typeerror:") {
        detected_errors.push("Uncaught TypeError detected in web app".to_string());
    }
    if lower_body.contains("uncaught referenceerror") || lower_body.contains("referenceerror:") {
        detected_errors.push("Uncaught ReferenceError detected in web app".to_string());
    }
    if lower_body.contains("cannot find module") || lower_body.contains("module not found") {
        detected_errors.push("Vite / Webpack Module Resolution Error".to_string());
    }
    if status_code >= 400 {
        detected_errors.push(format!("Server returned HTTP error status: {status_code}"));
    }

    let is_healthy = detected_errors.is_empty() && (200..300).contains(&status_code);
    let title = extract_html_title(&body).or_else(|| crate::agent::browser_tool::extract_title(&body));
    let interactive_elements = extract_interactive_elements(&body);
    let mut dom_outline = crate::agent::browser_tool::extract_dom_outline(&body);
    if dom_outline.is_empty() && status_code >= 400 {
        if let Ok(fallback) = crate::agent::browser_tool::inspect_url(url).await {
            dom_outline = fallback.dom_outline;
        }
    }

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
        dom_outline,
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
