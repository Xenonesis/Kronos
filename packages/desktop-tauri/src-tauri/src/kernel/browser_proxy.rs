use axum::{
    body::Body,
    extract::Query,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

static COOKIE_JAR: std::sync::OnceLock<Arc<RwLock<HashMap<String, String>>>> =
    std::sync::OnceLock::new();

fn get_cookie_jar() -> &'static Arc<RwLock<HashMap<String, String>>> {
    COOKIE_JAR.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

// Navigation capabilities name one URL, never daemon RPC authority. Remote iframe
// scripts can see their URL, so the local credential must not be included there.
struct NavigationTicket {
    target: String,
    created: std::time::Instant,
    cached_response: Option<CachedProxyResponse>,
}

struct CachedProxyResponse {
    status: reqwest::StatusCode,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
}

static NAVIGATION_TICKETS: std::sync::LazyLock<
    std::sync::Mutex<HashMap<String, NavigationTicket>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

pub fn issue_navigation_url(target: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(target).map_err(|e| e.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Browser navigation requires an http(s) URL without embedded credentials".into());
    }
    let token = crate::kernel::auth::generate_local_credential().map_err(|e| e.to_string())?;
    let mut tickets = NAVIGATION_TICKETS
        .lock()
        .map_err(|_| "Navigation capability store unavailable")?;
    tickets.retain(|_, ticket| ticket.created.elapsed() < std::time::Duration::from_secs(1800));
    if tickets.len() >= 256 {
        if let Some(oldest) = tickets
            .iter()
            .min_by_key(|(_, ticket)| ticket.created)
            .map(|(key, _)| key.clone())
        {
            tickets.remove(&oldest);
        }
    }
    tickets.insert(
        token.clone(),
        NavigationTicket {
            target: target.to_string(),
            created: std::time::Instant::now(),
            cached_response: None,
        },
    );
    let encoded = percent_encoding::utf8_percent_encode(target, percent_encoding::NON_ALPHANUMERIC);
    Ok(format!("/api/browser-proxy?url={encoded}&ticket={token}"))
}

pub fn has_navigation_ticket(uri: &axum::http::Uri) -> bool {
    let Ok(Query(query)) = Query::<HashMap<String, String>>::try_from_uri(uri) else {
        return false;
    };
    let (Some(token), Some(url)) = (query.get("ticket"), query.get("url")) else {
        return false;
    };
    NAVIGATION_TICKETS
        .lock()
        .ok()
        .and_then(|tickets| {
            tickets.get(token).map(|ticket| {
                ticket.target == *url
                    && ticket.created.elapsed() < std::time::Duration::from_secs(1800)
            })
        })
        .unwrap_or(false)
}

fn cache_navigation_response(
    ticket_url: &str,
    response: CachedProxyResponse,
) -> Result<(), String> {
    let uri: axum::http::Uri = ticket_url.parse().map_err(|error| format!("{error}"))?;
    let Query(query) = Query::<HashMap<String, String>>::try_from_uri(&uri)
        .map_err(|error| format!("{error}"))?;
    let token = query.get("ticket").ok_or("Navigation ticket is missing")?;
    let mut tickets = NAVIGATION_TICKETS
        .lock()
        .map_err(|_| "Navigation capability store unavailable")?;
    let ticket = tickets.get_mut(token).ok_or("Navigation ticket expired")?;
    ticket.cached_response = Some(response);
    Ok(())
}

static PROXY_CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("Create browser HTTP client")
});

