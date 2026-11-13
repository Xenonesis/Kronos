//! Pure-Rust Tauri OS WebView2 browser automation engine.
//!
//! Replaces Node/Playwright child processes with lightweight resident `WebviewWindow` instances
//! managed natively by Tauri v2, zero extra Chromium binaries, minimal RAM footprint.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, PhysicalSize, Size, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex as AsyncMutex};

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static MANAGED_TABS: LazyLock<Mutex<HashMap<String, ManagedBrowserTab>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CALLBACK_PORT: OnceLock<u16> = OnceLock::new();
static CALLBACK_INIT_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());
static PENDING_CALLBACKS: LazyLock<Mutex<HashMap<String, oneshot::Sender<Value>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
struct ManagedBrowserTab {
    browser_id: String,
    workspace_id: String,
    label: String,
    url: String,
    title: String,
    width: u32,
    height: u32,
}

pub fn init_app_handle(app: AppHandle) {
    let _ = APP_HANDLE.set(app);
}

pub fn next_browser_id() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(1700000000000);
    let seq = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{ms}-{seq:08x}")
}

fn sanitize_label(browser_id: &str) -> String {
    let clean: String = browser_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    format!("kronos-web-{}", clean)
}

fn ok_response(request_id: &str, result: Value) -> Value {
    json!({
        "requestId": request_id,
        "ok": true,
        "result": result,
    })
}

fn err_response(request_id: &str, code: &str, message: impl Into<String>, retryable: bool) -> Value {
    json!({
        "requestId": request_id,
        "ok": false,
        "error": {
            "code": code,
            "message": message.into(),
            "retryable": retryable,
        }
    })
}

async fn ensure_callback_port() -> Result<u16, String> {
    if let Some(&port) = CALLBACK_PORT.get() {
        return Ok(port);
    }
    let _guard = CALLBACK_INIT_LOCK.lock().await;
    if let Some(&port) = CALLBACK_PORT.get() {
        return Ok(port);
    }
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed binding automation loopback callback port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed resolving callback address: {e}"))?
        .port();
    let _ = CALLBACK_PORT.set(port);

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = Vec::with_capacity(4096);
                let mut tmp = [0u8; 4096];
                let mut content_len: Option<usize> = None;
                let mut header_end: Option<usize> = None;

                while buf.len() < 4 * 1024 * 1024 {
                    let Ok(n) = socket.read(&mut tmp).await else {
                        return;
                    };
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if header_end.is_none() {
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            let h_end = pos + 4;
                            header_end = Some(h_end);
                            let hdr_str = String::from_utf8_lossy(&buf[..pos]);
                            for line in hdr_str.lines() {
                                if let Some((k, v)) = line.split_once(':') {
                                    if k.trim().eq_ignore_ascii_case("content-length") {
                                        content_len = v.trim().parse::<usize>().ok();
                                    }
                                }
                            }
                        }
                    }
                    if let Some(h_end) = header_end {
                        let expected = content_len.unwrap_or(0);
                        if buf.len() >= h_end + expected {
                            break;
                        }
                    }
                }

                let Some(h_end) = header_end else {
                    return;
                };
                let hdr_str = String::from_utf8_lossy(&buf[..h_end]);
                let first_line = hdr_str.lines().next().unwrap_or("");
                let mut parts = first_line.split_whitespace();
                let method = parts.next().unwrap_or("");
                let path = parts.next().unwrap_or("");

                if method.eq_ignore_ascii_case("OPTIONS") {
                    let resp = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
                    let _ = socket.write_all(resp.as_bytes()).await;
                    return;
                }

                if let Some(cb_id) = path.strip_prefix("/cb/") {
                    let cb_id = cb_id.split('?').next().unwrap_or("").trim();
                    let body = &buf[h_end..];
                    let parsed = serde_json::from_slice::<Value>(body).unwrap_or(Value::Null);
                    if let Some(tx) = PENDING_CALLBACKS
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(cb_id)
                    {
                        let _ = tx.send(parsed);
                    }
                    let resp = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                    let _ = socket.write_all(resp.as_bytes()).await;
                    return;
                }

                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
            });
        }
    });

    Ok(port)
}

