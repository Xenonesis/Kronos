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

static PROVIDER_SNAPSHOT_CACHE: std::sync::LazyLock<
    tokio::sync::Mutex<Option<(Instant, String, Vec<serde_json::Value>)>>,
> = std::sync::LazyLock::new(|| tokio::sync::Mutex::new(None));
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

static SPEECH_ENGINE: std::sync::LazyLock<std::sync::Arc<tokio::sync::RwLock<crate::kernel::speech::SpeechEngine>>> =
    std::sync::LazyLock::new(|| {
        let engine = crate::kernel::speech::SpeechEngine::new(None)
            .unwrap_or_else(|_| crate::kernel::speech::SpeechEngine::new_mock());
        std::sync::Arc::new(tokio::sync::RwLock::new(engine))
    });

pub fn get_speech_engine() -> std::sync::Arc<tokio::sync::RwLock<crate::kernel::speech::SpeechEngine>> {
    SPEECH_ENGINE.clone()
}

static SHM_RING: std::sync::LazyLock<std::sync::Arc<tokio::sync::Mutex<crate::kernel::shm::ShmRingBuffer>>> =
    std::sync::LazyLock::new(|| {
        let ring = crate::kernel::shm::ShmRingBuffer::new("kronos_daemon_shm", 2 * 1024 * 1024)
            .unwrap_or_else(|e| panic!("Failed to initialize SHM ring buffer: {e}"));
        std::sync::Arc::new(tokio::sync::Mutex::new(ring))
    });

pub fn get_shm_ring() -> std::sync::Arc<tokio::sync::Mutex<crate::kernel::shm::ShmRingBuffer>> {
    SHM_RING.clone()
}

static AGENT_MANAGER: std::sync::OnceLock<crate::agent::manager::AgentManager> = std::sync::OnceLock::new();
static PLUGIN_MANAGER: std::sync::OnceLock<crate::agent::providers::plugins::NativePluginManager> = std::sync::OnceLock::new();

pub fn get_agent_manager() -> &'static crate::agent::manager::AgentManager {
    AGENT_MANAGER.get_or_init(|| {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join(".paseo");
        crate::agent::manager::AgentManager::new(home)
    })
}

