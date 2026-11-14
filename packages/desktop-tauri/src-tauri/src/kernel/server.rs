use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex};

use crate::kernel::protocol::{RpcEnvelope, RpcResponse};

#[path = "rpc_helpers.rs"]
mod rpc_helpers;

static PROVIDER_SNAPSHOT_CACHE: std::sync::LazyLock<
    tokio::sync::Mutex<Option<(Instant, String, Vec<serde_json::Value>)>>,
> = std::sync::LazyLock::new(|| tokio::sync::Mutex::new(None));
static PROVIDER_PUSH_TX: std::sync::LazyLock<broadcast::Sender<String>> =
    std::sync::LazyLock::new(|| broadcast::channel::<String>(16).0);

pub fn get_provider_push_tx() -> broadcast::Sender<String> {
    PROVIDER_PUSH_TX.clone()
}
static MCP_HUB: std::sync::OnceLock<crate::kernel::mcp::McpHub> = std::sync::OnceLock::new();

pub fn get_mcp_hub() -> &'static crate::kernel::mcp::McpHub {
    MCP_HUB.get_or_init(crate::kernel::mcp::McpHub::new)
}

static WATCHER_ENGINE: std::sync::LazyLock<crate::kernel::watcher::WatcherEngine> =
    std::sync::LazyLock::new(|| {
        let engine = crate::kernel::watcher::WatcherEngine::new(25);
        if let Ok(index) = crate::kernel::search::WorkspaceSearchIndex::new_in_memory() {
            engine.bind_search_index(Arc::new(index));
        }
        engine.bind_shadow_vfs(crate::kernel::vfs::get_global_shadow_vfs());
        engine
    });

pub fn get_watcher_engine() -> &'static crate::kernel::watcher::WatcherEngine {
    &WATCHER_ENGINE
}

static CRDT_ENGINE: std::sync::LazyLock<crate::kernel::crdt::CrdtEngine> =
    std::sync::LazyLock::new(crate::kernel::crdt::CrdtEngine::new);

pub fn get_crdt_engine() -> &'static crate::kernel::crdt::CrdtEngine {
    &CRDT_ENGINE
}

static SUBSCRIPTION_REGISTRY: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, String>>,
> = std::sync::OnceLock::new();
static BROWSER_HOSTS: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>,
> = std::sync::OnceLock::new();
static CREATION_SNAPSHOTS: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>,
> = std::sync::OnceLock::new();

fn register_subscription(id: String, kind: &str) -> String {
    let map = SUBSCRIPTION_REGISTRY.get_or_init(|| std::sync::RwLock::new(Default::default()));
    let mut guard = map.write().unwrap_or_else(|e| e.into_inner());
    if guard.len() > 512 {
        guard.clear();
    }
    guard.insert(id.clone(), kind.to_string());
    id
}

fn release_subscription(id: &str) -> bool {
    let Some(map) = SUBSCRIPTION_REGISTRY.get() else {
        return false;
    };
    map.write()
        .unwrap_or_else(|e| e.into_inner())
        .remove(id)
        .is_some()
}

pub fn get_registered_browser_hosts() -> Vec<serde_json::Value> {
    let Some(map) = BROWSER_HOSTS.get() else {
        return Vec::new();
    };
    map.read()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .cloned()
        .collect()
}

fn discover_default_whisper_model() -> Option<std::path::PathBuf> {
    let mut dirs_to_check = vec![crate::get_data_dir().join("models")];
    if let Some(home) = dirs::home_dir() {
        dirs_to_check.push(home.join(".paseo").join("models"));
        dirs_to_check.push(home.join(".kronos").join("models"));
    }
    if let Some(local) = dirs::data_local_dir() {
        dirs_to_check.push(local.join("Kronos").join("models"));
    }
    for dir in dirs_to_check {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if path.is_file() && name.starts_with("ggml-") && name.ends_with(".bin") {
                    return Some(path);
                }
            }
        }
    }
    None
}

static SPEECH_ENGINE: std::sync::LazyLock<
    std::sync::Arc<tokio::sync::RwLock<crate::kernel::speech::SpeechEngine>>,
> = std::sync::LazyLock::new(|| {
    let model_path = discover_default_whisper_model();
    let engine = match crate::kernel::speech::SpeechEngine::new(model_path.as_deref()) {
        Ok(engine) => engine,
        Err(err) => crate::kernel::speech::SpeechEngine::unavailable(err),
    };
    std::sync::Arc::new(tokio::sync::RwLock::new(engine))
});

pub fn get_speech_engine(
) -> std::sync::Arc<tokio::sync::RwLock<crate::kernel::speech::SpeechEngine>> {
    SPEECH_ENGINE.clone()
}

static SHM_RING: std::sync::LazyLock<
    std::sync::Arc<tokio::sync::Mutex<crate::kernel::shm::ShmRingBuffer>>,
> = std::sync::LazyLock::new(|| {
    let ring = crate::kernel::shm::ShmRingBuffer::new("kronos_daemon_shm", 2 * 1024 * 1024)
        .unwrap_or_else(|e| panic!("Failed to initialize SHM ring buffer: {e}"));
    std::sync::Arc::new(tokio::sync::Mutex::new(ring))
});

pub fn get_shm_ring() -> std::sync::Arc<tokio::sync::Mutex<crate::kernel::shm::ShmRingBuffer>> {
    SHM_RING.clone()
}

static AGENT_MANAGER: std::sync::OnceLock<crate::agent::manager::AgentManager> =
    std::sync::OnceLock::new();
static PLUGIN_MANAGER: std::sync::OnceLock<crate::agent::providers::plugins::NativePluginManager> =
    std::sync::OnceLock::new();

pub fn get_agent_manager() -> &'static crate::agent::manager::AgentManager {
    AGENT_MANAGER.get_or_init(|| {
        #[cfg(test)]
        let home = {
            static HOME: std::sync::LazyLock<tempfile::TempDir> = std::sync::LazyLock::new(|| {
                tempfile::tempdir().expect("Create isolated agent test home")
            });
            HOME.path().to_path_buf()
        };
        #[cfg(not(test))]
        let home = crate::get_data_dir();
        crate::agent::manager::AgentManager::new(home)
    })
}

pub fn get_plugin_manager() -> &'static crate::agent::providers::plugins::NativePluginManager {
    PLUGIN_MANAGER.get_or_init(crate::agent::providers::plugins::NativePluginManager::new)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSessionMeta {
    pub id: String,
    pub name: String,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub created_at: u64,
}

static TERMINAL_REGISTRY: std::sync::LazyLock<
    tokio::sync::RwLock<std::collections::HashMap<String, TerminalSessionMeta>>,
> = std::sync::LazyLock::new(|| tokio::sync::RwLock::new(std::collections::HashMap::new()));

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
    }))
    .into_response()
}

pub async fn version_check() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "kronos-rust-kernel"
    }))
}

pub async fn handle_indexed_history_http(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Json<serde_json::Value> {
    let scope = params
        .get("scope")
        .map(|s| s.as_str())
        .unwrap_or("workspace");
    let cwd = params.get("cwd").map(|s| s.as_str());
    let query = params
        .get("query")
        .or_else(|| params.get("search"))
        .map(|s| s.as_str());
    let sort = params
        .get("sort")
        .map(|s| s.as_str())
        .unwrap_or("last_updated");
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(250);

    let res =
        crate::agent::sessions::scan_all_indexed_sessions(scope, cwd, query, sort, limit).await;

    axum::response::Json(serde_json::json!({
        "ok": true,
        "scannedFilesCount": res.scanned_files_count,
        "scanned_files": res.scanned_files_count,
        "scanned_files_count": res.scanned_files_count,
        "totalSessions": res.total_sessions,
        "total_sessions": res.total_sessions,
        "returned_sessions": res.sessions.len(),
        "sessions": res.sessions
    }))
}

pub async fn handle_omp_sessions_list_http(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Json<serde_json::Value> {
    let cwd = params.get("cwd").map(|s| s.as_str());
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(50);

    let (sessions, _) = crate::agent::sessions::scan_omp_sessions_raw(limit, cwd).await;
    let mapped: Vec<serde_json::Value> = sessions
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "filePath": s.cwd.clone(),
                "cwd": s.cwd,
                "title": s.title,
                "firstPrompt": s.title,
                "lastPrompt": s.last_message_preview,
                "lastActivityAt": s.last_activity_at,
                "model": s.model,
                "thinkingOptionId": null,
                "turnCount": s.message_count,
                "toolCallCount": s.subagent_count,
            })
        })
        .collect();

    axum::response::Json(serde_json::json!({
        "ok": true,
        "sessions": mapped
    }))
}

