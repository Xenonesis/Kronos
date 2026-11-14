use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::{debug, info, warn};

/// Position in a text document (zero-based).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

/// Range in a text document.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

/// Location represents a location inside a resource.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspLocation {
    pub uri: String,
    pub range: LspRange,
}

/// Result of a hover request.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LspHoverResult {
    pub contents: String,
    pub range: Option<LspRange>,
}

/// Configuration for launching an LSP server process.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LspServerConfig {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub root_path: String,
    #[serde(default)]
    pub initialization_options: Option<Value>,
}

/// Status of an LSP server instance.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LspServerStatus {
    pub id: String,
    pub command: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub root_path: String,
    pub initialized: bool,
}

/// Active LSP process supervisor wrapping stdin/stdout and JSON-RPC 2.0 framing.
pub struct LspProcess {
    config: LspServerConfig,
    stdin: Mutex<Option<ChildStdin>>,
    next_request_id: AtomicU64,
    pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    child_pid: Option<u32>,
    is_initialized: Arc<RwLock<bool>>,
    child: Mutex<Option<Child>>,
}

impl LspProcess {
    /// Launches a new LSP child process and spawns the stdout reader loop.
    pub async fn launch(config: LspServerConfig) -> Result<Arc<Self>, String> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .current_dir(&config.root_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "Failed to spawn LSP process '{}' with args {:?}: {}",
                config.command, config.args, e
            )
        })?;

        let pid = child.id();
        let stdin = child.stdin.take().ok_or("Failed to open LSP stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to open LSP stdout")?;
        let stderr = child.stderr.take();

        let pending_requests: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let is_initialized = Arc::new(RwLock::new(false));

        let lsp_proc = Arc::new(Self {
            config: config.clone(),
            stdin: Mutex::new(Some(stdin)),
            next_request_id: AtomicU64::new(1),
            pending_requests: pending_requests.clone(),
            child_pid: pid,
            is_initialized: is_initialized.clone(),
            child: Mutex::new(Some(child)),
        });

        // Spawn async stdout reader thread
        let pending_clone = pending_requests.clone();
        let server_id = config.id.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                // Header parsing loop
                let mut content_length: Option<usize> = None;
                let mut line = String::new();

                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => {
                            debug!("[LSP:{}] stdout reached EOF", server_id);
                            return;
                        }
                        Ok(_) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                // End of headers
                                break;
                            }
                            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                                if let Ok(len) = rest.trim().parse::<usize>() {
                                    content_length = Some(len);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("[LSP:{}] stdout read_line error: {}", server_id, e);
                            return;
                        }
                    }
                }

                let len = match content_length {
                    Some(l) => l,
                    None => {
                        warn!("[LSP:{}] Missing Content-Length header", server_id);
                        continue;
                    }
                };

                let mut body = vec![0u8; len];
                if let Err(e) = reader.read_exact(&mut body).await {
                    warn!("[LSP:{}] Failed to read exact body of {} bytes: {}", server_id, len, e);
                    return;
                }

                // Parse JSON-RPC 2.0 payload
                if let Ok(val) = serde_json::from_slice::<Value>(&body) {
                    if let Some(id_num) = val.get("id").and_then(|v| v.as_u64()) {
                        let mut map = pending_clone.lock().await;
                        if let Some(tx) = map.remove(&id_num) {
                            if let Some(err) = val.get("error") {
                                let _ = tx.send(Err(err.to_string()));
                            } else {
                                let res = val.get("result").cloned().unwrap_or(Value::Null);
                                let _ = tx.send(Ok(res));
                            }
                        }
                    } else if let Some(method) = val.get("method").and_then(|m| m.as_str()) {
                        debug!("[LSP:{}] Notification received: {}", server_id, method);
                    }
                }
            }
        });

        // Spawn stderr logger
        if let Some(stderr) = stderr {
            let server_id = config.id.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 {
                        break;
                    }
                    debug!("[LSP:{} stderr] {}", server_id, line.trim_end());
                    line.clear();
                }
            });
        }

        // Initialize LSP server handshake
        if let Err(e) = lsp_proc.initialize_handshake().await {
            warn!("[LSP:{}] Initialize handshake returned: {}", config.id, e);
        } else {
            *is_initialized.write().await = true;
            info!("[LSP:{}] Handshake completed successfully", config.id);
        }

        Ok(lsp_proc)
    }

    /// Performs the initial `initialize` and `initialized` handshake sequence.
    async fn initialize_handshake(&self) -> Result<(), String> {
        let root_url = url::Url::from_directory_path(Path::new(&self.config.root_path))
            .map_err(|_| format!("Invalid root_path: {}", self.config.root_path))?
            .to_string();

        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_url,
            "rootPath": self.config.root_path,
            "capabilities": {
                "textDocument": {
                    "hover": {
                        "dynamicRegistration": true,
                        "contentFormat": ["markdown", "plaintext"]
                    },
                    "synchronization": {
                        "dynamicRegistration": true,
                        "willSave": false,
                        "willSaveWaitUntil": false,
                        "didSave": true
                    },
                    "completion": {
                        "dynamicRegistration": true,
                        "completionItem": {
                            "snippetSupport": true
                        }
                    },
                    "definition": {
                        "dynamicRegistration": true,
                        "linkSupport": true
                    },
                    "references": {
                        "dynamicRegistration": true
                    },
                    "documentSymbol": {
                        "dynamicRegistration": true,
                        "hierarchicalDocumentSymbolSupport": true
                    },
                    "formatting": {
                        "dynamicRegistration": true
                    }
                },
                "workspace": {
                    "applyEdit": true,
                    "workspaceFolders": true
                }
            },
            "initializationOptions": self.config.initialization_options
        });

        let _ = self.request("initialize", init_params).await?;
        self.notify("initialized", serde_json::json!({})).await?;
        Ok(())
    }

    /// Send a JSON-RPC request to the LSP server and wait for response.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let req_id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let rpc_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending_requests.lock().await;
            map.insert(req_id, tx);
        }

        if let Err(e) = self.send_raw(&rpc_req).await {
            let mut map = self.pending_requests.lock().await;
            map.remove(&req_id);
            return Err(e);
        }

        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("LSP channel dropped unexpectedly".to_string()),
            Err(_) => {
                let mut map = self.pending_requests.lock().await;
                map.remove(&req_id);
                Err(format!("LSP request '{}' (id={}) timed out", method, req_id))
            }
        }
    }

    /// Send a JSON-RPC notification to the LSP server without awaiting a response.
    pub async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        let rpc_notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        self.send_raw(&rpc_notif).await
    }

    /// Internal raw byte transmission over stdin with Content-Length framing.
    async fn send_raw(&self, payload: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
        let header = format!("Content-Length: {}\r\n\r\n", bytes.len());

        let mut stdin_guard = self.stdin.lock().await;
        if let Some(stdin) = stdin_guard.as_mut() {
            stdin
                .write_all(header.as_bytes())
                .await
                .map_err(|e| format!("Failed to write LSP header: {e}"))?;
            stdin
                .write_all(&bytes)
                .await
                .map_err(|e| format!("Failed to write LSP body: {e}"))?;
            stdin
                .flush()
                .await
                .map_err(|e| format!("Failed to flush LSP stdin: {e}"))?;
            Ok(())
        } else {
            Err("LSP stdin is closed".to_string())
        }
    }

    /// High-level hover lookup.
    pub async fn hover(&self, file_path: &Path, line: u32, character: u32) -> Result<Option<LspHoverResult>, String> {
        let file_url = url::Url::from_file_path(file_path)
            .map_err(|_| format!("Invalid file path: {}", file_path.display()))?
            .to_string();

        let params = serde_json::json!({
            "textDocument": { "uri": file_url },
            "position": { "line": line, "character": character }
        });

        let res = self.request("textDocument/hover", params).await?;
        if res.is_null() {
            return Ok(None);
        }

        let contents = if let Some(s) = res.get("contents").and_then(|c| c.as_str()) {
            s.to_string()
        } else if let Some(val) = res.get("contents").and_then(|c| c.get("value")).and_then(|v| v.as_str()) {
            val.to_string()
        } else {
            res.get("contents").map(|c| c.to_string()).unwrap_or_default()
        };

        let range = res.get("range").and_then(|r| serde_json::from_value::<LspRange>(r.clone()).ok());

        Ok(Some(LspHoverResult { contents, range }))
    }

    /// High-level goto definition lookup.
    pub async fn goto_definition(&self, file_path: &Path, line: u32, character: u32) -> Result<Vec<LspLocation>, String> {
        let file_url = url::Url::from_file_path(file_path)
            .map_err(|_| format!("Invalid file path: {}", file_path.display()))?
            .to_string();

        let params = serde_json::json!({
            "textDocument": { "uri": file_url },
            "position": { "line": line, "character": character }
        });

        let res = self.request("textDocument/definition", params).await?;
        if res.is_null() {
            return Ok(Vec::new());
        }

        if let Ok(locs) = serde_json::from_value::<Vec<LspLocation>>(res.clone()) {
            Ok(locs)
        } else if let Ok(loc) = serde_json::from_value::<LspLocation>(res) {
            Ok(vec![loc])
        } else {
            Ok(Vec::new())
        }
    }

    /// Notify server that a text document was opened.
    pub async fn did_open(&self, file_path: &Path, content: &str, language_id: &str) -> Result<(), String> {
        let file_url = url::Url::from_file_path(file_path)
            .map_err(|_| format!("Invalid file path: {}", file_path.display()))?
            .to_string();

        let params = serde_json::json!({
            "textDocument": {
                "uri": file_url,
                "languageId": language_id,
                "version": 1,
                "text": content
            }
        });

        self.notify("textDocument/didOpen", params).await
    }

    /// Notify server that a text document changed in full.
    pub async fn did_change(&self, file_path: &Path, new_content: &str, version: u32) -> Result<(), String> {
        let file_url = url::Url::from_file_path(file_path)
            .map_err(|_| format!("Invalid file path: {}", file_path.display()))?
            .to_string();

        let params = serde_json::json!({
            "textDocument": {
                "uri": file_url,
                "version": version
            },
            "contentChanges": [{
                "text": new_content
            }]
        });

        self.notify("textDocument/didChange", params).await
    }

    /// Returns current status of this LSP process.
    pub fn status(&self) -> LspServerStatus {
        let initialized = self.is_initialized.try_read().map(|b| *b).unwrap_or(false);
        LspServerStatus {
            id: self.config.id.clone(),
            command: self.config.command.clone(),
            running: self.child_pid.is_some(),
            pid: self.child_pid,
            root_path: self.config.root_path.clone(),
            initialized,
        }
    }

    /// Returns whether the LSP server has finished initialization handshake.
    pub async fn is_initialized(&self) -> bool {
        *self.is_initialized.read().await
    }

    /// Gracefully shutdown and kill the LSP child process.
    pub async fn shutdown(&self) -> Result<(), String> {
        let _ = self.request("shutdown", Value::Null).await;
        let _ = self.notify("exit", Value::Null).await;

        let mut child_guard = self.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            let _ = child.kill().await;
        }
        Ok(())
    }
}