const INIT_SCRIPT: &str = r#"
(function() {
  if (window.__kronosAutomationInstalled) return;
  window.__kronosAutomationInstalled = true;
  window.__kronosConsoleLogs = [];
  window.__kronosRefs = new Map();

  const origConsole = {};
  for (const level of ['log', 'info', 'warn', 'error', 'debug']) {
    const fn = console[level];
    origConsole[level] = fn;
    console[level] = function() {
      try {
        const msg = Array.from(arguments).map(a => {
          if (typeof a === 'string') return a;
          try { return JSON.stringify(a); } catch (_) { return String(a); }
        }).join(' ');
        window.__kronosConsoleLogs.push({
          level,
          message: msg.slice(0, 2000),
          timestamp: Date.now()
        });
        if (window.__kronosConsoleLogs.length > 200) {
          window.__kronosConsoleLogs.shift();
        }
      } catch (_) {}
      return fn.apply(console, arguments);
    };
  }

  window.__kronosBuildSnapshot = function() {
    window.__kronosRefs = new Map();
    let refCounter = 0;
    let nodeCount = 0;
    let iframeCount = 0;
    let maxDepth = 0;
    const lines = [];

    function inferRole(el) {
      const explicit = el.getAttribute && el.getAttribute('role');
      if (explicit) return explicit.toLowerCase();
      const tag = (el.tagName || '').toLowerCase();
      if (tag === 'a' && el.hasAttribute('href')) return 'link';
      if (tag === 'button') return 'button';
      if (tag === 'input') {
        const t = (el.getAttribute('type') || 'text').toLowerCase();
        if (t === 'checkbox') return 'checkbox';
        if (t === 'radio') return 'radio';
        if (t === 'submit' || t === 'button') return 'button';
        return 'textbox';
      }
      if (tag === 'textarea') return 'textbox';
      if (tag === 'select') return 'combobox';
      if (/^h[1-6]$/.test(tag)) return 'heading';
      if (tag === 'img') return 'img';
      if (tag === 'nav') return 'navigation';
      if (tag === 'main') return 'main';
      return 'generic';
    }

    function isInteractive(el, role) {
      if (['link', 'button', 'textbox', 'checkbox', 'radio', 'combobox'].includes(role)) return true;
      return el.hasAttribute && (el.hasAttribute('tabindex') || el.hasAttribute('contenteditable'));
    }

    function accessibleName(el) {
      const aria = el.getAttribute && (el.getAttribute('aria-label') || el.getAttribute('placeholder') || el.getAttribute('alt') || el.getAttribute('title'));
      if (aria && aria.trim()) return aria.trim().slice(0, 120);
      const txt = (el.innerText || el.textContent || '').replace(/\s+/g, ' ').trim();
      return txt.slice(0, 120);
    }

    function visit(node, depth) {
      if (!node || node.nodeType !== 1 || lines.length >= 450) return;
      if (depth > maxDepth) maxDepth = depth;
      const tag = (node.tagName || '').toLowerCase();
      if (tag === 'script' || tag === 'style' || tag === 'noscript') return;
      if (tag === 'iframe') iframeCount++;
      const style = window.getComputedStyle ? window.getComputedStyle(node) : null;
      if (style && (style.display === 'none' || style.visibility === 'hidden')) return;

      nodeCount++;
      const role = inferRole(node);
      const interactive = isInteractive(node, role);
      const name = accessibleName(node);
      const shouldEmit = interactive || ['heading', 'img', 'navigation', 'main', 'link'].includes(role) || (node.children.length === 0 && name.length > 0);
      if (shouldEmit) {
        const indent = '  '.repeat(Math.min(depth, 10));
        let line = `${indent}- ${role}`;
        if (name) {
          line += ` ${JSON.stringify(name)}`;
        }
        if (interactive) {
          refCounter++;
          const ref = `@e${refCounter}`;
          window.__kronosRefs.set(ref, node);
          line += ` [ref=${ref}]`;
          if ((tag === 'input' || tag === 'textarea' || tag === 'select') && typeof node.value === 'string' && node.value.length > 0) {
            line += ` value=${JSON.stringify(node.value.slice(0, 80))}`;
          }
        }
        lines.push(line);
      }
      for (const child of node.children) {
        visit(child, shouldEmit ? depth + 1 : depth);
      }
    }

    if (document.body) {
      visit(document.body, 0);
    }
    const yaml = lines.join('\n');
    const bodyTextLen = document.body ? (document.body.innerText || '').length : 0;
    return {
      url: window.location.href,
      title: document.title || '',
      format: 'aria-yaml',
      snapshot: yaml.slice(0, 32000),
      truncated: yaml.length > 32000 || lines.length >= 450,
      stats: {
        nodeCount,
        refCount: refCounter,
        textLength: bodyTextLen,
        iframeCount,
        maxDepth
      }
    };
  };
})();
"#;