pub fn get_plugin_manager() -> &'static crate::agent::providers::plugins::NativePluginManager {
    PLUGIN_MANAGER.get_or_init(crate::agent::providers::plugins::NativePluginManager::new)
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
            data: Some(crate::kernel::protocol::build_server_info_payload()),
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
            let sample_rate = envelope.payload.get("sampleRate")
                .and_then(|v| v.as_u64())
                .unwrap_or(16000) as u32;
            let channels = envelope.payload.get("channels")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u16;

            let samples_opt: Option<Vec<f32>> = if let Some(arr) = envelope.payload.get("samples").and_then(|v| v.as_array()) {
                Some(arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
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
                    let config = crate::kernel::speech::SpeechAudioConfig::new(sample_rate, channels);
                    let engine_lock = get_speech_engine();
                    let engine = engine_lock.read().await;

                    match engine.transcribe_with_config(&samples, config) {
                        Ok(transcript) => {
                            let duration_secs = samples.len() as f32 / (sample_rate as f32 * channels as f32);
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
                None => RpcResponse::err(&envelope.id, "speech/transcribe/response", "Missing 'samples' or 'pcmBase64' audio buffer"),
            }
        }
        "speech/load_model" => {
            let path_opt = envelope.payload.get("modelPath")
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
                None => RpcResponse::err(&envelope.id, "speech/load_model/response", "Missing 'modelPath' parameter"),
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
            let stream_id = envelope.payload.get("streamId")
                .or_else(|| envelope.payload.get("stream_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("telemetry-default");
            let payload_b64 = envelope.payload.get("payloadBase64")
                .or_else(|| envelope.payload.get("payload_base64"))
                .or_else(|| envelope.payload.get("payload"))
                .and_then(|v| v.as_str());

            match payload_b64 {
                Some(b64) => {
                    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                        .unwrap_or_else(|_| b64.as_bytes().to_vec());
                    let ring_lock = get_shm_ring();
                    let ring = ring_lock.lock().await;
                    let next_seq = ring.sequence() + 1;
                    match crate::kernel::rkyv_shm::write_rkyv_chunk_to_shm(&ring, next_seq, stream_id, bytes) {
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
                None => RpcResponse::err(&envelope.id, "shm/push/response", "Missing 'payloadBase64' parameter"),
            }
        }
        "shm/poll" => {
            let since_seq = envelope.payload.get("sinceSeq")
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

pub fn agent_record_to_snapshot(record: &crate::agent::types::StoredAgentRecord) -> serde_json::Value {
    let mut val = serde_json::json!({
        "id": record.id,
        "provider": record.provider,
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
            "supportsSessionPersistence": false,
            "supportsDynamicModes": false,
            "supportsMcpServers": true,
            "supportsReasoningStream": false,
            "supportsToolInvocations": true
        },
        "currentModeId": null,
        "availableModes": [],
        "pendingPermissions": [],
        "persistence": null,
        "title": record.title,
        "labels": record.labels,
    });
    if let Some(ref ws_id) = record.workspace_id {
        val["workspaceId"] = serde_json::json!(ws_id);
    }
    if let Some(ref cfg) = record.config {
        if let Some(m) = cfg.get("model") {
            val["model"] = m.clone();
        }
    }
    val
}

pub fn timeline_item_to_entry(
    provider: &str,
    item: &crate::agent::types::AgentTimelineItem,
    seq: u64,
) -> serde_json::Value {
    let (item_json, ts) = match item {
        crate::agent::types::AgentTimelineItem::UserMessage { id, text, timestamp } => (
            serde_json::json!({
                "type": "user_message",
                "text": text,
                "messageId": id,
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::AssistantMessage { id, text, timestamp } => (
            serde_json::json!({
                "type": "assistant_message",
                "text": text,
                "messageId": id,
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::Thinking { text, timestamp, .. } => (
            serde_json::json!({
                "type": "reasoning",
                "text": text,
            }),
            timestamp.clone(),
        ),
        crate::agent::types::AgentTimelineItem::ToolCall { name, input, timestamp, call_id, .. } => (
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
        crate::agent::types::AgentTimelineItem::ToolResult { output, timestamp, is_error, call_id, .. } => (
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
        crate::agent::types::AgentTimelineItem::ErrorNotice { message, timestamp, .. } => (
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
        "git/diff" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let staged = msg.get("staged")
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
                })
            }
        }
        "git/branches" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
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
                })
            }
        }
        "git/worktree/list" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
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
                })
            }
        }
        "terminal/snapshot" => {
            let payload = msg.get("payload");
            let terminal_id = msg.get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("term-default");
            let rows = msg.get("rows")
                .or_else(|| payload.and_then(|p| p.get("rows")))
                .and_then(|v| v.as_u64())
                .unwrap_or(24) as u16;
            let cols = msg.get("cols")
                .or_else(|| payload.and_then(|p| p.get("cols")))
                .and_then(|v| v.as_u64())
                .unwrap_or(80) as u16;

            let screen = crate::kernel::terminal::get_terminal_registry().get_or_create(terminal_id, rows, cols);
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
            let terminal_id = msg.get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("term-default");
            let data = msg.get("data")
                .or_else(|| payload.and_then(|p| p.get("data")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let screen = crate::kernel::terminal::get_terminal_registry().get_or_create(terminal_id, 24, 80);
            screen.process(data.as_bytes());
            serde_json::json!({
                "type": "terminal/feed/response",
                "requestId": request_id,
                "id": request_id,
                "success": true
            })
        }
        "agent/create" => {
            let payload = msg.get("payload");
            let provider = msg.get("provider")
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .and_then(|v| v.as_str())
                .unwrap_or("claude");
            let cwd = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let title = msg.get("title")
                .or_else(|| payload.and_then(|p| p.get("title")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let workspace_id = msg.get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let config = msg.get("config").or_else(|| payload.and_then(|p| p.get("config"))).cloned();

            let mgr = get_agent_manager();
            match mgr.create_agent(provider, cwd, title, workspace_id, config) {
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
                })
            }
        }
        "agent/get" => {
            let payload = msg.get("payload");
            let agent_id = msg.get("id")
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
            let agent_id = msg.get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let text = msg.get("text")
                .or_else(|| payload.and_then(|p| p.get("text")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            match mgr.send_message(agent_id, text) {
                Ok(()) => serde_json::json!({
                    "type": "agent/message/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": true
                }),
                Err(err) => serde_json::json!({
                    "type": "agent/message/response",
                    "requestId": request_id,
                    "id": request_id,
                    "success": false,
                    "error": err,
                    "payload": { "error": err }
                })
            }
        }
        "agent/timeline" => {
            let payload = msg.get("payload");
            let agent_id = msg.get("id")
                .or_else(|| payload.and_then(|p| p.get("id")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            let items = mgr.get_timeline(agent_id);
            serde_json::json!({
                "type": "agent/timeline/response",
                "requestId": request_id,
                "id": request_id,
                "success": true,
                "payload": { "items": items },
                "timeline": items
            })
        }
        "agent/cancel" => {
            let payload = msg.get("payload");
            let agent_id = msg.get("id")
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
                })
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
        "project.list.request" => {
            let store = crate::kernel::config_store::get_config_store();
            let projects = store.list_projects();
            let project_descriptors: Vec<serde_json::Value> = projects.iter().map(|p| {
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
            }).collect();

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
            let requested_cwd = msg.get("cwd")
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
                    let dir_name = canonical.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("project")
                        .to_string();

                    let normalized_path_lower = abs_path.to_lowercase().replace('\\', "/");
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&normalized_path_lower, &mut hasher);
                    let hash_val = std::hash::Hasher::finish(&hasher);
                    let project_id = format!("prj_{:016x}", hash_val);

                    let dur = std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default();
                    let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());

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
            let requested_cwd = msg.get("cwd")
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
                    let dir_name = canonical.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("project")
                        .to_string();

                    let normalized_path_lower = abs_path.to_lowercase().replace('\\', "/");
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::hash::Hash::hash(&normalized_path_lower, &mut hasher);
                    let hash_val = std::hash::Hasher::finish(&hasher);
                    let project_id = format!("prj_{:016x}", hash_val);
                    let workspace_id = format!("ws_{:016x}", hash_val);

                    let dur = std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default();
                    let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());

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
            let parent_path = msg.get("parentPath")
                .or_else(|| payload.and_then(|p| p.get("parentPath")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let name = msg.get("name")
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

                        let dur = std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default();
                        let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());

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
            let project_id = msg.get("projectId")
                .or_else(|| payload.and_then(|p| p.get("projectId")))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();

            let store = crate::kernel::config_store::get_config_store();
            let removed = store.remove_project(project_id).ok().flatten();
            let removed_ws = store.remove_workspaces_for_project(project_id).unwrap_or_default();
            let ws_ids: Vec<String> = removed_ws.into_iter().map(|w| w.id).collect();

            serde_json::json!({
                "type": "project.remove.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "projectId": project_id,
                    "accepted": removed.is_some(),
                    "removedWorkspaceIds": ws_ids,
                    "error": null
                }
            })
        }
        "project.icon.get.request" => {
            let payload = msg.get("payload");
            let p_id = msg.get("projectId")
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
            let sub_id = if msg.get("subscribe").is_some() || payload.and_then(|p| p.get("subscribe")).is_some() {
                Some(format!("sub-workspaces-{}", request_id))
            } else {
                None
            };
            let dur = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());
            let store = crate::kernel::config_store::get_config_store();
            let workspaces = store.list_workspaces();
            let projects = store.list_projects();
            let project_map: std::collections::HashMap<_, _> = projects.iter().map(|p| (p.id.clone(), p.clone())).collect();

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

            let empty_projects: Vec<serde_json::Value> = projects.into_iter().filter(|p| {
                !entries.iter().any(|e| e.get("projectId").and_then(|v| v.as_str()) == Some(p.id.as_str()))
            }).map(|p| {
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
            }).collect();

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
            let source = msg.get("source").or_else(|| payload.and_then(|p| p.get("source")));
            let source_path = source.and_then(|s| s.get("path")).and_then(|v| v.as_str())
                .or_else(|| source.and_then(|s| s.get("cwd")).and_then(|v| v.as_str()))
                .or_else(|| msg.get("path").and_then(|v| v.as_str()))
                .or_else(|| msg.get("cwd").and_then(|v| v.as_str()))
                .unwrap_or("")
                .trim();

            let project_id_req = source.and_then(|s| s.get("projectId")).and_then(|v| v.as_str())
                .or_else(|| msg.get("projectId").and_then(|v| v.as_str()))
                .unwrap_or("");

            let requested_id = msg.get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let store = crate::kernel::config_store::get_config_store();

            let (abs_path, project_id, dir_name) = if !source_path.is_empty() {
                let p = std::path::Path::new(source_path);
                let canonical = canonicalize_path(p);
                let abs = canonical.to_string_lossy().to_string();
                let name = canonical.file_name()
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
                    (".".to_string(), project_id_req.to_string(), "workspace".to_string())
                }
            } else {
                (".".to_string(), "prj_default".to_string(), "workspace".to_string())
            };

            let ws_id = if requested_id.starts_with("wks_") && requested_id.len() == 20 {
                requested_id.to_string()
            } else {
                let dur = std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default();
                format!("wks_{:016x}", dur.as_nanos() as u64)
            };

            let dur = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());

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
                "id": ws_id,
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

            serde_json::json!({
                "type": "workspace.create.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspace": workspace_descriptor,
                    "setupTerminalId": null,
                    "error": null
                }
            })
        }
        "agent.create.request" | "create_agent_request" => {
            let payload = msg.get("payload");
            let provider = msg.get("provider")
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .and_then(|v| v.as_str())
                .unwrap_or("codex");
            let cwd = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let title = msg.get("title")
                .or_else(|| payload.and_then(|p| p.get("title")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let ws_id = msg.get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let model = msg.get("model")
                .or_else(|| payload.and_then(|p| p.get("model")))
                .and_then(|v| v.as_str());

            let mut config = msg.get("config")
                .or_else(|| payload.and_then(|p| p.get("config")))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            if let Some(m) = model {
                if let Some(obj) = config.as_object_mut() {
                    obj.insert("model".to_string(), serde_json::json!(m));
                }
            }

            let mgr = get_agent_manager();
            match mgr.create_agent(provider, cwd, title, ws_id, Some(config)) {
                Ok(record) => {
                    let snapshot = agent_record_to_snapshot(&record);
                    serde_json::json!({
                        "type": "agent.create.response",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "agent": snapshot,
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
            serde_json::json!({
                "type": "paseo_worktree_list_response",
                "payload": {
                    "requestId": request_id,
                    "worktrees": [],
                    "error": null
                }
            })
        }
        "session.events.set_subscription.request" => {
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
            let sub_id = msg.get("subscriptionId")
                .or_else(|| payload.and_then(|p| p.get("subscriptionId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "subscription.release.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id
                }
            })
        }
        "fetch_agents_request" => {
            let payload = msg.get("payload");
            let sub_id = if msg.get("subscribe").is_some() || payload.and_then(|p| p.get("subscribe")).is_some() {
                Some(format!("sub-agents-{}", request_id))
            } else {
                None
            };
            let mgr = get_agent_manager();
            let agents = mgr.list_agents();
            let entries: Vec<serde_json::Value> = agents.into_iter().map(|rec| {
                serde_json::json!({
                    "agent": agent_record_to_snapshot(&rec),
                    "project": null
                })
            }).collect();

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
            let agent_id = msg.get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            let agent_opt = mgr.get_agent(agent_id).map(|r| agent_record_to_snapshot(&r));
            serde_json::json!({
                "type": "fetch_agent_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agent": agent_opt,
                    "project": null
                }
            })
        }
        "send_agent_message_request" => {
            let payload = msg.get("payload");
            let agent_id = msg.get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let text = msg.get("text")
                .or_else(|| payload.and_then(|p| p.get("text")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            match mgr.send_message(agent_id, text) {
                Ok(()) => {
                    serde_json::json!({
                        "type": "send_agent_message_response",
                        "requestId": request_id,
                        "payload": {
                            "requestId": request_id,
                            "agentId": agent_id,
                            "accepted": true
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
            let agent_id = msg.get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            let agent_opt = mgr.get_agent(agent_id);
            let provider = agent_opt.as_ref().map(|a| a.provider.as_str()).unwrap_or("codex");
            let snapshot = agent_opt.as_ref().map(agent_record_to_snapshot);
            let raw_items = mgr.get_timeline(agent_id);
            let total = raw_items.len() as u64;

            let entries: Vec<serde_json::Value> = raw_items
                .into_iter()
                .enumerate()
                .map(|(idx, item)| timeline_item_to_entry(provider, &item, (idx + 1) as u64))
                .collect();

            let (min_seq, max_seq, next_seq, start_cursor, end_cursor) = if total > 0 {
                (
                    1u64,
                    total,
                    total + 1,
                    Some(serde_json::json!({ "epoch": "epoch-1", "seq": 1 })),
                    Some(serde_json::json!({ "epoch": "epoch-1", "seq": total })),
                )
            } else {
                (0u64, 0u64, 1u64, None, None)
            };

            serde_json::json!({
                "type": "fetch_agent_timeline_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "agentId": agent_id,
                    "agent": snapshot,
                    "direction": "tail",
                    "projection": "projected",
                    "epoch": "epoch-1",
                    "reset": false,
                    "staleCursor": false,
                    "gap": false,
                    "window": {
                        "minSeq": min_seq,
                        "maxSeq": max_seq,
                        "nextSeq": next_seq
                    },
                    "startCursor": start_cursor,
                    "endCursor": end_cursor,
                    "hasOlder": false,
                    "hasNewer": false,
                    "entries": entries,
                    "error": null
                }
            })
        }
        "cancel_agent_request" => {
            let payload = msg.get("payload");
            let agent_id = msg.get("agentId")
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
            let agent_id = msg.get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let mgr = get_agent_manager();
            let _ = mgr.delete_agent(agent_id);
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
            let agent_id = msg.get("agentId")
                .or_else(|| payload.and_then(|p| p.get("agentId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let dur = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());

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
        "clear_agent_attention" => {
            serde_json::json!({
                "type": "clear_agent_attention_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "cleared": true
                }
            })
        }
        "agent.config.apply.request" => {
            serde_json::json!({
                "type": "agent.config.apply.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "accepted": true,
                    "notice": null
                }
            })
        }
        "set_agent_mode_request" => {
            serde_json::json!({
                "type": "set_agent_mode_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "accepted": true,
                    "notice": null
                }
            })
        }
        "set_agent_model_request" => {
            serde_json::json!({
                "type": "set_agent_model_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "accepted": true
                }
            })
        }
        "set_agent_feature_request" => {
            serde_json::json!({
                "type": "set_agent_feature_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "accepted": true
                }
            })
        }
        "set_agent_thinking_request" => {
            serde_json::json!({
                "type": "set_agent_thinking_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "accepted": true,
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
                    "commands": []
                }
            })
        }
        "agent.skills.get_status.request" => {
            serde_json::json!({
                "type": "agent.skills.get_status.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "skills": []
                }
            })
        }
        "agent.skills.reconcile.request" => {
            serde_json::json!({
                "type": "agent.skills.reconcile.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "success": true
                }
            })
        }
        "agent.skills.uninstall.request" => {
            serde_json::json!({
                "type": "agent.skills.uninstall.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "success": true
                }
            })
        }
        "workspace.label.list.request" => {
            let payload = msg.get("payload");
            let sub_id = if msg.get("subscribe").is_some() || payload.and_then(|p| p.get("subscribe")).is_some() {
                Some(format!("sub-labels-{}", request_id))
            } else {
                None
            };
            serde_json::json!({
                "type": "workspace.label.list.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": sub_id,
                    "labels": [],
                    "sync": {
                        "mode": "snapshot",
                        "generation": "gen-1",
                        "headSeq": 0,
                        "removals": []
                    }
                }
            })
        }
        "workspace.setup.subscribe.request" => {
            let payload = msg.get("payload");
            let ws_id = msg.get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "workspace.setup.subscribe.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-ws-setup-{}", request_id),
                    "workspaceId": ws_id,
                    "snapshot": null
                }
            })
        }
        "workspace_setup_status_request" | "workspace.setup.status.request" => {
            let payload = msg.get("payload");
            let ws_id = msg.get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "workspace_setup_status_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "status": null,
                    "step": null
                }
            })
        }
        "agent.timeline.set_subscription.request" => {
            let payload = msg.get("payload");
            let agent_ids = msg.get("agentIds")
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
        "subscribe_terminals_request" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            serde_json::json!({
                "type": "terminals_changed",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-terms-{}", request_id),
                    "cwd": cwd_str,
                    "terminals": []
                }
            })
        }
        "subscribe_terminal_request" => {
            let payload = msg.get("payload");
            let term_id = msg.get("terminalId")
                .or_else(|| payload.and_then(|p| p.get("terminalId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "subscribe_terminal_response",
                "requestId": request_id,
                "payload": {
                    "terminalId": term_id
                }
            })
        }
        "creation.subscribe.request" => {
            serde_json::json!({
                "type": "creation.subscribe.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-create-{}", request_id),
                    "snapshot": null,
                    "error": null
                }
            })
        }
        "subscribe_checkout_diff_request" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            serde_json::json!({
                "type": "subscribe_checkout_diff_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-diff-{}", request_id),
                    "cwd": cwd_str,
                    "files": [],
                    "error": null
                }
            })
        }
        "unsubscribe_checkout_diff_request" | "unsubscribe_terminals_request" | "unsubscribe_terminal_request" => {
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
            let dur = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());
            let cwd = msg.get("cwd").and_then(|value| value.as_str()).unwrap_or(".");
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
            let dur = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let now_iso = format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis());
            let entries = get_provider_snapshot_entries(".").await;
            let providers: Vec<serde_json::Value> = entries
                .iter()
                .filter_map(|entry| {
                    let provider = entry.get("provider")?.as_str()?;
                    let available = entry.get("models")?.as_array().is_some_and(|models| !models.is_empty());
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
            let provider = msg.get("provider")
                .or_else(|| payload.and_then(|p| p.get("provider")))
                .and_then(|v| v.as_str())
                .unwrap_or("codex");
            serde_json::json!({
                "type": "provider_diagnostic_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "provider": provider,
                    "diagnostic": "Provider ready"
                }
            })
        }
        "browser.host.register.request" => {
            serde_json::json!({
                "type": "browser.host.register.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-browser-{}", request_id)
                }
            })
        }
        "get_daemon_config_request" => {
            let store = crate::kernel::config_store::get_config_store();
            let cfg = store.get_config();
            serde_json::json!({
                "type": "get_daemon_config_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "config": {
                        "mcp": {
                            "enabled": cfg.mcp_enabled,
                            "injectIntoAgents": false
                        },
                        "relay": {
                            "enabled": cfg.relay_enabled,
                            "endpoint": cfg.relay_endpoint
                        },
                        "providers": {},
                        "autoArchiveAfterMerge": false,
                        "enableTerminalAgentHooks": false
                    }
                }
            })
        }
        "daemon.get_status.request" => {
            let server_id = crate::get_or_create_server_id();
            serde_json::json!({
                "type": "daemon.get_status.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "serverId": server_id,
                    "version": env!("CARGO_PKG_VERSION"),
                    "pid": std::process::id(),
                    "nodePath": "rust-embedded",
                    "listen": "127.0.0.1:6767",
                    "providers": []
                }
            })
        }
        "plugin.catalog.get.request" => {
            serde_json::json!({
                "type": "plugin.catalog.get.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "plugins": []
                }
            })
        }
        "plugin.list.request" => {
            serde_json::json!({
                "type": "plugin.list.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "plugins": []
                }
            })
        }
        "diagnostics.request" => {
            serde_json::json!({
                "type": "diagnostics.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "diagnostic": "ok"
                }
            })
        }
        "list_terminals_request" => {
            serde_json::json!({
                "type": "list_terminals_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "terminals": []
                }
            })
        }
        "workspace.script.list.request" => {
            let payload = msg.get("payload");
            let ws_id = msg.get("workspaceId")
                .or_else(|| payload.and_then(|p| p.get("workspaceId")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "workspace.script.list.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "scripts": [],
                    "error": null
                }
            })
        }
        "checkout_status_request" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            serde_json::json!({
                "type": "checkout_status_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "cwd": cwd_str,
                    "error": null,
                    "isGit": false,
                    "isPaseoOwnedWorktree": false,
                    "repoRoot": null,
                    "currentBranch": null,
                    "isDirty": null,
                    "baseRef": null,
                    "aheadBehind": null,
                    "aheadOfOrigin": null,
                    "behindOfOrigin": null,
                    "hasRemote": false,
                    "remoteUrl": null
                }
            })
        }
        "fs.file.subscribe.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path_str = msg.get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({
                "type": "fs.file.subscribe.response",
                "requestId": request_id,
                "payload": {
                    "subscriptionId": format!("sub-file-{}", request_id),
                    "initial": {
                        "status": "missing",
                        "cwd": cwd_str,
                        "path": path_str
                    },
                    "requestId": request_id
                }
            })
        }
        "fs.file.unsubscribe.request" => {
            let payload = msg.get("payload");
            let sub_id = msg.get("subscriptionId")
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
        "file_explorer_request" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path_opt = msg.get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str());

            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::file_service::list_directory(cwd, path_opt) {
                Ok(entries) => {
                    serde_json::json!({
                        "type": "file_explorer_response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": true,
                        "payload": {
                            "entries": entries
                        },
                        "entries": entries
                    })
                }
                Err(err) => {
                    serde_json::json!({
                        "type": "file_explorer_response",
                        "requestId": request_id,
                        "id": request_id,
                        "success": false,
                        "error": err.to_string(),
                        "payload": {
                            "error": err.to_string()
                        }
                    })
                }
            }
        }
        "fs.file.write.request" => {
            let payload = msg.get("payload");
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let rel_path = msg.get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let content_str = msg.get("content")
                .or_else(|| payload.and_then(|p| p.get("content")))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let cwd = std::path::Path::new(cwd_str);
            match crate::kernel::file_service::write_file(cwd, rel_path, content_str.as_bytes()) {
                Ok(()) => {
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
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let parent = msg.get("parentPath")
                .or_else(|| payload.and_then(|p| p.get("parentPath")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let name = msg.get("name")
                .or_else(|| payload.and_then(|p| p.get("name")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let kind = msg.get("kind")
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
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path = msg.get("path")
                .or_else(|| payload.and_then(|p| p.get("path")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new_name = msg.get("newName")
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
            let cwd_str = msg.get("cwd")
                .or_else(|| payload.and_then(|p| p.get("cwd")))
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let path = msg.get("path")
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
        msg_type if msg_type.starts_with("crdt/") || msg_type.starts_with("ioring/") || msg_type.starts_with("speech/") || msg_type.starts_with("shm/") => {
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
        msg_type if msg_type.contains("subscribe") || msg_type.contains("set_subscription") => {
            let resp_type = if msg_type.ends_with(".request") {
                format!("{}.response", &msg_type[..msg_type.len() - 8])
            } else if msg_type.ends_with("_request") {
                format!("{}_response", &msg_type[..msg_type.len() - 8])
            } else {
                format!("{}_response", msg_type)
            };
            serde_json::json!({
                "type": resp_type,
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "subscriptionId": format!("sub-gen-{}", request_id),
                    "error": null
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
            "vfs/watch" | "vfs/watch_subscribe" | "vfs/unwatch" | "vfs/watch_unsubscribe" | "ast/analyze" | "ast/parse" | "ast/symbols" | "crdt/join" | "crdt/update" | "crdt/sync" | "crdt/state" | "crdt/edit" | "ioring/status" | "ioring/read_batch" | "speech/status" | "speech/transcribe" | "speech/load_model" | "shm/status" | "shm/push" | "shm/poll" => {
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
            .merge(crate::kernel::browser_proxy::router())
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

pub(crate) fn get_provider_snapshot_metadata_entries() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "provider": "claude",
            "status": "ready",
            "enabled": true,
            "source": "runtime",
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
            "source": "runtime",
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
            "source": "runtime",
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
            "source": "runtime",
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
            "source": "runtime",
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
            "source": "runtime",
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
            "source": "runtime",
            "label": "Copilot",
            "description": "GitHub Copilot via Agent Client Protocol",
            "defaultModeId": "https://agentclientprotocol.com/protocol/session-modes#agent",
            "modes": [
                { "id": "https://agentclientprotocol.com/protocol/session-modes#agent", "label": "Agent", "description": "Default agent mode", "colorTier": "moderate" },
                { "id": "https://agentclientprotocol.com/protocol/session-modes#plan", "label": "Plan", "description": "Plan mode", "colorTier": "planning" },
                { "id": "allow-all", "label": "Allow All", "description": "Approve all Copilot tool requests", "colorTier": "dangerous" }
            ],
            "models": []
        })
    ]
}

async fn get_provider_snapshot_entries(cwd: &str) -> Vec<serde_json::Value> {
    let mut cache = PROVIDER_SNAPSHOT_CACHE.lock().await;
    if let Some((generated_at, cached_cwd, entries)) = cache.as_ref() {
        if cached_cwd == cwd && generated_at.elapsed() < Duration::from_secs(60) {
            return entries.clone();
        }
    }

    let (codex_models, claude_models, opencode_models, ollama_models, pi_models, omp_models, copilot_available) = tokio::join!(
        discover_codex_models(cwd),
        discover_claude_configured_model(),
        discover_opencode_models(cwd),
        discover_ollama_models(),
        discover_pi_models(cwd),
        discover_omp_models(cwd),
        discover_copilot_available()
    );
    let models_by_provider = [
        ("codex", codex_models),
        ("claude", claude_models),
        ("opencode", opencode_models),
        ("ollama", ollama_models),
        ("pi", pi_models),
        ("omp", omp_models),
    ];
    let mut entries = get_provider_snapshot_metadata_entries();

    for entry in &mut entries {
        let provider = entry.get("provider").and_then(serde_json::Value::as_str).unwrap_or("");
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

async fn discover_pi_models(cwd: &str) -> Vec<serde_json::Value> {
    let binary = std::env::var("PI_COMMAND")
        .or_else(|_| std::env::var("PI_ACP_PI_COMMAND"))
        .unwrap_or_else(|_| "pi".to_owned());
    discover_jsonl_cli_models(&binary, &["--mode", "rpc"], cwd, false, pi_models_from_response).await
}

async fn discover_omp_models(cwd: &str) -> Vec<serde_json::Value> {
    let binary = std::env::var("OMP_COMMAND").unwrap_or_else(|_| "omp".to_owned());
    discover_jsonl_cli_models(&binary, &["--mode", "rpc-ui"], cwd, true, omp_models_from_response).await
}

/// Returns `true` when the Copilot ACP binary is on PATH and exits without error.
/// Copilot populates its model list via ACP `session/new` at session time, so the
/// snapshot carries an empty model list; only binary availability matters here.
async fn discover_copilot_available() -> bool {
    let binary = std::env::var("COPILOT_COMMAND").unwrap_or_else(|_| "copilot".to_owned());
    tokio::process::Command::new(&binary)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .status()
        .await
        .map(|s| {
            // exit 127 = command not found; treat everything else as "binary exists"
            s.code().map(|c| c != 127).unwrap_or(true)
        })
        .unwrap_or(false)
}

async fn discover_jsonl_cli_models(
    binary: &str,
    args: &[&str],
    cwd: &str,
    wait_ready: bool,
    parse: fn(&serde_json::Value) -> Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    let discovery = async {
        let mut child = Command::new(binary)
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .ok()?;
        let mut stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let mut lines = BufReader::new(stdout).lines();

        if wait_ready {
            let ready = tokio::time::timeout(Duration::from_secs(2), async {
                while let Ok(Some(line)) = lines.next_line().await {
                    if serde_json::from_str::<serde_json::Value>(&line)
                        .ok()
                        .and_then(|value| value.get("type").and_then(serde_json::Value::as_str).map(str::to_owned))
                        .as_deref() == Some("ready")
                    {
                        return true;
                    }
                }
                false
            })
            .await
            .unwrap_or(false);
            if !ready {
                let _ = child.kill().await;
                return None;
            }
        }

        let request = serde_json::json!({
            "id": "kronos-model-catalog",
            "type": "get_available_models"
        });
        if stdin.write_all(format!("{request}\n").as_bytes()).await.is_err() {
            let _ = child.kill().await;
            return None;
        }
        let response = tokio::time::timeout(Duration::from_secs(3), async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                    if value.get("type").and_then(serde_json::Value::as_str) == Some("response")
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
        Some(response
            .filter(|value| value.get("success").and_then(serde_json::Value::as_bool) == Some(true))
            .map(|value| parse(&value["data"]))
            .unwrap_or_default())
    };

    tokio::time::timeout(Duration::from_secs(5), discovery)
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
            Some(dynamic_model(
                provider_name,
                &id,
                model.get("name").and_then(serde_json::Value::as_str),
                None,
                false,
                model.get("contextWindow").and_then(serde_json::Value::as_u64),
            ))
        })
        .collect()
}

async fn invalidate_provider_snapshot_cache() {
    *PROVIDER_SNAPSHOT_CACHE.lock().await = None;
}

async fn discover_codex_models(cwd: &str) -> Vec<serde_json::Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::process::Command;

    let discovery = async {
        let mut child = Command::new("codex")
            .arg("app-server")
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .ok()?;
        let mut stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let mut lines = BufReader::new(stdout).lines();

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
        if stdin.write_all(format!("{initialize}\n").as_bytes()).await.is_err() {
            let _ = child.kill().await;
            return None;
        }

        let initialized = tokio::time::timeout(Duration::from_secs(2), async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                    if value.get("id") == Some(&serde_json::json!(1)) {
                        return Some(());
                    }
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
        .is_some();

        if !initialized {
            let _ = child.kill().await;
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
        if stdin.write_all(format!("{notification}\n{request}\n").as_bytes()).await.is_err() {
            let _ = child.kill().await;
            return None;
        }

        let response = tokio::time::timeout(Duration::from_secs(2), async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
                    if value.get("id") == Some(&serde_json::json!(2)) {
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
        Some(response.map(|value| codex_models_from_response(&value)).unwrap_or_default())
    };

    tokio::time::timeout(Duration::from_secs(5), discovery)
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
            Some(dynamic_model(
                "codex",
                id,
                model.get("displayName").and_then(serde_json::Value::as_str),
                model.get("description").and_then(serde_json::Value::as_str),
                model.get("isDefault").and_then(serde_json::Value::as_bool).unwrap_or(false),
                model.get("contextWindow").and_then(serde_json::Value::as_u64),
            ))
        })
        .collect()
}

async fn discover_claude_configured_model() -> Vec<serde_json::Value> {
    let configured_model = std::env::var("ANTHROPIC_MODEL")
        .ok()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty());
    let model = if configured_model.is_some() {
        configured_model
    } else if let Ok(config_dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        read_claude_settings_model(std::path::Path::new(&config_dir)).await
    } else if let Some(home) = dirs::home_dir() {
        read_claude_settings_model(&home.join(".claude")).await
    } else {
        None
    };

    model
        .into_iter()
        .map(|id| dynamic_model("claude", &id, None, None, true, None))
        .collect()
}

pub(crate) async fn read_claude_settings_model(config_dir: &std::path::Path) -> Option<String> {
    let settings_path = config_dir.join("settings.json");
    let raw = tokio::fs::read_to_string(settings_path).await.ok()?;
    let settings: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let model = settings.get("model").and_then(serde_json::Value::as_str)
        .or_else(|| settings.pointer("/env/ANTHROPIC_MODEL").and_then(serde_json::Value::as_str))?;
    let model = model.trim();
    (!model.is_empty()).then(|| model.to_owned())
}

async fn discover_opencode_models(cwd: &str) -> Vec<serde_json::Value> {
    let base_url = normalize_http_endpoint(
        std::env::var("OPENCODE_SERVER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:4096".to_owned()),
    );
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(2)).build() {
        Ok(client) => client,
        Err(_) => return Vec::new(),
    };
    let response = match client
        .get(format!("{base_url}/provider"))
        .header("x-opencode-directory", cwd)
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
            let id = provider.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
            connected_ids.contains(id)
                || provider.get("source").and_then(serde_json::Value::as_str) == Some("api")
        })
        .flat_map(|provider| {
            let provider_id = provider.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
            provider.get("models").and_then(serde_json::Value::as_object)
                .into_iter()
                .flat_map(move |models| models.iter().filter_map(move |(model_id, details)| {
                    let id = format!("{provider_id}/{model_id}");
                    Some(dynamic_model(
                        "opencode",
                        &id,
                        details.get("name").and_then(serde_json::Value::as_str),
                        details.get("description").and_then(serde_json::Value::as_str),
                        details.get("default").and_then(serde_json::Value::as_bool).unwrap_or(false),
                        details.pointer("/limit/context").and_then(serde_json::Value::as_u64),
                    ))
                }))
        })
        .collect()
}
async fn discover_ollama_models() -> Vec<serde_json::Value> {
    let endpoint = normalize_http_endpoint(
        std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://127.0.0.1:11434".to_owned()),
    );
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(2)).build() {
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
            let parameter_size = model.pointer("/details/parameter_size")
                .and_then(serde_json::Value::as_str);
            let label = parameter_size
                .map(|size| format!("{name} ({size})"))
                .unwrap_or_else(|| name.to_owned());
            let family = model.pointer("/details/family")
                .and_then(serde_json::Value::as_str);
            Some(dynamic_model(
                "ollama",
                &format!("ollama/{name}"),
                Some(&label),
                family.map(|value| format!("Local Ollama model from the {value} family")).as_deref(),
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
