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

    /// Returns definitions of all registered tools in deterministic order.
    pub fn list_tools(&self) -> Vec<McpToolDefinition> {
        let tools = self.tools.read().unwrap();
        let order = self.tool_order.read().unwrap();
        order
            .iter()
            .filter_map(|name| tools.get(name).map(|t| t.definition.clone()))
            .collect()
    }

    /// Dispatches a call to a registered tool by name with arguments.
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<String, String> {
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

                let summary = ast_engine_clone
                    .parse_syntax(&source, &ext)
                    .map_err(|e| format!("AST parse failed: {e}"))?;
                serde_json::to_string_pretty(&summary)
                    .map_err(|e| format!("Serialization error: {e}"))
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