pub async fn handle_omp_session_inspect_http(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    if let Some(session) = crate::agent::sessions::inspect_omp_session(&id) {
        return (
            StatusCode::OK,
            axum::response::Json(serde_json::json!({
                "ok": true,
                "session": session
            })),
        )
            .into_response();
    }

    let mgr = get_agent_manager();
    if let Some(rec) = mgr.get_agent(&id) {
        let items = mgr.get_timeline(&id);
        let turns: Vec<serde_json::Value> = items
            .iter()
            .filter_map(|item| match item {
                crate::agent::types::AgentTimelineItem::UserMessage { text, timestamp, .. } => {
                    Some(serde_json::json!({ "role": "user", "text": text, "toolCalls": [], "timestamp": timestamp }))
                }
                crate::agent::types::AgentTimelineItem::AssistantMessage { text, timestamp, .. } => {
                    Some(serde_json::json!({ "role": "assistant", "text": text, "toolCalls": [], "timestamp": timestamp }))
                }
                _ => None,
            })
            .collect();
        let total_tool_calls = items
            .iter()
            .filter(|i| matches!(i, crate::agent::types::AgentTimelineItem::ToolCall { .. }))
            .count();
        let total_turns = turns.len();
        return (
            StatusCode::OK,
            axum::response::Json(serde_json::json!({
                "ok": true,
                "session": {
                    "id": rec.id,
                    "title": rec.title.unwrap_or_else(|| "Agent Session".to_string()),
                    "cwd": rec.cwd,
                    "turns": turns,
                    "fileDiffs": [],
                    "stats": {
                        "totalTurns": total_turns,
                        "totalToolCalls": total_tool_calls,
                        "filesModified": 0
                    }
                }
            })),
        )
            .into_response();
    }

    (
        StatusCode::NOT_FOUND,
        axum::response::Json(serde_json::json!({
            "ok": false,
            "error": format!("Session {id} not found")
        })),
    )
        .into_response()
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
            data: Some(crate::kernel::protocol::build_server_info_payload()),
            error: None,
        },
        "directory_suggestions_request" | "workspace/suggest_directory" => {
            let query = envelope
                .payload
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cwd_str = envelope
                .payload
                .get("cwd")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let include_files = envelope
                .payload
                .get("includeFiles")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let include_dirs = envelope
                .payload
                .get("includeDirectories")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let limit = envelope
                .payload
                .get("limit")
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

            RpcResponse::ok(
                &envelope.id,
                "directory_suggestions_response",
                Some(serde_json::json!({
                    "directories": directories,
                    "entries": entries,
                    "error": null,
                    "requestId": envelope.id
                })),
            )
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
        "vfs/watch" | "vfs/watch_subscribe" => {
            let path_opt = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .or_else(|| envelope.payload.as_str());
            let ignore: Vec<String> = envelope
                .payload
                .get("ignore")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            match path_opt {
                Some(path) => {
                    let engine = get_watcher_engine();
                    match engine.watch(path, &ignore) {
                        Ok(sub_id) => RpcResponse::ok(
                            &envelope.id,
                            "vfs/watch/response",
                            Some(serde_json::json!({
                                "subscriptionId": sub_id,
                                "path": path,
                            })),
                        ),
                        Err(err) => RpcResponse::err(&envelope.id, "vfs/watch/response", err),
                    }
                }
                None => RpcResponse::err(
                    &envelope.id,
                    "vfs/watch/response",
                    "Missing 'path' parameter for vfs/watch",
                ),
            }
        }
        "vfs/unwatch" | "vfs/watch_unsubscribe" => {
            let sub_id_opt = envelope
                .payload
                .get("subscriptionId")
                .or_else(|| envelope.payload.get("subscription_id"))
                .or_else(|| envelope.payload.get("sub_id"))
                .or_else(|| envelope.payload.get("id"))
                .and_then(|v| v.as_str())
                .or_else(|| envelope.payload.as_str());

            match sub_id_opt {
                Some(sub_id) => {
                    let engine = get_watcher_engine();
                    match engine.unwatch(sub_id) {
                        Ok(unwatched) => RpcResponse::ok(
                            &envelope.id,
                            "vfs/unwatch/response",
                            Some(serde_json::json!({
                                "unwatched": unwatched,
                                "subscriptionId": sub_id,
                            })),
                        ),
                        Err(err) => RpcResponse::err(&envelope.id, "vfs/unwatch/response", err),
                    }
                }
                None => RpcResponse::err(
                    &envelope.id,
                    "vfs/unwatch/response",
                    "Missing 'subscriptionId' parameter for vfs/unwatch",
                ),
            }
        }
        "ast/analyze" | "ast/parse" => {
            let path_opt = envelope.payload.get("path").and_then(|v| v.as_str());
            let source_opt = envelope.payload.get("source").and_then(|v| v.as_str());
            let ext_opt = envelope.payload.get("ext").and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| {
                        path_opt.and_then(|p| {
                            std::path::Path::new(p).extension().and_then(|e| e.to_str())
                        })
                    })
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| {
                                std::path::Path::new(path)
                                    .extension()
                                    .and_then(|e| e.to_str())
                            })
                            .unwrap_or("ts");
                        (content, ext.to_string())
                    }
                    Err(e) => {
                        return RpcResponse::err(
                            &envelope.id,
                            "ast/analyze/response",
                            format!("Failed to read file '{path}': {e}"),
                        );
                    }
                }
            } else {
                return RpcResponse::err(
                    &envelope.id,
                    "ast/analyze/response",
                    "Missing 'source' or 'path' parameter for ast/analyze",
                );
            };

            let engine = crate::kernel::ast::AstEngine::new();
            match engine.analyze(&source, &ext) {
                Ok(result) => RpcResponse::ok(
                    &envelope.id,
                    "ast/analyze/response",
                    serde_json::to_value(&result).ok(),
                ),
                Err(err) => RpcResponse::err(&envelope.id, "ast/analyze/response", err),
            }
        }
        "ast/symbols" => {
            let path_opt = envelope.payload.get("path").and_then(|v| v.as_str());
            let source_opt = envelope.payload.get("source").and_then(|v| v.as_str());
            let ext_opt = envelope.payload.get("ext").and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| {
                        path_opt.and_then(|p| {
                            std::path::Path::new(p).extension().and_then(|e| e.to_str())
                        })
                    })
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| {
                                std::path::Path::new(path)
                                    .extension()
                                    .and_then(|e| e.to_str())
                            })
                            .unwrap_or("ts");
                        (content, ext.to_string())
                    }
                    Err(e) => {
                        return RpcResponse::err(
                            &envelope.id,
                            "ast/symbols/response",
                            format!("Failed to read file '{path}': {e}"),
                        );
                    }
                }
            } else {
                return RpcResponse::err(
                    &envelope.id,
                    "ast/symbols/response",
                    "Missing 'source' or 'path' parameter for ast/symbols",
                );
            };

            let engine = crate::kernel::ast::AstEngine::new();
            match engine.extract_symbols(&source, &ext) {
                Ok(symbols) => RpcResponse::ok(
                    &envelope.id,
                    "ast/symbols/response",
                    serde_json::to_value(&symbols).ok(),
                ),
                Err(err) => RpcResponse::err(&envelope.id, "ast/symbols/response", err),
            }
        }
        "crdt/join" => {
            let room = envelope
                .payload
                .get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let engine = get_crdt_engine();
            let doc = engine.get_or_create_room(room);
            let text = doc.to_string();
            let len = doc.len();
            let sv = doc.export_state_vector();
            let update = doc.export_update();
            let sv_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &sv);
            let update_b64 =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &update);

            RpcResponse::ok(
                &envelope.id,
                "crdt/join/response",
                Some(serde_json::json!({
                    "room": room,
                    "text": text,
                    "length": len,
                    "stateVector": sv_b64,
                    "update": update_b64,
                })),
            )
        }
        "crdt/update" => {
            let room = envelope
                .payload
                .get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let source_id = envelope
                .payload
                .get("sourceId")
                .or_else(|| envelope.payload.get("source_id"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let update_b64_opt = envelope.payload.get("update").and_then(|v| v.as_str());

            if let Some(b64) = update_b64_opt {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                    Ok(bytes) => {
                        let engine = get_crdt_engine();
                        match engine.apply_update(room, &bytes, source_id) {
                            Ok(broadcasted_b64) => RpcResponse::ok(
                                &envelope.id,
                                "crdt/update/response",
                                Some(serde_json::json!({
                                    "room": room,
                                    "success": true,
                                    "update": broadcasted_b64,
                                })),
                            ),
                            Err(e) => RpcResponse::err(&envelope.id, "crdt/update/response", e),
                        }
                    }
                    Err(e) => RpcResponse::err(
                        &envelope.id,
                        "crdt/update/response",
                        format!("Invalid base64 update: {e}"),
                    ),
                }
            } else {
                RpcResponse::err(
                    &envelope.id,
                    "crdt/update/response",
                    "Missing 'update' base64 payload",
                )
            }
        }
        "crdt/sync" => {
            let room = envelope
                .payload
                .get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let sv_b64_opt = envelope
                .payload
                .get("stateVector")
                .or_else(|| envelope.payload.get("state_vector"))
                .and_then(|v| v.as_str());

            if let Some(b64) = sv_b64_opt {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                    Ok(sv_bytes) => {
                        let engine = get_crdt_engine();
                        match engine.sync_room(room, &sv_bytes) {
                            Ok(diff_bytes) => {
                                let diff_b64 = base64::Engine::encode(
                                    &base64::engine::general_purpose::STANDARD,
                                    &diff_bytes,
                                );
                                RpcResponse::ok(
                                    &envelope.id,
                                    "crdt/sync/response",
                                    Some(serde_json::json!({
                                        "room": room,
                                        "diff": diff_b64,
                                    })),
                                )
                            }
                            Err(e) => RpcResponse::err(&envelope.id, "crdt/sync/response", e),
                        }
                    }
                    Err(e) => RpcResponse::err(
                        &envelope.id,
                        "crdt/sync/response",
                        format!("Invalid base64 stateVector: {e}"),
                    ),
                }
            } else {
                RpcResponse::err(
                    &envelope.id,
                    "crdt/sync/response",
                    "Missing 'stateVector' parameter",
                )
            }
        }
        "crdt/state" => {
            let room = envelope
                .payload
                .get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let engine = get_crdt_engine();
            let doc = engine.get_or_create_room(room);
            let text = doc.to_string();
            let len = doc.len();
            let sv = doc.export_state_vector();
            let sv_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &sv);

            RpcResponse::ok(
                &envelope.id,
                "crdt/state/response",
                Some(serde_json::json!({
                    "room": room,
                    "text": text,
                    "length": len,
                    "stateVector": sv_b64,
                })),
            )
        }
        "crdt/edit" => {
            let room = envelope
                .payload
                .get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let action = envelope
                .payload
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("insert");
            let index = envelope
                .payload
                .get("index")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let text = envelope.payload.get("text").and_then(|v| v.as_str());
            let length = envelope
                .payload
                .get("length")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            let source_id = envelope
                .payload
                .get("sourceId")
                .or_else(|| envelope.payload.get("source_id"))
                .and_then(|v| v.as_str())
                .map(String::from);

            let engine = get_crdt_engine();
            match engine.apply_text_edit(room, action, index, text, length, source_id) {
                Ok(update_b64) => {
                    let doc = engine.get_or_create_room(room);
                    RpcResponse::ok(
                        &envelope.id,
                        "crdt/edit/response",
                        Some(serde_json::json!({
                            "room": room,
                            "text": doc.to_string(),
                            "length": doc.len(),
                            "update": update_b64,
                        })),
                    )
                }
                Err(e) => RpcResponse::err(&envelope.id, "crdt/edit/response", e),
            }
        }
        "ioring/status" => {
            let reader = crate::kernel::ioring::IoRingBatchReader::new(1024).ok();
            let is_active = reader
                .as_ref()
                .map(|r| r.is_kernel_ioring_active())
                .unwrap_or(false);
            let queue = reader.as_ref().map(|r| r.queue_size()).unwrap_or(0);

            RpcResponse::ok(
                &envelope.id,
                "ioring/status/response",
                Some(serde_json::json!({
                    "isKernelActive": is_active,
                    "queueSize": queue,
                    "platform": std::env::consts::OS,
                })),
            )
        }
        "ioring/read_batch" => {
            let paths_opt = envelope.payload.get("paths").and_then(|v| v.as_array());

            match paths_opt {
                Some(arr) => {
                    let path_bufs: Vec<std::path::PathBuf> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(std::path::PathBuf::from))
                        .collect();

                    match crate::kernel::ioring::IoRingBatchReader::new(1024) {
                        Ok(reader) => match reader.read_files_batched(&path_bufs) {
                            Ok(contents) => {
                                let mut results = Vec::with_capacity(path_bufs.len());
                                for (p, bytes) in path_bufs.iter().zip(contents.into_iter()) {
                                    let b64 = base64::Engine::encode(
                                        &base64::engine::general_purpose::STANDARD,
                                        &bytes,
                                    );
                                    results.push(serde_json::json!({
                                        "path": p.to_string_lossy(),
                                        "size": bytes.len(),
                                        "contentBase64": b64,
                                    }));
                                }
                                RpcResponse::ok(
                                    &envelope.id,
                                    "ioring/read_batch/response",
                                    Some(serde_json::json!({ "files": results })),
                                )
                            }
                            Err(e) => RpcResponse::err(
                                &envelope.id,
                                "ioring/read_batch/response",
                                e.to_string(),
                            ),
                        },
                        Err(e) => RpcResponse::err(
                            &envelope.id,
                            "ioring/read_batch/response",
                            e.to_string(),
                        ),
                    }
                }
                None => RpcResponse::err(
                    &envelope.id,
                    "ioring/read_batch/response",
                    "Missing 'paths' array",
                ),
            }
        }
        "speech/status" => {
            let engine_lock = get_speech_engine();
            let engine = engine_lock.read().await;
            RpcResponse::ok(
                &envelope.id,
                "speech/status/response",
                Some(serde_json::json!({
                    "isModelLoaded": engine.is_model_loaded(),
                    "targetSampleRate": 16000,
                    "targetChannels": 1,
                })),
            )
        }
        "speech/transcribe" => {
            let sample_rate = envelope
                .payload
                .get("sampleRate")
                .and_then(|v| v.as_u64())
                .unwrap_or(16000) as u32;
            let channels = envelope
                .payload
                .get("channels")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u16;

            let samples_opt: Option<Vec<f32>> = if let Some(arr) =
                envelope.payload.get("samples").and_then(|v| v.as_array())
            {
                Some(
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect(),
                )
            } else if let Some(b64) = envelope.payload.get("pcmBase64").and_then(|v| v.as_str()) {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                    Ok(bytes) => {
                        let floats: Vec<f32> = bytes
                            .chunks_exact(2)
                            .map(|c| {
                                let val = i16::from_le_bytes([c[0], c[1]]);
                                val as f32 / 32768.0
                            })
                            .collect();
                        Some(floats)
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            match samples_opt {
                Some(samples) => {
                    let config =
                        crate::kernel::speech::SpeechAudioConfig::new(sample_rate, channels);
                    let engine_lock = get_speech_engine();
                    let engine = engine_lock.read().await;

                    match engine.transcribe_with_config(&samples, config) {
                        Ok(transcript) => {
                            let duration_secs =
                                samples.len() as f32 / (sample_rate as f32 * channels as f32);
                            RpcResponse::ok(
                                &envelope.id,
                                "speech/transcribe/response",
                                Some(serde_json::json!({
                                    "transcript": transcript,
                                    "samplesCount": samples.len(),
                                    "durationSecs": duration_secs,
                                })),
                            )
                        }
                        Err(e) => RpcResponse::err(&envelope.id, "speech/transcribe/response", e),
                    }
                }
                None => RpcResponse::err(
                    &envelope.id,
                    "speech/transcribe/response",
                    "Missing 'samples' or 'pcmBase64' audio buffer",
                ),
            }
        }
        "speech/load_model" => {
            let path_opt = envelope
                .payload
                .get("modelPath")
                .or_else(|| envelope.payload.get("model_path"))
                .and_then(|v| v.as_str());

            match path_opt {
                Some(path_str) => {
                    let p = std::path::Path::new(path_str);
                    match crate::kernel::speech::SpeechEngine::new(Some(p)) {
                        Ok(new_engine) => {
                            let engine_lock = get_speech_engine();
                            let mut writer = engine_lock.write().await;
                            *writer = new_engine;
                            RpcResponse::ok(
                                &envelope.id,
                                "speech/load_model/response",
                                Some(serde_json::json!({
                                    "success": true,
                                    "modelPath": path_str,
                                    "isModelLoaded": true,
                                })),
                            )
                        }
                        Err(e) => RpcResponse::err(&envelope.id, "speech/load_model/response", e),
                    }
                }
                None => RpcResponse::err(
                    &envelope.id,
                    "speech/load_model/response",
                    "Missing 'modelPath' parameter",
                ),
            }
        }
        "shm/status" => {
            let ring_lock = get_shm_ring();
            let ring = ring_lock.lock().await;
            let cap = ring.capacity();
            let head = ring.write_head();
            let seq = ring.sequence();
            RpcResponse::ok(
                &envelope.id,
                "shm/status/response",
                Some(serde_json::json!({
                    "capacity": cap,
                    "headOffset": head,
                    "sequence": seq,
                })),
            )
        }
        "shm/push" => {
            let stream_id = envelope
                .payload
                .get("streamId")
                .or_else(|| envelope.payload.get("stream_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("telemetry-default");
            let payload_b64 = envelope
                .payload
                .get("payloadBase64")
                .or_else(|| envelope.payload.get("payload_base64"))
                .or_else(|| envelope.payload.get("payload"))
                .and_then(|v| v.as_str());

            match payload_b64 {
                Some(b64) => {
                    let bytes =
                        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                            .unwrap_or_else(|_| b64.as_bytes().to_vec());
                    let ring_lock = get_shm_ring();
                    let ring = ring_lock.lock().await;
                    let next_seq = ring.sequence() + 1;
                    match crate::kernel::rkyv_shm::write_rkyv_chunk_to_shm(
                        &ring, next_seq, stream_id, bytes,
                    ) {
                        Ok(seq) => RpcResponse::ok(
                            &envelope.id,
                            "shm/push/response",
                            Some(serde_json::json!({
                                "sequence": seq,
                                "streamId": stream_id,
                                "success": true,
                            })),
                        ),
                        Err(e) => RpcResponse::err(&envelope.id, "shm/push/response", e),
                    }
                }
                None => RpcResponse::err(
                    &envelope.id,
                    "shm/push/response",
                    "Missing 'payloadBase64' parameter",
                ),
            }
        }
        "shm/poll" => {
            let since_seq = envelope
                .payload
                .get("sinceSeq")
                .or_else(|| envelope.payload.get("since_seq"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let ring_lock = get_shm_ring();
            let ring = ring_lock.lock().await;
            match crate::kernel::rkyv_shm::read_rkyv_chunks_since(&ring, since_seq) {
                Ok(chunks) => {
                    let records: Vec<serde_json::Value> = chunks
                        .into_iter()
                        .map(|(seq, chunk)| {
                            let b64 = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &chunk.payload,
                            );
                            serde_json::json!({
                                "sequence": seq,
                                "streamId": chunk.stream_id,
                                "bytesLen": chunk.bytes_len,
                                "payloadBase64": b64,
                            })
                        })
                        .collect();
                    RpcResponse::ok(
                        &envelope.id,
                        "shm/poll/response",
                        Some(serde_json::json!({ "records": records })),
                    )
                }
                Err(e) => RpcResponse::err(&envelope.id, "shm/poll/response", e),
            }
        }
        "vfs/write_virtual" => {
            let path_str = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let content_str = envelope
                .payload
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if path_str.is_empty() {
                RpcResponse::err(&envelope.id, "vfs/write_virtual/response", "Missing 'path' parameter")
            } else {
                let vfs = crate::kernel::vfs::get_global_shadow_vfs();
                match vfs.write_virtual(path_str, content_str.as_bytes()) {
                    Ok(()) => RpcResponse::ok(
                        &envelope.id,
                        "vfs/write_virtual/response",
                        Some(serde_json::json!({
                            "success": true,
                            "path": path_str,
                            "dirtyCount": vfs.len(),
                        })),
                    ),
                    Err(e) => RpcResponse::err(&envelope.id, "vfs/write_virtual/response", e.to_string()),
                }
            }
        }
        "vfs/read_cow" => {
            let path_str = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if path_str.is_empty() {
                RpcResponse::err(&envelope.id, "vfs/read_cow/response", "Missing 'path' parameter")
            } else {
                let vfs = crate::kernel::vfs::get_global_shadow_vfs();
                let path = std::path::Path::new(path_str);
                let is_virtual = vfs.is_virtual(path);
                match vfs.read_file_cow(path) {
                    Ok(bytes) => {
                        let content = String::from_utf8_lossy(&bytes).to_string();
                        RpcResponse::ok(
                            &envelope.id,
                            "vfs/read_cow/response",
                            Some(serde_json::json!({
                                "path": path_str,
                                "isVirtual": is_virtual,
                                "content": content,
                                "size": bytes.len(),
                            })),
                        )
                    }
                    Err(e) => RpcResponse::err(&envelope.id, "vfs/read_cow/response", e.to_string()),
                }
            }
        }
        "vfs/flush" => {
            let vfs = crate::kernel::vfs::get_global_shadow_vfs();
            let paths = vfs.list_virtual();
            let mut flushed = 0usize;
            for p in &paths {
                if vfs.flush_to_disk(p, p).is_ok() {
                    let _ = vfs.remove_virtual(p);
                    flushed += 1;
                }
            }
            if vfs.is_empty() {
                let _ = vfs.clear();
            }
            RpcResponse::ok(
                &envelope.id,
                "vfs/flush/response",
                Some(serde_json::json!({
                    "success": true,
                    "flushedCount": flushed,
                    "dirtyCount": vfs.len(),
                })),
            )
        }
        "vfs/status" => {
            let vfs = crate::kernel::vfs::get_global_shadow_vfs();
            let dirty: Vec<String> = vfs
                .list_virtual()
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            RpcResponse::ok(
                &envelope.id,
                "vfs/status/response",
                Some(serde_json::json!({
                    "dirtyCount": dirty.len(),
                    "isEmpty": vfs.is_empty(),
                    "dirtyPaths": dirty,
                })),
            )
        }
        "vfs/discard" => {
            let path_opt = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let discard_all = envelope
                .payload
                .get("all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let vfs = crate::kernel::vfs::get_global_shadow_vfs();
            if discard_all || path_opt.is_none() {
                let count = vfs.len();
                match vfs.clear() {
                    Ok(()) => RpcResponse::ok(
                        &envelope.id,
                        "vfs/discard/response",
                        Some(serde_json::json!({
                            "success": true,
                            "discardedCount": count,
                            "dirtyCount": vfs.len(),
                        })),
                    ),
                    Err(e) => RpcResponse::err(&envelope.id, "vfs/discard/response", e.to_string()),
                }
            } else {
                let path_str = path_opt.unwrap();
                match vfs.remove_virtual(path_str) {
                    Ok(removed) => RpcResponse::ok(
                        &envelope.id,
                        "vfs/discard/response",
                        Some(serde_json::json!({
                            "success": true,
                            "path": path_str,
                            "discarded": removed.is_some(),
                            "dirtyCount": vfs.len(),
                        })),
                    ),
                    Err(e) => RpcResponse::err(&envelope.id, "vfs/discard/response", e.to_string()),
                }
            }
        }
        "vfs/diff" => {
            let path_opt = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let vfs = crate::kernel::vfs::get_global_shadow_vfs();

            if let Some(path_str) = path_opt {
                match vfs.diff_virtual_file(path_str) {
                    Some(diff) => RpcResponse::ok(
                        &envelope.id,
                        "vfs/diff/response",
                        Some(serde_json::to_value(diff).unwrap_or(serde_json::Value::Null)),
                    ),
                    None => RpcResponse::ok(
                        &envelope.id,
                        "vfs/diff/response",
                        Some(serde_json::json!({
                            "path": path_str,
                            "isVirtual": false,
                            "hasChanges": false,
                            "additions": 0,
                            "deletions": 0,
                            "diff": "",
                            "hunks": []
                        })),
                    ),
                }
            } else {
                let diffs = vfs.diff_all_virtual();
                RpcResponse::ok(
                    &envelope.id,
                    "vfs/diff/response",
                    Some(serde_json::json!({
                        "totalFiles": diffs.len(),
                        "files": diffs,
                    })),
                )
            }
        }
        "lsp/start" => {
            let id = envelope
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
                .to_string();
            let root_path = envelope
                .payload
                .get("rootPath")
                .or_else(|| envelope.payload.get("root_path"))
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .to_string();
            let command = envelope
                .payload
                .get("command")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| match id.as_str() {
                    "rust" => "rust-analyzer".to_string(),
                    "typescript" | "javascript" => "typescript-language-server".to_string(),
                    "python" => "pyright-langserver".to_string(),
                    other => other.to_string(),
                });
            let args: Vec<String> = envelope
                .payload
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_else(|| {
                    if command.contains("language-server") {
                        vec!["--stdio".to_string()]
                    } else {
                        vec![]
                    }
                });
            let init_options = envelope.payload.get("initializationOptions").cloned();

            let config = crate::kernel::lsp::LspServerConfig {
                id: id.clone(),
                command,
                args,
                root_path,
                initialization_options: init_options,
            };

            let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
            match mux.start_server(config).await {
                Ok(proc) => RpcResponse::ok(
                    &envelope.id,
                    "lsp/start/response",
                    Some(serde_json::to_value(proc.status()).unwrap_or(serde_json::Value::Null)),
                ),
                Err(err) => RpcResponse::err(&envelope.id, "lsp/start/response", err),
            }
        }
        "lsp/stop" => {
            let id = envelope
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if id.is_empty() {
                RpcResponse::err(&envelope.id, "lsp/stop/response", "Missing 'id' parameter")
            } else {
                let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
                match mux.stop_server(id).await {
                    Ok(stopped) => RpcResponse::ok(
                        &envelope.id,
                        "lsp/stop/response",
                        Some(serde_json::json!({ "id": id, "stopped": stopped })),
                    ),
                    Err(err) => RpcResponse::err(&envelope.id, "lsp/stop/response", err),
                }
            }
        }
        "lsp/status" => {
            let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
            let servers = mux.list_servers().await;
            RpcResponse::ok(
                &envelope.id,
                "lsp/status/response",
                Some(serde_json::json!({ "servers": servers })),
            )
        }
        "lsp/request" => {
            let id = envelope
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let method = envelope
                .payload
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let params = envelope
                .payload
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
            if let Some(proc) = mux.get_server(id).await {
                match proc.request(method, params).await {
                    Ok(res) => RpcResponse::ok(&envelope.id, "lsp/request/response", Some(res)),
                    Err(err) => RpcResponse::err(&envelope.id, "lsp/request/response", err),
                }
            } else {
                RpcResponse::err(
                    &envelope.id,
                    "lsp/request/response",
                    format!("LSP server '{id}' is not running"),
                )
            }
        }
        "lsp/notify" => {
            let id = envelope
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let method = envelope
                .payload
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let params = envelope
                .payload
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
            if let Some(proc) = mux.get_server(id).await {
                match proc.notify(method, params).await {
                    Ok(()) => RpcResponse::ok(
                        &envelope.id,
                        "lsp/notify/response",
                        Some(serde_json::json!({ "success": true })),
                    ),
                    Err(err) => RpcResponse::err(&envelope.id, "lsp/notify/response", err),
                }
            } else {
                RpcResponse::err(
                    &envelope.id,
                    "lsp/notify/response",
                    format!("LSP server '{id}' is not running"),
                )
            }
        }
        "lsp/hover" => {
            let path_str = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let line = envelope
                .payload
                .get("line")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let character = envelope
                .payload
                .get("character")
                .or_else(|| envelope.payload.get("col"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let file_path = std::path::Path::new(path_str);
            let explicit_id = envelope.payload.get("id").and_then(|v| v.as_str());
            let server_id = explicit_id
                .or_else(|| {
                    crate::kernel::lsp::LspMultiplexer::detect_language_and_cmd(file_path)
                        .map(|(lang, _, _)| lang)
                })
                .unwrap_or("rust");

            let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
            if let Some(proc) = mux.get_server(server_id).await {
                match proc.hover(file_path, line, character).await {
                    Ok(hover_opt) => RpcResponse::ok(
                        &envelope.id,
                        "lsp/hover/response",
                        Some(serde_json::json!({ "hover": hover_opt })),
                    ),
                    Err(err) => RpcResponse::err(&envelope.id, "lsp/hover/response", err),
                }
            } else {
                RpcResponse::err(
                    &envelope.id,
                    "lsp/hover/response",
                    format!("No active LSP server for '{server_id}'"),
                )
            }
        }
        "lsp/definition" => {
            let path_str = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let line = envelope
                .payload
                .get("line")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let character = envelope
                .payload
                .get("character")
                .or_else(|| envelope.payload.get("col"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let file_path = std::path::Path::new(path_str);
            let explicit_id = envelope.payload.get("id").and_then(|v| v.as_str());
            let server_id = explicit_id
                .or_else(|| {
                    crate::kernel::lsp::LspMultiplexer::detect_language_and_cmd(file_path)
                        .map(|(lang, _, _)| lang)
                })
                .unwrap_or("rust");

            let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
            if let Some(proc) = mux.get_server(server_id).await {
                match proc.goto_definition(file_path, line, character).await {
                    Ok(locs) => RpcResponse::ok(
                        &envelope.id,
                        "lsp/definition/response",
                        Some(serde_json::json!({ "locations": locs })),
                    ),
                    Err(err) => RpcResponse::err(&envelope.id, "lsp/definition/response", err),
                }
            } else {
                RpcResponse::err(
                    &envelope.id,
                    "lsp/definition/response",
                    format!("No active LSP server for '{server_id}'"),
                )
            }
        }
        "lsp/did_open" => {
            let path_str = envelope
                .payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let file_path = std::path::Path::new(path_str);
            let explicit_id = envelope.payload.get("id").and_then(|v| v.as_str());
            let detected = crate::kernel::lsp::LspMultiplexer::detect_language_and_cmd(file_path);
            let server_id = explicit_id
                .or_else(|| detected.map(|(lang, _, _)| lang))
                .unwrap_or("text");
            let language_id = envelope
                .payload
                .get("languageId")
                .and_then(|v| v.as_str())
                .unwrap_or(server_id);

            let content_str = if let Some(c) = envelope.payload.get("content").and_then(|v| v.as_str()) {
                c.to_string()
            } else {
                let vfs = crate::kernel::vfs::get_global_shadow_vfs();
                match vfs.read_file_cow(file_path) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(e) => return RpcResponse::err(&envelope.id, "lsp/did_open/response", e.to_string()),
                }
            };

            let mux = crate::kernel::lsp::get_global_lsp_multiplexer();
            if let Some(proc) = mux.get_server(server_id).await {
                match proc.did_open(file_path, &content_str, language_id).await {
                    Ok(()) => RpcResponse::ok(
                        &envelope.id,
                        "lsp/did_open/response",
                        Some(serde_json::json!({ "success": true, "path": path_str })),
                    ),
                    Err(err) => RpcResponse::err(&envelope.id, "lsp/did_open/response", err),
                }
            } else {
                RpcResponse::err(
                    &envelope.id,
                    "lsp/did_open/response",
                    format!("No active LSP server for '{server_id}'"),
                )
            }
        }
        "notes.list.request" | "notes/list" => {
            let store = crate::kernel::notes_store::get_notes_store();
            let query = envelope
                .payload
                .get("query")
                .and_then(|v| v.as_str());
            let notes = match query {
                Some(q) if !q.trim().is_empty() => store.search_notes(q.trim()),
                _ => store.list_notes(),
            };
            RpcResponse::ok(
                &envelope.id,
                "notes.list.response",
                Some(serde_json::json!({ "notes": notes, "success": true })),
            )
        }
        "notes.get.request" | "notes/get" => {
            let store = crate::kernel::notes_store::get_notes_store();
            let note_id = envelope
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let note = store.get_note(note_id);
            let success = note.is_some();
            RpcResponse::ok(
                &envelope.id,
                "notes.get.response",
                Some(serde_json::json!({ "note": note, "success": success })),
            )
        }
        "notes.save.request" | "notes/save" => {
            let store = crate::kernel::notes_store::get_notes_store();
            let note_val = envelope
                .payload
                .get("note")
                .cloned()
                .unwrap_or_else(|| envelope.payload.clone());
            match serde_json::from_value::<crate::kernel::notes_store::NoteRecord>(note_val) {
                Ok(note) => match store.save_note(note) {
                    Ok(saved) => RpcResponse::ok(
                        &envelope.id,
                        "notes.save.response",
                        Some(serde_json::json!({ "note": saved, "success": true })),
                    ),
                    Err(err) => RpcResponse::err(&envelope.id, "notes.save.response", err),
                },
                Err(err) => RpcResponse::err(
                    &envelope.id,
                    "notes.save.response",
                    format!("Invalid note record: {err}"),
                ),
            }
        }
        "notes.delete.request" | "notes/delete" => {
            let store = crate::kernel::notes_store::get_notes_store();
            let note_id = envelope
                .payload
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match store.delete_note(note_id) {
                Ok(deleted) => RpcResponse::ok(
                    &envelope.id,
                    "notes.delete.response",
                    Some(serde_json::json!({ "deleted": deleted, "success": true })),
                ),
                Err(err) => RpcResponse::err(&envelope.id, "notes.delete.response", err),
            }
        }
        "ci.check_status.request" | "ci/check_status" => {
            let workspace = envelope
                .payload
                .get("workspacePath")
                .or_else(|| envelope.payload.get("cwd"))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            match crate::kernel::ci_repair::check_ci_status(std::path::Path::new(workspace)) {
                Ok(status) => RpcResponse::ok(
                    &envelope.id,
                    "ci.check_status.response",
                    Some(serde_json::json!({ "status": status, "success": true })),
                ),
                Err(err) => RpcResponse::err(&envelope.id, "ci.check_status.response", err),
            }
        }
        "ci.get_failure_log.request" | "ci/get_failure_log" => {
            let workspace = envelope
                .payload
                .get("workspacePath")
                .or_else(|| envelope.payload.get("cwd"))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let run_id = envelope
                .payload
                .get("runId")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            match crate::kernel::ci_repair::get_ci_failure_log(std::path::Path::new(workspace), run_id) {
                Ok(log) => RpcResponse::ok(
                    &envelope.id,
                    "ci.get_failure_log.response",
                    Some(serde_json::json!({ "log": log, "success": true })),
                ),
                Err(err) => RpcResponse::err(&envelope.id, "ci.get_failure_log.response", err),
            }
        }
        other => RpcResponse::err(
            envelope.id,
            other,
            format!("Method '{}' not implemented", other),
        ),
    }
}
fn canonicalize_path(path: &std::path::Path) -> std::path::PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) => {
            let s = p.to_string_lossy();
            if let Some(stripped) = s.strip_prefix(r"\\?\") {
                std::path::PathBuf::from(stripped)
            } else {
                p
            }
        }
        Err(_) => path.to_path_buf(),
    }
}

fn default_available_modes() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "id": "build", "label": "Build", "description": "Standard coding execution mode" }),
        serde_json::json!({ "id": "architect", "label": "Architect", "description": "Deep architectural analysis & planning" }),
        serde_json::json!({ "id": "plan", "label": "Plan", "description": "Read-only exploration and structured planning" }),
        serde_json::json!({ "id": "review", "label": "Review", "description": "Code review, audit, and bug hunting" }),
        serde_json::json!({ "id": "yolo", "label": "Autonomous", "description": "Fast autonomous execution with guardrails" }),
    ]
}

pub fn agent_record_to_snapshot(
    record: &crate::agent::types::StoredAgentRecord,
) -> serde_json::Value {
    let mut val = serde_json::json!({
        "id": record.id,
        "provider": record.provider,
        "cwd": if record.cwd.is_empty() { ".".to_string() } else { record.cwd.clone() },
        "model": serde_json::Value::Null,
        "thinkingOptionId": null,
        "createdAt": record.created_at,
        "updatedAt": record.updated_at,
        "lastUserMessageAt": record.last_activity_at,
        "status": match record.last_status {
            crate::agent::types::AgentStatus::Idle => "idle",
            crate::agent::types::AgentStatus::Running => "running",
            crate::agent::types::AgentStatus::Paused => "paused",
            crate::agent::types::AgentStatus::Closed => "closed",
            crate::agent::types::AgentStatus::Error => "error",
        },
        "capabilities": {
            "canInterrupt": true,
            "canSendInput": true,
            "canSpawnTerminals": true,
            "supportsStreaming": true,
            "supportsSessionPersistence": matches!(record.provider.as_str(), "omp" | "pi" | "opencode" | "codex" | "claude"),
            "supportsDynamicModes": true,
            "supportsMcpServers": true,
            "supportsReasoningStream": matches!(record.provider.as_str(), "omp" | "pi" | "opencode" | "codex" | "claude"),
            "supportsToolInvocations": true
        },
        "currentModeId": serde_json::Value::Null,
        "availableModes": default_available_modes(),
        "pendingPermissions": [],
        "persistence": null,
        "title": record.title,
        "labels": record.labels,
    });
    if let Some(ref usage) = record.last_usage {
        val["lastUsage"] = usage.clone();
    }
    if let Some(ref err) = record.last_error {
        val["lastError"] = serde_json::json!(err);
    }
    if let Some(ref ws_id) = record.workspace_id {
        val["workspaceId"] = serde_json::json!(ws_id);
    }
    if let Some(ref cfg) = record.config {
        if let Some(m) = cfg.get("model") {
            val["model"] = m.clone();
        }
        if let Some(thinking) = cfg.get("thinkingOptionId") {
            val["thinkingOptionId"] = thinking.clone();
        }
        if let Some(mode) = cfg.get("modeId").or_else(|| cfg.get("persona")) {
            if !mode.is_null() {
                val["currentModeId"] = mode.clone();
            }
        }
        if let Some(archived) = cfg.get("archivedAt").filter(|v| !v.is_null()) {
            val["archivedAt"] = archived.clone();
        }
        if let Some(att) = cfg.get("requiresAttention").and_then(|v| v.as_bool()) {
            val["requiresAttention"] = serde_json::json!(att);
        }
        if let Some(reason) = cfg.get("attentionReason").filter(|v| !v.is_null()) {
            val["attentionReason"] = reason.clone();
        }
        if let Some(meta) = cfg.get("metadata").filter(|v| v.is_object()) {
            val["metadata"] = meta.clone();
        }
        if let Some(opts) = cfg.get("options").filter(|v| v.is_object()) {
            val["options"] = opts.clone();
        }
    }
    let handle = record
        .runtime_info
        .as_ref()
        .filter(|info| info["provider"].as_str() == Some(record.provider.as_str()))
        .and_then(|info| info["providerHandleId"].as_str())
        .or_else(|| {
            record
                .config
                .as_ref()
                .and_then(|config| config["providerHandleId"].as_str())
        })
        .filter(|handle| !handle.is_empty());
    if let Some(handle) = handle {
        val["persistence"] = serde_json::json!({"provider":record.provider, "sessionId":handle});
        val["runtimeInfo"] = serde_json::json!({"provider":record.provider, "sessionId":handle});
    }
    val
}

