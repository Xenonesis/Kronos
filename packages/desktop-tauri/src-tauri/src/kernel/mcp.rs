use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use serde::de::{self, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::kernel::ast::AstEngine;
use crate::kernel::git::GitEngine;
use crate::kernel::search::WorkspaceSearchIndex;

/// MCP Tool Definition representing a registered tool's schema and documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl Serialize for McpToolDefinition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("McpToolDefinition", 4)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("description", &self.description)?;
        state.serialize_field("inputSchema", &self.input_schema)?;
        state.serialize_field("input_schema", &self.input_schema)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for McpToolDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct McpToolDefinitionVisitor;

        impl<'de> Visitor<'de> for McpToolDefinitionVisitor {
            type Value = McpToolDefinition;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct McpToolDefinition")
            }

            fn visit_map<V>(self, mut map: V) -> Result<McpToolDefinition, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut name = None;
                let mut description = None;
                let mut input_schema = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "name" => {
                            name = Some(map.next_value()?);
                        }
                        "description" => {
                            description = Some(map.next_value()?);
                        }
                        "inputSchema" | "input_schema" => {
                            input_schema = Some(map.next_value()?);
                        }
                        _ => {
                            let _: Value = map.next_value()?;
                        }
                    }
                }

                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let description = description.unwrap_or_default();
                let input_schema = input_schema.unwrap_or_else(|| {
                    serde_json::json!({
                        "type": "object",
                        "properties": {}
                    })
                });

                Ok(McpToolDefinition {
                    name,
                    description,
                    input_schema,
                })
            }
        }

        deserializer.deserialize_map(McpToolDefinitionVisitor)
    }
}

type ToolCallback = Arc<dyn Fn(Value) -> Result<String, String> + Send + Sync>;

#[derive(Clone)]
struct RegisteredTool {
    definition: McpToolDefinition,
    handler: ToolCallback,
}

/// Native Model Context Protocol (MCP) Hub providing tool registry and JSON-RPC dispatch.
#[derive(Clone)]
pub struct McpHub {
    tools: Arc<RwLock<HashMap<String, RegisteredTool>>>,
    tool_order: Arc<RwLock<Vec<String>>>,
    search_index: Arc<WorkspaceSearchIndex>,
    ast_engine: Arc<AstEngine>,
    external_tools: Arc<RwLock<Vec<(ExternalMcpServerConfig, McpToolDefinition)>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMcpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: String, // "stdio" | "sse" | "http"
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMcpServerStatus {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub url: Option<String>,
    pub enabled: bool,
    pub status: String, // "connected" | "disabled" | "error" | "idle"
    pub tool_count: usize,
    pub last_error: Option<String>,
}

impl Default for McpHub {
    fn default() -> Self {
        Self::new()
    }
}

impl McpHub {
    /// Creates a new `McpHub` with all built-in tools initialized.
    pub fn new() -> Self {
        let search_index = Arc::new(
            WorkspaceSearchIndex::new_in_memory()
                .unwrap_or_else(|e| panic!("Failed to initialize WorkspaceSearchIndex: {e}")),
        );
        let ast_engine = Arc::new(AstEngine::new());

        let hub = Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            tool_order: Arc::new(RwLock::new(Vec::new())),
            search_index,
            ast_engine,
            external_tools: Arc::new(RwLock::new(Vec::new())),
        };

        hub.register_builtin_tools();
        hub
    }

    /// Registers a tool into the MCP Hub registry.
    pub fn register_tool(
        &self,
        definition: McpToolDefinition,
        handler: impl Fn(Value) -> Result<String, String> + Send + Sync + 'static,
    ) {
        let mut tools = self.tools.write().unwrap();
        let mut order = self.tool_order.write().unwrap();
        if !tools.contains_key(&definition.name) {
            order.push(definition.name.clone());
        }
        tools.insert(
            definition.name.clone(),
            RegisteredTool {
                definition,
                handler: Arc::new(handler),
            },
        );
    }