fn proxy_request(url: &reqwest::Url) -> reqwest::RequestBuilder {
    let mut request = PROXY_CLIENT.get(url.clone())
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .header(header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
        .header(header::ACCEPT_ENCODING, "identity");
    if let Some(cookies) = url.host_str().and_then(get_domain_cookies) {
        request = request.header(header::COOKIE, cookies);
    }
    request
}

#[derive(Deserialize)]
pub struct NavigationRequest {
    url: String,
}

// Only the authenticated operator endpoint may authorize redirect destinations.
// A page holding a GET ticket cannot mint a capability for a different site.
pub async fn handle_navigation_endpoint(
    Json(body): Json<NavigationRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    let mut target = reqwest::Url::parse(&body.url).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Invalid URL parameter: {}", error)
            })),
        )
    })?;

    for _ in 0..10 {
        if !matches!(target.scheme(), "http" | "https")
            || !target.username().is_empty()
            || target.password().is_some()
        {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Navigation requires an http(s) URL without embedded credentials"
                })),
            ));
        }
        let response = proxy_request(&target)
            .send()
            .await
            .map_err(|error| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": format!("Failed to connect to target website: {}", error)
                    })),
                )
            })?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "Redirect has no valid Location header"
                        }))
                    )
                })?;
            target = target
                .join(location)
                .map_err(|error| {
                    (
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({
                            "error": format!("Failed to resolve redirect: {}", error)
                        }))
                    )
                })?;
        } else {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await.map_err(|error| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": format!("Failed reading target response: {error}")
                    })),
                )
            })?;
            let url = issue_navigation_url(target.as_str()).map_err(|error| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({
                        "error": format!("Failed to issue navigation capability: {error}")
                    })),
                )
            })?;
            cache_navigation_response(
                &url,
                CachedProxyResponse {
                    status,
                    headers,
                    body: body.to_vec(),
                },
            )
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": error})),
                )
            })?;
            return Ok(Json(serde_json::json!({"url": url})));
        }
    }
    Err((
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "error": "Navigation exceeded ten redirects"
        })),
    ))
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
    clean.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    clean.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS, HEAD"),
    );
    clean.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("*"),
    );

    clean
}

pub fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let needle_bytes = needle.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    if needle_bytes.len() > haystack_bytes.len() {
        return None;
    }
    for i in 0..=(haystack_bytes.len() - needle_bytes.len()) {
        if haystack.is_char_boundary(i)
            && haystack_bytes[i..i + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes)
        {
            return Some(i);
        }
    }
    None
}

fn strip_x_frame_options_meta(haystack: &str) -> String {
    let mut out = String::with_capacity(haystack.len());
    let mut cur = 0;

    while cur < haystack.len() {
        if !haystack.is_char_boundary(cur) {
            cur += 1;
            continue;
        }

        let slice = &haystack[cur..];
        if let Some(pos) = find_ascii_case_insensitive(slice, "<meta") {
            let tag_start = cur + pos;
            if let Some(gt_rel) = haystack[tag_start..].find('>') {
                let tag_end = tag_start + gt_rel + 1;
                let tag_content = &haystack[tag_start..tag_end];
                let is_x_frame_options = tag_content
                    .as_bytes()
                    .windows("x-frame-options".len())
                    .any(|w| w.eq_ignore_ascii_case(b"x-frame-options"));

                if is_x_frame_options {
                    out.push_str(&haystack[cur..tag_start]);
                    cur = tag_end;
                    continue;
                }
            }

            let next_cur = (tag_start + 5).min(haystack.len());
            let mut safe_next = next_cur;
            while safe_next < haystack.len() && !haystack.is_char_boundary(safe_next) {
                safe_next += 1;
            }
            out.push_str(&haystack[cur..safe_next]);
            cur = safe_next;
        } else {
            out.push_str(&haystack[cur..]);
            break;
        }
    }

    out
}

pub fn inject_base_tag_into_html(html: &str, target_url: &str) -> String {
    let base_tag = format!("\n<base href=\"{target_url}\">");
    let re_meta = strip_x_frame_options_meta(html);

    if let Some(pos) = find_ascii_case_insensitive(&re_meta, "<head") {
        if let Some(close_pos) = re_meta[pos..].find('>') {
            let insert_idx = pos + close_pos + 1;
            if re_meta.is_char_boundary(insert_idx) {
                let mut res = String::with_capacity(re_meta.len() + base_tag.len() + 1);
                res.push_str(&re_meta[..insert_idx]);
                res.push_str(&base_tag);
                res.push_str(&re_meta[insert_idx..]);
                return res;
            }
        }
    }

    format!("{base_tag}\n{re_meta}")
}