pub fn timeline_item_to_entry(
    provider: &str,
    item: &crate::agent::types::AgentTimelineItem,
    seq: u64,
) -> serde_json::Value {
    let (item_json, ts) = match item {
        crate::agent::types::AgentTimelineItem::UserMessage {
            id,
            text,
            timestamp,
        } => (
            serde_json::json!({
                "type": "user_message",
                "text": text,
                "messageId": id,
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::AssistantMessage {
            id,
            text,
            timestamp,
        } => (
            serde_json::json!({
                "type": "assistant_message",
                "text": text,
                "messageId": id,
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::Thinking {
            text, timestamp, ..
        } => (
            serde_json::json!({
                "type": "reasoning",
                "text": text,
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::ToolCall {
            name,
            input,
            timestamp,
            call_id,
            ..
        } => (
            serde_json::json!({
                "type": "tool_call",
                "callId": call_id,
                "name": name,
                "status": "completed",
                "input": input,
                "error": null,
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::ToolResult {
            output,
            timestamp,
            is_error,
            call_id,
            ..
        } => (
            serde_json::json!({
                "type": "tool_call",
                "callId": call_id,
                "name": "result",
                "status": if *is_error { "failed" } else { "completed" },
                "output": output,
                "error": if *is_error { serde_json::json!({ "message": output }) } else { serde_json::Value::Null },
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::ErrorNotice {
            message, timestamp, ..
        } => (
            serde_json::json!({
                "type": "notification",
                "level": "error",
                "message": message,
            }),
            timestamp.clone(),
        ),
    };

    serde_json::json!({
        "provider": provider,
        "item": item_json,
        "timestamp": ts,
        "seqStart": seq,
        "seqEnd": seq,
        "sourceSeqRanges": [{ "startSeq": seq, "endSeq": seq }],
        "collapsed": []
    })
}

pub async fn dispatch_session_message(msg: &serde_json::Value) -> serde_json::Value {
    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let request_id = msg
        .get("requestId")
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
        "git/diff" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let staged = msg
                .get("staged")
                .or_else(|| payload.and_then(|p| p.get("staged")))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::git::GitEngine::get_diff(cwd, staged).await {
                Ok(diff) => serde_json::json!({
                    "type": "git/diff/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": { "diff": diff },
                    "diff": diff
                }),
                Err(err) => serde_json::json!({
                    "type": "git/diff/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": err,
                    "payload": { "error": err }
                }),
            }
        }
        "git/branches" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::git::GitEngine::list_branches(cwd).await {
                Ok(branches) => serde_json::json!({
                    "type": "git/branches/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": { "branches": branches },
                    "branches": branches
                }),
                Err(err) => serde_json::json!({
                    "type": "git/branches/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": err,
                    "payload": { "error": err }
                }),
            }
        }
        "git/worktree/list" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::git::GitEngine::worktree_list(cwd).await {
                Ok(worktrees) => serde_json::json!({
                    "type": "git/worktree/list/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": { "worktrees": worktrees },
                    "worktrees": worktrees
                }),
                Err(err) => serde_json::json!({
                    "type": "git/worktree/list/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": err,
                    "payload": { "error": err }
                }),
            }
        }
        "git/worktree/add" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path_str = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let branch = msg
                .get("branch")
                .or_else(|| payload.and_then(|p| p.get("branch")))
                .and_then(|v| v.as_str())
                .unwrap_or("sandbox");
            let create_branch = msg
                .get("createBranch")
                .or_else(|| payload.and_then(|p| p.get("createBranch")))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let cwd = std::path::Path::new(cwd_str);
            let target_path = std::path::Path::new(path_str);
            let res = crate::kernel::git::GitEngine::worktree_add(
                cwd,
                target_path,
                branch,
                create_branch,
            )
            .await;
            let success = res.is_ok();
            serde_json::json!({
                "type": "git/worktree/add/response",
                "requestId": request_id,
                "id": request_id,
                "success": success,
                "error": res.err(),
                "payload": {
                    "path": path_str,
                    "branch": branch
                }
            })
        }
        "git/worktree/remove" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path_str = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let cwd = std::path::Path::new(cwd_str);
            let target_path = std::path::Path::new(path_str);
            let res = crate::kernel::git::GitEngine::worktree_remove(cwd, target_path).await;
            let success = res.is_ok();
            serde_json::json!({
                "type": "git/worktree/remove/response",
                "requestId": request_id,
                "id": request_id,
                "success": success,
                "error": res.err()
            })
        }
        "terminal/snapshot" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("term-default");
            let rows = msg
                .get("rows")
                .or_else(|| payload.and_then(|p| p.get("rows")))
                .and_then(|v| v.as_u64())
                .unwrap_or(24) as u16;
            let cols = msg
                .get("cols")
                .or_else(|| payload.and_then(|p| p.get("cols")))
                .and_then(|v| v.as_u64())
                .unwrap_or(80) as u16;

            let screen = crate::kernel::terminal::get_terminal_registry().get_or_create(
                terminal_id,
                rows,
                cols,
            );
            let snapshot = screen.snapshot();
            serde_json::json!({
                "type": "terminal/snapshot/response",
                "requestId": request_id,
                "id": request_id,
                "success": true,
                "payload": snapshot,
                "snapshot": snapshot
            })
        }
        "terminal/feed" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("term-default");
            let data = msg
                .get("data")
                .or_else(|| payload.and_then(|p| p.get("data")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let screen =
                crate::kernel::terminal::get_terminal_registry().get_or_create(terminal_id, 24, 80);
            screen.process(data.as_bytes());
            serde_json::json!({
                "type": "terminal/feed/response",
                "requestId": request_id,
                "id": request_id,
                "success": true
            })
        }
        "create_terminal_request" => {
            let payload = msg.get("payload");
            let cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let workspace_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let name = msg
                .get("name")
                .or_else(|| payload.and_then(|p| p.get("name")))
                .and_then(|v| v.as_str())
                .unwrap_or("Terminal")
                .to_string();
            let command = msg
                .get("command")
                .or_else(|| payload.and_then(|p| p.get("command")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let args: Vec<String> = msg
                .get("args")
                .or_else(|| payload.and_then(|p| p.get("args")))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let size = msg
                .get("size")
                .or_else(|| payload.and_then(|p| p.get("size")));
            let rows = size
                .and_then(|s| s.get("rows"))
                .and_then(|v| v.as_u64())
                .unwrap_or(24) as u16;
            let cols = size
                .and_then(|s| s.get("cols"))
                .and_then(|v| v.as_u64())
                .unwrap_or(80) as u16;

            let now_dur = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let now_ms = now_dur.as_millis() as u64;
            let terminal_id = format!("term_{:x}_{:x}", now_dur.as_nanos(), std::process::id());

            #[cfg(windows)]
            let resolved_command = if let Some(cmd) = &command {
                if let Some(paths) = std::env::var_os("PATH") {
                    let mut found = None;
                    for dir in std::env::split_paths(&paths) {
                        for ext in &["exe", "cmd", "bat"] {
                            let p = dir.join(format!("{}.{}", cmd, ext));
                            if p.is_file() {
                                found = Some(p.to_string_lossy().to_string());
                                break;
                            }
                        }
                        if found.is_some() {
                            break;
                        }
                    }
                    found.or_else(|| Some(cmd.clone()))
                } else {
                    Some(cmd.clone())
                }
            } else {
                None
            };
            #[cfg(not(windows))]
            let resolved_command = command.clone();

            let opts = crate::native_pty::TerminalSpawnOptions {
                command: resolved_command,
                args,
                cwd: if cwd.is_empty() {
                    None
                } else {
                    Some(cwd.to_string())
                },
                cols,
                rows,
            };

            let pty = crate::native_pty::get_pty_manager();
            let spawn_res = pty.spawn_terminal(terminal_id.clone(), opts);
            if spawn_res.is_err() {
                let fallback_opts = crate::native_pty::TerminalSpawnOptions {
                    command: None,
                    args: vec![],
                    cwd: if cwd.is_empty() {
                        None
                    } else {
                        Some(cwd.to_string())
                    },
                    cols,
                    rows,
                };
                let _ = pty.spawn_terminal(terminal_id.clone(), fallback_opts);
            }

            let meta = TerminalSessionMeta {
                id: terminal_id.clone(),
                name: name.clone(),
                cwd: cwd.to_string(),
                workspace_id: workspace_id.clone(),
                title: Some(name.clone()),
                created_at: now_ms,
            };
            {
                let mut reg = TERMINAL_REGISTRY.write().await;
                reg.insert(terminal_id.clone(), meta);
            }

            let mut term_json = serde_json::json!({
                "id": terminal_id,
                "name": name,
                "cwd": cwd,
                "title": name,
                "activity": {
                    "state": "idle",
                    "attentionReason": null,
                    "changedAt": now_ms
                }
            });
            if let Some(ref ws_id) = workspace_id {
                term_json["workspaceId"] = serde_json::json!(ws_id);
            }

            serde_json::json!({
                "type": "create_terminal_response",
                "requestId": request_id,
                "payload": {
                    "terminal": term_json,
                    "error": null,
                    "requestId": request_id
                }
            })
        }
        "list_terminals_request" => {
            let payload = msg.get("payload");
            let cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str());
            let workspace_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str());

            let reg = TERMINAL_REGISTRY.read().await;
            let terminals: Vec<serde_json::Value> = reg
                .values()
                .filter(|t| {
                    if let Some(ws) = workspace_id {
                        if t.workspace_id.as_deref() != Some(ws) {
                            return false;
                        }
                    }
                    if let Some(c) = cwd {
                        if !t.cwd.is_empty() && t.cwd != c {
                            return false;
                        }
                    }
                    true
                })
                .map(|t| {
                    let mut v = serde_json::json!({
                        "id": t.id,
                        "name": t.name,
                        "cwd": t.cwd,
                        "title": t.title,
                        "activity": {
                            "state": "idle",
                            "attentionReason": null,
                            "changedAt": t.created_at
                        }
                    });
                    if let Some(ref ws) = t.workspace_id {
                        v["workspaceId"] = serde_json::json!(ws);
                    }
                    v
                })
                .collect();

            let mut resp_payload = serde_json::json!({
                "terminals": terminals,
                "requestId": request_id
            });
            if let Some(c) = cwd {
                resp_payload["cwd"] = serde_json::json!(c);
            }

            serde_json::json!({
                "type": "list_terminals_response",
                "requestId": request_id,
                "payload": resp_payload
            })
        }
        "subscribe_terminals_request" => {
            let payload = msg.get("payload");
            let cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let workspace_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str());

            let reg = TERMINAL_REGISTRY.read().await;
            let terminals: Vec<serde_json::Value> = reg
                .values()
                .filter(|t| {
                    if let Some(ws) = workspace_id {
                        if t.workspace_id.as_deref() != Some(ws) {
                            return false;
                        }
                    }
                    if !cwd.is_empty() && !t.cwd.is_empty() && t.cwd != cwd {
                        return false;
                    }
                    true
                })
                .map(|t| {
                    let mut v = serde_json::json!({
                        "id": t.id,
                        "name": t.name,
                        "title": t.title,
                        "activity": {
                            "state": "idle",
                            "attentionReason": null,
                            "changedAt": t.created_at
                        }
                    });
                    if let Some(ref ws) = t.workspace_id {
                        v["workspaceId"] = serde_json::json!(ws);
                    }
                    v
                })
                .collect();

            let mut resp_payload = serde_json::json!({
                "cwd": cwd,
                "terminals": terminals,
                "requestId": request_id
            });
            if let Some(ws) = workspace_id {
                resp_payload["workspaceId"] = serde_json::json!(ws);
            }

            serde_json::json!({
                "type": "terminals_changed",
                "requestId": request_id,
                "payload": resp_payload
            })
        }
        "unsubscribe_terminals_request" => {
            let payload = msg.get("payload");
            let cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let workspace_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str());

            let mut resp_payload = serde_json::json!({
                "cwd": cwd,
                "error": null
            });
            if let Some(ws) = workspace_id {
                resp_payload["workspaceId"] = serde_json::json!(ws);
            }

            serde_json::json!({
                "type": "unsubscribe_terminals_response",
                "requestId": request_id,
                "payload": resp_payload
            })
        }
        "subscribe_terminal_request" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            serde_json::json!({
                "type": "subscribe_terminal_response",
                "requestId": request_id,
                "payload": {
                    "terminalId": terminal_id,
                    "slot": 1,
                    "subscriptionId": format!("sub-terminal-{}", terminal_id),
                    "error": null,
                    "requestId": request_id
                }
            })
        }
        "unsubscribe_terminal_request" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            serde_json::json!({
                "type": "unsubscribe_terminal_response",
                "requestId": request_id,
                "payload": {
                    "terminalId": terminal_id,
                    "error": null
                }
            })
        }
        "terminal_input" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let message = msg
                .get("message")
                .or_else(|| payload.and_then(|p| p.get("message")));

            if let Some(m) = message {
                let msg_type = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if msg_type == "input" {
                    if let Some(data) = m.get("data").and_then(|v| v.as_str()) {
                        let _ =
                            crate::native_pty::get_pty_manager().write_terminal(terminal_id, data);
                    }
                } else if msg_type == "resize" {
                    let rows = m.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
                    let cols = m.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
                    let _ = crate::native_pty::get_pty_manager().resize_terminal(
                        terminal_id,
                        cols,
                        rows,
                    );
                }
            }

            serde_json::json!({
                "type": "terminal_input_response",
                "requestId": request_id,
                "payload": {
                    "success": true,
                    "terminalId": terminal_id
                }
            })
        }
        "kill_terminal_request" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let _ = crate::native_pty::get_pty_manager().kill_terminal(terminal_id);
            {
                let mut reg = TERMINAL_REGISTRY.write().await;
                reg.remove(terminal_id);
            }

            serde_json::json!({
                "type": "kill_terminal_response",
                "requestId": request_id,
                "payload": {
                    "terminalId": terminal_id,
                    "success": true,
                    "requestId": request_id
                }
            })
        }
        "terminal.rename.request" | "terminal_rename_request" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = msg
                .get("title")
                .or_else(|| payload.and_then(|p| p.get("title")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            {
                let mut reg = TERMINAL_REGISTRY.write().await;
                if let Some(t) = reg.get_mut(terminal_id) {
                    t.title = Some(title.to_string());
                }
            }

            serde_json::json!({
                "type": "terminal.rename.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "success": true,
                    "error": null
                }
            })
        }
        "capture_terminal_request" => {
            let payload = msg.get("payload");
            let terminal_id = msg
                .get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let strip_ansi = msg
                .get("stripAnsi")
                .or_else(|| payload.and_then(|p| p.get("stripAnsi")))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let start = msg
                .get("start")
                .or_else(|| payload.and_then(|p| p.get("start")))
                .and_then(|v| v.as_i64());
            let end = msg
                .get("end")
                .or_else(|| payload.and_then(|p| p.get("end")))
                .and_then(|v| v.as_i64());

            let (lines, total_lines) = match crate::native_pty::get_pty_manager()
                .capture_terminal_lines(terminal_id, strip_ansi, start, end)
            {
                Ok(res) => res,
                Err(_) => (Vec::new(), 0),
            };

            serde_json::json!({
                "type": "capture_terminal_response",
                "requestId": request_id,
                "payload": {
                    "terminalId": terminal_id,
                    "lines": lines,
                    "totalLines": total_lines,
                    "requestId": request_id
                }
            })
        }
        "agent/create" => {
            let payload = msg.get("payload");
            let provider = msg
                .get("provider")
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .and_then(|v| v.as_str())
                .unwrap_or("claude");
            let raw_cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let title = msg
                .get("title")
                .or_else(|| payload.and_then(|p| p.get("title")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let workspace_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resolved_cwd = crate::agent::runner::resolve_safe_cwd(
                std::path::Path::new(raw_cwd),
                workspace_id.as_deref(),
            );
            let cwd = resolved_cwd.to_string_lossy().to_string();
            let config = msg
                .get("config")
                .or_else(|| payload.and_then(|p| p.get("config")))
                .cloned();

            let mgr = get_agent_manager();
            match mgr.create_agent(provider, &cwd, title, workspace_id, config) {
                Ok(record) => serde_json::json!({
                    "type": "agent/create/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": record,
                    "agent": record
                }),
                Err(err) => serde_json::json!({
                    "type": "agent/create/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": err,
                    "payload": { "error": err }
                }),
            }
        }
        "agent/get" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            if let Some(record) = mgr.get_agent(agent_id) {
                serde_json::json!({
                    "type": "agent/get/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": record,
                    "agent": record
                })
            } else {
                serde_json::json!({
                    "type": "agent/get/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": format!("Agent {agent_id} not found"),
                    "payload": { "error": format!("Agent {agent_id} not found") }
                })
            }
        }
        "agent/list" => {
            let mgr = get_agent_manager();
            let agents = mgr.list_agents();
            serde_json::json!({
                "type": "agent/list/response",
                "requestId": request_id,
                "id": request_id,
                "success": true,
                "payload": { "agents": agents },
                "agents": agents
            })
        }
        "agent/message" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let text = msg
                .get("text")
                .or_else(|| payload.and_then(|p| p.get("text")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            match mgr.send_message(agent_id, text) {
                Ok(turn) => {
                    tokio::spawn(crate::agent::runner::run_agent_turn(turn));
                    serde_json::json!({
                        "type": "agent/message/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true
                    })
                }
                Err(err) => serde_json::json!({
                    "type": "agent/message/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": err,
                    "payload": { "error": err }
                }),
            }
        }
        "agent/timeline" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            match mgr
                .storage()
                .with_timeline(agent_id, <[crate::agent::types::AgentTimelineItem]>::to_vec)
            {
                Ok(items) => serde_json::json!({
                    "type": "agent/timeline/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true,
                    "payload": { "items": items },
                    "timeline": items
                }),
                Err(error) => serde_json::json!({
                    "type": "agent/timeline/response", "requestId":request_id, "id":request_id,
                    "success":false, "error":format!("Failed to read timeline: {error}")
                }),
            }
        }
        "agent/timeline/sqlite_search" => {
            let payload = msg.get("payload");
            let query = msg
                .get("query")
                .or_else(|| payload.and_then(|p| p.get("query")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let limit = msg
                .get("limit")
                .or_else(|| payload.and_then(|p| p.get("limit")))
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;

            if let Some(sqlite) = crate::kernel::sqlite_timeline::get_global_sqlite_timeline() {
                match sqlite.fts_search(query, agent_id, limit) {
                    Ok(hits) => serde_json::json!({
                        "type": "agent/timeline/sqlite_search/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": { "hits": hits }
                    }),
                    Err(e) => serde_json::json!({
                        "type": "agent/timeline/sqlite_search/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": format!("FTS search failed: {e}")
                    }),
                }
            } else {
                serde_json::json!({
                    "type": "agent/timeline/sqlite_search/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": "SQLite timeline not initialized"
                })
            }
        }
        "agent/timeline/sqlite_recent" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let limit = msg
                .get("limit")
                .or_else(|| payload.and_then(|p| p.get("limit")))
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;

            if let Some(sqlite) = crate::kernel::sqlite_timeline::get_global_sqlite_timeline() {
                match sqlite.get_recent(agent_id, limit) {
                    Ok(items) => serde_json::json!({
                        "type": "agent/timeline/sqlite_recent/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": { "items": items }
                    }),
                    Err(e) => serde_json::json!({
                        "type": "agent/timeline/sqlite_recent/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": format!("Query failed: {e}")
                    }),
                }
            } else {
                serde_json::json!({
                    "type": "agent/timeline/sqlite_recent/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": "SQLite timeline not initialized"
                })
            }
        }
        "agent/cancel" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            match mgr.cancel_agent(agent_id) {
                Ok(()) => serde_json::json!({
                    "type": "agent/cancel/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true
                }),
                Err(err) => serde_json::json!({
                    "type": "agent/cancel/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": err,
                    "payload": { "error": err }
                }),
            }
        }
        "providers/list" => {
            let mgr = get_agent_manager();
            let providers = mgr.registry().list();
            serde_json::json!({
                "type": "providers/list/response",
                "requestId": request_id,
                "id": request_id,
                "success": true,
                "payload": { "providers": providers },
                "providers": providers
            })
        }
        "plugins/usage" => {
            let mgr = get_plugin_manager();
            let metrics = mgr.get_metrics();
            serde_json::json!({
                "type": "plugins/usage/response",
                "requestId": request_id,
                "id": request_id,
                "success": true,
                "payload": { "metrics": metrics },
                "metrics": metrics
            })
        }
        "provider.usage.list.request" => {
            let now_iso = crate::agent::types::now_iso();
            let mut providers = Vec::new();
            let mgr = get_plugin_manager();
            let metrics = mgr.get_metrics();
            for m in metrics {
                if m.input_tokens > 0 || m.output_tokens > 0 {
                    let display_name = match m.provider_id.as_str() {
                        "claude" => "Claude Code",
                        "codex" => "OpenAI Codex",
                        "copilot" => "GitHub Copilot",
                        "cursor" => "Cursor",
                        "grok" => "Grok",
                        "kimi" => "Kimi",
                        "minimax" => "MiniMax",
                        "opencode-go" => "OpenCode Go",
                        "zai" => "Zai",
                        "antigravity" => "Antigravity",
                        _ => "AI Provider",
                    };
                    providers.push(serde_json::json!({
                        "providerId": m.provider_id,
                        "displayName": display_name,
                        "status": "available",
                        "planLabel": "Local Desktop",
                        "sourceLabel": "Local",
                        "fetchedAt": now_iso,
                        "windows": [],
                        "balances": [],
                        "details": [
                            { "id": "inputTokens", "label": "Input Tokens", "value": m.input_tokens.to_string() },
                            { "id": "outputTokens", "label": "Output Tokens", "value": m.output_tokens.to_string() },
                            { "id": "totalCost", "label": "Total Cost", "value": format!("${:.4}", m.total_cost_usd) }
                        ]
                    }));
                }
            }
            serde_json::json!({
                "type": "provider.usage.list.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "fetchedAt": now_iso,
                    "providers": providers
                }
            })
        }
        "usage.list_reports.request" => {
            let now_iso = crate::agent::types::now_iso();
            let mut reports = Vec::new();
            let mgr = get_plugin_manager();
            let metrics = mgr.get_metrics();

            let filter_agent_id = msg
                .get("agentId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str());

            let filter_report_ids: Option<Vec<String>> = msg
                .get("reportIds")
                .or_else(|| msg.get("payload").and_then(|p| p.get("reportIds")))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                });

            if let Some(aid) = filter_agent_id {
                if let Some(agent) = get_agent_manager().get_agent(aid) {
                    let (inp, out, cost) = if let Some(ref usage) = agent.last_usage {
                        let inp = usage
                            .get("inputTokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let out = usage
                            .get("outputTokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let cost = usage
                            .get("totalCostUsd")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        (inp, out, cost)
                    } else {
                        (0, 0, 0.0)
                    };
                    let provider_id = &agent.provider;
                    let display_name = match provider_id.as_str() {
                        "claude" => "Claude Code",
                        "codex" => "OpenAI Codex",
                        "copilot" => "GitHub Copilot",
                        "cursor" => "Cursor",
                        "grok" => "Grok",
                        "kimi" => "Kimi",
                        "minimax" => "MiniMax",
                        "opencode-go" => "OpenCode Go",
                        "zai" => "Zai",
                        "antigravity" => "Antigravity",
                        _ => "AI Agent",
                    };
                    reports.push(serde_json::json!({
                        "id": format!("{}:{}", provider_id, aid),
                        "sourceId": provider_id,
                        "sourceLabel": display_name,
                        "account": { "label": agent.title.as_deref().unwrap_or("Agent") },
                        "fetchedAt": now_iso,
                        "report": {
                            "status": "available",
                            "planLabel": "Local Desktop",
                            "windows": [],
                            "balances": [],
                            "details": [
                                { "id": "inputTokens", "label": "Input Tokens", "value": inp.to_string() },
                                { "id": "outputTokens", "label": "Output Tokens", "value": out.to_string() },
                                { "id": "totalCost", "label": "Total Cost", "value": format!("${:.4}", cost) }
                            ]
                        }
                    }));
                }
            } else {
                for m in metrics {
                    let is_specifically_requested = filter_report_ids
                        .as_ref()
                        .map_or(false, |ids| ids.iter().any(|id| id == &m.provider_id));

                    if m.input_tokens > 0 || m.output_tokens > 0 || is_specifically_requested {
                        let display_name = match m.provider_id.as_str() {
                            "claude" => "Claude Code",
                            "codex" => "OpenAI Codex",
                            "copilot" => "GitHub Copilot",
                            "cursor" => "Cursor",
                            "grok" => "Grok",
                            "kimi" => "Kimi",
                            "minimax" => "MiniMax",
                            "opencode-go" => "OpenCode Go",
                            "zai" => "Zai",
                            "antigravity" => "Antigravity",
                            _ => "AI Provider",
                        };
                        reports.push(serde_json::json!({
                            "id": m.provider_id.clone(),
                            "sourceId": m.provider_id.clone(),
                            "sourceLabel": display_name,
                            "account": {},
                            "fetchedAt": now_iso,
                            "report": {
                                "status": "available",
                                "planLabel": "Local Desktop",
                                "windows": [],
                                "balances": [],
                                "details": [
                                    { "id": "inputTokens", "label": "Input Tokens", "value": m.input_tokens.to_string() },
                                    { "id": "outputTokens", "label": "Output Tokens", "value": m.output_tokens.to_string() },
                                    { "id": "totalCost", "label": "Total Cost", "value": format!("${:.4}", m.total_cost_usd) }
                                ]
                            }
                        }));
                    }
                }
            }

            let push_tx = get_provider_push_tx();
            for report in &reports {
                let update_msg =
                    crate::kernel::protocol::wrap_session_outbound(serde_json::json!({
                        "type": "usage.list_reports.update",
                        "payload": {
                            "requestId": request_id,
                            "report": report
                        }
                    }));
                let _ = push_tx.send(update_msg.to_string());
            }

            serde_json::json!({
                "type": "usage.list_reports.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "error": null
                }
            })
        }
        "directory_suggestions_request" => {
            let payload = msg.get("payload");
            let query = msg
                .get("query")
                .or_else(|| payload.and_then(|p| p.get("query")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let include_files = msg
                .get("includeFiles")
                .or_else(|| payload.and_then(|p| p.get("includeFiles")))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let include_dirs = msg
                .get("includeDirectories")
                .or_else(|| payload.and_then(|p| p.get("includeDirectories")))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let limit = msg
                .get("limit")
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
        "notes.list.request" => {
            let payload = msg.get("payload");
            let query = msg
                .get("query")
                .or_else(|| payload.and_then(|p| p.get("query")))
                .and_then(|v| v.as_str());
            let store = crate::kernel::notes_store::get_notes_store();
            let notes = match query {
                Some(q) if !q.trim().is_empty() => store.search_notes(q.trim()),
                _ => store.list_notes(),
            };
            serde_json::json!({
                "type": "notes.list.response",
                "payload": {
                    "requestId": request_id,
                    "notes": notes,
                    "success": true
                }
            })
        }
        "notes.get.request" => {
            let payload = msg.get("payload");
            let note_id = msg
                .get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let store = crate::kernel::notes_store::get_notes_store();
            let note = store.get_note(note_id);
            let success = note.is_some();
            serde_json::json!({
                "type": "notes.get.response",
                "payload": {
                    "requestId": request_id,
                    "note": note,
                    "success": success
                }
            })
        }
        "notes.save.request" => {
            let payload = msg.get("payload");
            let note_val = msg
                .get("note")
                .or_else(|| payload.and_then(|p| p.get("note")))
                .or_else(|| payload)
                .cloned();
            let store = crate::kernel::notes_store::get_notes_store();
            let record_opt = note_val.and_then(|v| serde_json::from_value::<crate::kernel::notes_store::NoteRecord>(v).ok());
            match record_opt {
                Some(record) => match store.save_note(record) {
                    Ok(saved) => serde_json::json!({
                        "type": "notes.save.response",
                        "payload": {
                            "requestId": request_id,
                            "note": saved,
                            "success": true
                        }
                    }),
                    Err(err) => serde_json::json!({
                        "type": "notes.save.response",
                        "payload": {
                            "requestId": request_id,
                            "error": err,
                            "success": false
                        }
                    }),
                },
                None => serde_json::json!({
                    "type": "notes.save.response",
                    "payload": {
                        "requestId": request_id,
                        "error": "Invalid or missing note record",
                        "success": false
                    }
                }),
            }
        }
        "notes.delete.request" => {
            let payload = msg.get("payload");
            let note_id = msg
                .get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let store = crate::kernel::notes_store::get_notes_store();
            match store.delete_note(note_id) {
                Ok(deleted) => serde_json::json!({
                    "type": "notes.delete.response",
                    "payload": {
                        "requestId": request_id,
                        "deleted": deleted,
                        "success": true
                    }
                }),
                Err(err) => serde_json::json!({
                    "type": "notes.delete.response",
                    "payload": {
                        "requestId": request_id,
                        "error": err,
                        "success": false
                    }
                }),
            }
        }
        "ci.check_status.request" => {
            let payload = msg.get("payload");
            let workspace = msg
                .get("workspacePath")
                .or_else(|| msg.get("cwd"))
                .or_else(|| payload.and_then(|p| p.get("workspacePath").or_else(|| p.get("cwd"))))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            match crate::kernel::ci_repair::check_ci_status(std::path::Path::new(workspace)) {
                Ok(status) => serde_json::json!({
                    "type": "ci.check_status.response",
                    "payload": {
                        "requestId": request_id,
                        "status": status,
                        "success": true
                    }
                }),
                Err(err) => serde_json::json!({
                    "type": "ci.check_status.response",
                    "payload": {
                        "requestId": request_id,
                        "error": err,
                        "success": false
                    }
                }),
            }
        }
        "ci.get_failure_log.request" => {
            let payload = msg.get("payload");
            let workspace = msg
                .get("workspacePath")
                .or_else(|| msg.get("cwd"))
                .or_else(|| payload.and_then(|p| p.get("workspacePath").or_else(|| p.get("cwd"))))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let run_id = msg
                .get("runId")
                .or_else(|| payload.and_then(|p| p.get("runId")))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            match crate::kernel::ci_repair::get_ci_failure_log(std::path::Path::new(workspace), run_id) {
                Ok(log) => serde_json::json!({
                    "type": "ci.get_failure_log.response",
                    "payload": {
                        "requestId": request_id,
                        "log": log,
                        "success": true
                    }
                }),
                Err(err) => serde_json::json!({
                    "type": "ci.get_failure_log.response",
                    "payload": {
                        "requestId": request_id,
                        "error": err,
                        "success": false
                    }
                }),
            }
        }
        "project.list.request" => {
            let store = crate::kernel::config_store::get_config_store();
            let mut projects = store.list_projects();
            if projects.is_empty() {
                projects.push(store.get_or_create_scratchpad_project());
            }
            let project_descriptors: Vec<serde_json::Value> = projects
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "projectId": p.id,
                        "projectKey": p.id,
                        "projectDisplayName": p.name,
                        "projectCustomName": null,
                        "projectCustomIconRevision": null,
                        "projectIconRevision": "automatic:none:v1",
                        "projectRootPath": p.path,
                        "projectKind": "directory"
                    })
                })
                .collect();

            serde_json::json!({
                "type": "project.list.response",
                "payload": {
                    "requestId": request_id,
                    "projects": project_descriptors
                }
            })
        }
        "project.add.request" => {
            let payload = msg.get("payload");
            let requested_cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();

            if requested_cwd.is_empty() {
                serde_json::json!({
                    "type": "project.add.response",
                    "payload": {
                        "requestId": request_id,
                        "project": null,
                        "error": "Missing or empty directory path",
                        "errorCode": "directory_not_found"
                    }
                })
            } else {
                let path = std::path::Path::new(requested_cwd);
                if !path.is_dir() {
                    serde_json::json!({
                        "type": "project.add.response",
                        "payload": {
                            "requestId": request_id,
                            "project": null,
                            "error": format!("Directory not found: {}", requested_cwd),
                            "errorCode": "directory_not_found"
                        }
                    })
                } else {
                    let canonical = canonicalize_path(path);
                    let abs_path = canonical.to_string_lossy().to_string();
                    let dir_name = canonical
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("project")
                        .to_string();

                    let normalized_path_lower = abs_path.to_lowercase().replace('\\', "/");
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&normalized_path_lower, &mut hasher);
                    let hash_val = std::hash::Hasher::finish(&hasher);
                    let project_id = format!("prj_{:016x}", hash_val);

                    let now_iso = crate::agent::types::now_iso();

                    let store = crate::kernel::config_store::get_config_store();
                    let record = crate::kernel::config_store::ProjectRecord {
                        id: project_id.clone(),
                        name: dir_name.clone(),
                        path: abs_path.clone(),
                        created_at: now_iso,
                        default_workspace_id: None,
                    };
                    let _ = store.add_project(record);

                    let project_descriptor = serde_json::json!({
                        "projectId": project_id,
                        "projectKey": project_id,
                        "projectDisplayName": dir_name,
                        "projectCustomName": null,
                        "projectCustomIconRevision": null,
                        "projectIconRevision": "automatic:none:v1",
                        "projectRootPath": abs_path,
                        "projectKind": "directory"
                    });

                    serde_json::json!({
                        "type": "project.add.response",
                        "payload": {
                            "requestId": request_id,
                            "project": project_descriptor,
                            "error": null
                        }
                    })
                }
            }
        }
        "open_project_request" => {
            let payload = msg.get("payload");
            let requested_cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();

            if requested_cwd.is_empty() {
                serde_json::json!({
                    "type": "open_project_response",
                    "payload": {
                        "requestId": request_id,
                        "workspace": null,
                        "error": "Missing or empty directory path",
                        "errorCode": "directory_not_found"
                    }
                })
            } else {
                let path = std::path::Path::new(requested_cwd);
                if !path.is_dir() {
                    serde_json::json!({
                        "type": "open_project_response",
                        "payload": {
                            "requestId": request_id,
                            "workspace": null,
                            "error": format!("Directory not found: {}", requested_cwd),
                            "errorCode": "directory_not_found"
                        }
                    })
                } else {
                    let canonical = canonicalize_path(path);
                    let abs_path = canonical.to_string_lossy().to_string();
                    let dir_name = canonical
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("project")
                        .to_string();

                    let normalized_path_lower = abs_path.to_lowercase().replace('\\', "/");
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&normalized_path_lower, &mut hasher);
                    let hash_val = std::hash::Hasher::finish(&hasher);
                    let project_id = format!("prj_{:016x}", hash_val);
                    let workspace_id = format!("ws_{:016x}", hash_val);

                    let now_iso = crate::agent::types::now_iso();

                    let store = crate::kernel::config_store::get_config_store();
                    let record = crate::kernel::config_store::ProjectRecord {
                        id: project_id.clone(),
                        name: dir_name.clone(),
                        path: abs_path.clone(),
                        created_at: now_iso.clone(),
                        default_workspace_id: Some(workspace_id.clone()),
                    };
                    let _ = store.add_project(record);

                    let ws_record = crate::kernel::config_store::WorkspaceRecord {
                        id: workspace_id.clone(),
                        project_id: project_id.clone(),
                        name: dir_name.clone(),
                        path: abs_path.clone(),
                        kind: "directory".to_string(),
                    };
                    let _ = store.add_workspace(ws_record);

                    let workspace_descriptor = serde_json::json!({
                        "id": workspace_id,
                        "projectId": project_id,
                        "projectDisplayName": dir_name,
                        "projectCustomName": null,
                        "projectCustomIconRevision": null,
                        "projectRootPath": abs_path,
                        "workspaceDirectory": abs_path,
                        "setupStatus": null,
                        "projectKind": "directory",
                        "workspaceKind": "directory",
                        "name": dir_name,
                        "title": null,
                        "pinnedAt": null,
                        "background": false,
                        "archivingAt": null,
                        "status": "done",
                        "statusEnteredAt": null,
                        "activityAt": null,
                        "createdAt": now_iso,
                        "diffStat": null,
                        "scripts": [],
                        "gitRuntime": null,
                        "githubRuntime": null
                    });

                    serde_json::json!({
                        "type": "open_project_response",
                        "payload": {
                            "requestId": request_id,
                            "workspace": workspace_descriptor,
                            "error": null
                        }
                    })
                }
            }
        }
        "project.create_directory.request" => {
            let payload = msg.get("payload");
            let parent_path = msg
                .get("parentPath")
                .or_else(|| payload.and_then(|p| p.get("parentPath")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let name = msg
                .get("name")
                .or_else(|| payload.and_then(|p| p.get("name")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();

            if parent_path.is_empty() || name.is_empty() {
                serde_json::json!({
                    "type": "project.create_directory.response",
                    "payload": {
                        "requestId": request_id,
                        "directoryPath": null,
                        "project": null,
                        "error": "Parent path and directory name are required",
                        "errorCode": "invalid_parameters"
                    }
                })
            } else {
                let target = std::path::Path::new(parent_path).join(name);
                match std::fs::create_dir_all(&target) {
                    Ok(_) => {
                        let canonical = canonicalize_path(&target);
                        let abs_path = canonical.to_string_lossy().to_string();
                        let dir_name = name.to_string();

                        let normalized_path_lower = abs_path.to_lowercase().replace('\\', "/");
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        std::hash::Hash::hash(&normalized_path_lower, &mut hasher);
                        let hash_val = std::hash::Hasher::finish(&hasher);
                        let project_id = format!("prj_{:016x}", hash_val);

                        let now_iso = crate::agent::types::now_iso();

                        let store = crate::kernel::config_store::get_config_store();
                        let record = crate::kernel::config_store::ProjectRecord {
                            id: project_id.clone(),
                            name: dir_name.clone(),
                            path: abs_path.clone(),
                            created_at: now_iso,
                            default_workspace_id: None,
                        };
                        let _ = store.add_project(record);

                        let project_descriptor = serde_json::json!({
                            "projectId": project_id,
                            "projectKey": project_id,
                            "projectDisplayName": dir_name,
                            "projectCustomName": null,
                            "projectCustomIconRevision": null,
                            "projectIconRevision": "automatic:none:v1",
                            "projectRootPath": abs_path,
                            "projectKind": "directory"
                        });

                        serde_json::json!({
                            "type": "project.create_directory.response",
                            "payload": {
                                "requestId": request_id,
                                "directoryPath": abs_path,
                                "project": project_descriptor,
                                "error": null,
                                "errorCode": null
                            }
                        })
                    }
                    Err(e) => {
                        serde_json::json!({
                            "type": "project.create_directory.response",
                            "payload": {
                                "requestId": request_id,
                                "directoryPath": null,
                                "project": null,
                                "error": format!("Failed to create directory: {}", e),
                                "errorCode": "create_failed"
                            }
                        })
                    }
                }
            }
        }
        "project.remove.request" => {
            let payload = msg.get("payload");
            let project_id = msg
                .get("projectId")
                .or_else(|| payload.and_then(|p| p.get("projectId")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();

            let store = crate::kernel::config_store::get_config_store();
            let removed = store.remove_project(project_id).ok().flatten();
            let mut removed_ws = store
                .remove_workspaces_for_project(project_id)
                .unwrap_or_default();

            if let Some(ref p) = removed {
                if p.id != project_id {
                    if let Ok(extra) = store.remove_workspaces_for_project(&p.id) {
                        removed_ws.extend(extra);
                    }
                }
                if !p.path.is_empty() && p.path != project_id {
                    if let Ok(extra) = store.remove_workspaces_for_project(&p.path) {
                        removed_ws.extend(extra);
                    }
                }
            }

            let mut ws_ids: Vec<String> = removed_ws.into_iter().map(|w| w.id).collect();
            if ws_ids.is_empty() {
                if let Ok(Some(single_ws)) = store.remove_workspace(project_id) {
                    ws_ids.push(single_ws.id);
                }
            }
            ws_ids.sort();
            ws_ids.dedup();

            // Broadcast workspace_update event via provider push channel so client UI updates immediately
            let push_tx = get_provider_push_tx();
            if ws_ids.is_empty() {
                let update_msg = serde_json::json!({
                    "type": "workspace_update",
                    "payload": {
                        "kind": "remove",
                        "id": project_id,
                        "removedProjectId": project_id
                    }
                });
                let _ = push_tx.send(update_msg.to_string());
            } else {
                for ws_id in &ws_ids {
                    let update_msg = serde_json::json!({
                        "type": "workspace_update",
                        "payload": {
                            "kind": "remove",
                            "id": ws_id,
                            "removedProjectId": project_id
                        }
                    });
                    let _ = push_tx.send(update_msg.to_string());
                }
            }

            serde_json::json!({
                "type": "project.remove.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "projectId": project_id,
                    "accepted": true,
                    "removedWorkspaceIds": ws_ids,
                    "error": null
                }
            })
        }
        "project.icon.get.request" => {
            let payload = msg.get("payload");
            let p_id = msg
                .get("projectId")
                .or_else(|| payload.and_then(|p| p.get("projectId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "project.icon.get.response",
                "payload": {
                    "requestId": request_id,
                    "projectId": p_id,
                    "icon": null,
                    "error": null
                }
            })
        }
        "fetch_workspaces_request" => {
            let payload = msg.get("payload");
            let sub_id = if msg.get("subscribe").is_some()
                || payload.and_then(|p| p.get("subscribe")).is_some()
            {
                Some(format!("sub-workspaces-{}", request_id))
            } else {
                None
            };
            let now_iso = crate::agent::types::now_iso();
            let store = crate::kernel::config_store::get_config_store();
            let workspaces = store.list_workspaces();
            let projects = store.list_projects();
            let project_map: std::collections::HashMap<_, _> =
                projects.iter().map(|p| (p.id.clone(), p.clone())).collect();

            let entries: Vec<serde_json::Value> = workspaces.into_iter().map(|w| {
                let p_record = project_map.get(&w.project_id);
                let p_name = p_record.map(|p| p.name.clone()).unwrap_or_else(|| w.name.clone());
                let p_root = p_record.map(|p| p.path.clone()).unwrap_or_else(|| w.path.clone());
                serde_json::json!({
                    "id": w.id,
                    "projectId": w.project_id,
                    "projectDisplayName": p_name,
                    "projectCustomName": null,
                    "projectCustomIconRevision": null,
                    "projectRootPath": p_root,
                    "workspaceDirectory": w.path,
                    "setupStatus": null,
                    "projectKind": "directory",
                    "workspaceKind": if w.kind == "worktree" { "worktree" } else { "directory" },
                    "name": w.name,
                    "title": null,
                    "pinnedAt": null,
                    "background": false,
                    "archivingAt": null,
                    "status": "done",
                    "statusEnteredAt": null,
                    "activityAt": null,
                    "createdAt": now_iso,
                    "diffStat": null,
                    "scripts": [],
                    "gitRuntime": null,
                    "githubRuntime": null
                })
            }).collect();

            let empty_projects: Vec<serde_json::Value> = projects
                .into_iter()
                .filter(|p| {
                    !entries
                        .iter()
                        .any(|e| e.get("projectId").and_then(|v| v.as_str()) == Some(p.id.as_str()))
                })
                .map(|p| {
                    serde_json::json!({
                        "projectId": p.id,
                        "projectKey": p.id,
                        "projectDisplayName": p.name,
                        "projectCustomName": null,
                        "projectCustomIconRevision": null,
                        "projectIconRevision": "automatic:none:v1",
                        "projectRootPath": p.path,
                        "projectKind": "directory"
                    })
                })
                .collect();

            serde_json::json!({
                "type": "fetch_workspaces_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id,
                    "entries": entries,
                    "emptyProjects": empty_projects,
                    "pageInfo": {
                        "nextCursor": null,
                        "prevCursor": null,
                        "hasMore": false
                    }
                }
            })
        }
        "workspace.create.request" => {
            let payload = msg.get("payload");
            let source = msg
                .get("source")
                .or_else(|| payload.and_then(|p| p.get("source")));
            let source_path = source
                .and_then(|s| s.get("path"))
                .and_then(|v| v.as_str())
                .or_else(|| source.and_then(|s| s.get("cwd")).and_then(|v| v.as_str()))
                .or_else(|| msg.get("path").and_then(|v| v.as_str()))
                .or_else(|| msg.get("cwd").and_then(|v| v.as_str()))
                .unwrap_or("")
                .trim();

            let project_id_req = source
                .and_then(|s| s.get("projectId"))
                .and_then(|v| v.as_str())
                .or_else(|| msg.get("projectId").and_then(|v| v.as_str()))
                .unwrap_or("");

            let requested_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let store = crate::kernel::config_store::get_config_store();

            let (abs_path, project_id, dir_name) = if !source_path.is_empty() {
                let p = std::path::Path::new(source_path);
                let canonical = canonicalize_path(p);
                let abs = canonical.to_string_lossy().to_string();
                let name = canonical
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string();

                let pid = if !project_id_req.is_empty() {
                    project_id_req.to_string()
                } else {
                    let normalized = abs.to_lowercase().replace('\\', "/");
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&normalized, &mut hasher);
                    let h = std::hash::Hasher::finish(&hasher);
                    format!("prj_{:016x}", h)
                };
                (abs, pid, name)
            } else if !project_id_req.is_empty() {
                if let Some(proj) = store.get_project(project_id_req) {
                    (proj.path, proj.id, proj.name)
                } else {
                    let proj = store.get_or_create_scratchpad_project();
                    (proj.path, proj.id, proj.name)
                }
            } else {
                let proj = store.get_or_create_scratchpad_project();
                (proj.path, proj.id, proj.name)
            };

            let ws_id = if requested_id.starts_with("wks_") && requested_id.len() == 20 {
                requested_id.to_string()
            } else {
                let dur = std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default();
                format!("wks_{:016x}", dur.as_nanos() as u64)
            };

            let now_iso = crate::agent::types::now_iso();

            let ws_record = crate::kernel::config_store::WorkspaceRecord {
                id: ws_id.clone(),
                project_id: project_id.clone(),
                name: dir_name.clone(),
                path: abs_path.clone(),
                kind: "directory".to_string(),
            };
            let _ = store.add_workspace(ws_record);

            let is_git = std::path::Path::new(&abs_path).join(".git").exists();
            let project_kind = if is_git { "git" } else { "directory" };

            let workspace_descriptor = serde_json::json!({
                "id": ws_id.clone(),
                "projectId": project_id,
                "projectDisplayName": dir_name,
                "projectCustomName": null,
                "projectCustomIconRevision": null,
                "projectRootPath": abs_path,
                "workspaceDirectory": abs_path,
                "setupStatus": null,
                "projectKind": project_kind,
                "workspaceKind": "directory",
                "name": dir_name,
                "title": null,
                "pinnedAt": null,
                "background": false,
                "archivingAt": null,
                "status": "done",
                "statusEnteredAt": null,
                "activityAt": null,
                "createdAt": now_iso,
                "diffStat": null,
                "scripts": [],
                "gitRuntime": null,
                "githubRuntime": null
            });

            let agent_opt = msg
                .get("agent")
                .or_else(|| payload.and_then(|p| p.get("agent")));

            let idempotency_key = msg
                .get("idempotencyKey")
                .or_else(|| payload.and_then(|p| p.get("idempotencyKey")))
                .and_then(|v| v.as_str())
                .unwrap_or(request_id);

            let mgr = get_agent_manager();
            let created_agent_snapshot = if let Some(agent_val) = agent_opt {
                let agent_cfg = agent_val.get("config");
                let provider = agent_cfg
                    .and_then(|c| c.get("provider"))
                    .and_then(|v| v.as_str())
                    .or_else(|| agent_val.get("provider").and_then(|v| v.as_str()))
                    .unwrap_or("codex");
                let agent_cwd = agent_cfg
                    .and_then(|c| c.get("cwd"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&abs_path);
                let agent_title = agent_val
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let mut config = agent_cfg.cloned().unwrap_or_else(|| serde_json::json!({}));
                if let Some(m) = agent_cfg
                    .and_then(|c| c.get("model"))
                    .and_then(|v| v.as_str())
                    .or_else(|| agent_val.get("model").and_then(|v| v.as_str()))
                {
                    if let Some(obj) = config.as_object_mut() {
                        obj.insert("model".to_string(), serde_json::json!(m));
                    }
                }

                match mgr.create_agent(
                    provider,
                    agent_cwd,
                    agent_title,
                    Some(ws_id.clone()),
                    Some(config),
                ) {
                    Ok(mut rec) => {
                        if let Some(prompt) =
                            agent_val.get("initialPrompt").and_then(|v| v.as_str())
                        {
                            if !prompt.trim().is_empty() {
                                match mgr.send_message(&rec.id, prompt) {
                                    Ok(turn) => {
                                        rec = turn.agent.clone();
                                        tokio::spawn(crate::agent::runner::run_agent_turn(turn));
                                    }
                                    Err(error) => {
                                        return serde_json::json!({
                                            "type":"workspace.create.response", "requestId":request_id,
                                            "payload":{"requestId":request_id, "workspace":workspace_descriptor,
                                                "agent":agent_record_to_snapshot(&rec), "error":error}
                                        })
                                    }
                                }
                            }
                        }
                        Some(agent_record_to_snapshot(&rec))
                    }
                    Err(_) => None,
                }
            } else {
                // Default agent so workspace creation always provides an agent
                match mgr.create_agent("codex", &abs_path, None, Some(ws_id.clone()), None) {
                    Ok(rec) => Some(agent_record_to_snapshot(&rec)),
                    Err(_) => None,
                }
            };

            let mut resp_payload = serde_json::json!({
                "requestId": request_id,
                "workspace": workspace_descriptor,
                "setupTerminalId": null,
                "error": null
            });

            let mut creation_obj = serde_json::json!({
                "kind": "workspace",
                "idempotencyKey": idempotency_key,
                "revision": 1,
                "phase": "completed",
                "workspaceId": ws_id,
                "workspace": workspace_descriptor,
                "agentId": null,
                "agent": null,
                "error": null
            });

            if let Some(ref agent_snap) = created_agent_snapshot {
                resp_payload["agent"] = agent_snap.clone();
                let agent_id = agent_snap.get("id").and_then(|v| v.as_str()).unwrap_or("");
                creation_obj["agentId"] = serde_json::json!(agent_id);
                creation_obj["agent"] = agent_snap.clone();
            }
            {
                let map = CREATION_SNAPSHOTS.get_or_init(|| std::sync::RwLock::new(Default::default()));
                let mut guard = map.write().unwrap_or_else(|e| e.into_inner());
                if guard.len() > 128 {
                    guard.clear();
                }
                guard.insert(idempotency_key.to_string(), creation_obj.clone());
            }
            resp_payload["creation"] = creation_obj;

            serde_json::json!({
                "type": "workspace.create.response",
                "requestId": request_id,
                "payload": resp_payload
            })
        }
        "agent.create.request" | "create_agent_request" => {
            let payload = msg.get("payload");
            let source_config = msg
                .get("config")
                .or_else(|| payload.and_then(|p| p.get("config")));
            let model = msg
                .get("model")
                .or_else(|| payload.and_then(|p| p.get("model")))
                .or_else(|| source_config.and_then(|c| c.get("model")))
                .and_then(|v| v.as_str());
            let raw_provider = msg
                .get("provider")
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .or_else(|| source_config.and_then(|config| config.get("provider")))
                .and_then(|v| v.as_str());
            let provider = match (raw_provider, model) {
                (Some(p), _) if p != "codex" => p,
                (_, Some(m)) => crate::agent::registry::resolve_provider_for_model(m)
                    .unwrap_or(raw_provider.unwrap_or("codex")),
                (Some(p), _) => p,
                _ => "codex",
            };
            let raw_cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .or_else(|| source_config.and_then(|config| config.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let title = msg
                .get("title")
                .or_else(|| payload.and_then(|p| p.get("title")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let ws_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resolved_cwd = crate::agent::runner::resolve_safe_cwd(
                std::path::Path::new(raw_cwd),
                ws_id.as_deref(),
            );
            let cwd = resolved_cwd.to_string_lossy().to_string();

            let mut config = source_config
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            if let Some(m) = model {
                if let Some(obj) = config.as_object_mut() {
                    obj.insert("model".to_string(), serde_json::json!(m));
                    obj.insert("provider".to_string(), serde_json::json!(provider));
                }
            }
            if let Some(thinking) = msg
                .get("thinkingOptionId")
                .or_else(|| payload.and_then(|p| p.get("thinkingOptionId")))
            {
                config["thinkingOptionId"] = thinking.clone();
            }
            let idempotency_key = msg
                .get("idempotencyKey")
                .or_else(|| payload.and_then(|p| p.get("idempotencyKey")))
                .and_then(|v| v.as_str())
                .unwrap_or(request_id);

            let initial_prompt = msg
                .get("initialPrompt")
                .or_else(|| payload.and_then(|p| p.get("initialPrompt")))
                .and_then(|v| v.as_str());

            let is_worktree = msg
                .get("worktree")
                .or_else(|| payload.and_then(|p| p.get("worktree")))
                .or_else(|| source_config.and_then(|c| c.get("worktree")))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let mgr = get_agent_manager();
            let create_result = if is_worktree {
                let slug = format!("worktree-{}", &crate::agent::manager::uuid_v4_simple()[..8]);
                mgr.create_agent_in_worktree(
                    provider,
                    &cwd,
                    &slug,
                    None,
                    title,
                    ws_id.clone(),
                    Some(config),
                )
                .await
            } else {
                mgr.create_agent(provider, &cwd, title, ws_id.clone(), Some(config))
            };
            match create_result {
                Ok(mut record) => {
                    let mut prompt_started = false;
                    if let Some(prompt) = initial_prompt {
                        if !prompt.trim().is_empty() {
                            match mgr.send_message(&record.id, prompt) {
                                Ok(turn) => {
                                    record = turn.agent.clone();
                                    prompt_started = true;
                                    tokio::spawn(crate::agent::runner::run_agent_turn(turn));
                                }
                                Err(error) => {
                                    return serde_json::json!({
                                        "type":"agent.create.response", "requestId":request_id,
                                        "payload":{"requestId":request_id, "agent":agent_record_to_snapshot(&record), "error":error}
                                    })
                                }
                            }
                        }
                    }
                    crate::agent::runner::broadcast_agent_status_update(&record.id);
                    let snapshot = agent_record_to_snapshot(&record);
                    serde_json::json!({
                        "type": "agent.create.response",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "agent": snapshot.clone(),
                            "creation": {
                                "kind": "agent",
                                "idempotencyKey": idempotency_key,
                                "revision": 1,
                                "phase": if prompt_started { "prompt_started" } else { "completed" },
                                "agentId": &record.id,
                                "agent": snapshot.clone(),
                                "workspaceId": ws_id,
                                "error": null
                            },
                            "error": null
                        }
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "agent.create.response",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "agent": null,
                            "error": err
                        }
                    })
                }
            }
        }
        "paseo_worktree_list_request" => {
            let cwd = msg
                .get("cwd")
                .or_else(|| msg.get("payload").and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let repo_path = std::path::Path::new(cwd);
            let wt_res = crate::kernel::git::GitEngine::worktree_list(repo_path).await;
            let worktrees: Vec<serde_json::Value> = match wt_res {
                Ok(entries) => entries
                    .into_iter()
                    .map(|e| {
                        serde_json::json!({
                            "worktreePath": e.path,
                            "branch": e.branch,
                            "head": e.head,
                            "isBare": e.is_bare,
                        })
                    })
                    .collect(),
                Err(_) => Vec::new(),
            };
            serde_json::json!({
                "type": "paseo_worktree_list_response",
                "payload": {
                    "requestId": request_id,
                    "worktrees": worktrees,
                    "error": null
                }
            })
        }
        "create_paseo_worktree_request" => {
            let cwd = msg
                .get("cwd")
                .or_else(|| msg.get("payload").and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let slug = msg
                .get("worktreeSlug")
                .or_else(|| msg.get("payload").and_then(|p| p.get("worktreeSlug")))
                .and_then(|v| v.as_str())
                .unwrap_or("sandbox");
            let branch = format!("kronos/{}", slug);
            let target_path = std::path::Path::new(cwd)
                .join(".kronos")
                .join("worktrees")
                .join(slug);
            let _ =
                std::fs::create_dir_all(target_path.parent().unwrap_or(std::path::Path::new(".")));

            let res = crate::kernel::git::GitEngine::worktree_add(
                std::path::Path::new(cwd),
                &target_path,
                &branch,
                true,
            )
            .await;

            match res {
                Ok(_) => serde_json::json!({
                    "type": "create_paseo_worktree_response",
                    "payload": {
                        "requestId": request_id,
                        "worktreePath": target_path.to_string_lossy(),
                        "branch": branch,
                        "error": null
                    }
                }),
                Err(err) => serde_json::json!({
                    "type": "create_paseo_worktree_response",
                    "payload": {
                        "requestId": request_id,
                        "worktreePath": null,
                        "branch": null,
                        "error": err
                    }
                }),
            }
        }
        "paseo_worktree_archive_request" => {
            let cwd = msg
                .get("cwd")
                .or_else(|| msg.get("payload").and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let wt_path = msg
                .get("worktreePath")
                .or_else(|| msg.get("payload").and_then(|p| p.get("worktreePath")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let res = crate::kernel::git::GitEngine::worktree_remove(
                std::path::Path::new(cwd),
                std::path::Path::new(wt_path),
            )
            .await;
            serde_json::json!({
                "type": "paseo_worktree_archive_response",
                "payload": {
                    "requestId": request_id,
                    "success": res.is_ok(),
                    "error": res.err()
                }
            })
        }
        "session.events.set_subscription.request" => {
            let req_id = request_id.to_owned();
            let events = msg
                .get("events")
                .or_else(|| msg.get("payload").and_then(|p| p.get("events")))
                .and_then(|v| v.as_array());
            if let Some(arr) = events {
                if arr
                    .iter()
                    .any(|e| e.as_str() == Some("providers_snapshot_update"))
                {
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
                        let entries = get_provider_snapshot_entries(".").await;
                        let now_iso = crate::agent::types::now_iso();
                        let frame = serde_json::json!({
                            "type": "session",
                            "message": {
                                "type": "providers_snapshot_update",
                                "payload": {
                                    "subscriptionId": format!("sub-events-{}", req_id),
                                    "cwd": ".",
                                    "entries": entries,
                                    "generatedAt": now_iso
                                }
                            }
                        });
                        if let Ok(text) = serde_json::to_string(&frame) {
                            let _ = get_provider_push_tx().send(text);
                        }
                    });
                }
            }
            serde_json::json!({
                "type": "session.events.set_subscription.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-events-{}", request_id)
                }
            })
        }
        "subscription.release.request" => {
            let payload = msg.get("payload");
            let sub_id = msg
                .get("subscriptionId")
                .or_else(|| payload.and_then(|p| p.get("subscriptionId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let released = release_subscription(sub_id);
            serde_json::json!({
                "type": "subscription.release.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id,
                    "released": released
                }
            })
        }
        "fetch_agents_request" => {
            let payload = msg.get("payload");
            let sub_id = if msg.get("subscribe").is_some()
                || payload.and_then(|p| p.get("subscribe")).is_some()
            {
                Some(register_subscription(format!("sub-agents-{}", request_id), "agents"))
            } else {
                None
            };
            let mgr = get_agent_manager();
            let agents = mgr.list_agents();
            let mut placement_cache = std::collections::HashMap::new();
            let entries: Vec<serde_json::Value> = agents
                .into_iter()
                .map(|rec| {
                    let cwd = if rec.cwd.is_empty() {
                        ".".to_string()
                    } else {
                        rec.cwd.clone()
                    };
                    let project = rpc_helpers::inspect_project_placement(
                        &cwd,
                        rec.workspace_id.as_deref(),
                        &mut placement_cache,
                    );
                    serde_json::json!({
                        "agent": agent_record_to_snapshot(&rec),
                        "project": project
                    })
                })
                .collect();

            serde_json::json!({
                "type": "fetch_agents_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id,
                    "entries": entries,
                    "pageInfo": {
                        "nextCursor": null,
                        "prevCursor": null,
                        "hasMore": false
                    }
                }
            })
        }
        "fetch_agent_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            let agent_opt = mgr
                .get_agent(agent_id)
                .map(|r| agent_record_to_snapshot(&r));
            serde_json::json!({
                "type": "fetch_agent_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agent": agent_opt,
                    "project": null,
                    "error": null
                }
            })
        }
        "send_agent_message_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let text = msg
                .get("text")
                .or_else(|| payload.and_then(|p| p.get("text")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            match mgr.send_message(agent_id, text) {
                Ok(turn) => {
                    tokio::spawn(crate::agent::runner::run_agent_turn(turn));
                    serde_json::json!({
                        "type": "send_agent_message_response",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "agentId": agent_id,
                            "accepted": true,
                            "error": null
                        }
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "send_agent_message_response",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "agentId": agent_id,
                            "accepted": false,
                            "error": err
                        }
                    })
                }
            }
        }
        "fetch_agent_timeline_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            let agent = mgr.get_agent(agent_id);
            let provider = agent
                .as_ref()
                .map(|a| a.provider.as_str())
                .unwrap_or("codex");
            let page = mgr.storage().with_timeline(agent_id, |items| {
                crate::kernel::timeline::project_page(agent_id, provider, items, msg)
            });
            let mut result = match page {
                Ok(page) => page,
                Err(error) => {
                    let mut page =
                        crate::kernel::timeline::project_page(agent_id, provider, &[], msg);
                    page["error"] = serde_json::json!(format!("Failed to read timeline: {error}"));
                    page
                }
            };
            result["requestId"] = serde_json::json!(request_id);
            result["agentId"] = serde_json::json!(agent_id);
            result["agent"] = agent
                .as_ref()
                .map(agent_record_to_snapshot)
                .unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "type": "fetch_agent_timeline_response",
                "payload": result
            })
        }
        "cancel_agent_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            let res = mgr.cancel_agent(agent_id);
            serde_json::json!({
                "type": "cancel_agent_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "error": res.err()
                }
            })
        }
        "delete_agent_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            if let Err(error) = mgr.delete_agent_and_wait(agent_id).await {
                return serde_json::json!({
                    "type":"rpc_error", "requestId":request_id,
                    "payload":{"requestId":request_id, "requestType":"delete_agent_request", "error":error, "code":"delete_failed"}
                });
            }
            serde_json::json!({
                "type": "agent_deleted",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id
                }
            })
        }
        "archive_agent_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let delete_worktree = msg
                .get("deleteWorktreeFromDisk")
                .or_else(|| payload.and_then(|p| p.get("deleteWorktreeFromDisk")))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let now_iso = crate::agent::types::now_iso();
            let mgr = get_agent_manager();
            match mgr.archive_agent(agent_id, &now_iso) {
                Ok(_) => {
                    if delete_worktree {
                        let _ = mgr.remove_agent_worktree(agent_id).await;
                    }
                    crate::agent::runner::broadcast_agent_status_update(agent_id);
                    serde_json::json!({
                        "type": "agent_archived",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "agentId": agent_id,
                            "archivedAt": now_iso
                        }
                    })
                }
                Err(error) => serde_json::json!({
                    "type": "rpc_error",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "requestType": "archive_agent_request",
                        "error": error,
                        "code": "archive_failed"
                    }
                }),
            }
        }
        "fetch_recent_provider_sessions_request" => {
            let payload = msg.get("payload");
            let providers_list: Vec<String> = msg
                .get("providers")
                .or_else(|| payload.and_then(|p| p.get("providers")))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_else(|| {
                    vec![
                        "antigravity".to_string(),
                        "opencode".to_string(),
                        "codex".to_string(),
                        "pi".to_string(),
                        "omp".to_string(),
                        "claude".to_string(),
                    ]
                });

            let filter_cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str());

            let query = msg
                .get("query")
                .or_else(|| payload.and_then(|p| p.get("query")))
                .and_then(|v| v.as_str());

            let limit = msg
                .get("limit")
                .or_else(|| payload.and_then(|p| p.get("limit")))
                .and_then(|v| v.as_u64())
                .unwrap_or(20) as usize;

            let entries = crate::agent::sessions::discover_recent_provider_sessions(
                &providers_list,
                limit,
                filter_cwd,
                query,
            )
            .await;

            serde_json::json!({
                "type": "fetch_recent_provider_sessions_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "entries": entries,
                    "filteredAlreadyImportedCount": 0,
                    "providerErrors": []
                }
            })
        }
        "session/history/indexed" => {
            let payload = msg.get("payload");
            let scope = msg
                .get("scope")
                .or_else(|| payload.and_then(|p| p.get("scope")))
                .and_then(|v| v.as_str())
                .unwrap_or("workspace");
            let filter_cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str());
            let query = msg
                .get("query")
                .or_else(|| payload.and_then(|p| p.get("query")))
                .and_then(|v| v.as_str());
            let sort = msg
                .get("sort")
                .or_else(|| payload.and_then(|p| p.get("sort")))
                .and_then(|v| v.as_str())
                .unwrap_or("last_updated");
            let limit = msg
                .get("limit")
                .or_else(|| payload.and_then(|p| p.get("limit")))
                .and_then(|v| v.as_u64())
                .unwrap_or(150) as usize;

            let result = crate::agent::sessions::scan_all_indexed_sessions(
                scope, filter_cwd, query, sort, limit,
            )
            .await;

            serde_json::json!({
                "type": "session/history/indexed/response",
                "requestId": request_id,
                "payload": result
            })
        }
        "fetch_agent_history_request" => {
            let payload = msg.get("payload");
            let mgr = get_agent_manager();
            let mut agents = mgr.list_agents();

            let search_query = msg
                .get("search")
                .or_else(|| payload.and_then(|p| p.get("search")))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty());

            if let Some(ref query) = search_query {
                agents.retain(|rec| {
                    rec.title
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(query)
                        || rec.id.to_lowercase().contains(query)
                        || rec.cwd.to_lowercase().contains(query)
                        || rec.provider.to_lowercase().contains(query)
                });
            }

            let filter = msg
                .get("filter")
                .or_else(|| payload.and_then(|p| p.get("filter")));

            if let Some(filter_val) = filter {
                if let Some(statuses) = filter_val.get("statuses").and_then(|v| v.as_array()) {
                    let status_strs: Vec<&str> =
                        statuses.iter().filter_map(|s| s.as_str()).collect();
                    if !status_strs.is_empty() {
                        agents.retain(|rec| {
                            let s = match rec.last_status {
                                crate::agent::types::AgentStatus::Idle => "idle",
                                crate::agent::types::AgentStatus::Running => "running",
                                crate::agent::types::AgentStatus::Paused => "paused",
                                crate::agent::types::AgentStatus::Closed => "closed",
                                crate::agent::types::AgentStatus::Error => "error",
                            };
                            status_strs.contains(&s)
                        });
                    }
                }
            }

            let sort_arr = msg
                .get("sort")
                .or_else(|| payload.and_then(|p| p.get("sort")))
                .and_then(|v| v.as_array());

            let (sort_key, sort_dir) = if let Some(arr) = sort_arr {
                if let Some(first) = arr.first() {
                    let key = first
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("updated_at");
                    let dir = first
                        .get("direction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("desc");
                    (key, dir)
                } else {
                    ("updated_at", "desc")
                }
            } else {
                ("updated_at", "desc")
            };

            agents.sort_by(|a, b| {
                let ord = match sort_key {
                    "created_at" => a.created_at.cmp(&b.created_at),
                    "title" => a
                        .title
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.title.as_deref().unwrap_or("")),
                    _ => {
                        let a_ts = a.last_activity_at.as_deref().unwrap_or(&a.updated_at);
                        let b_ts = b.last_activity_at.as_deref().unwrap_or(&b.updated_at);
                        a_ts.cmp(b_ts)
                    }
                };
                if sort_dir == "asc" {
                    ord
                } else {
                    ord.reverse()
                }
            });

            let page_obj = msg
                .get("page")
                .or_else(|| payload.and_then(|p| p.get("page")));
            let limit = page_obj
                .and_then(|p| p.get("limit"))
                .and_then(|v| v.as_u64())
                .map(|l| l as usize)
                .unwrap_or(200)
                .clamp(1, 200);
            let cursor = page_obj
                .and_then(|p| p.get("cursor"))
                .and_then(|v| v.as_str());

            let start_idx = if let Some(cursor_id) = cursor {
                agents
                    .iter()
                    .position(|a| a.id == cursor_id)
                    .map(|pos| pos + 1)
                    .unwrap_or(0)
            } else {
                0
            };

            let remaining = if start_idx < agents.len() {
                &agents[start_idx..]
            } else {
                &[]
            };

            let page_slice = if remaining.len() > limit {
                &remaining[..limit]
            } else {
                remaining
            };

            let has_more = remaining.len() > limit;
            let next_cursor = if has_more {
                page_slice.last().map(|a| a.id.clone())
            } else {
                None
            };

            let mut placement_cache = std::collections::HashMap::new();
            let entries: Vec<serde_json::Value> = page_slice
                .iter()
                .map(|rec| {
                    let cwd = if rec.cwd.is_empty() {
                        ".".to_string()
                    } else {
                        rec.cwd.clone()
                    };
                    let project = rpc_helpers::inspect_project_placement(
                        &cwd,
                        rec.workspace_id.as_deref(),
                        &mut placement_cache,
                    );
                    serde_json::json!({
                        "agent": agent_record_to_snapshot(rec),
                        "project": project
                    })
                })
                .collect();

            serde_json::json!({
                "type": "fetch_agent_history_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "entries": entries,
                    "pageInfo": {
                        "nextCursor": next_cursor,
                        "prevCursor": serde_json::Value::Null,
                        "hasMore": has_more
                    },
                    "searchTruncated": false
                }
            })
        }
        "schedule/list" => {
            let schedules = crate::kernel::schedules::list_schedules();
            let summaries: Vec<serde_json::Value> = schedules
                .iter()
                .map(crate::kernel::schedules::to_schedule_summary)
                .collect();
            serde_json::json!({
                "type": "schedule/list/response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "schedules": summaries,
                    "error": serde_json::Value::Null
                }
            })
        }
        "schedule/create" => match crate::kernel::schedules::create_schedule(msg) {
            Ok(stored) => {
                let summary = crate::kernel::schedules::to_schedule_summary(&stored);
                serde_json::json!({
                    "type": "schedule/create/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": summary,
                        "error": serde_json::Value::Null
                    }
                })
            }
            Err(e) => {
                serde_json::json!({
                    "type": "schedule/create/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": serde_json::Value::Null,
                        "error": format!("Failed to create schedule: {e}")
                    }
                })
            }
        },
        "schedule/inspect" => {
            let sched_id = msg
                .get("scheduleId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("scheduleId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::kernel::schedules::load_schedule(sched_id) {
                Some(schedule) => serde_json::json!({
                    "type": "schedule/inspect/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": schedule,
                        "error": serde_json::Value::Null
                    }
                }),
                None => serde_json::json!({
                    "type": "schedule/inspect/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": serde_json::Value::Null,
                        "error": format!("Schedule {sched_id} not found")
                    }
                }),
            }
        }
        "schedule/logs" => {
            let sched_id = msg
                .get("scheduleId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("scheduleId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::kernel::schedules::load_schedule(sched_id) {
                Some(schedule) => {
                    let runs = schedule
                        .get("runs")
                        .cloned()
                        .unwrap_or(serde_json::json!([]));
                    serde_json::json!({
                        "type": "schedule/logs/response",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "runs": runs,
                            "error": serde_json::Value::Null
                        }
                    })
                }
                None => serde_json::json!({
                    "type": "schedule/logs/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "runs": [],
                        "error": format!("Schedule {sched_id} not found")
                    }
                }),
            }
        }
        "schedule/pause" => {
            let sched_id = msg
                .get("scheduleId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("scheduleId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::kernel::schedules::pause_schedule(sched_id) {
                Ok(summary) => serde_json::json!({
                    "type": "schedule/pause/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": summary,
                        "error": serde_json::Value::Null
                    }
                }),
                Err(e) => serde_json::json!({
                    "type": "schedule/pause/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": serde_json::Value::Null,
                        "error": e
                    }
                }),
            }
        }
        "schedule/resume" => {
            let sched_id = msg
                .get("scheduleId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("scheduleId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::kernel::schedules::resume_schedule(sched_id) {
                Ok(summary) => serde_json::json!({
                    "type": "schedule/resume/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": summary,
                        "error": serde_json::Value::Null
                    }
                }),
                Err(e) => serde_json::json!({
                    "type": "schedule/resume/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": serde_json::Value::Null,
                        "error": e
                    }
                }),
            }
        }
        "schedule/delete" => {
            let sched_id = msg
                .get("scheduleId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("scheduleId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            crate::kernel::schedules::delete_schedule(sched_id);
            serde_json::json!({
                "type": "schedule/delete/response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "scheduleId": sched_id,
                    "error": serde_json::Value::Null
                }
            })
        }
        "schedule/run-once" => {
            let sched_id = msg
                .get("scheduleId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("scheduleId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::kernel::schedules::execute_schedule_run(sched_id) {
                Ok(schedule) => serde_json::json!({
                    "type": "schedule/run-once/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": schedule,
                        "error": serde_json::Value::Null
                    }
                }),
                Err(err) => serde_json::json!({
                    "type": "schedule/run-once/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": serde_json::Value::Null,
                        "error": err
                    }
                }),
            }
        }
        "schedule/update" => {
            let sched_id = msg
                .get("scheduleId")
                .or_else(|| msg.get("payload").and_then(|p| p.get("scheduleId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match crate::kernel::schedules::update_schedule(sched_id, msg) {
                Ok(schedule) => serde_json::json!({
                    "type": "schedule/update/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": schedule,
                        "error": serde_json::Value::Null
                    }
                }),
                Err(e) => serde_json::json!({
                    "type": "schedule/update/response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "schedule": serde_json::Value::Null,
                        "error": e
                    }
                }),
            }
        }
        "import_agent_request" | "resume_agent_request" => {
            let payload = msg.get("payload");
            let handle = msg
                .get("handle")
                .or_else(|| payload.and_then(|p| p.get("handle")));
            let overrides = msg
                .get("overrides")
                .or_else(|| payload.and_then(|p| p.get("overrides")));
            let provider = msg
                .get("providerId")
                .or_else(|| msg.get("provider"))
                .or_else(|| payload.and_then(|p| p.get("providerId")))
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .or_else(|| handle.and_then(|handle| handle.get("provider")))
                .and_then(|v| v.as_str())
                .unwrap_or("opencode");

            let handle_id = msg
                .get("providerHandleId")
                .or_else(|| msg.get("sessionId"))
                .or_else(|| payload.and_then(|p| p.get("providerHandleId")))
                .or_else(|| payload.and_then(|p| p.get("sessionId")))
                .or_else(|| {
                    handle.and_then(|handle| {
                        handle
                            .get("nativeHandle")
                            .or_else(|| handle.get("sessionId"))
                    })
                })
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if handle_id.trim().is_empty() {
                return serde_json::json!({"type":"status", "requestId":request_id,
                    "payload":{"status":"agent_create_failed", "requestId":request_id,
                        "error":"An explicit provider session handle is required to resume"}});
            }

            let cwd = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .or_else(|| overrides.and_then(|overrides| overrides.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");

            let ws_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let title = overrides
                .and_then(|overrides| overrides.get("title"))
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .or_else(|| Some(format!("Imported {handle_id}")));
            let mgr = get_agent_manager();
            let mut config = overrides.cloned().unwrap_or_else(|| serde_json::json!({}));
            config["provider"] = serde_json::json!(provider);
            config["providerHandleId"] = serde_json::json!(handle_id);
            config["cwd"] = serde_json::json!(cwd);

            match mgr.create_agent(provider, cwd, title, ws_id, Some(config)) {
                Ok(record) => {
                    crate::agent::runner::broadcast_agent_status_update(&record.id);
                    let snapshot = agent_record_to_snapshot(&record);
                    serde_json::json!({
                        "type": "status",
                        "requestId": request_id,
                        "payload": {
                            "status": "agent_resumed",
                            "requestId": request_id,
                            "agent": snapshot,
                            "timelineSize": 0
                        }
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "status",
                        "requestId": request_id,
                        "payload": {
                            "status": "agent_create_failed",
                            "requestId": request_id,
                            "error": err
                        }
                    })
                }
            }
        }
        "clear_agent_attention" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mgr = get_agent_manager();
            let res = mgr.clear_agent_attention(agent_id);
            if res.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            serde_json::json!({
                "type": "clear_agent_attention_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "cleared": res.is_ok()
                }
            })
        }
        "agent.config.apply.request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let thinking = msg
                .get("config")
                .or_else(|| payload.and_then(|p| p.get("config")))
                .and_then(|config| config.get("thinkingOptionId"));
            let mgr = get_agent_manager();
            let result = match thinking {
                Some(serde_json::Value::Null) => mgr.set_agent_thinking(agent_id, None),
                Some(serde_json::Value::String(option)) => {
                    mgr.set_agent_thinking(agent_id, Some(option))
                }
                Some(_) => Err("Invalid thinkingOptionId".to_string()),
                None => mgr
                    .get_agent(agent_id)
                    .ok_or_else(|| format!("Agent {agent_id} not found")),
            };
            if result.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            serde_json::json!({
                "type": "agent.config.apply.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": result.is_ok(),
                    "error": result.as_ref().err(),
                    "notice": null
                }
            })
        }
        "set_agent_mode_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mode_id = msg
                .get("modeId")
                .or_else(|| payload.and_then(|p| p.get("modeId")))
                .and_then(|v| v.as_str());
            let mgr = get_agent_manager();
            let res = mgr.set_agent_mode(agent_id, mode_id);
            if res.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            serde_json::json!({
                "type": "set_agent_mode_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "error": res.err(),
                    "notice": null
                }
            })
        }
        "set_agent_model_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let model_id = msg
                .get("modelId")
                .or_else(|| payload.and_then(|p| p.get("modelId")))
                .or_else(|| msg.get("model"))
                .or_else(|| payload.and_then(|p| p.get("model")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let provider_opt = msg
                .get("provider")
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .and_then(|v| v.as_str());

            let mgr = get_agent_manager();
            let res = if !agent_id.is_empty() && !model_id.is_empty() {
                mgr.set_agent_model_with_provider(agent_id, model_id, provider_opt)
            } else {
                Err("Missing agentId or modelId".to_string())
            };

            if res.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }

            serde_json::json!({
                "type": "set_agent_model_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "error": res.err(),
                    "notice": null
                }
            })
        }
        "set_agent_feature_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let feature_id = msg
                .get("featureId")
                .or_else(|| payload.and_then(|p| p.get("featureId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let value = msg
                .get("value")
                .or_else(|| payload.and_then(|p| p.get("value")))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let mgr = get_agent_manager();
            let res = mgr.set_agent_feature(agent_id, feature_id, value);
            if res.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            serde_json::json!({
                "type": "set_agent_feature_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "error": res.err(),
                    "notice": null
                }
            })
        }
        "set_agent_thinking_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let thinking = msg
                .get("thinkingOptionId")
                .or_else(|| payload.and_then(|p| p.get("thinkingOptionId")));
            let result = match thinking {
                Some(serde_json::Value::Null) => {
                    get_agent_manager().set_agent_thinking(agent_id, None)
                }
                Some(serde_json::Value::String(option)) => {
                    get_agent_manager().set_agent_thinking(agent_id, Some(option))
                }
                _ => Err("Missing or invalid thinkingOptionId".to_string()),
            };
            if result.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            serde_json::json!({
                "type": "set_agent_thinking_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": result.is_ok(),
                    "error": result.as_ref().err(),
                    "notice": if result.as_ref().is_ok_and(|record| record.last_status == crate::agent::types::AgentStatus::Running) {
                        serde_json::json!({"type": "warning", "message": "Thinking level applies next turn"})
                    } else {
                        serde_json::Value::Null
                    }
                }
            })
        }
        "update_agent_request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mgr = get_agent_manager();
            if let Some(mut agent) = mgr.get_agent(agent_id) {
                let mode_id = payload
                    .and_then(|p| p.get("modeId"))
                    .or_else(|| msg.get("modeId"))
                    .and_then(|v| v.as_str());
                let model = payload
                    .and_then(|p| p.get("model"))
                    .or_else(|| msg.get("model"))
                    .and_then(|v| v.as_str());
                let thinking = payload
                    .and_then(|p| p.get("thinkingOptionId"))
                    .or_else(|| msg.get("thinkingOptionId"))
                    .and_then(|v| v.as_str());
                let title = payload
                    .and_then(|p| p.get("title"))
                    .or_else(|| msg.get("title"))
                    .and_then(|v| v.as_str());

                let mut cfg = agent.config.unwrap_or_else(|| serde_json::json!({}));
                if let Some(obj) = cfg.as_object_mut() {
                    if let Some(mode) = mode_id {
                        obj.insert("modeId".to_string(), serde_json::json!(mode));
                        obj.insert("persona".to_string(), serde_json::json!(mode));
                    }
                    if let Some(m) = model {
                        obj.insert("model".to_string(), serde_json::json!(m));
                    }
                    if let Some(t) = thinking {
                        obj.insert("thinkingOptionId".to_string(), serde_json::json!(t));
                    }
                }
                agent.config = Some(cfg);
                if let Some(ttl) = title {
                    agent.title = Some(ttl.to_string());
                }
                let _ = mgr.storage().save_record(agent);
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            serde_json::json!({
                "type": "update_agent_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": true,
                    "error": null,
                    "notice": null
                }
            })
        }
        "agent.detach.request" => {
            let payload = msg.get("payload");
            let agent_id = msg
                .get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let mgr = get_agent_manager();
            let res = mgr.detach_agent(agent_id);
            if res.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            serde_json::json!({
                "type": "agent.detach.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "error": res.err(),
                    "notice": null
                }
            })
        }
        "list_commands_request" => {
            serde_json::json!({
                "type": "list_commands_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "commands": [
                        {
                            "name": "council",
                            "description": "Activate multi-agent Council swarm: Architect, Implementer, Auditor",
                            "argumentHint": "<task description>",
                            "kind": "system"
                        },
                        {
                            "name": "swarm",
                            "description": "Coordinate concurrent subagents for parallel execution",
                            "argumentHint": "<task description>",
                            "kind": "system"
                        },
                        {
                            "name": "plan",
                            "description": "Formulate a step-by-step implementation plan before coding",
                            "argumentHint": "<feature or fix>",
                            "kind": "system"
                        },
                        {
                            "name": "compact",
                            "description": "Summarize and compact previous context to free tokens",
                            "argumentHint": "",
                            "kind": "system"
                        },
                        {
                            "name": "diff",
                            "description": "Inspect uncommitted git diffs across the workspace",
                            "argumentHint": "",
                            "kind": "system"
                        },
                        {
                            "name": "test",
                            "description": "Run test suites and verify changes",
                            "argumentHint": "[test filter]",
                            "kind": "system"
                        }
                    ]
                }
            })
        }
        "agent.skills.get_status.request" => {
            rpc_helpers::handle_agent_skills_rpc("get_status", msg, &request_id)
        }
        "agent.skills.reconcile.request" => {
            rpc_helpers::handle_agent_skills_rpc("reconcile", msg, &request_id)
        }
        "agent.skills.uninstall.request" => {
            rpc_helpers::handle_agent_skills_rpc("uninstall", msg, &request_id)
        }
        "agent.skills.save_selection.request" => {
            rpc_helpers::handle_agent_skills_rpc("save_selection", msg, &request_id)
        }
        "agent.skills.import_legacy_selection.request" => {
            rpc_helpers::handle_agent_skills_rpc("import_legacy", msg, &request_id)
        }
        "workspace.label.list.request" => {
            rpc_helpers::handle_workspace_label_list(msg, &request_id)
        }
        "workspace.label.assignment.set.request" => {
            rpc_helpers::handle_workspace_label_assignment_set(msg, &request_id)
        }
        "workspace.label.update.request" => {
            rpc_helpers::handle_workspace_label_update(msg, &request_id)
        }
        "workspace.label.delete.request" => {
            rpc_helpers::handle_workspace_label_delete(msg, &request_id)
        }
        "workspace.label.delete.inspect.request" => {
            rpc_helpers::handle_workspace_label_delete_inspect(msg, &request_id)
        }
        "workspace.setup.subscribe.request" => {
            let payload = msg.get("payload");
            let ws_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            rpc_helpers::handle_workspace_setup_subscribe(ws_id, &request_id)
        }
        "workspace_setup_status_request" | "workspace.setup.status.request" => {
            let payload = msg.get("payload");
            let ws_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            rpc_helpers::handle_workspace_setup_status(ws_id, &request_id)
        }
        "agent.timeline.set_subscription.request" => {
            let payload = msg.get("payload");
            let agent_ids = msg
                .get("agentIds")
                .or_else(|| payload.and_then(|p| p.get("agentIds")))
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            serde_json::json!({
                "type": "agent.timeline.set_subscription.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-timeline-{}", request_id),
                    "agentIds": agent_ids
                }
            })
        }
        "creation.subscribe.request" => {
            let payload = msg.get("payload");
            let key = msg
                .get("idempotencyKey")
                .or_else(|| payload.and_then(|p| p.get("idempotencyKey")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let snapshot = CREATION_SNAPSHOTS
                .get()
                .and_then(|m| m.read().ok())
                .and_then(|m| m.get(key).cloned())
                .unwrap_or(serde_json::Value::Null);
            let sub_id = register_subscription(format!("sub-create-{}", request_id), "creation");
            serde_json::json!({
                "type": "creation.subscribe.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id,
                    "snapshot": snapshot,
                    "error": null
                }
            })
        }
        "subscribe_checkout_diff_request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            rpc_helpers::handle_subscribe_checkout_diff(cwd_str, &request_id)
        }
        "checkout.diff.get.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            rpc_helpers::handle_checkout_diff_get(cwd_str, &request_id)
        }
        "unsubscribe_checkout_diff_request" => {
            serde_json::json!({
                "type": "unsubscribe_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "success": true
                }
            })
        }
        "get_providers_snapshot_request" => {
            let now_iso = crate::agent::types::now_iso();
            let cwd = msg
                .get("cwd")
                .and_then(|value| value.as_str())
                .unwrap_or(".");
            let entries = get_provider_snapshot_entries(cwd).await;
            serde_json::json!({
                "type": "get_providers_snapshot_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "entries": entries,
                    "generatedAt": now_iso
                }
            })
        }
        "list_available_providers_request" => {
            let now_iso = crate::agent::types::now_iso();
            let entries = get_provider_snapshot_entries(".").await;
            let providers: Vec<serde_json::Value> = entries
                .iter()
                .filter_map(|entry| {
                    let provider = entry.get("provider")?.as_str()?;
                    let available = entry
                        .get("models")?
                        .as_array()
                        .is_some_and(|models| !models.is_empty());
                    Some(serde_json::json!({ "provider": provider, "available": available }))
                })
                .collect();
            serde_json::json!({
                "type": "list_available_providers_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "providers": providers,
                    "fetchedAt": now_iso
                }
            })
        }
        "refresh_providers_snapshot_request" => {
            invalidate_provider_snapshot_cache().await;
            serde_json::json!({
                "type": "refresh_providers_snapshot_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "acknowledged": true
                }
            })
        }
        "provider_diagnostic_request" => {
            let payload = msg.get("payload");
            let provider = msg
                .get("provider")
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .and_then(|v| v.as_str())
                .unwrap_or("codex");
            rpc_helpers::handle_provider_diagnostic(provider, &request_id)
        }
        "browser.host.register.request" => {
            let sub_id = register_subscription(format!("sub-browser-{}", request_id), "browser_host");
            let entry = msg.get("payload").cloned().unwrap_or_else(|| msg.clone());
            let map = BROWSER_HOSTS.get_or_init(|| std::sync::RwLock::new(Default::default()));
            {
                let mut guard = map.write().unwrap_or_else(|e| e.into_inner());
                if guard.len() > 64 {
                    guard.clear();
                }
                guard.insert(sub_id.clone(), entry);
            }
            serde_json::json!({
                "type": "browser.host.register.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id
                }
            })
        }
        "browser.automation.execute.request" => {
            let payload = crate::browser_automation::execute_command_direct(msg).await;
            serde_json::json!({
                "type": "browser.automation.execute.response",
                "requestId": request_id,
                "payload": payload
            })
        }
        "get_daemon_config_request" => rpc_helpers::handle_get_daemon_config(&request_id),
        "set_daemon_config_request" => rpc_helpers::handle_set_daemon_config(msg, &request_id),
        "daemon.get_status.request" => rpc_helpers::handle_daemon_get_status(&request_id),
        "plugin.catalog.get.request" => rpc_helpers::handle_plugin_list_or_catalog(true, &request_id),
        "plugin.list.request" => rpc_helpers::handle_plugin_list_or_catalog(false, &request_id),
        "diagnostics.request" => {
            let agent_count = get_agent_manager().list_agents().len();
            let speech_loaded = get_speech_engine().read().await.is_model_loaded();
            rpc_helpers::handle_diagnostics_request(agent_count, speech_loaded, &request_id)
        }
        "workspace.script.list.request" => {
            let payload = msg.get("payload");
            let ws_id = msg
                .get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            rpc_helpers::handle_workspace_script_list(ws_id, &request_id)
        }
        "checkout_status_request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            rpc_helpers::handle_checkout_status(cwd_str, &request_id)
        }
        "fs.file.subscribe.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path_str = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sub_opt = msg
                .get("subscriptionId")
                .or_else(|| payload.and_then(|p| p.get("subscriptionId")))
                .and_then(|v| v.as_str());
            rpc_helpers::handle_file_subscribe(cwd_str, path_str, sub_opt, &request_id)
        }
        "fs.file.unsubscribe.request" => {
            let payload = msg.get("payload");
            let sub_id = msg
                .get("subscriptionId")
                .or_else(|| payload.and_then(|p| p.get("subscriptionId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "fs.file.unsubscribe.response",
                "requestId": request_id,
                "payload": {
                    "subscriptionId": sub_id,
                    "requestId": request_id
                }
            })
        }
        "mcp/request" | "mcp/jsonrpc" | "mcp_request" => {
            // Provide the response object in both `payload.response` (standard RPC envelope structure)
            // and top-level `response` to maintain compatibility across legacy WebSocket clients and newer MCP consumers.
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
        "vfs/watch" | "vfs/watch_subscribe" => {
            let payload = msg.get("payload");
            let path = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ignore: Vec<String> = msg
                .get("ignore")
                .or_else(|| payload.and_then(|p| p.get("ignore")))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            if path.is_empty() {
                serde_json::json!({
                    "type": "vfs/watch/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": "Missing 'path' parameter",
                    "payload": {
                        "error": "Missing 'path' parameter"
                    }
                })
            } else {
                let engine = get_watcher_engine();
                match engine.watch(path, &ignore) {
                    Ok(sub_id) => {
                        serde_json::json!({
                            "type": "vfs/watch/response",
                            "requestId": request_id,
                            "id": request_id,
                            "success": true,
                            "payload": {
                                "subscriptionId": sub_id,
                                "path": path,
                            },
                            "subscriptionId": sub_id,
                            "path": path,
                        })
                    }
                    Err(err) => {
                        serde_json::json!({
                            "type": "vfs/watch/response",
                            "requestId": request_id,
                            "id": request_id,
                            "success": false,
                            "error": err,
                            "payload": {
                                "error": err
                            }
                        })
                    }
                }
            }
        }
        "vfs/unwatch" | "vfs/watch_unsubscribe" => {
            let payload = msg.get("payload");
            let sub_id = msg
                .get("subscriptionId")
                .or_else(|| payload.and_then(|p| p.get("subscriptionId")))
                .or_else(|| msg.get("subscription_id"))
                .or_else(|| payload.and_then(|p| p.get("subscription_id")))
                .or_else(|| msg.get("sub_id"))
                .or_else(|| payload.and_then(|p| p.get("sub_id")))
                .or_else(|| msg.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if sub_id.is_empty() {
                serde_json::json!({
                    "type": "vfs/unwatch/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": "Missing 'subscriptionId' parameter",
                    "payload": {
                        "error": "Missing 'subscriptionId' parameter"
                    }
                })
            } else {
                let engine = get_watcher_engine();
                match engine.unwatch(sub_id) {
                    Ok(unwatched) => {
                        serde_json::json!({
                            "type": "vfs/unwatch/response",
                            "requestId": request_id,
                            "id": request_id,
                            "success": true,
                            "payload": {
                                "unwatched": unwatched,
                                "subscriptionId": sub_id,
                            },
                            "unwatched": unwatched,
                            "subscriptionId": sub_id,
                        })
                    }
                    Err(err) => {
                        serde_json::json!({
                            "type": "vfs/unwatch/response",
                            "requestId": request_id,
                            "id": request_id,
                            "success": false,
                            "error": err,
                            "payload": {
                                "error": err
                            }
                        })
                    }
                }
            }
        }
        "file_explorer_request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path_opt = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str());
            let mode = msg
                .get("mode")
                .or_else(|| payload.and_then(|p| p.get("mode")))
                .and_then(|v| v.as_str())
                .unwrap_or("list");

            let cwd = std::path::Path::new(cwd_str);

            if mode == "file" {
                let target_rel = path_opt.unwrap_or("");
                let full_path = cwd.join(target_rel);
                let meta = std::fs::metadata(&full_path).ok();
                let file_size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                const MAX_EXPLORER_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB limit for explorer preview

                if file_size > MAX_EXPLORER_FILE_SIZE {
                    let modified_at = meta
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| {
                            crate::agent::sessions::format_epoch_millis_iso(d.as_millis() as i64)
                        })
                        .unwrap_or_else(crate::agent::types::now_iso);
                    serde_json::json!({
                        "type": "file_explorer_response",
                        "payload": {
                            "cwd": cwd_str,
                            "path": target_rel,
                            "mode": "file",
                            "directory": serde_json::Value::Null,
                            "file": {
                                "path": target_rel,
                                "kind": "binary",
                                "encoding": "none",
                                "content": format!("[File exceeds maximum preview size of 10 MB (size: {} MB)]", file_size / (1024 * 1024)),
                                "mimeType": "application/octet-stream",
                                "size": file_size,
                                "modifiedAt": modified_at
                            },
                            "error": serde_json::Value::Null,
                            "requestId": request_id
                        }
                    })
                } else {
                    match crate::kernel::vfs::get_global_shadow_vfs().read_file_cow(&full_path) {
                        Ok(bytes) => {
                            let size = bytes.len() as u64;
                            let modified_at = meta
                                .and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| {
                                    crate::agent::sessions::format_epoch_millis_iso(
                                        d.as_millis() as i64
                                    )
                                })
                                .unwrap_or_else(crate::agent::types::now_iso);
                            let (kind, encoding, content) = match std::str::from_utf8(&bytes) {
                                Ok(s) => ("text", "utf-8", s.to_string()),
                                Err(_) => (
                                    "binary",
                                    "base64",
                                    base64::Engine::encode(
                                        &base64::engine::general_purpose::STANDARD,
                                        &bytes,
                                    ),
                                ),
                            };
                            let mime_type = if kind == "text" {
                                "text/plain"
                            } else {
                                "application/octet-stream"
                            };
                            serde_json::json!({
                                "type": "file_explorer_response",
                                "payload": {
                                    "cwd": cwd_str,
                                    "path": target_rel,
                                    "mode": "file",
                                    "directory": serde_json::Value::Null,
                                    "file": {
                                        "path": target_rel,
                                        "kind": kind,
                                        "encoding": encoding,
                                        "content": content,
                                        "mimeType": mime_type,
                                        "size": size,
                                        "modifiedAt": modified_at
                                    },
                                    "error": serde_json::Value::Null,
                                    "requestId": request_id
                                }
                            })
                        }
                        Err(err) => {
                            serde_json::json!({
                                "type": "file_explorer_response",
                                "payload": {
                                    "cwd": cwd_str,
                                    "path": target_rel,
                                    "mode": "file",
                                    "directory": serde_json::Value::Null,
                                    "file": serde_json::Value::Null,
                                    "error": err.to_string(),
                                    "requestId": request_id
                                }
                            })
                        }
                    }
                }
            } else {
                let target_path = path_opt.unwrap_or(".");
                match crate::kernel::file_service::list_directory(cwd, path_opt) {
                    Ok(entries) => {
                        let entries_val: Vec<serde_json::Value> = entries
                            .into_iter()
                            .map(|e| {
                                serde_json::json!({
                                    "name": e.name,
                                    "path": e.path,
                                    "kind": if e.is_directory { "directory" } else { "file" },
                                    "size": e.size,
                                    "modifiedAt": crate::agent::sessions::format_epoch_millis_iso(e.modified_ms as i64),
                                })
                            })
                            .collect();
                        serde_json::json!({
                            "type": "file_explorer_response",
                            "payload": {
                                "cwd": cwd_str,
                                "path": target_path,
                                "mode": "list",
                                "directory": {
                                    "path": target_path,
                                    "entries": entries_val
                                },
                                "file": serde_json::Value::Null,
                                "error": serde_json::Value::Null,
                                "requestId": request_id
                            }
                        })
                    }
                    Err(err) => {
                        serde_json::json!({
                            "type": "file_explorer_response",
                            "payload": {
                                "cwd": cwd_str,
                                "path": target_path,
                                "mode": "list",
                                "directory": serde_json::Value::Null,
                                "file": serde_json::Value::Null,
                                "error": err.to_string(),
                                "requestId": request_id
                            }
                        })
                    }
                }
            }
        }
        "fs.file.write.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let rel_path = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let content_str = msg
                .get("content")
                .or_else(|| payload.and_then(|p| p.get("content")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::file_service::write_file(cwd, rel_path, content_str.as_bytes()) {
                Ok(()) => {
                    let _ = crate::kernel::vfs::get_global_shadow_vfs().remove_virtual(&cwd.join(rel_path));
                    serde_json::json!({
                        "type": "fs.file.write.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": { "success": true }
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "fs.file.write.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": err.to_string(),
                        "payload": { "error": err.to_string() }
                    })
                }
            }
        }
        "fs.entry.create.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let parent = msg
                .get("parentPath")
                .or_else(|| payload.and_then(|p| p.get("parentPath")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let name = msg
                .get("name")
                .or_else(|| payload.and_then(|p| p.get("name")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let kind = msg
                .get("kind")
                .or_else(|| payload.and_then(|p| p.get("kind")))
                .and_then(|v| v.as_str())
                .unwrap_or("file");

            let cwd = std::path::Path::new(cwd_str);
            let is_dir = kind == "directory";
            match crate::kernel::file_service::create_entry(cwd, parent, name, is_dir) {
                Ok(path) => {
                    serde_json::json!({
                        "type": "fs.entry.create.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": { "success": true, "path": path }
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "fs.entry.create.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": err.to_string(),
                        "payload": { "error": err.to_string() }
                    })
                }
            }
        }
        "fs.entry.rename.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new_name = msg
                .get("newName")
                .or_else(|| payload.and_then(|p| p.get("newName")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::file_service::rename_entry(cwd, path, new_name) {
                Ok(new_path) => {
                    serde_json::json!({
                        "type": "fs.entry.rename.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": { "success": true, "path": new_path }
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "fs.entry.rename.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": err.to_string(),
                        "payload": { "error": err.to_string() }
                    })
                }
            }
        }
        "fs.entry.delete.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg
                .get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::file_service::delete_entry(cwd, path) {
                Ok(()) => {
                    serde_json::json!({
                        "type": "fs.entry.delete.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": { "success": true }
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "fs.entry.delete.response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": err.to_string(),
                        "payload": { "error": err.to_string() }
                    })
                }
            }
        }
        "ast/analyze" | "ast/parse" => {
            let payload = msg.get("payload");
            let path_opt = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str());
            let source_opt = msg
                .get("source")
                .or_else(|| payload.and_then(|p| p.get("source")))
                .and_then(|v| v.as_str());
            let ext_opt = msg
                .get("ext")
                .or_else(|| payload.and_then(|p| p.get("ext")))
                .and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| {
                        path_opt.and_then(|p| {
                            std::path::Path::new(p).extension().and_then(|e| e.to_str())
                        })
                    })
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| {
                                std::path::Path::new(path)
                                    .extension()
                                    .and_then(|e| e.to_str())
                            })
                            .unwrap_or("ts");
                        (content, ext.to_string())
                    }
                    Err(e) => {
                        return serde_json::json!({
                            "type": "ast/analyze/response",
                            "requestId": request_id,
                            "id": request_id,
                            "success": false,
                            "error": format!("Failed to read file '{path}': {e}"),
                            "payload": {
                                "error": format!("Failed to read file '{path}': {e}")
                            }
                        });
                    }
                }
            } else {
                return serde_json::json!({
                    "type": "ast/analyze/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": "Missing 'source' or 'path' parameter",
                    "payload": {
                        "error": "Missing 'source' or 'path' parameter"
                    }
                });
            };

            let engine = crate::kernel::ast::AstEngine::new();
            match engine.analyze(&source, &ext) {
                Ok(result) => {
                    let val = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
                    serde_json::json!({
                        "type": "ast/analyze/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": val,
                        "result": val,
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "ast/analyze/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": err,
                        "payload": {
                            "error": err
                        }
                    })
                }
            }
        }
        "ast/symbols" => {
            let payload = msg.get("payload");
            let path_opt = msg
                .get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str());
            let source_opt = msg
                .get("source")
                .or_else(|| payload.and_then(|p| p.get("source")))
                .and_then(|v| v.as_str());
            let ext_opt = msg
                .get("ext")
                .or_else(|| payload.and_then(|p| p.get("ext")))
                .and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| {
                        path_opt.and_then(|p| {
                            std::path::Path::new(p).extension().and_then(|e| e.to_str())
                        })
                    })
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| {
                                std::path::Path::new(path)
                                    .extension()
                                    .and_then(|e| e.to_str())
                            })
                            .unwrap_or("ts");
                        (content, ext.to_string())
                    }
                    Err(e) => {
                        return serde_json::json!({
                            "type": "ast/symbols/response",
                            "requestId": request_id,
                            "id": request_id,
                            "success": false,
                            "error": format!("Failed to read file '{path}': {e}"),
                            "payload": {
                                "error": format!("Failed to read file '{path}': {e}")
                            }
                        });
                    }
                }
            } else {
                return serde_json::json!({
                    "type": "ast/symbols/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": "Missing 'source' or 'path' parameter",
                    "payload": {
                        "error": "Missing 'source' or 'path' parameter"
                    }
                });
            };

            let engine = crate::kernel::ast::AstEngine::new();
            match engine.extract_symbols(&source, &ext) {
                Ok(symbols) => {
                    let val = serde_json::to_value(&symbols).unwrap_or(serde_json::Value::Null);
                    serde_json::json!({
                        "type": "ast/symbols/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": val,
                        "symbols": val,
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "ast/symbols/response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": err,
                        "payload": {
                            "error": err
                        }
                    })
                }
            }
        }
        msg_type
            if msg_type.starts_with("crdt/")
                || msg_type.starts_with("ioring/")
                || msg_type.starts_with("speech/")
                || msg_type.starts_with("shm/")
                || msg_type.starts_with("vfs/")
                || msg_type.starts_with("lsp/") =>
        {
            let env = RpcEnvelope::new(
                request_id,
                msg_type,
                msg.get("payload").cloned().unwrap_or_else(|| msg.clone()),
            );
            let resp = dispatch_envelope(env).await;
            serde_json::json!({
                "type": resp.msg_type,
                "requestId": resp.id,
                "id": resp.id,
                "success": resp.success,
                "payload": resp.data,
                "data": resp.data,
                "error": resp.error,
            })
        }
        "subscribe_providers_request" | "subscribe_providers" => {
            // ACK the subscription immediately, then push providers_snapshot_update via broadcast channel
            let req_id = request_id.to_owned();
            let cwd = msg
                .get("cwd")
                .and_then(|v| v.as_str())
                .unwrap_or(".")
                .to_owned();
            let push_tx = get_provider_push_tx();
            tokio::spawn(async move {
                let entries = get_provider_snapshot_entries(&cwd).await;
                let now_iso = crate::agent::types::now_iso();
                let update = serde_json::json!({
                    "type": "session",
                    "message": {
                        "type": "providers_snapshot_update",
                        "payload": {
                            "subscriptionId": format!("sub-providers-{}", req_id),
                            "cwd": cwd,
                            "entries": entries,
                            "generatedAt": now_iso
                        }
                    }
                });
                let _ = push_tx.send(serde_json::to_string(&update).unwrap_or_default());
            });
            let sub_id = register_subscription(format!("sub-providers-{}", request_id), "providers");
            serde_json::json!({
                "type": "subscribe_providers_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id,
                    "error": null
                }
            })
        }
        "workspace.setup.run.request" => {
            let ws_id = rpc_helpers::msg_str(msg, "workspaceId");
            rpc_helpers::handle_workspace_setup_run(ws_id, &request_id)
        }
        "checkout.refresh.request" => {
            let cwd_str = rpc_helpers::msg_str(msg, "cwd");
            rpc_helpers::handle_checkout_status(cwd_str, &request_id)
        }
        "checkout_commit_request" => rpc_helpers::handle_checkout_commit(msg, &request_id),
        "checkout_pull_request" => rpc_helpers::handle_checkout_pull(msg, &request_id),
        "checkout_push_request" => rpc_helpers::handle_checkout_push(msg, &request_id),
        "checkout_merge_request" => rpc_helpers::handle_checkout_merge(msg, &request_id, false),
        "checkout_merge_from_base_request" => {
            rpc_helpers::handle_checkout_merge(msg, &request_id, true)
        }
        "checkout.discard_changes.request" => {
            rpc_helpers::handle_checkout_discard_changes(msg, &request_id)
        }
        "checkout_switch_branch_request" => {
            rpc_helpers::handle_checkout_switch_branch(msg, &request_id)
        }
        "checkout.rename_branch.request" => {
            rpc_helpers::handle_checkout_rename_branch(msg, &request_id)
        }
        "checkout.commits.list.request" => {
            rpc_helpers::handle_checkout_commits_list(msg, &request_id)
        }
        "checkout.commits.file_diff.request" => {
            rpc_helpers::handle_checkout_commit_file_diff(msg, &request_id)
        }
        "stash_save_request" => rpc_helpers::handle_stash_save(msg, &request_id),
        "stash_pop_request" => rpc_helpers::handle_stash_pop(msg, &request_id),
        "stash_list_request" => rpc_helpers::handle_stash_list(msg, &request_id),
        "validate_branch_request" => rpc_helpers::handle_validate_branch(msg, &request_id),
        "branch_suggestions_request" => rpc_helpers::handle_branch_suggestions(msg, &request_id),
        "checkout_pr_status_request" => rpc_helpers::handle_checkout_pr_status(msg, &request_id),
        "checkout_pr_create_request" => rpc_helpers::handle_checkout_pr_create(msg, &request_id),
        "checkout_pr_merge_request" => rpc_helpers::handle_checkout_pr_merge(msg, &request_id),
        "archive_workspace_request" => {
            rpc_helpers::handle_workspace_meta_mutation("archive", msg, &request_id)
        }
        "workspace.title.set.request" => {
            rpc_helpers::handle_workspace_meta_mutation("title", msg, &request_id)
        }
        "workspace.pin.set.request" => {
            rpc_helpers::handle_workspace_meta_mutation("pin", msg, &request_id)
        }
        "workspace.clear_attention.request" => {
            rpc_helpers::handle_workspace_meta_mutation("clear_attention", msg, &request_id)
        }
        "workspace.mark_unread.request" => {
            rpc_helpers::handle_workspace_meta_mutation("mark_unread", msg, &request_id)
        }
        "workspace.base_branch.set.request" => {
            rpc_helpers::handle_workspace_meta_mutation("base_branch", msg, &request_id)
        }
        "workspace.metadata.update.request" => {
            rpc_helpers::handle_workspace_meta_mutation("metadata", msg, &request_id)
        }
        "project.rename.request" => rpc_helpers::handle_project_rename(msg, &request_id),
        "read_project_config_request" => {
            rpc_helpers::handle_read_project_config(msg, &request_id)
        }
        "write_project_config_request" => {
            rpc_helpers::handle_write_project_config(msg, &request_id)
        }
        "project.suggestions.request" => {
            rpc_helpers::handle_directory_suggestions(msg, &request_id, true)
        }
        "refresh_agent_request" => {
            let agent_id = rpc_helpers::msg_str(msg, "agentId");
            let mgr = get_agent_manager();
            if let Some(rec) = mgr.get_agent(agent_id) {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
                let mut cache = std::collections::HashMap::new();
                let cwd = if rec.cwd.is_empty() { "." } else { &rec.cwd };
                let project = rpc_helpers::inspect_project_placement(
                    cwd,
                    rec.workspace_id.as_deref(),
                    &mut cache,
                );
                serde_json::json!({
                    "type": "refresh_agent_response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "agent": agent_record_to_snapshot(&rec),
                        "project": project,
                        "error": null
                    }
                })
            } else {
                serde_json::json!({
                    "type": "refresh_agent_response",
                    "requestId": request_id,
                    "payload": {
                        "requestId": request_id,
                        "agent": null,
                        "error": format!("Agent {agent_id} not found")
                    }
                })
            }
        }
        "abort_request" => {
            let agent_id = rpc_helpers::msg_str(msg, "agentId");
            let res = get_agent_manager().cancel_agent(agent_id);
            serde_json::json!({
                "type": "abort_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "error": res.err()
                }
            })
        }
        "wait_for_finish_request" => {
            let agent_id = rpc_helpers::msg_str(msg, "agentId").to_string();
            let timeout_ms = msg
                .get("timeoutMs")
                .or_else(|| msg.get("payload").and_then(|p| p.get("timeoutMs")))
                .and_then(|v| v.as_u64())
                .unwrap_or(30_000)
                .min(120_000);
            let mgr = get_agent_manager();
            let deadline = Instant::now() + Duration::from_millis(timeout_ms);
            loop {
                let is_running = mgr
                    .get_agent(&agent_id)
                    .is_some_and(|r| r.last_status == crate::agent::types::AgentStatus::Running);
                if !is_running || Instant::now() >= deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let final_rec = mgr.get_agent(&agent_id);
            serde_json::json!({
                "type": "wait_for_finish_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "agent": final_rec.as_ref().map(agent_record_to_snapshot),
                    "error": if final_rec.is_none() { serde_json::json!(format!("Agent {agent_id} not found")) } else { serde_json::Value::Null }
                }
            })
        }
        "agent.metadata.update.request" => {
            let agent_id = rpc_helpers::msg_str(msg, "agentId");
            let patch = msg
                .get("metadata")
                .or_else(|| msg.get("payload").and_then(|p| p.get("metadata")))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let res = get_agent_manager().update_agent_metadata(agent_id, patch);
            if res.is_ok() {
                crate::agent::runner::broadcast_agent_status_update(agent_id);
            }
            let metadata = res
                .as_ref()
                .ok()
                .and_then(|r| r.config.as_ref())
                .and_then(|c| c.get("metadata"))
                .cloned();
            serde_json::json!({
                "type": "agent.metadata.update.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "metadata": metadata,
                    "error": res.err()
                }
            })
        }
        "agent.timeline.search.request" => {
            let agent_id = rpc_helpers::msg_str(msg, "agentId");
            let query = rpc_helpers::msg_str(msg, "query").to_ascii_lowercase();
            let mgr = get_agent_manager();
            let locations: Vec<serde_json::Value> = mgr
                .get_timeline(agent_id)
                .into_iter()
                .enumerate()
                .filter_map(|(seq, item)| {
                    let (role, text) = match &item {
                        crate::agent::types::AgentTimelineItem::UserMessage { text, .. } => {
                            ("user", text.as_str())
                        }
                        crate::agent::types::AgentTimelineItem::AssistantMessage { text, .. } => {
                            ("assistant", text.as_str())
                        }
                        _ => return None,
                    };
                    if query.is_empty() {
                        return None;
                    }
                    let count = text.to_ascii_lowercase().matches(&query).count();
                    (count > 0).then(|| serde_json::json!({ "seq": seq, "role": role, "count": count }))
                })
                .collect();
            serde_json::json!({
                "type": "agent.timeline.search.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "epoch": "v1",
                    "locations": locations,
                    "nextCursor": null,
                    "error": null
                }
            })
        }
        "agent.timeline.list_prompts.request" => {
            let agent_id = rpc_helpers::msg_str(msg, "agentId");
            let mgr = get_agent_manager();
            let prompts: Vec<serde_json::Value> = mgr
                .get_timeline(agent_id)
                .into_iter()
                .enumerate()
                .filter_map(|(seq, item)| match item {
                    crate::agent::types::AgentTimelineItem::UserMessage {
                        text, timestamp, ..
                    } => {
                        let preview: String = text.trim().chars().take(120).collect();
                        Some(serde_json::json!({
                            "seq": seq,
                            "timestamp": timestamp,
                            "preview": preview
                        }))
                    }
                    _ => None,
                })
                .collect();
            serde_json::json!({
                "type": "agent.timeline.list_prompts.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "epoch": "v1",
                    "prompts": prompts,
                    "error": null
                }
            })
        }
        "agent.timeline.append.request" => {
            let agent_id = rpc_helpers::msg_str(msg, "agentId");
            let text = rpc_helpers::msg_str(msg, "text");
            let role = rpc_helpers::msg_str(msg, "role");
            let mgr = get_agent_manager();
            let res = if role == "thinking" {
                mgr.append_thinking(agent_id, text)
            } else {
                mgr.append_assistant_message(agent_id, text)
            };
            serde_json::json!({
                "type": "agent.timeline.append.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "accepted": res.is_ok(),
                    "error": res.err()
                }
            })
        }
        "list_provider_models_request" => {
            let provider = rpc_helpers::msg_str(msg, "provider");
            let cwd = rpc_helpers::msg_str(msg, "cwd");
            let cwd_eff = if cwd.trim().is_empty() { "." } else { cwd };
            let entries = get_provider_snapshot_entries(cwd_eff).await;
            let models = entries
                .into_iter()
                .find(|e| e.get("provider").and_then(|p| p.as_str()) == Some(provider))
                .and_then(|e| e.get("models").cloned())
                .unwrap_or_else(|| serde_json::json!([]));
            serde_json::json!({
                "type": "list_provider_models_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "provider": provider,
                    "models": models,
                    "fetchedAt": crate::agent::types::now_iso(),
                    "error": null
                }
            })
        }
        "list_provider_modes_request" => {
            let provider = rpc_helpers::msg_str(msg, "provider");
            serde_json::json!({
                "type": "list_provider_modes_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "provider": provider,
                    "modes": default_available_modes(),
                    "fetchedAt": crate::agent::types::now_iso(),
                    "error": null
                }
            })
        }
        "list_provider_features_request" => {
            rpc_helpers::handle_list_provider_features(msg, &request_id)
        }
        "provider.controls.list.request" => {
            rpc_helpers::handle_provider_controls_list(msg, &request_id)
        }
        "daemon.get_pairing_offer.request" => {
            rpc_helpers::handle_daemon_get_pairing_offer(&request_id)
        }
        "dictation_stream_start" => {
            rpc_helpers::handle_dictation_stream_start(msg)
        }
        "dictation_stream_chunk" => {
            rpc_helpers::handle_dictation_stream_chunk(msg)
        }
        "dictation_stream_finish" => {
            rpc_helpers::handle_dictation_stream_finish(msg).await
        }
        "dictation_stream_cancel" => {
            rpc_helpers::handle_dictation_stream_cancel(msg)
        }
        "set_voice_mode" => {
            rpc_helpers::handle_set_voice_mode(msg, &request_id)
        }
        "voice_audio_chunk" => {
            rpc_helpers::handle_voice_audio_chunk(msg, &request_id).await
        }
        "audio_played" => {
            serde_json::json!({
                "type": "status",
                "requestId": request_id,
                "payload": {
                    "status": "acknowledged",
                    "requestId": request_id
                }
            })
        }
        other => {
            serde_json::json!({
                "type": "rpc_error",
                "requestId": request_id,
                "id": request_id,
                "payload": {
                    "requestId": request_id,
                    "request_id": request_id,
                    "requestType": other,
                    "error": format!("Unsupported message type in EmbeddedDaemon: {other}"),
                    "code": "not_implemented"
                }
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
            _ => {
                let res = dispatch_session_message(&val).await;
                return serde_json::to_string(&res).ok();
            }
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
    #[cfg(test)]
    pub(crate) access: Arc<crate::kernel::access::AccessControl>,
    #[cfg(test)]
    _auth_home: tempfile::TempDir,
}

pub type EmbeddedDaemon = DaemonHandle;

impl DaemonHandle {
    pub async fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_internal(6767, true, None).await
    }

    pub async fn start_test(port: u16) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_internal(port, false, None).await
    }

    pub async fn start_daemon(
        port: u16,
        custom_pipe: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) if is_production || port == 6767 => {
                eprintln!(
                    "[kernel] Port {} unavailable ({}), falling back to OS ephemeral port",
                    port, e
                );
                tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?
            }
            Err(e) => return Err(Box::new(e)),
        };
        let actual_port = listener.local_addr()?.port();
        crate::daemon::set_active_daemon_port(actual_port);

        #[cfg(test)]
        let auth_home = tempfile::tempdir()?;
        #[cfg(test)]
        let home = auth_home.path().to_path_buf();
        #[cfg(not(test))]
        let home = crate::get_data_dir();
        #[cfg(not(test))]
        if AGENT_MANAGER.get().is_none() {
            let manager = crate::agent::manager::AgentManager::try_new(home.clone())?;
            let _ = AGENT_MANAGER.set(manager);
        }
        #[cfg(not(test))]
        crate::kernel::schedules::start_scheduler_engine();
        #[cfg(not(test))]
        crate::kernel::config_store::get_config_store().sanitize_orphaned_workspaces();
        let access = Arc::new(crate::kernel::access::AccessControl::load(
            &home,
            actual_port,
        )?);
        let ws_routes = Router::new()
            .route(
                "/",
                get({
                    let tx = shutdown_tx.clone();
                    let access = access.clone();
                    move |ws: WebSocketUpgrade| {
                        let rx = tx.subscribe();
                        let access = access.clone();
                        async move {
                            ws.max_message_size(16 * 1024 * 1024)
                                .on_upgrade(move |socket| handle_ws(socket, rx, access))
                        }
                    }
                }),
            )
            .route(
                "/ws",
                get({
                    let tx = shutdown_tx.clone();
                    let access = access.clone();
                    move |ws: WebSocketUpgrade| {
                        let rx = tx.subscribe();
                        let access = access.clone();
                        async move {
                            ws.max_message_size(16 * 1024 * 1024)
                                .on_upgrade(move |socket| handle_ws(socket, rx, access))
                        }
                    }
                }),
            );
        let app = Router::new()
            .route("/api/health", get(health_check))
            .route("/health", get(health_check))
            .route("/api/version", get(version_check))
            .route(
                "/api/agent-history/indexed",
                get(handle_indexed_history_http),
            )
            .route("/api/omp-sessions", get(handle_omp_sessions_list_http))
            .route(
                "/api/omp-sessions/{id}",
                get(handle_omp_session_inspect_http),
            )
            .merge(crate::kernel::browser_proxy::router())
            .merge(ws_routes)
            .layer(axum::middleware::from_fn_with_state(
                access.clone(),
                guard_http,
            ));

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
            let h = tokio::spawn(run_named_pipe(custom.clone(), p_rx, access.clone()));
            Some(h)
        } else if is_production {
            let p_rx = shutdown_tx.subscribe();
            let h = tokio::spawn(run_named_pipe(
                r"\\.\pipe\kronos-ipc".to_string(),
                p_rx,
                access.clone(),
            ));
            Some(h)
        } else {
            let p_rx = shutdown_tx.subscribe();
            let pipe_name = format!(r"\\.\pipe\kronos-ipc-{}", actual_port);
            let h = tokio::spawn(run_named_pipe(pipe_name, p_rx, access.clone()));
            Some(h)
        };

        #[cfg(not(windows))]
        let pipe_task = if let Some(custom) = &custom_pipe {
            let p_rx = shutdown_tx.subscribe();
            let h = tokio::spawn(run_unix_socket(custom.clone(), p_rx, access.clone()));
            Some(h)
        } else if is_production {
            let p_rx = shutdown_tx.subscribe();
            let h = tokio::spawn(run_unix_socket(
                "/tmp/kronos-ipc.sock".to_string(),
                p_rx,
                access.clone(),
            ));
            Some(h)
        } else {
            let p_rx = shutdown_tx.subscribe();
            let sock_path = format!("/tmp/kronos-ipc-{}.sock", actual_port);
            let h = tokio::spawn(run_unix_socket(sock_path, p_rx, access.clone()));
            Some(h)
        };

        Ok(Self {
            port: actual_port,
            shutdown_tx,
            server_task: Arc::new(Mutex::new(Some(server_handle))),
            pipe_task: Arc::new(Mutex::new(pipe_task)),
            #[cfg(test)]
            access,
            #[cfg(test)]
            _auth_home: auth_home,
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

async fn guard_http(
    axum::extract::State(access): axum::extract::State<Arc<crate::kernel::access::AccessControl>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !access.allows_headers(request.headers()) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    let path = request.uri().path();
    let public = matches!(
        path,
        "/health"
            | "/api/health"
            | "/api/version"
            | "/"
            | "/ws"
            | "/api/agent-history/indexed"
            | "/api/omp-sessions"
    ) || path.starts_with("/api/omp-sessions/");
    let proxy_ticket = request.method() == axum::http::Method::GET
        && path == "/api/browser-proxy"
        && crate::kernel::browser_proxy::has_navigation_ticket(request.uri());
    if !public && !proxy_ticket && !access.accepts_bearer(request.headers()) {
        return axum::http::StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

async fn close_for_resync(socket: &mut WebSocket) {
    let _ = socket
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code: 1013,
            reason: "Stream overflow; reconnect and resynchronize".into(),
        })))
        .await;
}

