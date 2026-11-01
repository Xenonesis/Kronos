use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    routing::get,
    Router,
};
use tokio::sync::{broadcast, Mutex};
use crate::kernel::protocol::{RpcEnvelope, RpcResponse};

static MCP_HUB: std::sync::OnceLock<crate::kernel::mcp::McpHub> = std::sync::OnceLock::new();

pub fn get_mcp_hub() -> &'static crate::kernel::mcp::McpHub {
    MCP_HUB.get_or_init(crate::kernel::mcp::McpHub::new)
}

fn extract_jsonrpc_request_str(val: &serde_json::Value) -> Option<String> {
    if let Some(req) = val.get("request") {
        if let Some(s) = req.as_str() {
            return Some(s.to_string());
        } else if req.is_object() || req.is_array() {
            return serde_json::to_string(req).ok();
        }
    }
    if let Some(payload) = val.get("payload") {
        if let Some(s) = payload.as_str() {
            return Some(s.to_string());
        }
        if let Some(req) = payload.get("request") {
            if let Some(s) = req.as_str() {
                return Some(s.to_string());
            } else if req.is_object() || req.is_array() {
                return serde_json::to_string(req).ok();
            }
        }
        if payload.get("jsonrpc").is_some() {
            return serde_json::to_string(payload).ok();
        }
    }
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    if val.get("jsonrpc").is_some() {
        return serde_json::to_string(val).ok();
    }
    None
}

pub async fn health_check() -> axum::response::Response {
    use axum::response::IntoResponse;
    axum::Json(serde_json::json!({
        "status": "ok",
        "engine": "kronos-rust-kernel",
        "version": env!("CARGO_PKG_VERSION")
    })).into_response()
}

pub async fn version_check() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "kronos-rust-kernel"
    }))
}
pub async fn dispatch_envelope(envelope: RpcEnvelope) -> RpcResponse {
    match envelope.msg_type.as_str() {
        "ping" => RpcResponse {
            id: envelope.id,
            msg_type: "pong".to_string(),
            success: true,
            data: None,
            error: None,
        },
        "hello" => RpcResponse {
            id: envelope.id,
            msg_type: "server_info".to_string(),
            success: true,
            data: Some(serde_json::json!({
                "status": "server_info",
                "protocolVersion": 1,
                "serverId": "kronos-embedded-daemon",
                "hostname": "localhost",
                "version": env!("CARGO_PKG_VERSION"),
                "desktopManaged": true,
                "features": {
                    "connectedHosts": true,
                    "hostMultiplexing": true
                }
            })),
            error: None,
        },
        "directory_suggestions_request" | "workspace/suggest_directory" => {
            let query = envelope.payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let cwd_str = envelope.payload.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
            let include_files = envelope.payload.get("includeFiles").and_then(|v| v.as_bool()).unwrap_or(true);
            let include_dirs = envelope.payload.get("includeDirectories").and_then(|v| v.as_bool()).unwrap_or(false);
            let limit = envelope.payload.get("limit").and_then(|v| v.as_u64()).map(|l| l as usize).unwrap_or(50).min(100);

            let root_path = std::path::Path::new(cwd_str);
            let (directories, entries) = crate::kernel::search::suggest_directory_entries(
                root_path,
                query,
                include_files,
                include_dirs,
                limit,
            );

            RpcResponse::ok(&envelope.id, "directory_suggestions_response", Some(serde_json::json!({
                "directories": directories,
                "entries": entries,
                "error": null,
                "requestId": envelope.id
            })))
        }
        "mcp/request" | "mcp/jsonrpc" | "mcp_request" => {
            let req_str = extract_jsonrpc_request_str(&envelope.payload).unwrap_or_default();
            let hub = get_mcp_hub();
            match hub.handle_jsonrpc(&req_str) {
                Some(res) => RpcResponse::ok(
                    &envelope.id,
                    "mcp/response",
                    Some(serde_json::json!({ "response": res })),
                ),
                None => RpcResponse::ok(
                    &envelope.id,
                    "mcp/response",
                    Some(serde_json::json!({
                        "response": null,
                        "notification": true
                    })),
                ),
            }
        }
        other => RpcResponse::err(
            envelope.id,
            other,
            format!("Method '{}' not implemented", other),
        ),
    }
}