#[derive(Deserialize)]
pub struct ProxyQuery {
    pub url: Option<String>,
    pub ticket: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct CookieRequestBody {
    pub host: Option<String>,
    pub cookies: Option<String>,
}

pub async fn handle_cookies_endpoint(Json(body): Json<CookieRequestBody>) -> impl IntoResponse {
    if let (Some(host), Some(cookies)) = (body.host, body.cookies) {
        set_domain_cookies(&host, &cookies);
        let count = cookies.split(';').count();
        (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "host": host, "count": count })),
        )
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
    if let Some(token) = query.ticket.as_deref() {
        let cached = NAVIGATION_TICKETS
            .lock()
            .ok()
            .and_then(|mut tickets| {
                tickets
                    .get_mut(token)
                    .filter(|ticket| {
                        ticket.target == target_url
                            && ticket.created.elapsed() < std::time::Duration::from_secs(1800)
                    })
                    .and_then(|ticket| ticket.cached_response.take())
            });
        if let Some(cached) = cached {
            let mut headers = sanitize_proxy_headers(&cached.headers);
            let content_type = cached
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");
            let body = if content_type.contains("text/html") {
                headers.remove(header::CONTENT_LENGTH);
                headers.insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                );
                Body::from(inject_base_tag_into_html(
                    &String::from_utf8_lossy(&cached.body),
                    parsed.as_str(),
                ))
            } else {
                Body::from(cached.body)
            };
            let mut response = Response::builder()
                .status(cached.status.as_u16())
                .body(body)
                .unwrap();
            *response.headers_mut() = headers;
            return Ok(response);
        }
    }
    let req_builder = proxy_request(&parsed);

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
                // A visible URL ticket must not grant access to another host's
                // cookies. Stable redirects were resolved at authenticated issue.
                let redirect_url = resolved.as_str();
                let mut resp = Response::builder()
                    .status(StatusCode::FOUND)
                    .body(Body::empty())
                    .unwrap();
                resp.headers_mut().insert(
                    header::LOCATION,
                    HeaderValue::from_str(&redirect_url).unwrap(),
                );
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

        let mut resp = Response::builder()
            .status(status)
            .body(Body::from(transformed))
            .unwrap();
        *resp.headers_mut() = sanitized_headers;
        Ok(resp)
    } else {
        let body_bytes = upstream_res.bytes().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "Failed reading upstream bytes", "details": e.to_string() })),
            )
        })?;

        let mut resp = Response::builder()
            .status(status)
            .body(Body::from(body_bytes))
            .unwrap();
        *resp.headers_mut() = sanitized_headers;
        Ok(resp)
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/api/browser-proxy/navigation", post(handle_navigation_endpoint))
        .route("/api/browser-proxy", get(handle_browser_proxy))
        .route("/api/browser-proxy/cookies", post(handle_cookies_endpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_capability_cannot_authorize_another_url() {
        let issued = issue_navigation_url("https://example.test/page").unwrap();
        assert!(has_navigation_ticket(&issued.parse().unwrap()));
        let changed = issued.replace("example", "attacker");
        assert!(!has_navigation_ticket(&changed.parse().unwrap()));
        assert!(!has_navigation_ticket(
            &"/api/browser-proxy?url=https%3A%2F%2Fexample.test%2Fpage"
                .parse()
                .unwrap()
        ));
        assert!(issue_navigation_url("file:///secret").is_err());
        assert!(issue_navigation_url("https://user:password@example.test/").is_err());
    }

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
    fn test_inject_base_tag_multibyte_unicode_safety() {
        // Multi-byte characters (Hindi, emojis, Greek, Cyrillic) combined with meta frame-busting tags
        let html = "<html><head><meta http-equiv=\"X-Frame-Options\" content=\"DENY\"><title>नमस्ते 🚀 Welt Straße</title></head><body>परीक्षण 🌟</body></html>";
        let target = "https://example.com/unicode";
        let injected = inject_base_tag_into_html(html, target);
        assert!(injected.contains("<base href=\"https://example.com/unicode\">"));
        assert!(!injected.to_lowercase().contains("x-frame-options"));
        assert!(injected.contains("नमस्ते"));
        assert!(injected.contains("परीक्षण 🌟"));
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
            HeaderValue::from_static(
                "default-src 'self'; frame-ancestors 'none'; script-src 'self'",
            ),
        );

        let sanitized = sanitize_proxy_headers(&headers);
        assert!(!sanitized.contains_key("x-frame-options"));
        let csp = sanitized
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
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