async fn reject_socket(socket: &mut WebSocket, reason: &str, announce: bool) {
    if announce {
        let rejection =
            serde_json::json!({"type":"hello.rejected", "reason":reason, "accepts":["password"]});
        let _ = socket
            .send(Message::Text(rejection.to_string().into()))
            .await;
    }
    let (code, message) = match reason {
        "incompatible_protocol" => (4003, "Incompatible protocol version"),
        "incorrect_password" => (4401, "Incorrect password"),
        _ => (4401, "Password required"),
    };
    let _ = socket
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code,
            reason: message.into(),
        })))
        .await;
}

async fn serve_ipc<S>(
    stream: S,
    mut shutdown: broadcast::Receiver<()>,
    access: Arc<crate::kernel::access::AccessControl>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut frame = Vec::new();
    let admission = tokio::time::timeout(Duration::from_secs(10), async {
        (&mut reader)
            .take(8193)
            .read_until(b'\n', &mut frame)
            .await
            .map_err(|_| "password_required")?;
        if frame.len() > 8192 || frame.last() != Some(&b'\n') {
            return Err("password_required");
        }
        let hello = serde_json::from_slice(&frame).map_err(|_| "password_required")?;
        access.authenticate_hello(&hello).await
    })
    .await
    .unwrap_or(Err("password_required"));
    if let Err(reason) = admission {
        let rejection =
            serde_json::json!({"type":"hello.rejected", "reason":reason, "accepts":["password"]});
        let _ = writer.write_all(format!("{rejection}\n").as_bytes()).await;
        return;
    }
    let info = crate::kernel::protocol::build_server_info_response();
    if writer
        .write_all(format!("{info}\n").as_bytes())
        .await
        .is_err()
    {
        return;
    }
    loop {
        frame.clear();
        let mut limited = (&mut reader).take(16 * 1024 * 1024 + 1);
        tokio::select! {
            _ = shutdown.recv() => break,
            result = limited.read_until(b'\n', &mut frame) => {
                if result.is_err() || frame.is_empty() || frame.len() > 16 * 1024 * 1024
                    || frame.last() != Some(&b'\n') { break; }
                let Ok(text) = std::str::from_utf8(&frame) else { break; };
                if let Some(reply) = process_incoming_frame(text.trim()).await {
                    if writer.write_all(format!("{reply}\n").as_bytes()).await.is_err() { break; }
                }
            }
        }
    }
}