pub async fn dispatch_session_message(msg: &serde_json::Value) -> serde_json::Value {
    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let request_id = msg.get("requestId")
        .or_else(|| msg.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match msg_type {
        "kernel/telemetry" => {
            let tele = crate::kernel::telemetry::get_kernel_telemetry();
            serde_json::json!({
                "type": "kernel/telemetry/response",
                "requestId": request_id,
                "payload": tele
            })
        }
        "daemon/get_status" | "daemon_get_status" => {
            serde_json::json!({
                "type": "daemon/status",
                "requestId": request_id,
                "payload": {
                    "running": true,
                    "engine": "rust",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })
        }
        "file/inspect" => {
            let path = msg.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let p = std::path::Path::new(path);
            let exists = p.exists();
            let is_dir = p.is_dir();
            let byte_size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            serde_json::json!({
                "type": "file/inspect/response",
                "requestId": request_id,
                "payload": {
                    "path": path,
                    "exists": exists,
                    "isDir": is_dir,
                    "byteSize": byte_size
                }
            })
        }
        "git/status" => {
            let path = msg.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let summary = crate::kernel::git::GitEngine::status_summary(path).ok();
            serde_json::json!({
                "type": "git/status/response",
                "requestId": request_id,
                "payload": summary
            })
        }
        "directory_suggestions_request" => {
            let payload = msg.get("payload");
            let query = msg.get("query")
                .or_else(|| payload.and_then(|p| p.get("query")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let include_files = msg.get("includeFiles")
                .or_else(|| payload.and_then(|p| p.get("includeFiles")))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let include_dirs = msg.get("includeDirectories")
                .or_else(|| payload.and_then(|p| p.get("includeDirectories")))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let limit = msg.get("limit")
                .or_else(|| payload.and_then(|p| p.get("limit")))
                .and_then(|v| v.as_u64())
                .map(|l| l as usize)
                .unwrap_or(50)
                .min(100);

            let root_path = std::path::Path::new(cwd_str);
            let (directories, entries) = crate::kernel::search::suggest_directory_entries(
                root_path,
                query,
                include_files,
                include_dirs,
                limit,
            );

            serde_json::json!({
                "type": "directory_suggestions_response",
                "payload": {
                    "directories": directories,
                    "entries": entries,
                    "error": null,
                    "requestId": request_id
                }
            })
        }
        "mcp/request" | "mcp/jsonrpc" | "mcp_request" => {
            let req_str = extract_jsonrpc_request_str(msg).unwrap_or_default();
            let hub = get_mcp_hub();
            match hub.handle_jsonrpc(&req_str) {
                Some(res) => serde_json::json!({
                    "type": "mcp/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": {
                        "response": res
                    },
                    "response": res
                }),
                None => serde_json::json!({
                    "type": "mcp/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": {
                        "response": null,
                        "notification": true
                    },
                    "response": null,
                    "notification": true
                }),
            }
        }
        other => {
            serde_json::json!({
                "type": format!("{}/response", other),
                "requestId": request_id,
                "success": true,
                "payload": {}
            })
        }
    }
}

pub async fn process_incoming_frame(text: &str) -> Option<String> {
    let val: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(err) => {
            let err_resp = RpcResponse::err("unknown", "error", format!("Invalid JSON: {}", err));
            return serde_json::to_string(&err_resp).ok();
        }
    };

    // 1. Direct RPC envelope with explicit ID
    if val.get("id").is_some() && val.get("type").is_some() {
        if let Ok(envelope) = serde_json::from_value::<RpcEnvelope>(val.clone()) {
            let resp = dispatch_envelope(envelope).await;
            return serde_json::to_string(&resp).ok();
        }
    }

    // 2. Canonical WebSocket wire envelopes (ping, hello, session)
    if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
        match msg_type {
            "ping" => {
                return Some(r#"{"type":"pong"}"#.to_string());
            }
            "hello" => {
                let resp = crate::kernel::protocol::build_server_info_response();
                return serde_json::to_string(&resp).ok();
            }
            "session" => {
                if let Some(inner) = val.get("message") {
                    let res = dispatch_session_message(inner).await;
                    let wrapped = crate::kernel::protocol::wrap_session_outbound(res);
                    return serde_json::to_string(&wrapped).ok();
                }
            }
            "mcp/request" | "mcp/jsonrpc" | "mcp_request" => {
                let res = dispatch_session_message(&val).await;
                return serde_json::to_string(&res).ok();
            }
            _ => {}
        }
    }

    let err_resp = RpcResponse::err("unknown", "error", "Unsupported message format");
    serde_json::to_string(&err_resp).ok()
}

pub struct DaemonHandle {
    pub port: u16,
    shutdown_tx: broadcast::Sender<()>,
    server_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pipe_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

pub type EmbeddedDaemon = DaemonHandle;

impl DaemonHandle {
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_internal(6767, true, None).await
    }

    pub async fn start_test(port: u16) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_internal(port, false, None).await
    }

    pub async fn start_daemon(port: u16, custom_pipe: Option<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let is_production = port == 6767 && custom_pipe.is_none();
        Self::start_internal(port, is_production, custom_pipe).await
    }

    async fn start_internal(
        port: u16,
        is_production: bool,
        custom_pipe: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (shutdown_tx, _) = broadcast::channel(16);
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let actual_port = listener.local_addr()?.port();

        let app = Router::new()
            .route("/api/health", get(health_check))
            .route("/health", get(health_check))
            .route("/api/version", get(version_check))
            .route("/", get({
                let tx = shutdown_tx.clone();
                move |ws: WebSocketUpgrade| {
                    let rx = tx.subscribe();
                    async move {
                        ws.on_upgrade(move |socket| handle_ws(socket, rx))
                    }
                }
            }))
            .route("/ws", get({
                let tx = shutdown_tx.clone();
                move |ws: WebSocketUpgrade| {
                    let rx = tx.subscribe();
                    async move {
                        ws.on_upgrade(move |socket| handle_ws(socket, rx))
                    }
                }
            }))
            .fallback(get({
                let tx = shutdown_tx.clone();
                move |ws: WebSocketUpgrade| {
                    let rx = tx.subscribe();
                    async move {
                        ws.on_upgrade(move |socket| handle_ws(socket, rx))
                    }
                }
            }));

        let mut s_rx = shutdown_tx.subscribe();
        let server_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = s_rx.recv().await;
                })
                .await;
        });

        #[cfg(windows)]
        let pipe_task = if let Some(custom) = &custom_pipe {
            let p_rx = shutdown_tx.subscribe();
            let h = tokio::spawn(run_named_pipe(custom.clone(), p_rx));
            Some(h)
        } else if is_production {
            let p_rx = shutdown_tx.subscribe();
            let h = tokio::spawn(run_named_pipe(r"\\.\pipe\kronos-ipc".to_string(), p_rx));
            Some(h)
        } else {
            let p_rx = shutdown_tx.subscribe();
            let pipe_name = format!(r"\\.\pipe\kronos-ipc-{}", actual_port);
            let h = tokio::spawn(run_named_pipe(pipe_name, p_rx));
            Some(h)
        };

        #[cfg(not(windows))]
        let pipe_task = if let Some(custom) = &custom_pipe {
            let p_rx = shutdown_tx.subscribe();
            let h = tokio::spawn(run_unix_socket(custom.clone(), p_rx));
            Some(h)
        } else if is_production {
            let p_rx = shutdown_tx.subscribe();
            let h = tokio::spawn(run_unix_socket("/tmp/kronos-ipc.sock".to_string(), p_rx));
            Some(h)
        } else {
            let p_rx = shutdown_tx.subscribe();
            let sock_path = format!("/tmp/kronos-ipc-{}.sock", actual_port);
            let h = tokio::spawn(run_unix_socket(sock_path, p_rx));
            Some(h)
        };

        Ok(Self {
            port: actual_port,
            shutdown_tx,
            server_task: Arc::new(Mutex::new(Some(server_handle))),
            pipe_task: Arc::new(Mutex::new(pipe_task)),
        })
    }

    pub async fn stop(&self) {
        let _ = self.shutdown_tx.send(());

        let mut task_opt = self.server_task.lock().await;
        if let Some(task) = task_opt.take() {
            let _ = tokio::time::timeout(Duration::from_millis(1500), task).await;
        }

        let mut pipe_opt = self.pipe_task.lock().await;
        if let Some(task) = pipe_opt.take() {
            task.abort();
        }
    }
}