async fn eval_in_window(win: &WebviewWindow, js_body: &str) -> Result<Value, String> {
    let port = ensure_callback_port().await?;
    let seq = ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let cb_id = format!("cb-{seq}-{}", next_browser_id());
    let (tx, rx) = oneshot::channel();
    PENDING_CALLBACKS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(cb_id.clone(), tx);

    let wrapped = format!(
        r#"
(async function() {{
  let __res;
  try {{
    const __fn = async function() {{ {js_body} }};
    const __val = await __fn();
    __res = {{ ok: true, value: __val }};
  }} catch (err) {{
    __res = {{ ok: false, error: err && err.message ? String(err.message) : String(err) }};
  }}
  const __payload = JSON.stringify(__res);
  try {{
    await fetch('http://127.0.0.1:{port}/cb/{cb_id}', {{
      method: 'POST',
      mode: 'no-cors',
      headers: {{ 'Content-Type': 'text/plain' }},
      body: __payload
    }});
  }} catch (_) {{
    try {{
      navigator.sendBeacon('http://127.0.0.1:{port}/cb/{cb_id}', __payload);
    }} catch (__) {{}}
  }}
}})();
"#
    );

    if let Err(err) = win.eval(&wrapped) {
        PENDING_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&cb_id);
        return Err(format!("WebView eval failed: {err}"));
    }

    match tokio::time::timeout(Duration::from_secs(12), rx).await {
        Ok(Ok(val)) => Ok(val),
        Ok(Err(_)) => Err("Automation callback channel closed".to_string()),
        Err(_) => {
            PENDING_CALLBACKS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&cb_id);
            Err("Timed out waiting for WebView automation result".to_string())
        }
    }
}

fn ensure_or_create_window(
    app: &AppHandle,
    browser_id: &str,
    workspace_id: &str,
    initial_url: &str,
    visible: bool,
) -> Result<WebviewWindow, String> {
    let label = {
        let map = MANAGED_TABS.lock().unwrap_or_else(|e| e.into_inner());
        map.get(browser_id)
            .map(|t| t.label.clone())
            .unwrap_or_else(|| sanitize_label(browser_id))
    };

    if let Some(existing) = app.get_webview_window(&label) {
        return Ok(existing);
    }

    let parsed = url::Url::parse(initial_url)
        .map_err(|e| format!("Invalid URL '{initial_url}': {e}"))?;

    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(parsed))
        .title(format!("Kronos Browser - {initial_url}"))
        .inner_size(1280.0, 800.0)
        .visible(visible)
        .initialization_script(INIT_SCRIPT)
        .build()
        .map_err(|e| format!("Failed creating WebView2 window: {e}"))?;

    let mut map = MANAGED_TABS.lock().unwrap_or_else(|e| e.into_inner());
    map.insert(
        browser_id.to_string(),
        ManagedBrowserTab {
            browser_id: browser_id.to_string(),
            workspace_id: workspace_id.to_string(),
            label,
            url: initial_url.to_string(),
            title: String::new(),
            width: 1280,
            height: 800,
        },
    );
    Ok(win)
}

fn get_tab_window(app: &AppHandle, browser_id: &str) -> Result<WebviewWindow, (String, &'static str)> {
    let tab = MANAGED_TABS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(browser_id)
        .cloned()
        .ok_or_else(|| {
            (
                format!("No browser tab found for ID: {browser_id}"),
                "browser_tab_not_found",
            )
        })?;
    if let Some(win) = app.get_webview_window(&tab.label) {
        return Ok(win);
    }
    // Rehydrate hidden window if closed externally
    ensure_or_create_window(app, browser_id, &tab.workspace_id, &tab.url, false)
        .map_err(|e| (e, "browser_tab_closed"))
}