async fn handle_ws(
    mut socket: WebSocket,
    mut shutdown_rx: broadcast::Receiver<()>,
    access: Arc<crate::kernel::access::AccessControl>,
) {
    // Do not create subscriptions or emit private state before a verified hello.
    let mut announce_rejection = false;
    let admission = tokio::time::timeout(Duration::from_secs(10), async {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) if text.len() <= 8192 => {
                let hello = serde_json::from_str::<serde_json::Value>(&text)
                    .map_err(|_| "password_required")?;
                announce_rejection = hello.get("auth").is_some()
                    || hello["capabilities"]["hello_rejection"].as_bool() == Some(true);
                access.authenticate_hello(&hello).await
            }
            _ => Err("password_required"),
        }
    })
    .await
    .unwrap_or(Err("password_required"));
    if let Err(reason) = admission {
        reject_socket(&mut socket, reason, announce_rejection).await;
        return;
    }
    if socket
        .send(Message::Text(
            crate::kernel::protocol::build_server_info_response()
                .to_string()
                .into(),
        ))
        .await
        .is_err()
    {
        return;
    }
    let mut watcher_rx = get_watcher_engine().subscribe_events();
    let mut crdt_rx = get_crdt_engine().subscribe();
    let mut term_output_rx = crate::native_pty::get_pty_manager().subscribe_output();
    let mut provider_push_rx = get_provider_push_tx().subscribe();
    let mut subscribed_terminals: std::collections::HashMap<String, u8> =
        std::collections::HashMap::new();
    let mut watcher_active = true;
    let mut crdt_active = true;
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            term_chunk = term_output_rx.recv() => {
                match term_chunk {
                    Ok(chunk) => {
                        if let Some(&slot) = subscribed_terminals.get(&chunk.terminal_id) {
                            let mut frame = Vec::with_capacity(2 + chunk.data.len());
                            frame.push(0x01);
                            frame.push(slot);
                            frame.extend_from_slice(&chunk.data);
                            if socket.send(Message::Binary(frame.into())).await.is_err() { break; }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        close_for_resync(&mut socket).await;
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            provider_update = provider_push_rx.recv() => {
                match provider_update {
                    Ok(text) => {
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Reconnect invokes the client's authoritative history reconciliation.
                        close_for_resync(&mut socket).await;
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            events_res = watcher_rx.recv(), if watcher_active => {
                match events_res {
                    Ok(events) => {
                        let frame = serde_json::json!({
                            "type": "vfs/change_events",
                            "payload": events,
                        });
                        if let Ok(text) = serde_json::to_string(&frame) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        close_for_resync(&mut socket).await;
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        watcher_active = false;
                    }
                }
            }
            crdt_res = crdt_rx.recv(), if crdt_active => {
                match crdt_res {
                    Ok(event) => {
                        let frame = serde_json::json!({
                            "type": "crdt/update_event",
                            "payload": {
                                "room": event.room,
                                "update": event.update_base64,
                                "sourceId": event.source_id,
                            }
                        });
                        if let Ok(text) = serde_json::to_string(&frame) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        close_for_resync(&mut socket).await;
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        crdt_active = false;
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            let inner = if v.get("type").and_then(|t| t.as_str()) == Some("session") {
                                v.get("message").unwrap_or(&v)
                            } else {
                                &v
                            };
                            let msg_type = inner.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if msg_type == "subscribe_terminal_request" {
                                if let Some(term_id) = inner.get("terminalId").and_then(|id| id.as_str()) {
                                    let next_slot = (subscribed_terminals.len() + 1) as u8;
                                    subscribed_terminals.insert(term_id.to_string(), next_slot);
                                }
                            } else if msg_type == "unsubscribe_terminal_request" || msg_type == "kill_terminal_request" {
                                if let Some(term_id) = inner.get("terminalId").and_then(|id| id.as_str()) {
                                    subscribed_terminals.remove(term_id);
                                }
                            }
                        }
                        if let Some(reply) = process_incoming_frame(&text).await {
                            if socket.send(Message::Text(reply.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if bytes.len() >= 2 && (bytes[0] == 0x02 || bytes[0] == 0x03) {
                            let opcode = bytes[0];
                            let slot = bytes[1];
                            let payload = &bytes[2..];
                            if opcode == 0x02 {
                                if let Some((term_id, _)) = subscribed_terminals.iter().find(|(_, &s)| s == slot) {
                                    if let Ok(text) = std::str::from_utf8(payload) {
                                        let _ = crate::native_pty::get_pty_manager().write_terminal(term_id, text);
                                    }
                                }
                            } else if opcode == 0x03 {
                                if let Some((term_id, _)) = subscribed_terminals.iter().find(|(_, &s)| s == slot) {
                                    if let Ok(resize_val) = serde_json::from_slice::<serde_json::Value>(payload) {
                                        let rows = resize_val.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
                                        let cols = resize_val.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
                                        let _ = crate::native_pty::get_pty_manager().resize_terminal(term_id, cols, rows);
                                    }
                                }
                            }
                        } else if let Ok(text) = std::str::from_utf8(&bytes) {
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
async fn run_named_pipe(
    pipe_name: String,
    mut shutdown_rx: broadcast::Receiver<()>,
    access: Arc<crate::kernel::access::AccessControl>,
) {
    use tokio::net::windows::named_pipe::ServerOptions;

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

        tokio::spawn(serve_ipc(server, shutdown_rx.resubscribe(), access.clone()));
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
async fn run_unix_socket(
    sock_path: String,
    mut shutdown_rx: broadcast::Receiver<()>,
    access: Arc<crate::kernel::access::AccessControl>,
) {
    use tokio::net::UnixListener;

    let p = std::path::Path::new(&sock_path);
    safe_remove_socket_file(p);
    let listener = match UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(_) => return,
    };
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)).is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                break;
            }
            conn = listener.accept() => {
                match conn {
                    Ok((stream, _)) => {
                        tokio::spawn(serve_ipc(stream, shutdown_rx.resubscribe(), access.clone()));
                    }
                    Err(_) => break,
                }
            }
        }
    }
    safe_remove_socket_file(p);
}

pub(crate) fn get_provider_snapshot_metadata_entries() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "provider": "claude",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "Claude",
            "description": "Anthropic Claude with MCP, tools, and reasoning",
            "defaultModeId": "auto",
            "modes": [
                { "id": "auto", "label": "Auto mode", "description": "Review permission prompts automatically", "colorTier": "moderate" },
                { "id": "plan", "label": "Plan Mode", "description": "Analyze codebase without executing tools or edits", "colorTier": "planning" },
                { "id": "bypassPermissions", "label": "Bypass", "description": "Skip all permission prompts", "colorTier": "dangerous" }
            ],
            "models": []
        }),
        serde_json::json!({
            "provider": "codex",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "Codex",
            "description": "OpenAI Codex workspace agent",
            "defaultModeId": "auto",
            "modes": [
                { "id": "auto", "label": "Default Permissions", "description": "Edit files and run commands with Codex default approval flow", "colorTier": "moderate" },
                { "id": "full-access", "label": "Full Access", "description": "Edit files, run commands, and access network without prompts", "colorTier": "dangerous" }
            ],
            "models": []
        }),
        serde_json::json!({
            "provider": "opencode",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "OpenCode",
            "description": "Open-source coding assistant with multi-provider model support",
            "defaultModeId": "build",
            "modes": [
                { "id": "build", "label": "Build", "description": "Allows edits and tool execution", "colorTier": "moderate" },
                { "id": "plan", "label": "Plan", "description": "Read-only planning mode that avoids edits", "colorTier": "planning" }
            ],
            "models": []
        }),
        serde_json::json!({
            "provider": "ollama",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "Ollama",
            "description": "Local Ollama models",
            "defaultModeId": null,
            "modes": [],
            "models": []
        }),
        serde_json::json!({
            "provider": "pi",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "Pi",
            "description": "Minimal terminal-based coding agent",
            "defaultModeId": null,
            "modes": [],
            "models": []
        }),
        serde_json::json!({
            "provider": "omp",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "Oh My Pi",
            "description": "Multi-provider coding agent with native approvals and subagents",
            "defaultModeId": "full",
            "modes": [
                { "id": "full", "label": "Full Access", "description": "Yolo approval mode so tools run without prompts", "colorTier": "dangerous" },
                { "id": "write", "label": "Write Approval", "description": "Reads are free, writes require approval", "colorTier": "moderate" },
                { "id": "ask", "label": "Always Ask", "description": "Always ask approval for write and exec tools", "colorTier": "safe" }
            ],
            "models": []
        }),
        serde_json::json!({
            "provider": "copilot",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "Copilot",
            "description": "GitHub Copilot via Agent Client Protocol",
            "defaultModeId": "https://agentclientprotocol.com/protocol/session-modes#agent",
            "modes": [
                { "id": "https://agentclientprotocol.com/protocol/session-modes#agent", "label": "Agent", "description": "Default agent mode", "colorTier": "moderate" },
                { "id": "https://agentclientprotocol.com/protocol/session-modes#plan", "label": "Plan", "description": "Plan mode", "colorTier": "planning" },
                { "id": "allow-all", "label": "Allow All", "description": "Approve all Copilot tool requests", "colorTier": "dangerous" }
            ],
            "models": []
        }),
        serde_json::json!({
            "provider": "lmstudio",
            "status": "ready",
            "enabled": true,
            "source": "builtin",
            "label": "LM Studio",
            "description": "Local models via LM Studio local inference server",
            "defaultModeId": null,
            "modes": [],
            "models": []
        }),
    ]
}

pub(crate) async fn get_provider_snapshot_entries(cwd: &str) -> Vec<serde_json::Value> {
    let mut cache = PROVIDER_SNAPSHOT_CACHE.lock().await;
    if let Some((generated_at, cached_cwd, entries)) = cache.as_ref() {
        if cached_cwd == cwd && generated_at.elapsed() < Duration::from_secs(60) {
            return entries.clone();
        }
    }

    let (
        codex_models,
        claude_models,
        opencode_models,
        ollama_models,
        lmstudio_models,
        pi_models,
        omp_models,
        copilot_available,
    ) = tokio::join!(
        discover_codex_models(cwd),
        discover_claude_configured_model(),
        discover_opencode_models(cwd),
        discover_ollama_models(),
        discover_lmstudio_models(),
        discover_pi_models(cwd),
        discover_omp_models(cwd),
        discover_copilot_available()
    );
    let models_by_provider = [
        ("codex", codex_models),
        ("claude", claude_models),
        ("opencode", opencode_models),
        ("ollama", ollama_models),
        ("lmstudio", lmstudio_models),
        ("pi", pi_models),
        ("omp", omp_models),
    ];
    let mut entries = get_provider_snapshot_metadata_entries();

    for entry in &mut entries {
        let provider = entry
            .get("provider")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        // ACP-binary providers (copilot) get availability from binary check, not model count.
        // Their model list is populated dynamically at session time via ACP session/new.
        if provider == "copilot" {
            if !copilot_available {
                entry["status"] = serde_json::json!("unavailable");
            }
            continue;
        }
        let models = models_by_provider
            .iter()
            .find(|(candidate, _)| *candidate == provider)
            .map(|(_, models)| models.clone())
            .unwrap_or_default();
        entry["models"] = serde_json::Value::Array(models);
        if entry["models"].as_array().is_some_and(Vec::is_empty) {
            entry["status"] = serde_json::json!("unavailable");
        }
    }
    *cache = Some((Instant::now(), cwd.to_owned(), entries.clone()));
    entries
}

pub(crate) async fn discover_pi_models(cwd: &str) -> Vec<serde_json::Value> {
    let binary = std::env::var("PI_COMMAND")
        .or_else(|_| std::env::var("PI_ACP_PI_COMMAND"))
        .unwrap_or_else(|_| "pi".to_owned());
    discover_jsonl_cli_models(
        &binary,
        &["--mode", "rpc"],
        cwd,
        false,
        pi_models_from_response,
    )
    .await
}

async fn discover_omp_models(cwd: &str) -> Vec<serde_json::Value> {
    let binary = std::env::var("OMP_COMMAND").unwrap_or_else(|_| "omp".to_owned());
    discover_jsonl_cli_models(
        &binary,
        &["--mode", "rpc-ui"],
        cwd,
        true,
        omp_models_from_response,
    )
    .await
}

/// Returns `true` when the Copilot binary is available.
async fn discover_copilot_available() -> bool {
    if crate::agent::registry::resolve_command("copilot").is_some() {
        return true;
    }
    if let Some(gh) = crate::agent::registry::resolve_command("gh") {
        let mut cmd = tokio::process::Command::new(gh);
        cmd.args(["copilot", "--version"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        return cmd.status().await.map(|s| s.success()).unwrap_or(false);
    }
    false
}

async fn discover_jsonl_cli_models(
    binary_name: &str,
    args: &[&str],
    cwd: &str,
    wait_ready: bool,
    parse: fn(&serde_json::Value) -> Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let binary = match crate::agent::registry::resolve_command(binary_name) {
        Some(b) => b,
        None => return Vec::new(),
    };
    let safe_cwd = crate::agent::runner::resolve_safe_cwd(std::path::Path::new(cwd), None);
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    let discovery = async {
        let mut cmd = Command::new(&binary);
        cmd.args(args)
            .current_dir(&safe_cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let mut child = cmd.spawn().ok()?;
        let mut stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let mut lines = BufReader::new(stdout).lines();
        if wait_ready {
            let ready_obj = tokio::time::timeout(Duration::from_secs(3), async {
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                        if value.get("type").and_then(serde_json::Value::as_str) == Some("ready") {
                            return Some(value);
                        }
                    }
                }
                None
            })
            .await
            .ok()
            .flatten();

            if ready_obj.is_none() {
                let _ = child.kill().await;
                return None;
            }

            // Negotiate protocol v2 if reported supported
            let supports_v2 = ready_obj
                .as_ref()
                .and_then(|r| r.get("supportedProtocolVersions"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|v| v.as_i64() == Some(2)))
                .unwrap_or(false);

            if supports_v2 {
                let neg = serde_json::json!({
                    "id": "neg",
                    "type": "negotiate_protocol",
                    "protocolVersion": 2
                });
                let _ = stdin.write_all(format!("{neg}\n").as_bytes()).await;
                let _ = stdin.flush().await;
                let _ = tokio::time::timeout(Duration::from_secs(2), async {
                    while let Ok(Some(line)) = lines.next_line().await {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                            if value.get("id").and_then(serde_json::Value::as_str) == Some("neg") {
                                break;
                            }
                        }
                    }
                })
                .await;
            }
        }

        let request = serde_json::json!({
            "id": "kronos-model-catalog",
            "type": "get_available_models"
        });
        if stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .is_err()
        {
            let _ = child.kill().await;
            return None;
        }
        let _ = stdin.flush().await;
        let response = tokio::time::timeout(Duration::from_secs(6), async {
            let mut chunks: Vec<serde_json::Value> = Vec::new();
            let mut expected_count: Option<usize> = None;

            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                    if value.get("type").and_then(serde_json::Value::as_str) == Some("rpc_chunk") {
                        if let Some(cnt) = value.get("count").and_then(|v| v.as_u64()) {
                            expected_count = Some(cnt as usize);
                        }
                        chunks.push(value);
                        if let Some(target) = expected_count {
                            if chunks.len() >= target {
                                chunks.sort_by_key(|c| {
                                    c.get("index").and_then(|v| v.as_u64()).unwrap_or(0)
                                });
                                use base64::Engine;
                                let mut full_bytes = Vec::new();
                                for chunk in &chunks {
                                    if let Some(data_str) =
                                        chunk.get("data").and_then(|v| v.as_str())
                                    {
                                        if let Ok(decoded) =
                                            base64::engine::general_purpose::STANDARD
                                                .decode(data_str)
                                        {
                                            full_bytes.extend_from_slice(&decoded);
                                        }
                                    }
                                }
                                if let Ok(reassembled) =
                                    serde_json::from_slice::<serde_json::Value>(&full_bytes)
                                {
                                    return Some(reassembled);
                                }
                            }
                        }
                    } else if value.get("type").and_then(serde_json::Value::as_str)
                        == Some("response")
                        && value.get("id").and_then(serde_json::Value::as_str)
                            == Some("kronos-model-catalog")
                    {
                        return Some(value);
                    }
                }
            }
            None
        })
        .await
        .ok()
        .flatten();
        let _ = child.kill().await;
        Some(
            response
                .filter(|value| {
                    value.get("success").and_then(serde_json::Value::as_bool) == Some(true)
                })
                .map(|value| parse(&value["data"]))
                .unwrap_or_default(),
        )
    };

    tokio::time::timeout(Duration::from_secs(10), discovery)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub(crate) fn pi_models_from_response(data: &serde_json::Value) -> Vec<serde_json::Value> {
    runtime_cli_models(data, "pi")
}

pub(crate) fn omp_models_from_response(data: &serde_json::Value) -> Vec<serde_json::Value> {
    runtime_cli_models(data, "omp")
}

fn extract_context_window(value: &serde_json::Value) -> Option<u64> {
    value
        .get("contextWindow")
        .or_else(|| value.get("context_window"))
        .or_else(|| value.get("contextWindowMaxTokens"))
        .or_else(|| value.get("modelContextWindow"))
        .or_else(|| value.get("model_context_window"))
        .or_else(|| value.pointer("/limit/context"))
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
        })
}

fn runtime_cli_models(data: &serde_json::Value, provider_name: &str) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    data.get("models")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let provider = model.get("provider").and_then(serde_json::Value::as_str)?;
            let model_id = model.get("id").and_then(serde_json::Value::as_str)?;
            if provider.trim().is_empty() || model_id.trim().is_empty() {
                return None;
            }
            let id = format!("{provider}/{model_id}");
            if !seen.insert(id.clone()) {
                return None;
            }
            let mut definition = dynamic_model(
                provider_name,
                &id,
                model.get("name").and_then(serde_json::Value::as_str),
                None,
                false,
                extract_context_window(model),
            );
            apply_runtime_thinking_options(&mut definition, model, provider_name);
            Some(definition)
        })
        .collect()
}

