//! Kronos Native Web Search & Web Scraping Engine (Claude Code / Devin / OpenCode Parity)
//!
//! Provides zero-API-key web searching (DuckDuckGo HTML + Instant Answer fallback)
//! and high-fidelity webpage content scraping & HTML-to-Markdown extraction.
//! Enables all models (including free OpenCode/Beam models and local Ollama) to retrieve
//! live web documentation, real-time news, and API references without errors.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebFetchResult {
    pub url: String,
    pub title: Option<String>,
    pub content_markdown: String,
    pub character_count: usize,
    pub status_code: u16,
}

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Decodes DuckDuckGo redirection link (e.g. //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com)
pub fn decode_duckduckgo_url(raw_href: &str) -> String {
    if let Some(pos) = raw_href.find("uddg=") {
        let encoded = &raw_href[pos + 5..];
        let end = encoded.find('&').unwrap_or(encoded.len());
        let to_decode = &encoded[..end];
        let decoded = percent_encoding::percent_decode_str(to_decode).decode_utf8_lossy();
        return decoded.to_string();
    }
    if raw_href.starts_with("//") {
        return format!("https:{raw_href}");
    }
    raw_href.to_string()
}

/// Decodes common HTML entities into UTF-8 characters.
pub fn decode_html_entities(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&copy;", "©")
        .replace("&reg;", "®")
        .replace("&euro;", "€")
        .replace("&pound;", "£")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&amp;", "&") // must be last
}

/// Strips tags matching `<tag ...>...</tag>`.
fn strip_tag_block(html: &str, tag_name: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let open_pattern = format!("<{tag_name}");
    let close_pattern = format!("</{tag_name}>");

    let mut remaining = html;
    while let Some(start) = remaining.to_lowercase().find(&open_pattern) {
        result.push_str(&remaining[..start]);
        let after_open = &remaining[start..];
        if let Some(end) = after_open.to_lowercase().find(&close_pattern) {
            remaining = &after_open[end + close_pattern.len()..];
        } else {
            // Unclosed tag block, truncate rest
            remaining = "";
            break;
        }
    }
    result.push_str(remaining);
    result
}

/// Strips all `<tag>` and `</tag>` from text.
pub fn strip_html_tags(html: &str) -> String {
    let mut clean = String::with_capacity(html.len());
    let mut inside = false;
    for ch in html.chars() {
        if ch == '<' {
            inside = true;
        } else if ch == '>' {
            inside = false;
        } else if !inside {
            clean.push(ch);
        }
    }
    clean
}

/// Converts HTML webpage content into clean, token-efficient Markdown.
pub fn html_to_clean_markdown(html: &str, max_chars: usize) -> (Option<String>, String) {
    // Extract title
    let title = {
        let lower = html.to_lowercase();
        if let Some(start) = lower.find("<title>") {
            if let Some(end) = lower[start..].find("</title>") {
                let raw_title = &html[start + 7..start + end];
                Some(decode_html_entities(raw_title).trim().to_string())
            } else {
                None
            }
        } else {
            None
        }
    };

    // Remove non-content structural blocks
    let mut cleaned = html.to_string();
    for tag in &["script", "style", "noscript", "svg", "nav", "footer", "header", "aside", "iframe"] {
        cleaned = strip_tag_block(&cleaned, tag);
    }

    // Convert headings
    for h in (1..=6).rev() {
        let open_tag = format!("<h{h}");
        let close_tag = format!("</h{h}>");
        let hash_prefix = "#".repeat(h);

        let mut next_cleaned = String::with_capacity(cleaned.len());
        let mut rem = cleaned.as_str();
        while let Some(start) = rem.to_lowercase().find(&open_tag) {
            next_cleaned.push_str(&rem[..start]);
            let after_open = &rem[start..];
            if let Some(close_open_tag) = after_open.find('>') {
                let inner_start = close_open_tag + 1;
                if let Some(end) = after_open[inner_start..].to_lowercase().find(&close_tag) {
                    let inner = &after_open[inner_start..inner_start + end];
                    let text = strip_html_tags(inner);
                    next_cleaned.push_str(&format!("\n\n{hash_prefix} {}\n\n", text.trim()));
                    rem = &after_open[inner_start + end + close_tag.len()..];
                    continue;
                }
            }
            next_cleaned.push_str(after_open);
            rem = "";
            break;
        }
        next_cleaned.push_str(rem);
        cleaned = next_cleaned;
    }

    // Convert list items
    let mut next_cleaned = String::with_capacity(cleaned.len());
    let mut rem = cleaned.as_str();
    while let Some(start) = rem.to_lowercase().find("<li") {
        next_cleaned.push_str(&rem[..start]);
        let after_open = &rem[start..];
        if let Some(close_open_tag) = after_open.find('>') {
            let inner_start = close_open_tag + 1;
            if let Some(end) = after_open[inner_start..].to_lowercase().find("</li>") {
                let inner = &after_open[inner_start..inner_start + end];
                let text = strip_html_tags(inner);
                next_cleaned.push_str(&format!("\n- {}\n", text.trim()));
                rem = &after_open[inner_start + end + 5..];
                continue;
            }
        }
        next_cleaned.push_str(after_open);
        rem = "";
        break;
    }
    next_cleaned.push_str(rem);
    cleaned = next_cleaned;

    // Convert paragraphs & linebreaks
    cleaned = cleaned.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    cleaned = cleaned.replace("<p>", "\n\n").replace("</p>", "\n\n");

    // Strip remaining tags
    cleaned = strip_html_tags(&cleaned);
    cleaned = decode_html_entities(&cleaned);

    // Normalize whitespace
    let mut lines = Vec::new();
    let mut empty_streak = 0;
    for line in cleaned.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            empty_streak += 1;
            if empty_streak <= 1 {
                lines.push("");
            }
        } else {
            empty_streak = 0;
            lines.push(trimmed);
        }
    }

    let mut markdown = lines.join("\n");
    if markdown.chars().count() > max_chars {
        let truncated: String = markdown.chars().take(max_chars).collect();
        markdown = format!("{truncated}\n\n[... content truncated to fit model context budget ...]");
    }

    (title, markdown)
}

