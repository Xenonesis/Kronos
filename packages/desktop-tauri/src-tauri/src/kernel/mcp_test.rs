#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::kernel::mcp::{McpHub, McpToolDefinition};

    #[test]
    fn test_native_mcp_tool_execution() {
        let hub = McpHub::new();

        // 1. read_file tool execution on Cargo.toml
        let res = hub
            .call_tool("read_file", json!({ "path": "Cargo.toml" }))
            .expect("Failed to execute read_file on Cargo.toml");
        assert!(
            res.contains("paseo-desktop") || res.contains("Kronos"),
            "Expected Cargo.toml to contain 'paseo-desktop' or 'Kronos', got: {res}"
        );

        // 2. write_file tool execution
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join(format!("kronos_mcp_test_{}.txt", std::process::id()));
        let test_file_str = test_file.to_str().unwrap();

        let write_res = hub
            .call_tool(
                "write_file",
                json!({
                    "path": test_file_str,
                    "content": "Kronos Hyper-Native Kernel MCP Hub Verified"
                }),
            )
            .expect("Failed to execute write_file");
        assert!(write_res.contains("Successfully wrote"));

        // Verify write by reading it back with read_file
        let read_back = hub
            .call_tool("read_file", json!({ "path": test_file_str }))
            .expect("Failed to read back written file");
        assert_eq!(read_back, "Kronos Hyper-Native Kernel MCP Hub Verified");
        let _ = std::fs::remove_file(&test_file);

        // 3. git_status tool execution
        let git_res = hub
            .call_tool("git_status", json!({ "path": "." }))
            .expect("Failed to execute git_status");
        let git_json: serde_json::Value =
            serde_json::from_str(&git_res).expect("git_status should produce valid JSON");
        assert!(
            git_json.get("is_dirty").is_some(),
            "Expected git_status output to contain is_dirty"
        );

        // 4. search_code tool execution
        hub.index_document(
            "kernel/mcp_doc.ts",
            "export class McpHubKernelDispatcher { dispatch(): void {} }",
        )
        .expect("Failed to index test document");
        let search_res = hub
            .call_tool("search_code", json!({ "query": "McpHubKernelDispatcher" }))
            .expect("Failed to execute search_code");
        let search_json: serde_json::Value =
            serde_json::from_str(&search_res).expect("search_code should return valid JSON");
        assert!(search_json.is_array());
        assert!(
            !search_json.as_array().unwrap().is_empty(),
            "Expected at least one search result"
        );

        // 5. ast_inspect tool execution
        let ast_res = hub
            .call_tool(
                "ast_inspect",
                json!({
                    "path": "test_ast.ts",
                    "source": "export function nativeMcpDispatch(name: string): boolean { return true; }"
                }),
            )
            .expect("Failed to execute ast_inspect");
        let ast_json: serde_json::Value =
            serde_json::from_str(&ast_res).expect("ast_inspect should return valid JSON");
        assert_eq!(ast_json["root_kind"], "program");
        let decls = ast_json["declarations"].as_array().expect("declarations array");
        assert!(
            decls.iter().any(|d| d.as_str().map_or(false, |s| s.contains("nativeMcpDispatch"))),
            "Expected declaration list to include nativeMcpDispatch"
        );

        // 6. Unknown tool execution returns Err
        let unknown_res = hub.call_tool("non_existent_tool_xyz", json!({}));
        assert!(unknown_res.is_err());
        assert!(unknown_res.unwrap_err().contains("Unknown tool"));
    }

    #[test]
    fn test_native_mcp_list_tools() {
        let hub = McpHub::new();
        let tools = hub.list_tools();

        assert_eq!(tools.len(), 7, "Expected 7 built-in MCP tools");

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"write_file"));
        assert!(tool_names.contains(&"git_status"));
        assert!(tool_names.contains(&"search_code"));
        assert!(tool_names.contains(&"ast_inspect"));
        assert!(tool_names.contains(&"suggest_directory"));
        assert!(tool_names.contains(&"terminal_exec"));

        for tool in &tools {
            assert!(!tool.name.is_empty(), "Tool name must not be empty");
            assert!(
                !tool.description.is_empty(),
                "Tool description must not be empty for {}",
                tool.name
            );
            assert_eq!(
                tool.input_schema["type"], "object",
                "Tool inputSchema must have type object"
            );
            assert!(
                tool.input_schema["properties"].is_object(),
                "Tool inputSchema must define properties object"
            );
        }

        // Test custom tool registration into registry
        hub.register_tool(
            McpToolDefinition {
                name: "custom_echo".to_string(),
                description: "Custom echo tool for testing".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }),
            },
            |args| {
                let msg = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                Ok(format!("Echo: {msg}"))
            },
        );

        let updated_tools = hub.list_tools();
        assert_eq!(updated_tools.len(), 8);
        assert_eq!(updated_tools[7].name, "custom_echo");

        let custom_res = hub
            .call_tool("custom_echo", json!({ "message": "hello world" }))
            .expect("Custom tool call should succeed");
        assert_eq!(custom_res, "Echo: hello world");
    }

    #[test]
    fn test_native_mcp_jsonrpc_dispatch() {
        let hub = McpHub::new();

        // 1. tools/list roundtrip
        let list_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        })
        .to_string();

        let list_resp_str = hub.handle_jsonrpc(&list_req).expect("Response expected for tools/list");
        let list_resp: serde_json::Value =
            serde_json::from_str(&list_resp_str).expect("Valid JSON response from tools/list");

        assert_eq!(list_resp["jsonrpc"], "2.0");
        assert_eq!(list_resp["id"], 1);
        let tools_array = list_resp["result"]["tools"]
            .as_array()
            .expect("tools array");
        assert_eq!(tools_array.len(), 7);

        // Verify schema serialized with both inputSchema and input_schema
        assert!(tools_array[0]["inputSchema"].is_object());
        assert!(tools_array[0]["input_schema"].is_object());

        // 2. tools/call roundtrip for read_file
        let call_req = json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "tools/call",
            "params": {
                "name": "read_file",
                "arguments": {
                    "path": "Cargo.toml"
                }
            }
        })
        .to_string();

        let call_resp_str = hub.handle_jsonrpc(&call_req).expect("Response expected for tools/call");
        let call_resp: serde_json::Value =
            serde_json::from_str(&call_resp_str).expect("Valid JSON response from tools/call");

        assert_eq!(call_resp["jsonrpc"], "2.0");
        assert_eq!(call_resp["id"], 100);
        assert_eq!(call_resp["result"]["isError"], false);
        let content_arr = call_resp["result"]["content"]
            .as_array()
            .expect("content array");
        assert_eq!(content_arr[0]["type"], "text");
        let content_text = content_arr[0]["text"].as_str().expect("text string");
        assert!(
            content_text.contains("paseo-desktop") || content_text.contains("Kronos"),
            "Expected read_file to read Cargo.toml content"
        );

        // 3. tools/call roundtrip for write_file
        let temp_dir = std::env::temp_dir();
        let rpc_file = temp_dir.join(format!("kronos_rpc_test_{}.txt", std::process::id()));
        let rpc_file_str = rpc_file.to_str().unwrap();

        let write_rpc_req = json!({
            "jsonrpc": "2.0",
            "id": 101,
            "method": "tools/call",
            "params": {
                "name": "write_file",
                "arguments": {
                    "path": rpc_file_str,
                    "content": "JSON-RPC Write Verified"
                }
            }
        })
        .to_string();

        let write_rpc_resp_str = hub.handle_jsonrpc(&write_rpc_req).expect("Response expected for tools/call");
        let write_rpc_resp: serde_json::Value =
            serde_json::from_str(&write_rpc_resp_str).expect("Valid JSON response");
        assert_eq!(write_rpc_resp["id"], 101);
        assert_eq!(write_rpc_resp["result"]["isError"], false);

        let read_back = std::fs::read_to_string(&rpc_file).expect("File written on disk");
        assert_eq!(read_back, "JSON-RPC Write Verified");
        let _ = std::fs::remove_file(&rpc_file);

        // 4. JSON-RPC Error Handling Tests

        // Parse Error (-32700)
        let parse_err_resp = hub.handle_jsonrpc("invalid json string {{{").expect("Error response expected for parse error");
        let parse_err_json: serde_json::Value = serde_json::from_str(&parse_err_resp).unwrap();
        assert_eq!(parse_err_json["jsonrpc"], "2.0");
        assert_eq!(parse_err_json["error"]["code"], -32700);

        // Invalid Request (-32600): bad jsonrpc version
        let bad_version_req = json!({
            "jsonrpc": "1.0",
            "id": 2,
            "method": "tools/list"
        })
        .to_string();
        let bad_version_resp: serde_json::Value =
            serde_json::from_str(&hub.handle_jsonrpc(&bad_version_req).expect("Response expected")).unwrap();
        assert_eq!(bad_version_resp["error"]["code"], -32600);

        // Method Not Found (-32601)
        let unknown_method_req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "unsupported_method_call"
        })
        .to_string();
        let unknown_method_resp: serde_json::Value =
            serde_json::from_str(&hub.handle_jsonrpc(&unknown_method_req).expect("Response expected")).unwrap();
        assert_eq!(unknown_method_resp["error"]["code"], -32601);

        // Invalid Params (-32602): missing tool name in tools/call
        let missing_name_req = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {}
        })
        .to_string();
        let missing_name_resp: serde_json::Value =
            serde_json::from_str(&hub.handle_jsonrpc(&missing_name_req).expect("Response expected")).unwrap();
        assert_eq!(missing_name_resp["error"]["code"], -32602);

        // Unknown tool name in tools/call (-32601)
        let unknown_tool_req = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "non_existent_tool_abc"
            }
        })
        .to_string();
        let unknown_tool_resp: serde_json::Value =
            serde_json::from_str(&hub.handle_jsonrpc(&unknown_tool_req).expect("Response expected")).unwrap();
        assert_eq!(unknown_tool_resp["error"]["code"], -32601);

        // Initialize and Ping support
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "initialize"
        })
        .to_string();
        let init_resp: serde_json::Value =
            serde_json::from_str(&hub.handle_jsonrpc(&init_req).expect("Response expected")).unwrap();
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "kronos-mcp-hub");
        assert_eq!(init_resp["result"]["serverInfo"]["version"], "0.15.0-beta.1");

        let ping_req = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "ping"
        })
        .to_string();
        let ping_resp: serde_json::Value =
            serde_json::from_str(&hub.handle_jsonrpc(&ping_req).expect("Response expected")).unwrap();
        assert!(ping_resp["result"].is_object());
    }

    #[test]
    fn test_native_mcp_notification_silencing() {
        let hub = McpHub::new();

        // Notifications (no 'id' field) MUST NOT yield a response
        let init_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }).to_string();
        assert_eq!(hub.handle_jsonrpc(&init_notification), None);

        let cancel_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": "req-1" }
        }).to_string();
        assert_eq!(hub.handle_jsonrpc(&cancel_notification), None);
    }

    #[test]
    fn test_native_mcp_suggest_directory_tool() {
        let hub = McpHub::new();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "req-dir-1",
            "method": "tools/call",
            "params": {
                "name": "suggest_directory",
                "arguments": {
                    "cwd": ".",
                    "query": "src",
                    "limit": 10
                }
            }
        }).to_string();

        let res_str = hub.handle_jsonrpc(&req).expect("Response expected for request with id");
        let res: serde_json::Value = serde_json::from_str(&res_str).unwrap();
        assert_eq!(res["id"], "req-dir-1");
        assert_eq!(res["result"]["isError"], false);
    }

    #[test]
    fn test_native_mcp_terminal_exec_tool() {
        let hub = McpHub::new();
        let (command, args) = if cfg!(windows) {
            ("cmd.exe", vec!["/C", "echo KronosKernelReady"])
        } else {
            ("sh", vec!["-c", "echo KronosKernelReady"])
        };

        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "req-exec-1",
            "method": "tools/call",
            "params": {
                "name": "terminal_exec",
                "arguments": {
                    "command": command,
                    "args": args
                }
            }
        }).to_string();

        let res_str = hub.handle_jsonrpc(&req).expect("Response expected for request with id");
        let res: serde_json::Value = serde_json::from_str(&res_str).unwrap();
        assert_eq!(res["result"]["isError"], false);
        let output = res["result"]["text"].as_str().unwrap();
        assert!(output.contains("KronosKernelReady"));
    }
}