/// Multiplexer managing active language servers per project.
#[derive(Default)]
pub struct LspMultiplexer {
    servers: Arc<RwLock<HashMap<String, Arc<LspProcess>>>>,
}

impl LspMultiplexer {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Starts or gets an existing LSP server process.
    pub async fn start_server(&self, config: LspServerConfig) -> Result<Arc<LspProcess>, String> {
        let id = config.id.clone();
        {
            let map = self.servers.read().await;
            if let Some(existing) = map.get(&id) {
                return Ok(existing.clone());
            }
        }

        let proc = LspProcess::launch(config).await?;
        let mut map = self.servers.write().await;
        map.insert(id, proc.clone());
        Ok(proc)
    }

    /// Stops an active LSP server process.
    pub async fn stop_server(&self, id: &str) -> Result<bool, String> {
        let proc_opt = {
            let mut map = self.servers.write().await;
            map.remove(id)
        };

        if let Some(proc) = proc_opt {
            let _ = proc.shutdown().await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Gets an active LSP server by ID.
    pub async fn get_server(&self, id: &str) -> Option<Arc<LspProcess>> {
        let map = self.servers.read().await;
        map.get(id).cloned()
    }

    /// Returns status list of all managed servers.
    pub async fn list_servers(&self) -> Vec<LspServerStatus> {
        let map = self.servers.read().await;
        map.values().map(|p| p.status()).collect()
    }

    /// Automatically infers language ID and default server command from file extension.
    pub fn detect_language_and_cmd(path: &Path) -> Option<(&'static str, &'static str, &'static [&'static str])> {
        let ext = path.extension()?.to_str()?.to_lowercase();
        match ext.as_str() {
            "rs" => Some(("rust", "rust-analyzer", &[])),
            "ts" | "tsx" => Some(("typescript", "typescript-language-server", &["--stdio"])),
            "js" | "jsx" | "mjs" | "cjs" => Some(("javascript", "typescript-language-server", &["--stdio"])),
            "py" => Some(("python", "pyright-langserver", &["--stdio"])),
            "go" => Some(("go", "gopls", &[])),
            "json" => Some(("json", "vscode-json-language-server", &["--stdio"])),
            _ => None,
        }
    }
}

static GLOBAL_LSP_MULTIPLEXER: std::sync::OnceLock<Arc<LspMultiplexer>> = std::sync::OnceLock::new();

/// Returns global process-wide LSP multiplexer.
pub fn get_global_lsp_multiplexer() -> Arc<LspMultiplexer> {
    GLOBAL_LSP_MULTIPLEXER
        .get_or_init(|| Arc::new(LspMultiplexer::new()))
        .clone()
}