fn apply_runtime_thinking_options(
    definition: &mut serde_json::Value,
    model: &serde_json::Value,
    provider: &str,
) {
    if model.get("reasoning").and_then(serde_json::Value::as_bool) != Some(true) {
        return;
    }
    if provider == "pi" {
        // Mirrors Pi's getSupportedThinkingLevels and upward-first clampThinkingLevel.
        let levels = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
        let mapping = model
            .get("thinkingLevelMap")
            .and_then(serde_json::Value::as_object);
        let supported: Vec<&str> = levels
            .iter()
            .copied()
            .filter(|level| {
                let mapped = mapping.and_then(|mapping| mapping.get(*level));
                !mapped.is_some_and(serde_json::Value::is_null)
                    && (!matches!(*level, "xhigh" | "max") || mapped.is_some())
            })
            .collect();
        let default = levels[3..]
            .iter()
            .copied()
            .find(|level| supported.contains(level))
            .or_else(|| supported.last().copied());
        let options = supported
            .iter()
            .map(|level| thinking_option(level, None))
            .collect();
        set_model_thinking_options(definition, options, default);
    } else if provider == "omp" {
        // Current OMP getSupportedEfforts treats a missing ladder as uncontrollable.
        let mut seen = std::collections::HashSet::new();
        let efforts: Vec<&str> = model
            .pointer("/thinking/efforts")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .filter(|effort| !effort.trim().is_empty() && seen.insert(*effort))
            .collect();
        if efforts.is_empty() {
            return;
        }
        let default = model
            .pointer("/thinking/defaultLevel")
            .and_then(serde_json::Value::as_str)
            .filter(|level| efforts.contains(level))
            .or_else(|| efforts.first().copied());
        // Off and Auto are OMP session selectors, not entries in the model effort ladder.
        let mut options = vec![thinking_option("off", None), thinking_option("auto", None)];
        options.extend(
            efforts
                .iter()
                .filter(|level| !matches!(**level, "off" | "auto"))
                .map(|level| thinking_option(level, None)),
        );
        set_model_thinking_options(definition, options, default);
    }
}