async fn handle_ws(mut socket: WebSocket, mut shutdown_rx: broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(reply) = process_incoming_frame(&text).await {
                            if socket.send(Message::Text(reply.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            if let Some(reply) = process_incoming_frame(text).await {
                                if socket.send(Message::Text(reply.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(windows)]
async fn run_named_pipe(pipe_name: String, mut shutdown_rx: broadcast::Receiver<()>) {
    use tokio::net::windows::named_pipe::ServerOptions;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let mut first = true;
    loop {
        let server_res = ServerOptions::new()
            .first_pipe_instance(first)
            .create(&pipe_name);

        let server = match server_res {
            Ok(s) => {
                first = false;
                s
            }
            Err(_) => {
                tokio::select! {
                    _ = shutdown_rx.recv() => break,
                    _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
                }
            }
        };

        tokio::select! {
            _ = shutdown_rx.recv() => {
                break;
            }
            conn = server.connect() => {
                if conn.is_err() {
                    continue;
                }
            }
        }

        let mut client_shutdown = shutdown_rx.resubscribe();
        tokio::spawn(async move {
            let (reader, mut writer) = tokio::io::split(server);
            let mut lines = BufReader::new(reader).lines();

            loop {
                tokio::select! {
                    _ = client_shutdown.recv() => {
                        break;
                    }
                    line = lines.next_line() => {
                        match line {
                            Ok(Some(l)) => {
                                let trimmed = l.trim();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                if let Some(mut reply) = process_incoming_frame(trimmed).await {
                                    reply.push('\n');
                                    if writer.write_all(reply.as_bytes()).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            _ => break,
                        }
                    }
                }
            }
        });
    }
}

fn is_socket_file(meta: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        meta.file_type().is_socket()
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

/// Validates whether a file path is safe to remove as a Unix domain socket.
/// Permits removal only if:
/// 1. The existing file is an actual Unix socket, OR
/// 2. The path resides in a designated temporary directory (/tmp, /var/tmp, /dev/shm, or std::env::temp_dir())
///    and is not a directory or symlink.
pub(crate) fn is_safe_socket_path_to_remove(path: &std::path::Path) -> bool {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if is_socket_file(&meta) {
            return true;
        }

        if meta.is_dir() || meta.file_type().is_symlink() {
            return false;
        }

        if let Ok(canonical_path) = path.canonicalize() {
            let allowed_temp_dirs = [
                std::env::temp_dir(),
                std::path::PathBuf::from("/tmp"),
                std::path::PathBuf::from("/var/tmp"),
                std::path::PathBuf::from("/dev/shm"),
            ];

            for temp_dir in &allowed_temp_dirs {
                if let Ok(canonical_temp) = temp_dir.canonicalize() {
                    if canonical_path.starts_with(&canonical_temp) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[allow(dead_code)]
pub(crate) fn safe_remove_socket_file(path: &std::path::Path) {
    if is_safe_socket_path_to_remove(path) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(not(windows))]
async fn run_unix_socket(sock_path: String, mut shutdown_rx: broadcast::Receiver<()>) {
    use tokio::net::UnixListener;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let p = std::path::Path::new(&sock_path);
    safe_remove_socket_file(p);
    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(_) => return,
    };

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                break;
            }
            conn = listener.accept() => {
                match conn {
                    Ok((stream, _)) => {
                        let mut client_shutdown = shutdown_rx.resubscribe();
                        tokio::spawn(async move {
                            let (reader, mut writer) = tokio::io::split(stream);
                            let mut lines = BufReader::new(reader).lines();
                            loop {
                                tokio::select! {
                                    _ = client_shutdown.recv() => break,
                                    line = lines.next_line() => {
                                        match line {
                                            Ok(Some(l)) => {
                                                let trimmed = l.trim();
                                                if trimmed.is_empty() {
                                                    continue;
                                                }
                                                if let Ok(envelope) = serde_json::from_str::<RpcEnvelope>(trimmed) {
                                                    let resp = dispatch_envelope(envelope).await;
                                                    if let Ok(mut json) = serde_json::to_string(&resp) {
                                                        json.push('\n');
                                                        if writer.write_all(json.as_bytes()).await.is_err() {
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            _ => break,
                                        }
                                    }
                                }
                            }
                        });
                    }
                    Err(_) => break,
                }
            }
        }
    }
    safe_remove_socket_file(p);
}