    /// Returns definitions of all registered built-in + cached external MCP tools.
    pub fn list_tools(&self) -> Vec<McpToolDefinition> {
        let tools = self.tools.read().unwrap();
        let order = self.tool_order.read().unwrap();
        let mut all: Vec<McpToolDefinition> = order
            .iter()
            .filter_map(|name| tools.get(name).map(|t| t.definition.clone()))
            .collect();
        if let Ok(ext) = self.external_tools.read() {
            for (srv, def) in ext.iter() {
                if srv.enabled {
                    let mut cloned = def.clone();
                    cloned.name = format!("{}::{}", srv.id, def.name);
                    cloned.description = format!("[{}] {}", srv.name, def.description);
                    all.push(cloned);
                }
            }
        }
        all
    }

    /// Dispatches a call to a registered built-in or external MCP tool by name with arguments.
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<String, String> {
        if let Some((srv_id, tool_name)) = name.split_once("::") {
            let ext_cfg = {
                let ext = self
                    .external_tools
                    .read()
                    .map_err(|e| format!("Lock poisoned: {e}"))?;
                ext.iter()
                    .find(|(srv, def)| srv.id == srv_id && def.name == tool_name)
                    .map(|(srv, _)| srv.clone())
                    .or_else(|| {
                        Self::load_external_servers()
                            .into_iter()
                            .find(|s| s.id == srv_id)
                    })
            }
            .ok_or_else(|| format!("Unknown external MCP server/tool: '{name}'"))?;
            if !ext_cfg.enabled {
                return Err(format!("External MCP server '{}' is disabled", ext_cfg.name));
            }
            return Self::call_external_tool(&ext_cfg, tool_name, arguments);
        }

        let tool = {
            let tools = self.tools.read().map_err(|e| format!("Lock poisoned: {e}"))?;
            tools
                .get(name)
                .cloned()
                .ok_or_else(|| format!("Unknown tool: '{name}'"))?
        };
        (tool.handler)(arguments)
    }

    /// Returns a reference to the underlying `WorkspaceSearchIndex`.
    pub fn search_index(&self) -> &Arc<WorkspaceSearchIndex> {
        &self.search_index
    }

    /// Returns a reference to the underlying `AstEngine`.
    pub fn ast_engine(&self) -> &Arc<AstEngine> {
        &self.ast_engine
    }

    /// Convenience helper to index a document directly into the hub's search index.
    pub fn index_document(&self, path: &str, text: &str) -> Result<(), String> {
        self.search_index.index_document(path, text)
    }