fn thinking_option(id: &str, description: Option<&str>) -> serde_json::Value {
    let mut option = serde_json::json!({ "id": id, "label": id });
    if let Some(description) = description.filter(|description| !description.trim().is_empty()) {
        option["description"] = serde_json::json!(description);
    }
    option
}

fn set_model_thinking_options(
    model: &mut serde_json::Value,
    mut options: Vec<serde_json::Value>,
    default: Option<&str>,
) {
    if options.is_empty() {
        return;
    }
    let default = default.filter(|id| {
        options
            .iter()
            .any(|option| option["id"].as_str() == Some(*id))
    });
    for option in &mut options {
        option["isDefault"] = serde_json::json!(option["id"].as_str() == default);
    }
    model["thinkingOptions"] = serde_json::Value::Array(options);
    if let Some(default) = default {
        model["defaultThinkingOptionId"] = serde_json::json!(default);
    }
}

async fn invalidate_provider_snapshot_cache() {
    *PROVIDER_SNAPSHOT_CACHE.lock().await = None;
}

pub(crate) async fn discover_codex_models(cwd: &str) -> Vec<serde_json::Value> {
    let binary = match crate::agent::registry::resolve_command("codex") {
        Some(b) => b,
        None => return Vec::new(),
    };
    let safe_cwd = crate::agent::runner::resolve_safe_cwd(std::path::Path::new(cwd), None);

    let discovery = async {
        let mut client = crate::agent::providers::codex::CodexAppServerClient::spawn_with_bin(
            &binary,
            &safe_cwd,
        )
        .await
        .ok()?;

        let initialize = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": { "name": "kronos", "title": "Kronos", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {
                    "experimentalApi": true,
                    "mcpServerOpenaiFormElicitation": true
                }
            }
        });
        if client.send_json(&initialize).await.is_err() {
            let _ = client.kill().await;
            return None;
        }

        let initialized = tokio::time::timeout(Duration::from_secs(4), async {
            while let Ok(Some(value)) = client.read_json().await {
                if value.get("id") == Some(&serde_json::json!(1)) {
                    return Some(());
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
        .is_some();

        if !initialized {
            let _ = client.kill().await;
            return None;
        }

        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "model/list",
            "params": {}
        });
        if client.send_json(&notification).await.is_err()
            || client.send_json(&request).await.is_err()
        {
            let _ = client.kill().await;
            return None;
        }

        let response = tokio::time::timeout(Duration::from_secs(4), async {
            while let Ok(Some(value)) = client.read_json().await {
                if value.get("id") == Some(&serde_json::json!(2)) {
                    return Some(value);
                }
            }
            None
        })
        .await
        .ok()
        .flatten();
        let _ = client.kill().await;
        Some(
            response
                .map(|value| codex_models_from_response(&value))
                .unwrap_or_default(),
        )
    };

    tokio::time::timeout(Duration::from_secs(10), discovery)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub(crate) fn codex_models_from_response(response: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    response
        .pointer("/result/data")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let id = model.get("id")?.as_str()?;
            if id.trim().is_empty() || !seen.insert(id) {
                return None;
            }
            let mut definition = dynamic_model(
                "codex",
                id,
                model.get("displayName").and_then(serde_json::Value::as_str),
                model.get("description").and_then(serde_json::Value::as_str),
                model
                    .get("isDefault")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                extract_context_window(model),
            );
            let mut effort_ids = std::collections::HashSet::new();
            let mut options: Vec<serde_json::Value> = model
                .get("supportedReasoningEfforts")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|effort| {
                    let id = effort.get("reasoningEffort")?.as_str()?.trim();
                    if id.is_empty() || !effort_ids.insert(id) {
                        return None;
                    }
                    Some(thinking_option(
                        id,
                        effort
                            .get("description")
                            .and_then(serde_json::Value::as_str),
                    ))
                })
                .collect();
            let default = model
                .get("defaultReasoningEffort")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty());
            if let Some(default) = default {
                if effort_ids.insert(default) {
                    options.push(thinking_option(default, None));
                }
            }
            set_model_thinking_options(&mut definition, options, default);
            Some(definition)
        })
        .collect()
}

