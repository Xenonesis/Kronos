use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use axum::{
    body::Body,
    extract::Query,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

static COOKIE_JAR: std::sync::OnceLock<Arc<RwLock<HashMap<String, String>>>> = std::sync::OnceLock::new();

fn get_cookie_jar() -> &'static Arc<RwLock<HashMap<String, String>>> {
    COOKIE_JAR.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

pub fn set_domain_cookies(host: &str, cookies: &str) {
    if let Ok(mut jar) = get_cookie_jar().write() {
        jar.insert(host.to_lowercase(), cookies.to_string());
    }
}

pub fn get_domain_cookies(host: &str) -> Option<String> {
    get_cookie_jar()
        .read()
        .ok()
        .and_then(|jar| jar.get(&host.to_lowercase()).cloned())
}

pub fn sanitize_proxy_headers(raw_headers: &reqwest::header::HeaderMap) -> HeaderMap {
    let mut clean = HeaderMap::new();

    for (key, val) in raw_headers {
        let key_str = key.as_str().to_lowercase();
        // Skip frame-blocking and report-only headers
        if key_str == "x-frame-options" || key_str == "content-security-policy-report-only" {
            continue;
        }

        if key_str == "content-security-policy" {
            if let Ok(csp_str) = val.to_str() {
                // Remove frame-ancestors directive
                let mut parts = Vec::new();
                for dir in csp_str.split(';') {
                    let trimmed = dir.trim();
                    if !trimmed.starts_with("frame-ancestors") && !trimmed.is_empty() {
                        parts.push(trimmed);
                    }
                }
                let rewritten = parts.join("; ");
                if let Ok(new_val) = HeaderValue::from_str(&rewritten) {
                    clean.insert(key.clone(), new_val);
                }
                continue;
            }
        }

        clean.insert(key.clone(), val.clone());
    }

    // Permissive CORS
    clean.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    clean.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS, HEAD"),
    );
    clean.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("*"));

    clean
}

pub fn inject_base_tag_into_html(html: &str, target_url: &str) -> String {
    let base_tag = format!("\n<base href=\"{target_url}\">");

    // Strip meta frame-busting tags
    let re_meta = regex_lite_case_insensitive_replace(html, "<meta[^>]+http-equiv=[\"']X-Frame-Options[\"'][^>]*>", "");

    // Inject <base href="..."> into <head>
    if let Some(pos) = re_meta.to_lowercase().find("<head") {
        if let Some(close_pos) = re_meta[pos..].find('>') {
            let insert_idx = pos + close_pos + 1;
            let mut res = String::with_capacity(re_meta.len() + base_tag.len() + 1);
            res.push_str(&re_meta[..insert_idx]);
            res.push_str(&base_tag);
            res.push_str(&re_meta[insert_idx..]);
            return res;
        }
    }

    format!("{base_tag}\n{re_meta}")
}

fn regex_lite_case_insensitive_replace(haystack: &str, _pat_desc: &str, _rep: &str) -> String {
    // Basic meta tag frame-busting stripper
    let lower = haystack.to_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut cur = 0;

    while let Some(pos) = lower[cur..].find("<meta") {
        let tag_start = cur + pos;
        if let Some(tag_end_offset) = lower[tag_start..].find('>') {
            let tag_end = tag_start + tag_end_offset + 1;
            let tag_slice = &lower[tag_start..tag_end];
            if tag_slice.contains("x-frame-options") {
                out.push_str(&haystack[cur..tag_start]);
                cur = tag_end;
                continue;
            }
        }
        out.push_str(&haystack[cur..=tag_start]);
        cur = tag_start + 1;
    }
    out.push_str(&haystack[cur..]);
    out
}

#[derive(Deserialize)]
pub struct ProxyQuery {
    pub url: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct CookieRequestBody {
    pub host: Option<String>,
    pub cookies: Option<String>,
}

pub async fn handle_cookies_endpoint(
    Json(body): Json<CookieRequestBody>,
) -> impl IntoResponse {
    if let (Some(host), Some(cookies)) = (body.host, body.cookies) {
        set_domain_cookies(&host, &cookies);
        let count = cookies.split(';').count();
        (StatusCode::OK, Json(serde_json::json!({ "ok": true, "host": host, "count": count })))
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Missing host or cookies in body" })),
        )
    }
}

