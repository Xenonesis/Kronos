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

static WATCHER_ENGINE: std::sync::LazyLock<crate::kernel::watcher::WatcherEngine> =
    std::sync::LazyLock::new(|| {
        let engine = crate::kernel::watcher::WatcherEngine::new(25);
        if let Ok(index) = crate::kernel::search::WorkspaceSearchIndex::new_in_memory() {
            engine.bind_search_index(Arc::new(index));
        }
        engine.bind_shadow_vfs(Arc::new(crate::kernel::vfs::ShadowVfs::new()));
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
                    "hostMultiplexing": true,
                    "projectAdd": true,
                    "stableProjectIdentity": true,
                    "projectGithubClone": true,
                    "workspaceGithubRepositorySearch": true,
                    "projectCreateDirectory": true
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
        "vfs/watch" | "vfs/watch_subscribe" => {
            let path_opt = envelope.payload.get("path")
                .and_then(|v| v.as_str())
                .or_else(|| envelope.payload.as_str());
            let ignore: Vec<String> = envelope.payload.get("ignore")
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
                        Err(err) => RpcResponse::err(
                            &envelope.id,
                            "vfs/watch/response",
                            err,
                        ),
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
            let sub_id_opt = envelope.payload.get("subscriptionId")
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
                        Err(err) => RpcResponse::err(
                            &envelope.id,
                            "vfs/unwatch/response",
                            err,
                        ),
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
            let path_opt = envelope.payload.get("path")
                .and_then(|v| v.as_str());
            let source_opt = envelope.payload.get("source")
                .and_then(|v| v.as_str());
            let ext_opt = envelope.payload.get("ext")
                .and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| path_opt.and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str())))
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| std::path::Path::new(path).extension().and_then(|e| e.to_str()))
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
                Err(err) => RpcResponse::err(
                    &envelope.id,
                    "ast/analyze/response",
                    err,
                ),
            }
        }
        "ast/symbols" => {
            let path_opt = envelope.payload.get("path")
                .and_then(|v| v.as_str());
            let source_opt = envelope.payload.get("source")
                .and_then(|v| v.as_str());
            let ext_opt = envelope.payload.get("ext")
                .and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| path_opt.and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str())))
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| std::path::Path::new(path).extension().and_then(|e| e.to_str()))
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
                Err(err) => RpcResponse::err(
                    &envelope.id,
                    "ast/symbols/response",
                    err,
                ),
            }
        }
        "crdt/join" => {
            let room = envelope.payload.get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let engine = get_crdt_engine();
            let doc = engine.get_or_create_room(room);
            let text = doc.to_string();
            let len = doc.len();
            let sv = doc.export_state_vector();
            let update = doc.export_update();
            let sv_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &sv);
            let update_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &update);

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
            let room = envelope.payload.get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let source_id = envelope.payload.get("sourceId")
                .or_else(|| envelope.payload.get("source_id"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let update_b64_opt = envelope.payload.get("update")
                .and_then(|v| v.as_str());

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
                    Err(e) => RpcResponse::err(&envelope.id, "crdt/update/response", format!("Invalid base64 update: {e}")),
                }
            } else {
                RpcResponse::err(&envelope.id, "crdt/update/response", "Missing 'update' base64 payload")
            }
        }
        "crdt/sync" => {
            let room = envelope.payload.get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let sv_b64_opt = envelope.payload.get("stateVector")
                .or_else(|| envelope.payload.get("state_vector"))
                .and_then(|v| v.as_str());

            if let Some(b64) = sv_b64_opt {
                match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                    Ok(sv_bytes) => {
                        let engine = get_crdt_engine();
                        match engine.sync_room(room, &sv_bytes) {
                            Ok(diff_bytes) => {
                                let diff_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &diff_bytes);
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
                    Err(e) => RpcResponse::err(&envelope.id, "crdt/sync/response", format!("Invalid base64 stateVector: {e}")),
                }
            } else {
                RpcResponse::err(&envelope.id, "crdt/sync/response", "Missing 'stateVector' parameter")
            }
        }
        "crdt/state" => {
            let room = envelope.payload.get("room")
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
            let room = envelope.payload.get("room")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            let action = envelope.payload.get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("insert");
            let index = envelope.payload.get("index")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let text = envelope.payload.get("text")
                .and_then(|v| v.as_str());
            let length = envelope.payload.get("length")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            let source_id = envelope.payload.get("sourceId")
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
            let is_active = reader.as_ref().map(|r| r.is_kernel_ioring_active()).unwrap_or(false);
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
            let paths_opt = envelope.payload.get("paths")
                .and_then(|v| v.as_array());

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
                            Err(e) => RpcResponse::err(&envelope.id, "ioring/read_batch/response", e.to_string()),
                        },
                        Err(e) => RpcResponse::err(&envelope.id, "ioring/read_batch/response", e.to_string()),
                    }
                }
                None => RpcResponse::err(&envelope.id, "ioring/read_batch/response", "Missing 'paths' array"),
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
            let path = msg.get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ignore: Vec<String> = msg.get("ignore")
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
            let sub_id = msg.get("subscriptionId")
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
        "ast/analyze" | "ast/parse" => {
            let payload = msg.get("payload");
            let path_opt = msg.get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str());
            let source_opt = msg.get("source")
                .or_else(|| payload.and_then(|p| p.get("source")))
                .and_then(|v| v.as_str());
            let ext_opt = msg.get("ext")
                .or_else(|| payload.and_then(|p| p.get("ext")))
                .and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| path_opt.and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str())))
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| std::path::Path::new(path).extension().and_then(|e| e.to_str()))
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
            let path_opt = msg.get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str());
            let source_opt = msg.get("source")
                .or_else(|| payload.and_then(|p| p.get("source")))
                .and_then(|v| v.as_str());
            let ext_opt = msg.get("ext")
                .or_else(|| payload.and_then(|p| p.get("ext")))
                .and_then(|v| v.as_str());

            let (source, ext) = if let Some(src) = source_opt {
                let ext = ext_opt
                    .or_else(|| path_opt.and_then(|p| std::path::Path::new(p).extension().and_then(|e| e.to_str())))
                    .unwrap_or("ts");
                (src.to_string(), ext.to_string())
            } else if let Some(path) = path_opt {
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let ext = ext_opt
                            .or_else(|| std::path::Path::new(path).extension().and_then(|e| e.to_str()))
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
        msg_type if msg_type.starts_with("crdt/") || msg_type.starts_with("ioring/") => {
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
            "vfs/watch" | "vfs/watch_subscribe" | "vfs/unwatch" | "vfs/watch_unsubscribe" | "ast/analyze" | "ast/parse" | "ast/symbols" | "crdt/join" | "crdt/update" | "crdt/sync" | "crdt/state" | "crdt/edit" | "ioring/status" | "ioring/read_batch" => {
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
    let mut watcher_rx = get_watcher_engine().subscribe_events();
    let mut crdt_rx = get_crdt_engine().subscribe();
    let mut watcher_active = true;
    let mut crdt_active = true;
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
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
                        continue;
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
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        crdt_active = false;
                    }
                }
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