async fn discover_claude_configured_model() -> Vec<serde_json::Value> {
    // 1. Dynamic discovery from environment or settings.json
    let preferred_id = {
        let from_env = std::env::var("ANTHROPIC_MODEL")
            .ok()
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty());
        if from_env.is_some() {
            from_env
        } else if let Ok(config_dir) = std::env::var("CLAUDE_CONFIG_DIR") {
            read_claude_settings_model(std::path::Path::new(&config_dir)).await
        } else if let Some(home) = dirs::home_dir() {
            read_claude_settings_model(&home.join(".claude")).await
        } else {
            None
        }
    };

    if let Some(ref model_id) = preferred_id {
        return vec![dynamic_model(
            "claude",
            model_id,
            Some(model_id),
            Some("Configured Claude model"),
            true,
            None,
        )];
    }

    Vec::new()
}

pub(crate) async fn read_claude_settings_model(config_dir: &std::path::Path) -> Option<String> {
    let settings_path = config_dir.join("settings.json");
    let raw = tokio::fs::read_to_string(settings_path).await.ok()?;
    let settings: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let model = settings
        .get("model")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            settings
                .pointer("/env/ANTHROPIC_MODEL")
                .and_then(serde_json::Value::as_str)
        })?;
    let model = model.trim();
    (!model.is_empty()).then(|| model.to_owned())
}

async fn discover_opencode_models(cwd: &str) -> Vec<serde_json::Value> {
    let safe_cwd = crate::agent::runner::resolve_safe_cwd(std::path::Path::new(cwd), None);

    // 1. Dynamic discovery via opencode CLI
    if let Some(binary) = crate::agent::registry::resolve_command("opencode") {
        // Try `models --verbose` first for detailed JSON capabilities
        let mut cmd = tokio::process::Command::new(&binary);
        cmd.args(["models", "--verbose"])
            .current_dir(&safe_cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);

        if let Ok(Ok(out)) = tokio::time::timeout(Duration::from_secs(5), cmd.output()).await {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let models = opencode_models_from_cli_output(&text);
                if !models.is_empty() {
                    return models;
                }
            }
        }

        // Fallback: newer OpenCode CLI (e.g. v2.2+) lists available models via `opencode models`
        let mut fallback_cmd = tokio::process::Command::new(&binary);
        fallback_cmd
            .args(["models"])
            .current_dir(&safe_cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        fallback_cmd.creation_flags(0x08000000);

        if let Ok(Ok(out)) =
            tokio::time::timeout(Duration::from_secs(5), fallback_cmd.output()).await
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let models = opencode_models_from_plain_cli_output(&text);
                if !models.is_empty() {
                    return models;
                }
            }
        }
    }

    // 2. Fall back to OpenCode HTTP server if running
    let base_url = normalize_http_endpoint(
        std::env::var("OPENCODE_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:4096".to_owned()),
    );
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };
    let response = match client
        .get(format!("{base_url}/provider"))
        .header("x-opencode-directory", safe_cwd.to_string_lossy().to_string())
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => response,
        _ => return Vec::new(),
    };
    let value = match response.json::<serde_json::Value>().await {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    opencode_models_from_response(&value)
}

pub(crate) fn opencode_models_from_response(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let providers = value.pointer("/all").and_then(serde_json::Value::as_array);
    let connected_ids: std::collections::HashSet<&str> = value
        .pointer("/connected")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .collect();

    providers
        .into_iter()
        .flatten()
        .filter(|provider| {
            let id = provider
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            connected_ids.contains(id)
                || provider.get("source").and_then(serde_json::Value::as_str) == Some("api")
        })
        .flat_map(|provider| {
            let provider_id = provider
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            provider
                .get("models")
                .and_then(serde_json::Value::as_object)
                .into_iter()
                .flat_map(move |models| {
                    models.iter().filter_map(move |(model_id, details)| {
                        opencode_model_definition(provider_id, model_id, details)
                    })
                })
        })
        .collect()
}

pub(crate) fn opencode_models_from_cli_output(text: &str) -> Vec<serde_json::Value> {
    let mut models = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut remaining = text;
    // `models --verbose` prints each lookup ID followed by a complete, possibly
    // multiline JSON object. Bare lookup IDs alone are not capability metadata.
    while let Some(start) = remaining.find('{') {
        let mut stream = serde_json::Deserializer::from_str(&remaining[start..])
            .into_iter::<serde_json::Value>();
        let Some(Ok(details)) = stream.next() else {
            break;
        };
        let next_offset = start + stream.byte_offset();
        if next_offset <= remaining.len() && remaining.is_char_boundary(next_offset) {
            remaining = &remaining[next_offset..];
        } else {
            break;
        }
        let Some(provider) = details
            .get("providerID")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(id) = details.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if let Some(model) = opencode_model_definition(provider, id, &details) {
            if seen.insert(model["id"].as_str().unwrap_or_default().to_owned()) {
                models.push(model);
            }
        }
    }
    models
}

pub(crate) fn opencode_models_from_plain_cli_output(text: &str) -> Vec<serde_json::Value> {
    let mut models = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("USAGE")
            || trimmed.starts_with("DESCRIPTION")
            || trimmed.starts_with("ARGUMENTS")
            || trimmed.starts_with("FLAGS")
            || trimmed.starts_with("GLOBAL FLAGS")
            || trimmed.starts_with("ERROR")
        {
            continue;
        }
        if trimmed.contains('/') {
            if seen.insert(trimmed.to_string()) {
                let label = trimmed.rsplit('/').next().unwrap_or(trimmed);
                let model = dynamic_model("opencode", trimmed, Some(label), None, false, None);
                models.push(model);
            }
        }
    }
    models
}

fn opencode_model_definition(
    provider: &str,
    id: &str,
    details: &serde_json::Value,
) -> Option<serde_json::Value> {
    if provider.trim().is_empty() || id.trim().is_empty() {
        return None;
    }
    let mut model = dynamic_model(
        "opencode",
        &format!("{provider}/{id}"),
        details.get("name").and_then(serde_json::Value::as_str),
        details
            .get("description")
            .and_then(serde_json::Value::as_str),
        details
            .get("default")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        extract_context_window(details),
    );
    if let Some(variants) = details
        .get("variants")
        .and_then(serde_json::Value::as_object)
        .filter(|variants| {
            !variants.is_empty()
                && details
                    .pointer("/capabilities/reasoning")
                    .or_else(|| details.get("reasoning"))
                    .and_then(serde_json::Value::as_bool)
                    != Some(false)
        })
    {
        // OpenCode's Default omits the variant and lets its own runtime resolve it.
        let mut options = vec![serde_json::json!({ "id": "default", "label": "Default" })];
        options.extend(
            variants
                .keys()
                .filter(|id| id.as_str() != "default")
                .map(|id| thinking_option(id, None)),
        );
        set_model_thinking_options(&mut model, options, Some("default"));
    }
    Some(model)
}
async fn discover_ollama_models() -> Vec<serde_json::Value> {
    let endpoint = normalize_http_endpoint(
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".to_owned()),
    );
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };
    let response = match client.get(format!("{endpoint}/api/tags")).send().await {
        Ok(response) if response.status().is_success() => response,
        _ => return Vec::new(),
    };
    match response.json::<serde_json::Value>().await {
        Ok(value) => ollama_models_from_response(&value),
        Err(_) => Vec::new(),
    }
}

async fn discover_lmstudio_models() -> Vec<serde_json::Value> {
    let endpoint = normalize_http_endpoint(
        std::env::var("LMSTUDIO_HOST").unwrap_or_else(|_| "http://127.0.0.1:1234".to_owned()),
    );
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
    {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };
    let response = match client.get(format!("{endpoint}/v1/models")).send().await {
        Ok(response) if response.status().is_success() => response,
        _ => return Vec::new(),
    };
    match response.json::<serde_json::Value>().await {
        Ok(value) => lmstudio_models_from_response(&value),
        Err(_) => Vec::new(),
    }
}

pub(crate) fn lmstudio_models_from_response(
    response: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    response
        .get("data")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let id = model.get("id")?.as_str()?;
            if id.trim().is_empty() || !seen.insert(id) {
                return None;
            }
            Some(dynamic_model(
                "lmstudio",
                &format!("lmstudio/{id}"),
                Some(id),
                Some("Local model served via LM Studio (127.0.0.1:1234)"),
                false,
                None,
            ))
        })
        .collect()
}

pub(crate) fn ollama_models_from_response(response: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    response
        .get("models")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let name = model.get("name")?.as_str()?;
            if name.trim().is_empty() || !seen.insert(name) {
                return None;
            }
            let parameter_size = model
                .pointer("/details/parameter_size")
                .and_then(serde_json::Value::as_str);
            let label = parameter_size
                .map(|size| format!("{name} ({size})"))
                .unwrap_or_else(|| name.to_owned());
            let family = model
                .pointer("/details/family")
                .and_then(serde_json::Value::as_str);
            Some(dynamic_model(
                "ollama",
                &format!("ollama/{name}"),
                Some(&label),
                family
                    .map(|value| format!("Local Ollama model from the {value} family"))
                    .as_deref(),
                false,
                None,
            ))
        })
        .collect()
}

pub(crate) fn normalize_http_endpoint(endpoint: String) -> String {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_owned()
    } else {
        format!("http://{endpoint}")
    }
}

fn dynamic_model(
    provider: &str,
    id: &str,
    label: Option<&str>,
    description: Option<&str>,
    is_default: bool,
    context_window: Option<u64>,
) -> serde_json::Value {
    let label = label.filter(|value| !value.trim().is_empty()).unwrap_or(id);
    let mut model = serde_json::json!({
        "provider": provider,
        "id": id,
        "label": label,
        "description": description.unwrap_or(""),
        "isDefault": is_default,
        "isSelectable": true
    });
    if let Some(context_window) = context_window {
        model["contextWindowMaxTokens"] = serde_json::json!(context_window);
    }
    model
}