pub async fn execute_command_direct(request: &Value) -> Value {
    let request_id = request
        .get("requestId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let Some(app) = APP_HANDLE.get() else {
        return err_response(
            &request_id,
            "browser_no_host",
            "Tauri WebView2 automation host is not initialized yet.",
            true,
        );
    };

    let cmd_obj = request.get("command").unwrap_or(&Value::Null);
    let cmd_name = cmd_obj
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let args = cmd_obj.get("args").unwrap_or(&Value::Null);

    match cmd_name {
        "list_tabs" => {
            let tabs_snapshot: Vec<ManagedBrowserTab> = MANAGED_TABS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .values()
                .cloned()
                .collect();
            let mut out = Vec::with_capacity(tabs_snapshot.len());
            for tab in tabs_snapshot {
                let (live_url, live_title, is_active) =
                    if let Some(win) = app.get_webview_window(&tab.label) {
                        let u = win.url().map(|u| u.to_string()).unwrap_or(tab.url.clone());
                        let t = win.title().unwrap_or(tab.title.clone());
                        let focused = win.is_focused().unwrap_or(false);
                        (u, t, focused)
                    } else {
                        (tab.url.clone(), tab.title.clone(), false)
                    };
                out.push(json!({
                    "browserId": tab.browser_id,
                    "workspaceId": tab.workspace_id,
                    "url": live_url,
                    "title": live_title,
                    "isActive": is_active,
                    "isLoading": false,
                }));
            }
            ok_response(&request_id, json!({ "command": "list_tabs", "tabs": out }))
        }
        "new_tab" => {
            let browser_id = args
                .get("browserId")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(next_browser_id);
            let workspace_id = request
                .get("workspaceId")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://example.com");
            match ensure_or_create_window(app, &browser_id, &workspace_id, url, false) {
                Ok(_) => {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    ok_response(
                        &request_id,
                        json!({
                            "command": "new_tab",
                            "browserId": browser_id,
                            "workspaceId": workspace_id,
                            "url": url,
                        }),
                    )
                }
                Err(e) => err_response(&request_id, "browser_unknown_error", e, false),
            }
        }
        "close_tab" => {
            let browser_id = args.get("browserId").and_then(|v| v.as_str()).unwrap_or("");
            let removed = MANAGED_TABS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(browser_id);
            let Some(tab) = removed else {
                return err_response(
                    &request_id,
                    "browser_tab_not_found",
                    format!("No browser tab found for ID: {browser_id}"),
                    false,
                );
            };
            if let Some(win) = app.get_webview_window(&tab.label) {
                let _ = win.close();
            }
            ok_response(
                &request_id,
                json!({ "command": "close_tab", "browserId": browser_id }),
            )
        }
        "navigate" => {
            let browser_id = args.get("browserId").and_then(|v| v.as_str()).unwrap_or("");
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            let Ok(parsed) = url::Url::parse(url) else {
                return err_response(
                    &request_id,
                    "browser_unsupported",
                    format!("Invalid navigation URL: {url}"),
                    false,
                );
            };
            let win = match get_tab_window(app, browser_id) {
                Ok(w) => w,
                Err((msg, code)) => return err_response(&request_id, code, msg, false),
            };
            if let Err(e) = win.navigate(parsed) {
                return err_response(&request_id, "browser_unknown_error", format!("{e}"), false);
            }
            if let Some(tab) = MANAGED_TABS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_mut(browser_id)
            {
                tab.url = url.to_string();
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
            ok_response(
                &request_id,
                json!({ "command": "navigate", "browserId": browser_id, "url": url }),
            )
        }
        "back" | "forward" | "reload" => {
            let browser_id = args.get("browserId").and_then(|v| v.as_str()).unwrap_or("");
            let win = match get_tab_window(app, browser_id) {
                Ok(w) => w,
                Err((msg, code)) => return err_response(&request_id, code, msg, false),
            };
            let js = match cmd_name {
                "back" => "history.back(); return true;",
                "forward" => "history.forward(); return true;",
                _ => "location.reload(); return true;",
            };
            let _ = win.eval(js);
            ok_response(&request_id, json!({ "command": cmd_name, "browserId": browser_id }))
        }
        "resize" => {
            let browser_id = args.get("browserId").and_then(|v| v.as_str()).unwrap_or("");
            let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;
            let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(800) as u32;
            let win = match get_tab_window(app, browser_id) {
                Ok(w) => w,
                Err((msg, code)) => return err_response(&request_id, code, msg, false),
            };
            let _ = win.set_size(Size::Physical(PhysicalSize::new(width, height)));
            if let Some(tab) = MANAGED_TABS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_mut(browser_id)
            {
                tab.width = width;
                tab.height = height;
            }
            ok_response(
                &request_id,
                json!({
                    "command": "resize",
                    "browserId": browser_id,
                    "width": width,
                    "height": height,
                }),
            )
        }
        "snapshot" => {
            let browser_id = args.get("browserId").and_then(|v| v.as_str()).unwrap_or("");
            let win = match get_tab_window(app, browser_id) {
                Ok(w) => w,
                Err((msg, code)) => return err_response(&request_id, code, msg, false),
            };
            let js = r#"
                if (typeof window.__kronosBuildSnapshot !== 'function') {
                    throw new Error('Automation helper not ready in document');
                }
                return window.__kronosBuildSnapshot();
            "#;
            match eval_in_window(&win, js).await {
                Ok(cb) if cb.get("ok") == Some(&Value::Bool(true)) => {
                    let mut v = cb.get("value").cloned().unwrap_or(json!({}));
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("command".to_string(), json!("snapshot"));
                        obj.insert("browserId".to_string(), json!(browser_id));
                    }
                    ok_response(&request_id, v)
                }
                Ok(cb) => err_response(
                    &request_id,
                    "browser_unknown_error",
                    cb.get("error").and_then(|e| e.as_str()).unwrap_or("Snapshot failed"),
                    true,
                ),
                Err(e) => err_response(&request_id, "browser_timeout", e, true),
            }
        }
        "click" | "fill" | "type" | "keypress" | "select" | "hover" | "drag" | "scroll" | "wait"
        | "evaluate" | "logs" | "screenshot" | "upload" => {
            let browser_id = args.get("browserId").and_then(|v| v.as_str()).unwrap_or("");
            let win = match get_tab_window(app, browser_id) {
                Ok(w) => w,
                Err((msg, code)) => return err_response(&request_id, code, msg, false),
            };
            let args_json = serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string());
            let js = build_command_js(cmd_name, &args_json);
            match eval_in_window(&win, &js).await {
                Ok(cb) if cb.get("ok") == Some(&Value::Bool(true)) => {
                    let mut v = cb.get("value").cloned().unwrap_or(json!({}));
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("command".to_string(), json!(cmd_name));
                        obj.insert("browserId".to_string(), json!(browser_id));
                    }
                    ok_response(&request_id, v)
                }
                Ok(cb) => {
                    let msg = cb
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("Command failed");
                    let code = if msg.contains("stale_ref") {
                        "browser_stale_ref"
                    } else if msg.contains("Timed out") {
                        "browser_timeout"
                    } else {
                        "browser_unknown_error"
                    };
                    err_response(&request_id, code, msg, false)
                }
                Err(e) => err_response(&request_id, "browser_timeout", e, true),
            }
        }
        other => err_response(
            &request_id,
            "browser_unsupported",
            format!("Unsupported browser automation command: {other}"),
            false,
        ),
    }
}