pub async fn handle_browser_proxy(
    Query(query): Query<ProxyQuery>,
) -> Result<Response<Body>, (StatusCode, Json<serde_json::Value>)> {
    let raw_target = match &query.url {
        Some(u) if !u.trim().is_empty() => u.trim(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Missing required 'url' query parameter" })),
            ));
        }
    };

    let target_url = if raw_target.starts_with("http://") || raw_target.starts_with("https://") {
        raw_target.to_string()
    } else {
        format!("https://{raw_target}")
    };

    let parsed = reqwest::Url::parse(&target_url).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid URL parameter", "details": e.to_string() })),
        )
    })?;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to create HTTP client", "details": e.to_string() })),
            )
        })?;

    let mut req_builder = client.get(parsed.as_str());

    // Host and user-agent
    req_builder = req_builder.header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36");
    req_builder = req_builder.header(header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8");
    req_builder = req_builder.header(header::ACCEPT_ENCODING, "identity"); // Disable gzip/br to allow <base> injection

    // Attach custom cookies if present
    if let Some(host) = parsed.host_str() {
        if let Some(cookies) = get_domain_cookies(host) {
            req_builder = req_builder.header(header::COOKIE, cookies);
        }
    }

    let upstream_res = req_builder.send().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": "Failed to connect to target website",
                "details": e.to_string(),
                "url": target_url
            })),
        )
    })?;

    let status = upstream_res.status();

    // Handle redirects
    if status.is_redirection() {
        if let Some(loc) = upstream_res.headers().get(header::LOCATION) {
            if let Ok(loc_str) = loc.to_str() {
                let resolved = parsed.join(loc_str).unwrap_or_else(|_| parsed.clone());
                let encoded = percent_encoding::utf8_percent_encode(resolved.as_str(), percent_encoding::NON_ALPHANUMERIC);
                let redirect_url = format!("/api/browser-proxy?url={encoded}");
                let mut resp = Response::builder()
                    .status(StatusCode::FOUND)
                    .body(Body::empty())
                    .unwrap();
                resp.headers_mut().insert(header::LOCATION, HeaderValue::from_str(&redirect_url).unwrap());
                return Ok(resp);
            }
        }
    }

    let mut sanitized_headers = sanitize_proxy_headers(upstream_res.headers());
    let content_type = upstream_res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let is_html = content_type.contains("text/html");

    if is_html {
        let body_bytes = upstream_res.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed reading upstream HTML", "details": e.to_string() })),
            )
        })?;

        let raw_html = String::from_utf8_lossy(&body_bytes);
        let transformed = inject_base_tag_into_html(&raw_html, parsed.as_str());

        sanitized_headers.remove(header::CONTENT_LENGTH);
        sanitized_headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );

        let mut resp = Response::builder().status(status).body(Body::from(transformed)).unwrap();
        *resp.headers_mut() = sanitized_headers;
        Ok(resp)
    } else {
        let body_bytes = upstream_res.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed reading upstream bytes", "details": e.to_string() })),
            )
        })?;

        let mut resp = Response::builder().status(status).body(Body::from(body_bytes)).unwrap();
        *resp.headers_mut() = sanitized_headers;
        Ok(resp)
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/api/browser-proxy", get(handle_browser_proxy))
        .route("/api/browser-proxy/cookies", post(handle_cookies_endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_base_tag_into_html_with_head() {
        let html = "<html><head><title>Test</title></head><body>Hello</body></html>";
        let target = "https://example.com/sub/page.html";
        let injected = inject_base_tag_into_html(html, target);
        assert!(injected.contains("<base href=\"https://example.com/sub/page.html\">"));
        assert!(injected.find("<base").unwrap() > injected.find("<head").unwrap());
    }

    #[test]
    fn test_inject_base_tag_into_html_without_head() {
        let html = "<div>Fragment</div>";
        let target = "https://example.com/";
        let injected = inject_base_tag_into_html(html, target);
        assert!(injected.contains("<base href=\"https://example.com/\">"));
    }

    #[test]
    fn test_sanitize_proxy_headers_strips_frame_options() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            header::HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        );
        headers.insert(
            header::HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'self'; frame-ancestors 'none'; script-src 'self'"),
        );

        let sanitized = sanitize_proxy_headers(&headers);
        assert!(!sanitized.contains_key("x-frame-options"));
        let csp = sanitized.get("content-security-policy").unwrap().to_str().unwrap();
        assert!(!csp.contains("frame-ancestors"));
        assert!(csp.contains("default-src 'self'"));
        assert_eq!(sanitized.get("access-control-allow-origin").unwrap(), "*");
    }

    #[test]
    fn test_domain_cookies_storage() {
        set_domain_cookies("github.com", "user_session=abc123xyz; logged_in=yes");
        let retrieved = get_domain_cookies("github.com").expect("Cookies should exist");
        assert!(retrieved.contains("user_session=abc123xyz"));

        // Case-insensitive check
        let retrieved_upper = get_domain_cookies("GITHUB.COM").expect("Case insensitive match");
        assert_eq!(retrieved, retrieved_upper);
    }
}