/// Parses search results from DuckDuckGo HTML search page.
pub fn parse_duckduckgo_html(html: &str, limit: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let lower = html.to_lowercase();
    let mut offset = 0;

    // Look for results links
    while let Some(pos) = lower[offset..].find("class=\"result__snippet\"") {
        let snippet_idx = offset + pos;

        // Find the closest preceding result anchor
        let slice_before = &html[..snippet_idx];
        let title_and_url = if let Some(a_idx) = slice_before.rfind("class=\"result__a\"") {
            let tag_open = slice_before[..a_idx].rfind("<a ").unwrap_or(0);
            let tag_content = &slice_before[tag_open..];
            if let Some(tag_close) = tag_content.find('>') {
                let full_open_tag = &tag_content[..tag_close];
                let href = if let Some(h_start) = full_open_tag.find("href=\"") {
                    let h_val = &full_open_tag[h_start + 6..];
                    if let Some(h_end) = h_val.find('"') {
                        decode_duckduckgo_url(&h_val[..h_end])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let inner_start = tag_open + tag_close + 1;
                let title = if let Some(close_a) = html[inner_start..snippet_idx].find("</a>") {
                    decode_html_entities(&strip_html_tags(&html[inner_start..inner_start + close_a]))
                        .trim()
                        .to_string()
                } else {
                    "Search Result".to_string()
                };

                Some((title, href))
            } else {
                None
            }
        } else {
            None
        };

        // Extract snippet text
        let snippet = if let Some(snip_open_end) = html[snippet_idx..].find('>') {
            let snip_start = snippet_idx + snip_open_end + 1;
            if let Some(snip_close) = html[snip_start..].find("</a>") {
                decode_html_entities(&strip_html_tags(&html[snip_start..snip_start + snip_close]))
                    .trim()
                    .to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if let Some((title, url)) = title_and_url {
            if !url.is_empty() && !title.is_empty() {
                results.push(SearchResult { title, url, snippet });
                if results.len() >= limit {
                    break;
                }
            }
        }

        offset = snippet_idx + 24;
    }

    results
}

/// Fallback search parser for DuckDuckGo Instant Answer JSON API.
pub fn parse_duckduckgo_json_api(json_str: &str, limit: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let val: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return results,
    };

    // Primary abstract
    if let (Some(heading), Some(abstract_text), Some(abstract_url)) = (
        val["Heading"].as_str(),
        val["AbstractText"].as_str(),
        val["AbstractURL"].as_str(),
    ) {
        if !abstract_text.is_empty() && !abstract_url.is_empty() {
            results.push(SearchResult {
                title: heading.to_string(),
                url: abstract_url.to_string(),
                snippet: abstract_text.to_string(),
            });
        }
    }

    // Related topics
    if let Some(topics) = val["RelatedTopics"].as_array() {
        for topic in topics {
            if results.len() >= limit {
                break;
            }
            if let (Some(text), Some(url)) = (topic["Text"].as_str(), topic["FirstURL"].as_str()) {
                if !text.is_empty() && !url.is_empty() {
                    let title = text.split(" - ").next().unwrap_or(text).to_string();
                    results.push(SearchResult {
                        title,
                        url: url.to_string(),
                        snippet: text.to_string(),
                    });
                }
            }
        }
    }

    results
}

/// Performs a live web search using DuckDuckGo HTML search with JSON fallback.
pub async fn search_web(query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(DEFAULT_USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    // 1. Try DuckDuckGo HTML search
    let encoded_q = percent_encoding::utf8_percent_encode(trimmed, percent_encoding::NON_ALPHANUMERIC).to_string();
    let html_url = format!("https://html.duckduckgo.com/html/?q={encoded_q}");

    if let Ok(resp) = client.get(&html_url).send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.text().await {
                let parsed = parse_duckduckgo_html(&body, limit);
                if !parsed.is_empty() {
                    return Ok(parsed);
                }
            }
        }
    }

    // 2. Fallback to DuckDuckGo Instant Answer API
    let api_url = format!("https://api.duckduckgo.com/?q={encoded_q}&format=json&no_html=1&skip_disambig=1");
    let resp = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| format!("Web search request failed: {e}"))?;

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read search response: {e}"))?;

    let results = parse_duckduckgo_json_api(&body, limit);
    Ok(results)
}

/// Fetches any web URL and converts its content into clean, readable Markdown.
pub async fn fetch_url(url_str: &str, max_chars: Option<usize>) -> Result<WebFetchResult, String> {
    let trimmed = url_str.trim();
    if trimmed.is_empty() {
        return Err("Target URL cannot be empty".to_string());
    }

    let parsed_url = if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        format!("https://{trimmed}")
    } else {
        trimmed.to_string()
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(DEFAULT_USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let resp = client
        .get(&parsed_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL {parsed_url}: {e}"))?;

    let status_code = resp.status().as_u16();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    let budget = max_chars.unwrap_or(15_000);
    let (title, content_markdown) = html_to_clean_markdown(&body, budget);
    let character_count = content_markdown.chars().count();

    Ok(WebFetchResult {
        url: parsed_url,
        title,
        content_markdown,
        character_count,
        status_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_duckduckgo_url() {
        let raw = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.nvidia.com%2Fen-us%2F&rut=123";
        let decoded = decode_duckduckgo_url(raw);
        assert_eq!(decoded, "https://www.nvidia.com/en-us/");
    }

    #[test]
    fn test_decode_html_entities() {
        let raw = "NVIDIA &amp; AI &quot;Revolution&quot; &mdash; 2026 &#39;Breakthrough&#39;";
        let decoded = decode_html_entities(raw);
        assert_eq!(decoded, "NVIDIA & AI \"Revolution\" — 2026 'Breakthrough'");
    }

    #[test]
    fn test_html_to_clean_markdown() {
        let html = r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>Kronos AI Documentation</title>
                    <style>body { color: red; }</style>
                    <script>console.log("secret");</script>
                </head>
                <body>
                    <nav><a href="/home">Home</a></nav>
                    <h1>Introduction to Kronos</h1>
                    <p>Kronos is a <b>high-performance</b> AI coding engine.</p>
                    <h2>Key Features</h2>
                    <ul>
                        <li>Autonomous Agent Runner</li>
                        <li>Ripgrep Fast Search</li>
                    </ul>
                    <footer>Copyright 2026</footer>
                </body>
            </html>
        "#;

        let (title, markdown) = html_to_clean_markdown(html, 5000);
        assert_eq!(title.as_deref(), Some("Kronos AI Documentation"));
        assert!(markdown.contains("# Introduction to Kronos"));
        assert!(markdown.contains("## Key Features"));
        assert!(markdown.contains("- Autonomous Agent Runner"));
        assert!(markdown.contains("- Ripgrep Fast Search"));
        assert!(!markdown.contains("console.log"));
        assert!(!markdown.contains("color: red"));
        assert!(!markdown.contains("Copyright 2026"));
    }

    #[test]
    fn test_parse_duckduckgo_html_mock() {
        let mock_html = r#"
            <div class="result results_links results_links_deep web-result ">
                <h2 class="result__title">
                    <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Freact.dev%2F&rut=1">React Official Docs</a>
                </h2>
                <a class="result__snippet" href="...">The library for web and native user interfaces. Build user interfaces out of individual pieces called components.</a>
            </div>
        "#;

        let results = parse_duckduckgo_html(mock_html, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "React Official Docs");
        assert_eq!(results[0].url, "https://react.dev/");
        assert!(results[0].snippet.contains("The library for web and native user interfaces"));
    }

    #[test]
    fn test_parse_duckduckgo_json_api() {
        let mock_json = r#"{
            "Heading": "Rust (programming language)",
            "AbstractText": "Rust is a multi-paradigm, general-purpose programming language designed for performance and safety.",
            "AbstractURL": "https://en.wikipedia.org/wiki/Rust_(programming_language)",
            "RelatedTopics": [
                {
                    "FirstURL": "https://www.rust-lang.org/",
                    "Text": "Rust Programming Language Official Website"
                }
            ]
        }"#;

        let results = parse_duckduckgo_json_api(mock_json, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Rust (programming language)");
        assert_eq!(results[0].url, "https://en.wikipedia.org/wiki/Rust_(programming_language)");
        assert_eq!(results[1].url, "https://www.rust-lang.org/");
    }
}