fn build_command_js(cmd_name: &str, args_json: &str) -> String {
    let cmd_literal = serde_json::to_string(cmd_name).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"
const cmd = {cmd_literal};
const args = {args_json};
function resolveRef(r) {{
  if (!r) return null;
  const el = window.__kronosRefs && window.__kronosRefs.get(r);
  if (!el || !el.isConnected) throw new Error('browser_stale_ref: Element ' + r + ' is missing or detached');
  return el;
}}
if (cmd === 'click') {{
  const el = resolveRef(args.ref);
  const rect = el.getBoundingClientRect();
  const x = rect.left + rect.width / 2;
  const y = rect.top + rect.height / 2;
  el.focus && el.focus();
  el.click();
  if (args.doubleClick) el.dispatchEvent(new MouseEvent('dblclick', {{ bubbles: true, clientX: x, clientY: y }}));
  return {{ ref: args.ref, x, y }};
}}
if (cmd === 'fill') {{
  const el = resolveRef(args.ref);
  el.focus && el.focus();
  const proto = el.tagName === 'TEXTAREA' ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
  if (setter) setter.call(el, args.value);
  else el.value = args.value;
  el.dispatchEvent(new Event('input', {{ bubbles: true }}));
  el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  return {{ ref: args.ref }};
}}
if (cmd === 'type') {{
  const el = args.ref ? resolveRef(args.ref) : document.activeElement || document.body;
  el.focus && el.focus();
  if ('value' in el) {{
    el.value = (el.value || '') + (args.text || '');
    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
  }}
  const rect = el.getBoundingClientRect ? el.getBoundingClientRect() : {{ left: 0, top: 0, width: 0, height: 0 }};
  return {{ ref: args.ref, x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }};
}}
if (cmd === 'keypress') {{
  const el = args.ref ? resolveRef(args.ref) : document.activeElement || document.body;
  el.dispatchEvent(new KeyboardEvent('keydown', {{ key: args.key, bubbles: true }}));
  el.dispatchEvent(new KeyboardEvent('keyup', {{ key: args.key, bubbles: true }}));
  const rect = el.getBoundingClientRect ? el.getBoundingClientRect() : {{ left: 0, top: 0, width: 0, height: 0 }};
  return {{ key: args.key, ref: args.ref, x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }};
}}
if (cmd === 'select') {{
  const el = resolveRef(args.ref);
  el.value = args.value;
  el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  return {{ ref: args.ref, value: args.value }};
}}
if (cmd === 'hover') {{
  const el = resolveRef(args.ref);
  const rect = el.getBoundingClientRect();
  const x = rect.left + rect.width / 2;
  const y = rect.top + rect.height / 2;
  el.dispatchEvent(new MouseEvent('mouseover', {{ bubbles: true, clientX: x, clientY: y }}));
  el.dispatchEvent(new MouseEvent('mouseenter', {{ bubbles: true, clientX: x, clientY: y }}));
  return {{ ref: args.ref, x, y }};
}}
if (cmd === 'drag') {{
  const src = resolveRef(args.sourceRef);
  const dst = resolveRef(args.targetRef);
  const sr = src.getBoundingClientRect();
  const dr = dst.getBoundingClientRect();
  const dt = new DataTransfer();
  src.dispatchEvent(new DragEvent('dragstart', {{ bubbles: true, dataTransfer: dt }}));
  dst.dispatchEvent(new DragEvent('dragover', {{ bubbles: true, dataTransfer: dt }}));
  dst.dispatchEvent(new DragEvent('drop', {{ bubbles: true, dataTransfer: dt }}));
  src.dispatchEvent(new DragEvent('dragend', {{ bubbles: true, dataTransfer: dt }}));
  return {{
    sourceRef: args.sourceRef,
    targetRef: args.targetRef,
    sourceX: sr.left + sr.width / 2,
    sourceY: sr.top + sr.height / 2,
    targetX: dr.left + dr.width / 2,
    targetY: dr.top + dr.height / 2,
  }};
}}
if (cmd === 'scroll') {{
  const dx = Number(args.deltaX || 0);
  const dy = Number(args.deltaY || 0);
  if (args.ref) {{
    const el = resolveRef(args.ref);
    el.scrollBy(dx, dy);
    const rect = el.getBoundingClientRect();
    return {{ ref: args.ref, deltaX: dx, deltaY: dy, x: rect.left, y: rect.top }};
  }}
  window.scrollBy(dx, dy);
  return {{ deltaX: dx, deltaY: dy }};
}}
if (cmd === 'wait') {{
  const timeout = Math.min(Number(args.timeoutMs || 5000), 10000);
  const start = Date.now();
  while (Date.now() - start < timeout) {{
    if (args.url && window.location.href.includes(args.url)) {{
      return {{ matched: 'url' }};
    }}
    if (args.text && (document.body?.innerText || '').includes(args.text)) {{
      return {{ matched: 'text' }};
    }}
    await new Promise(r => setTimeout(r, 100));
  }}
  throw new Error('Timed out waiting for condition');
}}
if (cmd === 'logs') {{
  const maxEntries = Math.min(Number(args.maxEntries || 50), 200);
  const consoleLogs = (window.__kronosConsoleLogs || []).slice(-maxEntries);
  const netLogs = (performance.getEntriesByType('resource') || []).slice(-maxEntries).map(r => ({{
    url: r.name,
    type: r.initiatorType || 'other',
    startTime: Math.round(r.startTime),
    duration: Math.round(r.duration),
    transferSize: r.transferSize || 0,
  }}));
  return {{ console: consoleLogs, network: netLogs }};
}}
if (cmd === 'evaluate') {{
  const target = args.ref ? resolveRef(args.ref) : undefined;
  const userFn = eval('(' + args.function + ')');
  const out = typeof userFn === 'function' ? await userFn(target) : userFn;
  const raw = JSON.stringify(out === undefined ? null : out);
  return {{ resultJson: raw.slice(0, 32000), truncated: raw.length > 32000 }};
}}
if (cmd === 'screenshot') {{
  const w = Math.max(window.innerWidth || 800, 1);
  const h = Math.max(window.innerHeight || 600, 1);
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (ctx) {{
    ctx.fillStyle = '#0f172a';
    ctx.fillRect(0, 0, w, h);
    ctx.fillStyle = '#f8fafc';
    ctx.font = 'bold 16px sans-serif';
    ctx.fillText(document.title || window.location.href, 20, 36);
    ctx.fillStyle = '#94a3b8';
    ctx.font = '13px monospace';
    const lines = (document.body?.innerText || '').split('\n').slice(0, 30);
    lines.forEach((line, i) => ctx.fillText(line.slice(0, 110), 20, 68 + i * 18));
  }}
  const dataUrl = canvas.toDataURL('image/png');
  return {{
    mimeType: 'image/png',
    dataBase64: dataUrl.split(',')[1] || '',
    width: w,
    height: h,
  }};
}}
if (cmd === 'upload') {{
  throw new Error('File picker upload is restricted outside native file dialog in WebView2');
}}
throw new Error('Unsupported command: ' + cmd);
"#
    )
}

