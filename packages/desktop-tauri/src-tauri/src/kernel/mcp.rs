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
    pub fn handle_jsonrpc(&self, request_json: &str) -> String {
        let parsed: Value = match serde_json::from_str(request_json) {
            Ok(v) => v,
            Err(e) => {
                return json_rpc_error(Value::Null, -32700, &format!("Parse error: {e}"));
            }
        };

        let obj = match parsed.as_object() {
            Some(o) => o,
            None => {
                return json_rpc_error(
                    Value::Null,
                    -32600,
                    "Invalid Request: payload must be a JSON object",
                );
            }
        };

        let id = obj.get("id").cloned().unwrap_or(Value::Null);

        match obj.get("jsonrpc").and_then(|v| v.as_str()) {
            Some("2.0") => {}
            _ => {
                return json_rpc_error(id, -32600, "Invalid Request: 'jsonrpc' must be '2.0'");
            }
        }

        let method = match obj.get("method").and_then(|v| v.as_str()) {
            Some(m) => m,
            None => {
                return json_rpc_error(id, -32600, "Invalid Request: missing or invalid 'method'");
            }
        };

        match method {
            "tools/list" => {
                let tools = self.list_tools();
                json_rpc_result(
                    id,
                    serde_json::json!({
                        "tools": tools
                    }),
                )
            }
            "tools/call" => {
                let params = obj.get("params").cloned().unwrap_or(Value::Null);
                let tool_name = match params.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return json_rpc_error(id, -32602, "Invalid params: missing tool 'name'");
                    }
                };
                let arguments = params
                    .get("arguments")
                    .or_else(|| params.get("args"))
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match self.call_tool(tool_name, arguments) {
                    Ok(output) => json_rpc_result(
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
                    ),
                    Err(e) => {
                        if e.contains("Unknown tool") || e.contains("not found") {
                            json_rpc_error(id, -32601, &e)
                        } else {
                            json_rpc_result(
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
                            )
                        }
                    }
                }
            }
            "initialize" => json_rpc_result(
                id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": "kronos-mcp-hub",
                        "version": "0.14.0"
                    }
                }),
            ),
            "ping" => json_rpc_result(id, serde_json::json!({})),
            other => json_rpc_error(id, -32601, &format!("Method not found: '{other}'")),
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