    /// Handles incoming JSON-RPC 2.0 requests, executing methods and formatting responses.
    /// Notifications (requests without an 'id') MUST NOT receive any response and return `None`.
    pub fn handle_jsonrpc(&self, request_json: &str) -> Option<String> {
        let parsed: Value = match serde_json::from_str(request_json) {
            Ok(v) => v,
            Err(e) => {
                return Some(json_rpc_error(Value::Null, -32700, &format!("Parse error: {e}")));
            }
        };

        let obj = match parsed.as_object() {
            Some(o) => o,
            None => {
                return Some(json_rpc_error(
                    Value::Null,
                    -32600,
                    "Invalid Request: payload must be a JSON object",
                ));
            }
        };

        let has_id = obj.contains_key("id") && !obj["id"].is_null();
        let id = obj.get("id").cloned().unwrap_or(Value::Null);

        match obj.get("jsonrpc").and_then(|v| v.as_str()) {
            Some("2.0") => {}
            _ => {
                if !has_id {
                    return None;
                }
                return Some(json_rpc_error(id, -32600, "Invalid Request: 'jsonrpc' must be '2.0'"));
            }
        }

        let method = match obj.get("method").and_then(|v| v.as_str()) {
            Some(m) => m,
            None => {
                if !has_id {
                    return None;
                }
                return Some(json_rpc_error(id, -32600, "Invalid Request: missing or invalid 'method'"));
            }
        };

        // Notifications (missing/null 'id' or notifications/*) MUST NOT receive any response
        if !has_id || method == "notifications/initialized" || method.starts_with("notifications/") {
            return None;
        }

        match method {
            "tools/list" => {
                let tools = self.list_tools();
                Some(json_rpc_result(
                    id,
                    serde_json::json!({
                        "tools": tools
                    }),
                ))
            }
            "tools/call" => {
                let params = obj.get("params").cloned().unwrap_or(Value::Null);
                let tool_name = match params.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Some(json_rpc_error(id, -32602, "Invalid params: missing tool 'name'"));
                    }
                };
                let arguments = params
                    .get("arguments")
                    .or_else(|| params.get("args"))
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match self.call_tool(tool_name, arguments) {
                    Ok(output) => Some(json_rpc_result(
                        id,
                        serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": output
                                }
                            ],
                            "text": output,
                            "value": output,
                            "isError": false
                        }),
                    )),
                    Err(e) => {
                        if e.contains("Unknown tool") || e.contains("not found") {
                            Some(json_rpc_error(id, -32601, &e))
                        } else {
                            Some(json_rpc_result(
                                id,
                                serde_json::json!({
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": e
                                        }
                                    ],
                                    "text": e,
                                    "error": e,
                                    "isError": true
                                }),
                            ))
                        }
                    }
                }
            }
            "initialize" => {
                let params = obj.get("params").cloned().unwrap_or(Value::Null);
                let client_version = params.get("protocolVersion").and_then(|v| v.as_str());
                let protocol_version = match client_version {
                    Some(v) if !v.is_empty() => v,
                    _ => "2024-11-05",
                };
                Some(json_rpc_result(
                    id,
                    serde_json::json!({
                        "protocolVersion": protocol_version,
                        "capabilities": {
                            "tools": {
                                "listChanged": false
                            }
                        },
                        "serverInfo": {
                            "name": "kronos-mcp-hub",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                ))
            }
            "ping" => Some(json_rpc_result(id, serde_json::json!({}))),
            other => Some(json_rpc_error(id, -32601, &format!("Method not found: '{other}'"))),
        }
    }

    fn register_builtin_tools(&self) {
        // 1. read_file
        self.register_tool(
            McpToolDefinition {
                name: "read_file".to_string(),
                description: "Read file content from the local filesystem".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read"
                        }
                    },
                    "required": ["path"]
                }),
            },
            |arguments| {
                let path_str = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
                std::fs::read_to_string(path_str)
                    .map_err(|e| format!("Failed to read file '{path_str}': {e}"))
            },
        );

        // 2. write_file
        self.register_tool(
            McpToolDefinition {
                name: "write_file".to_string(),
                description: "Write content to a file in the local filesystem".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write into the file"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
            |arguments| {
                let path_str = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'path'".to_string())?;
                let content_str = arguments
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'content'".to_string())?;

                let path = Path::new(path_str);
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                }
                std::fs::write(path, content_str)
                    .map_err(|e| format!("Failed to write file '{path_str}': {e}"))?;
                Ok(format!(
                    "Successfully wrote {} bytes to {path_str}",
                    content_str.len()
                ))
            },
        );

        // 3. git_status
        self.register_tool(
            McpToolDefinition {
                name: "git_status".to_string(),
                description: "Inspect git repository status and branch summary via GitEngine"
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the git repository (default: current directory)"
                        }
                    },
                    "required": ["path"]
                }),
            },
            |arguments| {
                let path_str = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let summary = GitEngine::status_summary(path_str)
                    .map_err(|e| format!("Git status failed for '{path_str}': {e}"))?;
                serde_json::to_string_pretty(&summary)
                    .map_err(|e| format!("Serialization error: {e}"))
            },
        );

        // 4. search_code
        let search_index_clone = self.search_index.clone();
        self.register_tool(
            McpToolDefinition {
                name: "search_code".to_string(),
                description: "Search workspace documents using fast in-memory Tantivy index"
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Lexical search query to find in workspace documents"
                        }
                    },
                    "required": ["query"]
                }),
            },
            move |arguments| {
                let query_str = arguments
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'query'".to_string())?;
                let results = search_index_clone
                    .search(query_str)
                    .map_err(|e| format!("Search failed: {e}"))?;
                serde_json::to_string_pretty(&results)
                    .map_err(|e| format!("Serialization error: {e}"))
            },
        );

        // 5. ast_inspect
        let ast_engine_clone = self.ast_engine.clone();
        self.register_tool(
            McpToolDefinition {
                name: "ast_inspect".to_string(),
                description:
                    "Inspect Abstract Syntax Tree (AST) declarations via native Tree-Sitter engine"
                        .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the code file to inspect with Tree-Sitter"
                        },
                        "source": {
                            "type": "string",
                            "description": "Optional inline source code string to inspect directly"
                        },
                        "detailed": {
                            "type": "boolean",
                            "description": "Return full semantic symbols, imports, and syntax errors"
                        }
                    },
                    "required": ["path"]
                }),
            },
            move |arguments| {
                let path_str = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'path'".to_string())?;

                let (source, ext) =
                    if let Some(src) = arguments.get("source").and_then(|v| v.as_str()) {
                        let ext = Path::new(path_str)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("ts")
                            .to_string();
                        (src.to_string(), ext)
                    } else {
                        let content = std::fs::read_to_string(path_str)
                            .map_err(|e| format!("Failed to read file '{path_str}': {e}"))?;
                        let ext = Path::new(path_str)
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("ts")
                            .to_string();
                        (content, ext)
                    };

                let detailed = arguments.get("detailed").and_then(|v| v.as_bool()).unwrap_or(true);
                if detailed {
                    let result = ast_engine_clone
                        .analyze(&source, &ext)
                        .map_err(|e| format!("AST parse failed: {e}"))?;
                    serde_json::to_string_pretty(&result)
                        .map_err(|e| format!("Serialization error: {e}"))
                } else {
                    let summary = ast_engine_clone
                        .parse_syntax(&source, &ext)
                        .map_err(|e| format!("AST parse failed: {e}"))?;
                    serde_json::to_string_pretty(&summary)
                        .map_err(|e| format!("Serialization error: {e}"))
                }
            },
        );

        // 6. suggest_directory
        self.register_tool(
            McpToolDefinition {
                name: "suggest_directory".to_string(),
                description: "Suggest files and directories matching a query within a workspace root"
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "cwd": {
                            "type": "string",
                            "description": "Path to directory root to scan (default: .)"
                        },
                        "query": {
                            "type": "string",
                            "description": "Filter query string for directory and file suggestions"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of entries to return (default: 50)"
                        },
                        "include_files": {
                            "type": "boolean",
                            "description": "Whether to include matching files (default: true)"
                        },
                        "include_directories": {
                            "type": "boolean",
                            "description": "Whether to include matching directories (default: true)"
                        }
                    }
                }),
            },
            |arguments| {
                let cwd_str = arguments
                    .get("cwd")
                    .or_else(|| arguments.get("path"))
                    .or_else(|| arguments.get("root"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let query = arguments
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|l| l as usize)
                    .unwrap_or(50);
                let include_files = arguments
                    .get("include_files")
                    .or_else(|| arguments.get("includeFiles"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let include_directories = arguments
                    .get("include_directories")
                    .or_else(|| arguments.get("includeDirectories"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let root_path = Path::new(cwd_str);
                let (directories, entries) = crate::kernel::search::suggest_directory_entries(
                    root_path,
                    query,
                    include_files,
                    include_directories,
                    limit,
                );

                serde_json::to_string_pretty(&serde_json::json!({
                    "directories": directories,
                    "entries": entries,
                }))
                .map_err(|e| format!("Serialization error: {e}"))
            },
        );

        // 7. terminal_exec
        self.register_tool(
            McpToolDefinition {
                name: "terminal_exec".to_string(),
                description: "Execute a command in a child process with a 15-second safety timeout"
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Command or executable binary to run"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arguments to pass to the executable"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory to execute command in (optional)"
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "description": "Execution timeout in seconds (default: 15)"
                        }
                    },
                    "required": ["command"]
                }),
            },
            |arguments| {
                let cmd_str = arguments
                    .get("command")
                    .or_else(|| arguments.get("cmd"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'command'".to_string())?;

                let args_vec: Vec<String> = arguments
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let full_cmd = if args_vec.is_empty() {
                    cmd_str.to_string()
                } else {
                    format!("{cmd_str} {}", args_vec.join(" "))
                };
                let policy = crate::agent::guardrails::get_permission_policy();
                let risk = crate::agent::guardrails::evaluate_command_risk(&full_cmd);
                if policy.execution_mode != "yolo" && risk.is_destructive() {
                    return Err(format!(
                        "Blocked by Guarded Execution Policy ({}): {}",
                        risk.matched_pattern.unwrap_or_else(|| "destructive".to_string()),
                        risk.reason
                            .unwrap_or_else(|| "Destructive shell command denied in guarded mode".to_string())
                    ));
                }

                let timeout_secs = arguments
                    .get("timeout_secs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(15);
                let timeout = std::time::Duration::from_secs(timeout_secs);

                let mut cmd = std::process::Command::new(cmd_str);
                cmd.args(&args_vec);
                if let Some(cwd) = arguments.get("cwd").and_then(|v| v.as_str()) {
                    cmd.current_dir(cwd);
                }
                cmd.stdin(std::process::Stdio::null());
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::piped());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(0x08000000);
                }

                let mut child = cmd
                    .spawn()
                    .map_err(|e| format!("Failed to spawn command '{cmd_str}': {e}"))?;

                let stdout_handle = child.stdout.take();
                let stderr_handle = child.stderr.take();

                let out_thread = std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    if let Some(mut stream) = stdout_handle {
                        use std::io::Read;
                        let _ = stream.read_to_end(&mut buf);
                    }
                    buf
                });

                let err_thread = std::thread::spawn(move || {
                    let mut buf = Vec::new();
                    if let Some(mut stream) = stderr_handle {
                        use std::io::Read;
                        let _ = stream.read_to_end(&mut buf);
                    }
                    buf
                });

                let start = std::time::Instant::now();
                let status = loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break Ok(status),
                        Ok(None) => {
                            if start.elapsed() >= timeout {
                                let _ = child.kill();
                                break Err(format!(
                                    "Command execution timed out after {timeout_secs} seconds"
                                ));
                            }
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                        Err(e) => break Err(format!("Failed waiting for child process: {e}")),
                    }
                }?;

                let stdout_bytes = out_thread.join().unwrap_or_default();
                let stderr_bytes = err_thread.join().unwrap_or_default();
                let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
                let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

                if status.success() {
                    if stdout.is_empty() && !stderr.is_empty() {
                        Ok(stderr)
                    } else {
                        Ok(stdout)
                    }
                } else {
                    Err(format!(
                        "Command failed with status {}:\nStdout: {}\nStderr: {}",
                        status, stdout, stderr
                    ))
                }
            },
        );

        // 8. web_search
        self.register_tool(
            McpToolDefinition {
                name: "web_search".to_string(),
                description: "Search the live web using zero-API-key DuckDuckGo engine"
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum number of results (default: 6)"
                        }
                    },
                    "required": ["query"]
                }),
            },
            |arguments| {
                let query = arguments
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'query'".to_string())?
                    .to_string();
                let max_results = arguments
                    .get("max_results")
                    .or_else(|| arguments.get("maxResults"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(6) as usize;
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| format!("Failed creating runtime: {e}"))?;
                    let res = rt.block_on(crate::agent::web_search::search_web(
                        &query,
                        max_results,
                    ))?;
                    serde_json::to_string_pretty(&res)
                        .map_err(|e| format!("Serialization error: {e}"))
                })
                .join()
                .map_err(|_| "web_search thread panicked".to_string())?
            },
        );

        // 9. web_fetch
        self.register_tool(
            McpToolDefinition {
                name: "web_fetch".to_string(),
                description: "Fetch a webpage URL and extract clean Markdown text".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "HTTP/HTTPS URL to scrape"
                        }
                    },
                    "required": ["url"]
                }),
            },
            |arguments| {
                let url = arguments
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing required parameter 'url'".to_string())?
                    .to_string();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| format!("Failed creating runtime: {e}"))?;
                    let res = rt.block_on(crate::agent::web_search::fetch_url(&url, Some(15_000)))?;
                    serde_json::to_string_pretty(&res)
                        .map_err(|e| format!("Serialization error: {e}"))
                })
                .join()
                .map_err(|_| "web_fetch thread panicked".to_string())?
            },
        );
    }

    fn servers_config_path() -> Option<std::path::PathBuf> {
        let base = dirs::home_dir()?.join(".paseo");
        Some(base.join("mcp_servers.json"))
    }

    pub fn load_external_servers() -> Vec<ExternalMcpServerConfig> {
        let Some(path) = Self::servers_config_path() else {
            return Vec::new();
        };
        if !path.is_file() {
            return Vec::new();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save_external_servers(servers: &[ExternalMcpServerConfig]) -> Result<(), String> {
        let path = Self::servers_config_path()
            .ok_or_else(|| "Unable to locate ~/.paseo config directory".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed creating MCP config directory: {e}"))?;
        }
        let body = serde_json::to_string_pretty(servers)
            .map_err(|e| format!("Failed serializing MCP servers: {e}"))?;
        std::fs::write(&path, body).map_err(|e| format!("Failed writing mcp_servers.json: {e}"))
    }

    fn send_stdio_rpc(
        cfg: &ExternalMcpServerConfig,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        use std::io::{BufRead, BufReader, Write};
        if cfg.command.trim().is_empty() {
            return Err("Missing command for stdio MCP server".to_string());
        }
        let mut cmd = std::process::Command::new(&cfg.command);
        cmd.args(&cfg.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed spawning MCP server '{}': {e}", cfg.command))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "MCP child missing stdin pipe".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "MCP child missing stdout pipe".to_string())?;

        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "kronos", "version": env!("CARGO_PKG_VERSION") }
            }
        });
        let init_notif = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let target_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": method,
            "params": params
        });

        writeln!(stdin, "{init_req}").and_then(|_| writeln!(stdin, "{init_notif}"))
            .and_then(|_| writeln!(stdin, "{target_req}"))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("Failed writing to MCP stdin: {e}"))?;
        drop(stdin);

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                    if val.get("id").and_then(|i| i.as_i64()) == Some(2) {
                        let _ = tx.send(Ok(val));
                        return;
                    }
                }
            }
            let _ = tx.send(Err("MCP server exited before responding".to_string()));
        });

        let response = rx
            .recv_timeout(std::time::Duration::from_secs(8))
            .map_err(|_| "Timed out waiting for stdio MCP response".to_string())?;
        let _ = child.kill();
        let _ = child.wait();
        let val = response?;
        if let Some(err) = val.get("error") {
            return Err(err.to_string());
        }
        Ok(val.get("result").cloned().unwrap_or(Value::Null))
    }

    fn send_http_rpc(
        cfg: &ExternalMcpServerConfig,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let url = cfg
            .url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .ok_or_else(|| "Missing endpoint URL for HTTP/SSE MCP server".to_string())?;
        let method_owned = method.to_string();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed creating HTTP runtime: {e}"))?;
            rt.block_on(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(8))
                    .build()
                    .map_err(|e| format!("HTTP client error: {e}"))?;
                let payload = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": method_owned,
                    "params": params
                });
                let resp = client
                    .post(&url)
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| format!("MCP HTTP request failed: {e}"))?;
                if !resp.status().is_success() {
                    return Err(format!("MCP server returned HTTP {}", resp.status()));
                }
                let val: Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("Invalid MCP JSON response: {e}"))?;
                if let Some(err) = val.get("error") {
                    return Err(err.to_string());
                }
                Ok(val.get("result").cloned().unwrap_or(Value::Null))
            })
        })
        .join()
        .map_err(|_| "MCP HTTP thread panicked".to_string())?
    }

    fn rpc_external(
        cfg: &ExternalMcpServerConfig,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        if cfg.transport.eq_ignore_ascii_case("stdio") {
            Self::send_stdio_rpc(cfg, method, params)
        } else {
            Self::send_http_rpc(cfg, method, params)
        }
    }

    pub fn probe_and_sync_external_server(
        &self,
        cfg: &ExternalMcpServerConfig,
    ) -> ExternalMcpServerStatus {
        if !cfg.enabled {
            if let Ok(mut ext) = self.external_tools.write() {
                ext.retain(|(s, _)| s.id != cfg.id);
            }
            return ExternalMcpServerStatus {
                id: cfg.id.clone(),
                name: cfg.name.clone(),
                transport: cfg.transport.clone(),
                command: cfg.command.clone(),
                args: cfg.args.clone(),
                url: cfg.url.clone(),
                enabled: cfg.enabled,
                status: "disabled".to_string(),
                tool_count: 0,
                last_error: None,
            };
        }

        match Self::rpc_external(cfg, "tools/list", serde_json::json!({})) {
            Ok(result) => {
                let parsed: Vec<McpToolDefinition> = result
                    .get("tools")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let count = parsed.len();
                if let Ok(mut ext) = self.external_tools.write() {
                    ext.retain(|(s, _)| s.id != cfg.id);
                    for def in parsed {
                        ext.push((cfg.clone(), def));
                    }
                }
                ExternalMcpServerStatus {
                    id: cfg.id.clone(),
                    name: cfg.name.clone(),
                    transport: cfg.transport.clone(),
                    command: cfg.command.clone(),
                    args: cfg.args.clone(),
                    url: cfg.url.clone(),
                    enabled: cfg.enabled,
                    status: "connected".to_string(),
                    tool_count: count,
                    last_error: None,
                }
            }
            Err(err) => ExternalMcpServerStatus {
                id: cfg.id.clone(),
                name: cfg.name.clone(),
                transport: cfg.transport.clone(),
                command: cfg.command.clone(),
                args: cfg.args.clone(),
                url: cfg.url.clone(),
                enabled: cfg.enabled,
                status: "error".to_string(),
                tool_count: 0,
                last_error: Some(err),
            },
        }
    }

    pub fn call_external_tool(
        cfg: &ExternalMcpServerConfig,
        tool_name: &str,
        arguments: Value,
    ) -> Result<String, String> {
        let result = Self::rpc_external(
            cfg,
            "tools/call",
            serde_json::json!({
                "name": tool_name,
                "arguments": arguments
            }),
        )?;
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            let texts: Vec<String> = content
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(str::to_string))
                .collect();
            if !texts.is_empty() {
                return Ok(texts.join("\n"));
            }
        }
        serde_json::to_string_pretty(&result).map_err(|e| format!("Serialization error: {e}"))
    }
}

fn json_rpc_result(id: Value, result: Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string()
}

fn json_rpc_error(id: Value, code: i64, message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
    .to_string()
}