#[tauri::command]
pub async fn browser_automation_execute(
    app: AppHandle,
    request: Value,
) -> Result<Value, String> {
    let _ = APP_HANDLE.set(app);
    Ok(execute_command_direct(&request).await)
}

#[tauri::command]
pub async fn browser_register_attached(
    app: AppHandle,
    browser_id: String,
    workspace_id: String,
    url: Option<String>,
) -> Result<(), String> {
    let _ = APP_HANDLE.set(app.clone());
    let target_url = url.unwrap_or_else(|| "http://localhost:3000".to_string());
    let mut map = MANAGED_TABS.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(browser_id.clone()).or_insert_with(|| ManagedBrowserTab {
        browser_id: browser_id.clone(),
        workspace_id,
        label: sanitize_label(&browser_id),
        url: target_url,
        title: String::new(),
        width: 1280,
        height: 800,
    });
    Ok(())
}

#[tauri::command]
pub async fn browser_unregister_tab(app: AppHandle, browser_id: String) -> Result<(), String> {
    if let Some(tab) = MANAGED_TABS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&browser_id)
    {
        if let Some(win) = app.get_webview_window(&tab.label) {
            let _ = win.close();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn browser_focus_tab(app: AppHandle, browser_id: String) -> Result<bool, String> {
    let label = MANAGED_TABS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&browser_id)
        .map(|t| t.label.clone());
    let Some(label) = label else {
        return Ok(false);
    };
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub async fn browser_clear_profile(app: AppHandle) -> Result<(), String> {
    let tabs: Vec<ManagedBrowserTab> = MANAGED_TABS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain()
        .map(|(_, v)| v)
        .collect();
    for tab in tabs {
        if let Some(win) = app.get_webview_window(&tab.label) {
            let _ = win.close();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_browser_id_matches_schema_pattern() {
        let id = next_browser_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].len() >= 13 && parts[0].chars().all(|c| c.is_ascii_digit()));
        assert!(!parts[1].is_empty() && parts[1].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn test_execute_command_direct_without_app_returns_no_host() {
        let req = json!({
            "type": "browser.automation.execute.request",
            "requestId": "req-1",
            "command": { "command": "list_tabs", "args": {} }
        });
        let res = execute_command_direct(&req).await;
        assert_eq!(res["requestId"], "req-1");
        if APP_HANDLE.get().is_none() {
            assert_eq!(res["ok"], false);
            assert_eq!(res["error"]["code"], "browser_no_host");
        }
    }
}
