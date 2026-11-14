#[cfg(test)]
mod tests {
    use crate::kernel::protocol::RpcEnvelope;
    use crate::kernel::server::EmbeddedDaemon;
    use crate::kernel::server::{dispatch_envelope, dispatch_session_message};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::connect_async;

    async fn authenticate_socket(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        daemon: &EmbeddedDaemon,
    ) {
        let hello = serde_json::json!({
            "type":"hello", "protocolVersion":1, "clientId":"native-integration", "clientType":"cli",
            "auth":{"kind":"localCredential","token":daemon.access.local_credential}
        });
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                hello.to_string(),
            ))
            .await
            .unwrap();
        let response = tokio::time::timeout(std::time::Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let info: serde_json::Value = serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(info["message"]["payload"]["status"], "server_info");
    }

    #[tokio::test]
    async fn test_embedded_daemon_mcp_envelope_dispatch() {
        let envelope = RpcEnvelope {
            id: "mcp-env-1".to_string(),
            msg_type: "mcp/request".to_string(),
            payload: serde_json::json!({
                "request": serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "inner-1",
                    "method": "tools/list"
                }).to_string()
            }),
        };

        let response = dispatch_envelope(envelope).await;
        assert!(response.success);
        assert_eq!(response.msg_type, "mcp/response");
        let data = response.data.expect("Response data expected");
        let inner_json: &str = data["response"].as_str().expect("inner response string");
        let inner_val: serde_json::Value = serde_json::from_str(inner_json).unwrap();
        assert_eq!(inner_val["id"], "inner-1");
        assert!(inner_val["result"]["tools"].as_array().unwrap().len() >= 5);
    }

    #[tokio::test]
    async fn test_embedded_daemon_mcp_envelope_tool_call() {
        let envelope = RpcEnvelope {
            id: "mcp-call-env".to_string(),
            msg_type: "mcp/jsonrpc".to_string(),
            payload: serde_json::json!({
                "request": {
                    "jsonrpc": "2.0",
                    "id": "inner-call-1",
                    "method": "tools/call",
                    "params": {
                        "name": "git_status",
                        "arguments": {
                            "path": "."
                        }
                    }
                }
            }),
        };

        let response = dispatch_envelope(envelope).await;
        assert!(response.success);
        assert_eq!(response.msg_type, "mcp/response");
        let data = response.data.expect("Response data expected");
        let inner_json: &str = data["response"].as_str().expect("inner response string");
        let inner_val: serde_json::Value = serde_json::from_str(inner_json).unwrap();
        assert_eq!(inner_val["id"], "inner-call-1");
        assert!(inner_val["result"]["content"].is_array());
    }

    #[tokio::test]
    async fn test_embedded_daemon_mcp_envelope_notification() {
        let envelope = RpcEnvelope {
            id: "mcp-notif-env".to_string(),
            msg_type: "mcp/request".to_string(),
            payload: serde_json::json!({
                "request": {
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized"
                }
            }),
        };

        let response = dispatch_envelope(envelope).await;
        assert!(response.success);
        assert_eq!(response.msg_type, "mcp/response");
        let data = response.data.expect("Response data expected");
        assert!(data["response"].is_null());
        assert_eq!(data["notification"], true);
    }

    #[tokio::test]
    async fn test_embedded_daemon_mcp_session_message_dispatch() {
        let session_msg = serde_json::json!({
            "type": "mcp/request",
            "requestId": "sess-req-1",
            "payload": {
                "request": serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "sess-inner-1",
                    "method": "tools/list"
                }).to_string()
            }
        });

        let resp = dispatch_session_message(&session_msg).await;
        assert_eq!(resp["type"], "mcp/response");
        assert_eq!(resp["requestId"], "sess-req-1");
        assert_eq!(resp["success"], true);
        let inner_str = resp["payload"]["response"]
            .as_str()
            .expect("inner response string");
        let inner_val: serde_json::Value = serde_json::from_str(inner_str).unwrap();
        assert_eq!(inner_val["id"], "sess-inner-1");
        assert!(inner_val["result"]["tools"].as_array().unwrap().len() >= 5);
    }

    #[tokio::test]
    async fn test_embedded_daemon_mcp_session_message_tool_call_and_notification() {
        let call_msg = serde_json::json!({
            "type": "mcp_request",
            "requestId": "sess-call-1",
            "request": {
                "jsonrpc": "2.0",
                "id": "sess-call-inner",
                "method": "tools/call",
                "params": {
                    "name": "git_status",
                    "arguments": { "path": "." }
                }
            }
        });

        let resp = dispatch_session_message(&call_msg).await;
        assert_eq!(resp["type"], "mcp/response");
        assert_eq!(resp["requestId"], "sess-call-1");
        let inner_str = resp["payload"]["response"].as_str().expect("response str");
        let inner_val: serde_json::Value = serde_json::from_str(inner_str).unwrap();
        assert_eq!(inner_val["id"], "sess-call-inner");
        assert!(inner_val["result"]["content"].is_array());

        let notif_msg = serde_json::json!({
            "type": "mcp/jsonrpc",
            "requestId": "sess-notif-1",
            "payload": {
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }
        });

        let notif_resp = dispatch_session_message(&notif_msg).await;
        assert_eq!(notif_resp["type"], "mcp/response");
        assert_eq!(notif_resp["requestId"], "sess-notif-1");
        assert!(notif_resp["payload"]["response"].is_null());
        assert_eq!(notif_resp["payload"]["notification"], true);
    }

    #[tokio::test]
    async fn test_embedded_daemon_mcp_websocket_flow() {
        let daemon = EmbeddedDaemon::start_test(0)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", daemon.port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        let msg = serde_json::json!({
            "id": "ws-mcp-env",
            "type": "mcp/request",
            "payload": {
                "request": {
                    "jsonrpc": "2.0",
                    "id": "ws-inner-req",
                    "method": "tools/list"
                }
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                msg.to_string(),
            ))
            .await
            .unwrap();

        let response = ws_stream.next().await.unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(parsed["id"], "ws-mcp-env");
        assert_eq!(parsed["type"], "mcp/response");
        assert_eq!(parsed["success"], true);
        let inner_str = parsed["data"]["response"].as_str().unwrap();
        let inner_val: serde_json::Value = serde_json::from_str(inner_str).unwrap();
        assert_eq!(inner_val["id"], "ws-inner-req");
        assert!(inner_val["result"]["tools"].as_array().unwrap().len() >= 5);

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_websocket_handshake() {
        let daemon = EmbeddedDaemon::start_test(6799)
            .await
            .expect("Failed to start daemon");
        let url = "ws://127.0.0.1:6799";
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        let msg = serde_json::json!({
            "id": "ping-1",
            "type": "ping",
            "payload": {}
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                msg.to_string(),
            ))
            .await
            .unwrap();

        let response = ws_stream.next().await.unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(parsed["id"], "ping-1");
        assert_eq!(parsed["type"], "pong");
        assert_eq!(parsed["success"], true);
        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_canonical_hello_and_ping() {
        let port = 6795;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

        // 1. Send Canonical Hello Handshake
        let hello = serde_json::json!({
            "type": "hello",
            "clientId": "cid-integration-test",
            "clientType": "cli",
            "protocolVersion": 1,
            "auth": {"kind":"localCredential","token":daemon.access.local_credential}
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                hello.to_string(),
            ))
            .await
            .unwrap();

        let mut parsed: serde_json::Value = serde_json::Value::Null;
        for _ in 0..10 {
            if let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(1000), ws_stream.next()).await
            {
                if let Ok(text) = msg.to_text() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                        if val
                            .get("message")
                            .and_then(|m| m.get("payload"))
                            .and_then(|p| p.get("status"))
                            .and_then(|v| v.as_str())
                            == Some("server_info")
                        {
                            parsed = val;
                            break;
                        }
                    }
                }
            }
        }
        assert_eq!(parsed["type"], "session");
        assert_eq!(parsed["message"]["type"], "status");
        assert_eq!(parsed["message"]["payload"]["status"], "server_info");
        assert_eq!(parsed["message"]["payload"]["protocolVersion"], 1);
        assert!(!parsed["message"]["payload"]["serverId"]
            .as_str()
            .unwrap()
            .is_empty());

        // 2. Send Simple Ping
        let ping = serde_json::json!({ "type": "ping" });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                ping.to_string(),
            ))
            .await
            .unwrap();
        let mut parsed_pong: serde_json::Value = serde_json::Value::Null;
        for _ in 0..10 {
            if let Ok(Some(Ok(msg))) =
                tokio::time::timeout(std::time::Duration::from_millis(500), ws_stream.next()).await
            {
                if let Ok(text) = msg.to_text() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                        if val.get("type").and_then(|v| v.as_str()) == Some("pong") {
                            parsed_pong = val;
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }
        assert_eq!(parsed_pong["type"], "pong");

        // 3. Send Session Telemetry Query
        let tele_query = serde_json::json!({
            "type": "session",
            "message": {
                "type": "kernel/telemetry",
                "requestId": "req-tele-test"
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                tele_query.to_string(),
            ))
            .await
            .unwrap();

        let parsed_tele = wait_for_rpc_response(&mut ws_stream, "req-tele-test").await;
        assert!(
            parsed_tele["payload"]["uptime_secs"].is_number()
                || parsed_tele["message"]["payload"]["uptime_secs"].is_number()
                || parsed_tele["data"]["uptime_secs"].is_number()
        );

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_unknown_method_dispatch() {
        let daemon = EmbeddedDaemon::start_test(0)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", daemon.port);
        let (mut ws_stream, _) = connect_async(&url).await.expect("Failed to connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        let msg = serde_json::json!({
            "id": "req-unknown-1",
            "type": "workspace.unimplemented",
            "payload": {}
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                msg.to_string(),
            ))
            .await
            .unwrap();

        let parsed = wait_for_rpc_response(&mut ws_stream, "req-unknown-1").await;
        assert_eq!(parsed["id"], "req-unknown-1");
        assert_eq!(parsed["success"], false);
        assert!(parsed["error"].is_string());
        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_directory_suggestions_dispatch() {
        let port = 6794;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        let dir_query = serde_json::json!({
            "type": "session",
            "message": {
                "type": "directory_suggestions_request",
                "cwd": ".",
                "query": "search.rs",
                "includeFiles": true,
                "includeDirectories": false,
                "limit": 10,
                "requestId": "req-dir-sugg-test"
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                dir_query.to_string(),
            ))
            .await
            .unwrap();

        let mut parsed: serde_json::Value = serde_json::Value::Null;
        while let Some(msg) = ws_stream.next().await {
            let val: serde_json::Value =
                serde_json::from_str(msg.unwrap().to_text().unwrap()).unwrap();
            if val["message"]["type"] == "directory_suggestions_response" {
                parsed = val;
                break;
            }
        }
        assert_eq!(parsed["type"], "session");
        assert_eq!(parsed["message"]["type"], "directory_suggestions_response");
        assert_eq!(
            parsed["message"]["payload"]["requestId"],
            "req-dir-sugg-test"
        );
        assert!(parsed["message"]["payload"]["error"].is_null());

        let entries = parsed["message"]["payload"]["entries"]
            .as_array()
            .expect("entries array");
        assert!(
            !entries.is_empty(),
            "Should return matching search.rs entry"
        );
        let matched = entries.iter().any(|e| {
            e["path"]
                .as_str()
                .map(|p| p.ends_with("search.rs"))
                .unwrap_or(false)
        });
        assert!(
            matched,
            "Expected search.rs in returned suggestions payload"
        );

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_directory_suggestions_envelope_dispatch() {
        let envelope = crate::kernel::protocol::RpcEnvelope {
            id: "envelope-sugg-1".to_string(),
            msg_type: "directory_suggestions_request".to_string(),
            payload: serde_json::json!({
                "cwd": ".",
                "query": "search.rs",
                "includeFiles": true,
                "includeDirectories": false,
                "limit": 10
            }),
        };
        let resp = crate::kernel::server::dispatch_envelope(envelope).await;
        assert_eq!(resp.id, "envelope-sugg-1");
        assert_eq!(resp.msg_type, "directory_suggestions_response");
        assert!(resp.success);
        let data = resp.data.expect("response data");
        assert_eq!(data["requestId"], "envelope-sugg-1");
        assert!(data["error"].is_null());
        let entries = data["entries"].as_array().expect("entries array");
        assert!(!entries.is_empty());
        let matched = entries.iter().any(|e| {
            e["path"]
                .as_str()
                .map(|p| p.ends_with("search.rs"))
                .unwrap_or(false)
        });
        assert!(
            matched,
            "Expected search.rs in returned suggestions payload"
        );
    }

    #[tokio::test]
    async fn test_terminal_wire_protocol_full_roundtrip() {
        let create_req = serde_json::json!({
            "type": "create_terminal_request",
            "cwd": ".",
            "workspaceId": "ws-test-term-1",
            "name": "Terminal Test",
            "requestId": "req-term-create-1"
        });
        let create_resp = crate::kernel::server::dispatch_session_message(&create_req).await;
        assert_eq!(create_resp["type"], "create_terminal_response");
        assert_eq!(create_resp["requestId"], "req-term-create-1");
        assert!(create_resp["payload"]["error"].is_null());
        let terminal = &create_resp["payload"]["terminal"];
        let term_id = terminal["id"].as_str().expect("terminal id");
        assert!(term_id.starts_with("term_"));
        assert_eq!(terminal["name"], "Terminal Test");
        assert_eq!(terminal["workspaceId"], "ws-test-term-1");
        assert_eq!(terminal["activity"]["state"], "idle");

        // List terminals
        let list_req = serde_json::json!({
            "type": "list_terminals_request",
            "workspaceId": "ws-test-term-1",
            "requestId": "req-term-list-1"
        });
        let list_resp = crate::kernel::server::dispatch_session_message(&list_req).await;
        assert_eq!(list_resp["type"], "list_terminals_response");
        let terms = list_resp["payload"]["terminals"]
            .as_array()
            .expect("terminals array");
        assert!(terms.iter().any(|t| t["id"] == term_id));

        // Subscribe terminals
        let sub_terms_req = serde_json::json!({
            "type": "subscribe_terminals_request",
            "cwd": ".",
            "workspaceId": "ws-test-term-1",
            "requestId": "req-term-sub-1"
        });
        let sub_terms_resp = crate::kernel::server::dispatch_session_message(&sub_terms_req).await;
        assert_eq!(sub_terms_resp["type"], "terminals_changed");

        // Terminal input
        let input_req = serde_json::json!({
            "type": "terminal_input",
            "terminalId": term_id,
            "message": {
                "type": "input",
                "data": "echo test\r\n"
            }
        });
        let input_resp = crate::kernel::server::dispatch_session_message(&input_req).await;
        assert_eq!(input_resp["type"], "terminal_input_response");

        // Rename terminal
        let rename_req = serde_json::json!({
            "type": "terminal.rename.request",
            "terminalId": term_id,
            "title": "Renamed Terminal",
            "requestId": "req-term-rename-1"
        });
        let rename_resp = crate::kernel::server::dispatch_session_message(&rename_req).await;
        assert_eq!(rename_resp["type"], "terminal.rename.response");
        assert_eq!(rename_resp["payload"]["success"], true);

        // Kill terminal
        let kill_req = serde_json::json!({
            "type": "kill_terminal_request",
            "terminalId": term_id,
            "requestId": "req-term-kill-1"
        });
        let kill_resp = crate::kernel::server::dispatch_session_message(&kill_req).await;
        assert_eq!(kill_resp["type"], "kill_terminal_response");
        assert_eq!(kill_resp["payload"]["success"], true);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_embedded_daemon_named_pipe_ipc() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::windows::named_pipe::ClientOptions;

        let port = 6796;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let pipe_name = format!(r"\\.\pipe\kronos-ipc-{}", port);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = ClientOptions::new()
            .open(&pipe_name)
            .expect("Failed to open pipe");
        let (reader, mut writer) = tokio::io::split(client);
        let mut lines = BufReader::new(reader).lines();
        let hello = serde_json::json!({
            "type":"hello", "protocolVersion":1,
            "auth":{"kind":"localCredential","token":daemon.access.local_credential}
        });
        writer
            .write_all(format!("{hello}\n").as_bytes())
            .await
            .unwrap();
        let info = lines.next_line().await.unwrap().unwrap();
        assert!(info.contains("server_info"));

        let msg = serde_json::json!({
            "id": "pipe-ping-1",
            "type": "ping",
            "payload": {}
        });
        writer
            .write_all(format!("{}\n", msg).as_bytes())
            .await
            .unwrap();

        let resp_line = lines.next_line().await.unwrap().expect("Response line");
        let parsed: serde_json::Value = serde_json::from_str(&resp_line).unwrap();
        assert_eq!(parsed["id"], "pipe-ping-1");
        assert_eq!(parsed["type"], "pong");
        assert_eq!(parsed["success"], true);

        daemon.stop().await;
    }

    #[test]
    fn test_safe_socket_path_validation() {
        use crate::kernel::server::is_safe_socket_path_to_remove;

        // Path in temp directory should be allowed
        let temp_dir = std::env::temp_dir();
        let safe_file = temp_dir.join("kronos_test_safe_socket.sock");
        let _ = std::fs::write(&safe_file, b"test");
        assert!(is_safe_socket_path_to_remove(&safe_file));
        let _ = std::fs::remove_file(&safe_file);

        // Path in current working directory (not temp, not socket) should NOT be allowed
        let local_file = std::path::Path::new("Cargo.toml");
        assert!(!is_safe_socket_path_to_remove(local_file));

        // Path attempting traversal escape outside temp directory should NOT be allowed
        // e.g., <temp_dir>/../<current_dir>/Cargo.toml resolves to Cargo.toml
        let traversal_path = temp_dir.join("..").join("kronos_test_traversal.sock");
        assert!(!is_safe_socket_path_to_remove(&traversal_path));

        let unix_style_traversal = std::path::Path::new("/tmp/../var/log/important.txt");
        assert!(!is_safe_socket_path_to_remove(unix_style_traversal));

        // Non-existent path returns false
        let nonexistent = std::path::Path::new("/nonexistent_path/fake.sock");
        assert!(!is_safe_socket_path_to_remove(nonexistent));
    }

    #[tokio::test]
    async fn test_daemon_vfs_watch_and_unwatch_envelope() {
        let temp = tempfile::tempdir().expect("tempdir");
        let watch_req = RpcEnvelope {
            id: "watch-1".to_string(),
            msg_type: "vfs/watch".to_string(),
            payload: serde_json::json!({
                "path": temp.path().to_str().unwrap(),
                "ignore": [".git", "target"]
            }),
        };

        let watch_res = dispatch_envelope(watch_req).await;
        assert!(watch_res.success);
        let sub_id = watch_res.data.unwrap()["subscriptionId"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(!sub_id.is_empty());

        let unwatch_req = RpcEnvelope {
            id: "unwatch-1".to_string(),
            msg_type: "vfs/unwatch".to_string(),
            payload: serde_json::json!({
                "subscriptionId": sub_id
            }),
        };
        let unwatch_res = dispatch_envelope(unwatch_req).await;
        assert!(unwatch_res.success);
    }

    #[tokio::test]
    async fn test_daemon_vfs_watch_and_unwatch_session_message() {
        let temp = tempfile::tempdir().expect("tempdir");
        let watch_msg = serde_json::json!({
            "type": "vfs/watch",
            "requestId": "sess-watch-1",
            "path": temp.path().to_str().unwrap(),
            "ignore": [".git"]
        });

        let watch_res = dispatch_session_message(&watch_msg).await;
        assert_eq!(watch_res["type"], "vfs/watch/response");
        assert_eq!(watch_res["requestId"], "sess-watch-1");
        assert_eq!(watch_res["success"], true);
        let sub_id = watch_res["subscriptionId"].as_str().unwrap().to_string();
        assert!(!sub_id.is_empty());
        assert_eq!(watch_res["payload"]["subscriptionId"], sub_id);

        let unwatch_msg = serde_json::json!({
            "type": "vfs/unwatch",
            "requestId": "sess-unwatch-1",
            "subscriptionId": sub_id
        });

        let unwatch_res = dispatch_session_message(&unwatch_msg).await;
        assert_eq!(unwatch_res["type"], "vfs/unwatch/response");
        assert_eq!(unwatch_res["requestId"], "sess-unwatch-1");
        assert_eq!(unwatch_res["success"], true);
        assert_eq!(unwatch_res["unwatched"], true);
        assert_eq!(unwatch_res["payload"]["unwatched"], true);

        // Error cases
        let missing_path = serde_json::json!({
            "type": "vfs/watch",
            "requestId": "err-1"
        });
        let err_res = dispatch_session_message(&missing_path).await;
        assert_eq!(err_res["success"], false);

        let missing_sub = serde_json::json!({
            "type": "vfs/unwatch",
            "requestId": "err-2"
        });
        let err_sub = dispatch_session_message(&missing_sub).await;
        assert_eq!(err_sub["success"], false);
    }

    #[tokio::test]
    async fn test_daemon_vfs_envelope_errors() {
        let missing_path = RpcEnvelope {
            id: "err-env-1".to_string(),
            msg_type: "vfs/watch".to_string(),
            payload: serde_json::json!({}),
        };
        let res1 = dispatch_envelope(missing_path).await;
        assert!(!res1.success);
        assert!(res1.error.is_some());

        let missing_sub = RpcEnvelope {
            id: "err-env-2".to_string(),
            msg_type: "vfs/unwatch".to_string(),
            payload: serde_json::json!({}),
        };
        let res2 = dispatch_envelope(missing_sub).await;
        assert!(!res2.success);
        assert!(res2.error.is_some());

        let nonexistent = RpcEnvelope {
            id: "err-env-3".to_string(),
            msg_type: "vfs/watch".to_string(),
            payload: serde_json::json!({
                "path": "nonexistent_directory_for_kronos_test_12345"
            }),
        };
        let res3 = dispatch_envelope(nonexistent).await;
        assert!(!res3.success);
    }

    #[tokio::test]
    async fn test_embedded_daemon_vfs_websocket_events_forwarding() {
        let daemon = EmbeddedDaemon::start_test(0)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", daemon.port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        let temp = tempfile::tempdir().expect("tempdir");
        let watch_msg = serde_json::json!({
            "id": "ws-watch-1",
            "type": "vfs/watch",
            "payload": {
                "path": temp.path().to_str().unwrap()
            }
        });

        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                watch_msg.to_string(),
            ))
            .await
            .unwrap();

        let parsed_reply = wait_for_rpc_response(&mut ws_stream, "ws-watch-1").await;
        assert_eq!(parsed_reply["id"], "ws-watch-1");
        assert_eq!(parsed_reply["success"], true);
        let sub_id = parsed_reply["data"]["subscriptionId"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(!sub_id.is_empty());

        // Write a test file to trigger a filesystem event
        let file_path = temp.path().join("vfs_stream_test.txt");
        std::fs::write(&file_path, b"hello vfs stream").expect("write test file");

        // Receive the broadcasted vfs/change_events frame
        let mut received_event = false;
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(3));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    break;
                }
                maybe_frame = ws_stream.next() => {
                    if let Some(Ok(frame)) = maybe_frame {
                        if let Ok(text) = frame.to_text() {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                                if json.get("type").and_then(|v| v.as_str()) == Some("vfs/change_events") {
                                    let events = json["payload"].as_array().expect("events array");
                                    assert!(!events.is_empty());
                                    received_event = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(
            received_event,
            "Expected to receive vfs/change_events frame over WebSocket"
        );

        // Unwatch
        let unwatch_msg = serde_json::json!({
            "id": "ws-unwatch-1",
            "type": "vfs/unwatch",
            "payload": {
                "subscriptionId": sub_id
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                unwatch_msg.to_string(),
            ))
            .await
            .unwrap();

        let parsed_unwatch = wait_for_rpc_response(&mut ws_stream, "ws-unwatch-1").await;
        assert_eq!(parsed_unwatch["id"], "ws-unwatch-1");
        assert_eq!(parsed_unwatch["success"], true);

        daemon.stop().await;
    }

    async fn wait_for_rpc_response<S>(ws_stream: &mut S, expected_id: &str) -> serde_json::Value
    where
        S: futures_util::StreamExt<
                Item = Result<
                    tokio_tungstenite::tungstenite::Message,
                    tokio_tungstenite::tungstenite::Error,
                >,
            > + Unpin,
    {
        while let Some(Ok(frame)) = ws_stream.next().await {
            if let Ok(text) = frame.to_text() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                    let id_opt = json
                        .get("id")
                        .or_else(|| json.get("requestId"))
                        .or_else(|| {
                            json.get("message")
                                .and_then(|m| m.get("id").or_else(|| m.get("requestId")))
                        })
                        .and_then(|v| v.as_str());
                    if id_opt == Some(expected_id) {
                        return json;
                    }
                }
            }
        }
        panic!("Did not receive response for ID: {}", expected_id);
    }

    #[tokio::test]
    async fn test_embedded_daemon_ast_analyze_and_symbols_envelopes() {
        let port = 6791;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        let source_code = r#"
            export interface AppConfig {
                env: string;
                port: number;
            }

            export async function startApp(config: AppConfig): Promise<boolean> {
                return true;
            }
        "#;

        // 1. Test ast/analyze RPC envelope
        let analyze_envelope = serde_json::json!({
            "id": "ast-analyze-1",
            "type": "ast/analyze",
            "payload": {
                "source": source_code,
                "ext": "ts"
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                analyze_envelope.to_string(),
            ))
            .await
            .unwrap();

        let parsed_analyze = wait_for_rpc_response(&mut ws_stream, "ast-analyze-1").await;
        assert_eq!(parsed_analyze["id"], "ast-analyze-1");
        assert_eq!(parsed_analyze["success"], true);
        let symbols = parsed_analyze
            .get("data")
            .or_else(|| parsed_analyze.get("payload"))
            .and_then(|v| v.get("symbols"))
            .and_then(|v| v.as_array())
            .expect("symbols array");
        assert!(symbols.iter().any(|s| s["name"] == "AppConfig"));
        assert!(symbols.iter().any(|s| s["name"] == "startApp"));

        // 2. Test ast/symbols RPC envelope
        let symbols_envelope = serde_json::json!({
            "id": "ast-symbols-1",
            "type": "ast/symbols",
            "payload": {
                "source": source_code,
                "ext": "ts"
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                symbols_envelope.to_string(),
            ))
            .await
            .unwrap();

        let parsed_symbols = wait_for_rpc_response(&mut ws_stream, "ast-symbols-1").await;
        assert_eq!(parsed_symbols["id"], "ast-symbols-1");
        assert_eq!(parsed_symbols["success"], true);
        let symbols_list = parsed_symbols
            .get("data")
            .or_else(|| parsed_symbols.get("payload"))
            .and_then(|v| v.as_array())
            .expect("symbols payload array");
        assert_eq!(symbols_list.len(), 2);

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_crdt_realtime_collaboration() {
        let port = 6789;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);

        // 1. Connect Client 1 (Human Editor)
        let (mut client1, _) = connect_async(&url).await.expect("Client 1 connect");
        authenticate_socket(&mut client1, &daemon).await;

        // 2. Connect Client 2 (Agent A)
        let (mut client2, _) = connect_async(&url).await.expect("Client 2 connect");
        authenticate_socket(&mut client2, &daemon).await;

        let room_id = "test-collab-room";

        // Client 1 joins the room
        let join_msg = serde_json::json!({
            "id": "crdt-join-1",
            "type": "crdt/join",
            "payload": {
                "room": room_id
            }
        });
        client1
            .send(tokio_tungstenite::tungstenite::Message::Text(
                join_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_join = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let frame = client1.next().await.unwrap().unwrap();
                if let Ok(text) = frame.to_text() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                        if val.get("id").and_then(|v| v.as_str()) == Some("crdt-join-1") {
                            break val;
                        }
                    }
                }
            }
        })
        .await
        .expect("timed out waiting for crdt-join-1 response");
        assert_eq!(parsed_join["id"], "crdt-join-1");
        assert_eq!(parsed_join["success"], true);

        // Client 2 performs an edit via crdt/edit
        let edit_msg = serde_json::json!({
            "id": "crdt-edit-agent",
            "type": "crdt/edit",
            "payload": {
                "room": room_id,
                "action": "insert",
                "index": 0,
                "text": "const kernelStatus = 'READY';",
                "sourceId": "agent-alpha"
            }
        });
        client2
            .send(tokio_tungstenite::tungstenite::Message::Text(
                edit_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_edit = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let frame = client2.next().await.unwrap().unwrap();
                if let Ok(text) = frame.to_text() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                        if val.get("id").and_then(|v| v.as_str()) == Some("crdt-edit-agent") {
                            break val;
                        }
                    }
                }
            }
        })
        .await
        .expect("timed out waiting for crdt-edit-agent response");
        assert_eq!(parsed_edit["id"], "crdt-edit-agent");
        assert_eq!(parsed_edit["success"], true);

        // Client 1 should receive the broadcasted crdt/update_event frame
        let mut received_crdt_event = false;
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(3));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    break;
                }
                maybe_frame = client1.next() => {
                    if let Some(Ok(frame)) = maybe_frame {
                        if let Ok(text) = frame.to_text() {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                                if json.get("type").and_then(|v| v.as_str()) == Some("crdt/update_event") {
                                    assert_eq!(json["payload"]["room"], room_id);
                                    assert_eq!(json["payload"]["sourceId"], "agent-alpha");
                                    assert!(!json["payload"]["update"].as_str().unwrap().is_empty());
                                    received_crdt_event = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            received_crdt_event,
            "Client 1 should receive real-time CRDT update event from Agent A"
        );

        // Client 1 queries room state
        let state_msg = serde_json::json!({
            "id": "crdt-state-1",
            "type": "crdt/state",
            "payload": {
                "room": room_id
            }
        });
        client1
            .send(tokio_tungstenite::tungstenite::Message::Text(
                state_msg.to_string(),
            ))
            .await
            .unwrap();
        let state_reply = client1.next().await.unwrap().unwrap();
        let parsed_state: serde_json::Value =
            serde_json::from_str(state_reply.to_text().unwrap()).unwrap();
        assert_eq!(parsed_state["id"], "crdt-state-1");
        assert_eq!(parsed_state["success"], true);
        let state_data = parsed_state
            .get("data")
            .or_else(|| parsed_state.get("payload"))
            .unwrap();
        assert_eq!(state_data["text"], "const kernelStatus = 'READY';");

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_ioring_envelopes() {
        let port = 6788;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(&url).await.expect("connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        // 1. Test ioring/status
        let status_msg = serde_json::json!({
            "id": "ioring-status-1",
            "type": "ioring/status",
            "payload": {}
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                status_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_status = wait_for_rpc_response(&mut ws_stream, "ioring-status-1").await;
        assert_eq!(parsed_status["success"], true);
        let status_data = parsed_status
            .get("data")
            .or_else(|| parsed_status.get("payload"))
            .unwrap();
        assert!(status_data["isKernelActive"].is_boolean());
        assert!(status_data["queueSize"].as_u64().unwrap() >= 16);

        // 2. Test ioring/read_batch
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let test_file = temp_dir.path().join("batch_read_sample.txt");
        std::fs::write(&test_file, b"sample ioring batch content").unwrap();

        let read_batch_msg = serde_json::json!({
            "id": "ioring-read-1",
            "type": "ioring/read_batch",
            "payload": {
                "paths": [test_file.to_str().unwrap()]
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                read_batch_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_read = wait_for_rpc_response(&mut ws_stream, "ioring-read-1").await;
        assert_eq!(parsed_read["success"], true);
        let files = parsed_read
            .get("data")
            .or_else(|| parsed_read.get("payload"))
            .and_then(|v| v.get("files"))
            .and_then(|v| v.as_array())
            .expect("files array");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["size"], 27);

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_speech_envelopes() {
        let port = 6787;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(&url).await.expect("connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        // 1. Test speech/status
        let status_msg = serde_json::json!({
            "id": "speech-status-1",
            "type": "speech/status",
            "payload": {}
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                status_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_status = wait_for_rpc_response(&mut ws_stream, "speech-status-1").await;
        assert_eq!(parsed_status["success"], true);
        let status_data = parsed_status
            .get("data")
            .or_else(|| parsed_status.get("payload"))
            .unwrap();
        assert_eq!(status_data["targetSampleRate"], 16000);
        assert_eq!(status_data["targetChannels"], 1);

        // 2. Test speech/transcribe with synthetic voiced samples
        let synthetic_samples: Vec<f64> =
            (0..16000).map(|i| (i as f64 * 0.05).sin() * 0.75).collect();

        let transcribe_msg = serde_json::json!({
            "id": "speech-transcribe-1",
            "type": "speech/transcribe",
            "payload": {
                "samples": synthetic_samples,
                "sampleRate": 16000,
                "channels": 1
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                transcribe_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_trans = wait_for_rpc_response(&mut ws_stream, "speech-transcribe-1").await;
        let is_loaded = status_data["isModelLoaded"].as_bool().unwrap_or(false);
        if is_loaded {
            assert_eq!(parsed_trans["success"], true);
            let trans_data = parsed_trans
                .get("data")
                .or_else(|| parsed_trans.get("payload"))
                .unwrap();
            assert!(trans_data["transcript"].is_string());
            assert_eq!(trans_data["samplesCount"], 16000);
        } else {
            assert_eq!(parsed_trans["success"], false);
            assert!(
                parsed_trans["error"]
                    .as_str()
                    .unwrap_or("")
                    .contains("ggml-*.bin")
            );
        }

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_shm_envelopes() {
        let port = 6786;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(&url).await.expect("connect");
        authenticate_socket(&mut ws_stream, &daemon).await;

        // 1. Test shm/status
        let status_msg = serde_json::json!({
            "id": "shm-status-1",
            "type": "shm/status",
            "payload": {}
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                status_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_status = wait_for_rpc_response(&mut ws_stream, "shm-status-1").await;
        assert_eq!(parsed_status["success"], true);
        let status_data = parsed_status
            .get("data")
            .or_else(|| parsed_status.get("payload"))
            .unwrap();
        assert!(status_data["capacity"].as_u64().unwrap() > 0);

        // 2. Test shm/push
        let raw_payload = b"rkyv high speed terminal chunk";
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, raw_payload);
        let push_msg = serde_json::json!({
            "id": "shm-push-1",
            "type": "shm/push",
            "payload": {
                "streamId": "terminal-stream-1",
                "payloadBase64": b64
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                push_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_push = wait_for_rpc_response(&mut ws_stream, "shm-push-1").await;
        assert_eq!(parsed_push["success"], true);
        let push_data = parsed_push
            .get("data")
            .or_else(|| parsed_push.get("payload"))
            .unwrap();
        let seq = push_data["sequence"].as_u64().unwrap();
        assert!(seq > 0);

        // 3. Test shm/poll
        let poll_msg = serde_json::json!({
            "id": "shm-poll-1",
            "type": "shm/poll",
            "payload": {
                "sinceSeq": seq - 1
            }
        });
        ws_stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                poll_msg.to_string(),
            ))
            .await
            .unwrap();
        let parsed_poll = wait_for_rpc_response(&mut ws_stream, "shm-poll-1").await;
        assert_eq!(parsed_poll["success"], true);
        let records = parsed_poll
            .get("data")
            .or_else(|| parsed_poll.get("payload"))
            .and_then(|v| v.get("records"))
            .and_then(|v| v.as_array())
            .expect("records array");
        assert!(!records.is_empty());
        assert_eq!(records.last().unwrap()["streamId"], "terminal-stream-1");

        daemon.stop().await;
    }
    #[tokio::test]
    async fn test_embedded_daemon_unsupported_session_message_returns_rpc_error() {
        let session_msg = serde_json::json!({
            "type": "project.unknown_unsupported.request",
            "requestId": "req-unsupported-99",
            "payload": {}
        });

        let resp = dispatch_session_message(&session_msg).await;
        assert_eq!(resp["type"], "rpc_error");
        assert_eq!(resp["payload"]["requestId"], "req-unsupported-99");
        assert_eq!(
            resp["payload"]["requestType"],
            "project.unknown_unsupported.request"
        );
        assert_eq!(resp["payload"]["code"], "not_implemented");
    }

    #[tokio::test]
    async fn test_embedded_daemon_truthful_features() {
        let server_info = crate::kernel::protocol::build_server_info_payload();
        let features = &server_info["features"];
        // Unimplemented features must be explicitly false until ported
        assert_eq!(features["connectedHosts"], false);
        assert_eq!(features["projectAdd"], true);
        assert_eq!(features["workspaceGithubRepositorySearch"], true);
        // Kernel modules must be true
        assert_eq!(features["kernelTelemetry"], true);
        assert_eq!(features["kernelAst"], true);
        assert_eq!(features["kernelCrdt"], true);
        assert_eq!(features["kernelIoRing"], true);
        assert_eq!(features["kernelShm"], true);
        assert_eq!(features["kernelSpeech"], true);
        assert_eq!(features["providerUsageList"], true);
        assert_eq!(features["usageSources"], true);
    }

    #[tokio::test]
    async fn test_embedded_daemon_usage_reporting() {
        use crate::kernel::server::dispatch_session_message;

        // 1. provider.usage.list.request
        let prov_usage_req = serde_json::json!({
            "type": "provider.usage.list.request",
            "requestId": "req-prov-usage-1"
        });
        let prov_resp = dispatch_session_message(&prov_usage_req).await;
        assert_eq!(prov_resp["type"], "provider.usage.list.response");
        assert_eq!(prov_resp["payload"]["requestId"], "req-prov-usage-1");
        assert!(prov_resp["payload"]["providers"].is_array());

        // 2. usage.list_reports.request (global, empty)
        let list_usage_req = serde_json::json!({
            "type": "usage.list_reports.request",
            "requestId": "req-usage-reports-1"
        });
        let list_resp = dispatch_session_message(&list_usage_req).await;
        assert_eq!(list_resp["type"], "usage.list_reports.response");
        assert_eq!(list_resp["payload"]["requestId"], "req-usage-reports-1");
        assert_eq!(list_resp["payload"]["error"], serde_json::Value::Null);

        // 3. usage.list_reports.request with specific reportIds
        let specific_req = serde_json::json!({
            "type": "usage.list_reports.request",
            "requestId": "req-usage-reports-2",
            "reportIds": ["claude"]
        });
        let specific_resp = dispatch_session_message(&specific_req).await;
        assert_eq!(specific_resp["type"], "usage.list_reports.response");
        assert_eq!(specific_resp["payload"]["requestId"], "req-usage-reports-2");
        assert_eq!(specific_resp["payload"]["error"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_embedded_daemon_browser_proxy_endpoints() {
        let port = 6792;
        let daemon = EmbeddedDaemon::start_test(port)
            .await
            .expect("Failed to start daemon");

        let client = reqwest::Client::new();

        // 1. Test POST /api/browser-proxy/cookies
        let cookies_resp = client
            .post(format!("http://127.0.0.1:{port}/api/browser-proxy/cookies"))
            .bearer_auth(&daemon.access.local_credential)
            .json(&serde_json::json!({
                "host": "test-domain.com",
                "cookies": "sid=12345; auth=ok"
            }))
            .send()
            .await
            .expect("Failed to post cookies");

        assert_eq!(cookies_resp.status(), reqwest::StatusCode::OK);
        let cookies_json: serde_json::Value = cookies_resp.json().await.unwrap();
        assert_eq!(cookies_json["ok"], true);
        assert_eq!(cookies_json["host"], "test-domain.com");
        assert_eq!(cookies_json["count"], 2);

        // 2. Test GET /api/browser-proxy with missing url
        let missing_url_resp = client
            .get(format!("http://127.0.0.1:{port}/api/browser-proxy"))
            .bearer_auth(&daemon.access.local_credential)
            .send()
            .await
            .expect("Failed to call browser proxy");
        assert_eq!(missing_url_resp.status(), reqwest::StatusCode::BAD_REQUEST);

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_browser_proxy_ticket_cannot_expand_authority() {
        use std::sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        };
        let redirect = Arc::new(AtomicBool::new(false));
        let secrets = Arc::new(AtomicUsize::new(0));
        let upstream = axum::Router::new()
            .route(
                "/start",
                axum::routing::get({
                    let redirect = redirect.clone();
                    move || {
                        let redirect = redirect.clone();
                        async move {
                            use axum::response::IntoResponse;
                            if redirect.load(Ordering::SeqCst) {
                                axum::response::Redirect::temporary("/secret").into_response()
                            } else {
                                axum::response::Html("<h1>Selected page</h1>").into_response()
                            }
                        }
                    }
                }),
            )
            .route(
                "/secret",
                axum::routing::get({
                    let secrets = secrets.clone();
                    move || {
                        let secrets = secrets.clone();
                        async move {
                            secrets.fetch_add(1, Ordering::SeqCst);
                            "other-target-secret"
                        }
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_port = listener.local_addr().unwrap().port();
        let upstream_task = tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });
        let daemon = EmbeddedDaemon::start_test(0).await.unwrap();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let base = format!("http://127.0.0.1:{}", daemon.port);
        let selected = format!("http://127.0.0.1:{upstream_port}/start");
        let issue = client
            .post(format!("{base}/api/browser-proxy/navigation"))
            .bearer_auth(&daemon.access.local_credential)
            .json(&serde_json::json!({"url":selected}))
            .send()
            .await
            .unwrap();
        assert_eq!(issue.status(), reqwest::StatusCode::OK);
        let issue: serde_json::Value = issue.json().await.unwrap();
        let ticket_path = issue["url"].as_str().unwrap();
        let initial = client
            .get(format!("{base}{ticket_path}"))
            .send()
            .await
            .unwrap();
        assert!(initial.text().await.unwrap().contains("Selected page"));
        assert_eq!(
            client
                .head(format!("{base}{ticket_path}"))
                .send()
                .await
                .unwrap()
                .status(),
            reqwest::StatusCode::UNAUTHORIZED
        );
        redirect.store(true, Ordering::SeqCst);
        let changed = client
            .get(format!("{base}{ticket_path}"))
            .send()
            .await
            .unwrap();
        assert!(changed.status().is_redirection());
        assert_eq!(
            changed.headers()[reqwest::header::LOCATION],
            format!("http://127.0.0.1:{upstream_port}/secret")
        );
        assert_eq!(
            secrets.load(Ordering::SeqCst),
            0,
            "A page ticket must not fetch another target's secret"
        );
        let unauthorized = client
            .post(format!("{base}/api/browser-proxy/navigation"))
            .json(&serde_json::json!({"url":selected}))
            .send()
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);
        // The trusted operator may deliberately resolve and authorize that redirect.
        let permitted = client
            .post(format!("{base}/api/browser-proxy/navigation"))
            .bearer_auth(&daemon.access.local_credential)
            .json(&serde_json::json!({"url":selected}))
            .send()
            .await
            .unwrap();
        assert_eq!(permitted.status(), reqwest::StatusCode::OK);
        let permitted: serde_json::Value = permitted.json().await.unwrap();
        assert_ne!(permitted["url"], issue["url"]);
        daemon.stop().await;
        upstream_task.abort();
    }

    #[tokio::test]
    async fn test_embedded_daemon_file_service_wire_protocol() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let root = dir.path();
        let root_str = root.to_str().unwrap();

        // 1. Create a file
        let create_req = serde_json::json!({
            "type": "fs.entry.create.request",
            "requestId": "req-create-1",
            "cwd": root_str,
            "parentPath": "",
            "name": "hello.txt",
            "kind": "file"
        });
        let create_resp = dispatch_session_message(&create_req).await;
        assert_eq!(create_resp["type"], "fs.entry.create.response");
        assert_eq!(create_resp["success"], true);
        assert!(root.join("hello.txt").exists());

        // 2. Write content
        let write_req = serde_json::json!({
            "type": "fs.file.write.request",
            "requestId": "req-write-1",
            "cwd": root_str,
            "path": "hello.txt",
            "content": "Rust Kernel VFS File Service"
        });
        let write_resp = dispatch_session_message(&write_req).await;
        assert_eq!(write_resp["type"], "fs.file.write.response");
        assert_eq!(write_resp["success"], true);
        assert_eq!(
            std::fs::read_to_string(root.join("hello.txt")).unwrap(),
            "Rust Kernel VFS File Service"
        );

        // 3. File explorer request
        let explore_req = serde_json::json!({
            "type": "file_explorer_request",
            "requestId": "req-explore-1",
            "cwd": root_str
        });
        let explore_resp = dispatch_session_message(&explore_req).await;
        assert_eq!(explore_resp["type"], "file_explorer_response");
        assert!(explore_resp["payload"]["error"].is_null());
        let entries = explore_resp["payload"]["directory"]["entries"]
            .as_array()
            .expect("entries array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "hello.txt");

        // 4. Delete entry
        let delete_req = serde_json::json!({
            "type": "fs.entry.delete.request",
            "requestId": "req-del-1",
            "cwd": root_str,
            "path": "hello.txt"
        });
        let delete_resp = dispatch_session_message(&delete_req).await;
        assert_eq!(delete_resp["type"], "fs.entry.delete.response");
        assert_eq!(delete_resp["success"], true);
        assert!(!root.join("hello.txt").exists());
    }

    #[tokio::test]
    async fn test_embedded_daemon_git_wire_protocol() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let root = dir.path();
        let root_str = root.to_str().unwrap();

        // Initialize git repo
        let init_out = tokio::process::Command::new("git")
            .args(&["init"])
            .current_dir(root)
            .output()
            .await
            .expect("git init");
        assert!(init_out.status.success());

        // Configure local git user for test commit
        let _ = tokio::process::Command::new("git")
            .args(&["config", "user.name", "Kronos Test"])
            .current_dir(root)
            .output()
            .await;
        let _ = tokio::process::Command::new("git")
            .args(&["config", "user.email", "test@kronos.local"])
            .current_dir(root)
            .output()
            .await;

        // Create and commit a file
        std::fs::write(root.join("file1.txt"), b"line1\n").unwrap();
        let _ = tokio::process::Command::new("git")
            .args(&["add", "file1.txt"])
            .current_dir(root)
            .output()
            .await;
        let _ = tokio::process::Command::new("git")
            .args(&["commit", "-m", "init commit"])
            .current_dir(root)
            .output()
            .await;

        // 1. Test git/branches wire call
        let branches_req = serde_json::json!({
            "type": "git/branches",
            "requestId": "req-branches-1",
            "cwd": root_str
        });
        let branches_resp = dispatch_session_message(&branches_req).await;
        assert_eq!(branches_resp["type"], "git/branches/response");
        assert_eq!(branches_resp["success"], true);
        let branches = branches_resp["payload"]["branches"]
            .as_array()
            .expect("branches array");
        assert!(!branches.is_empty());

        // 2. Modify file and test git/diff wire call
        std::fs::write(root.join("file1.txt"), b"line1\nline2\n").unwrap();
        let diff_req = serde_json::json!({
            "type": "git/diff",
            "requestId": "req-diff-1",
            "cwd": root_str,
            "staged": false
        });
        let diff_resp = dispatch_session_message(&diff_req).await;
        assert_eq!(diff_resp["type"], "git/diff/response");
        assert_eq!(diff_resp["success"], true);
        let diff_text = diff_resp["payload"]["diff"].as_str().expect("diff str");
        assert!(diff_text.contains("+line2"));

        // 3. Test git/worktree/list wire call
        let wt_req = serde_json::json!({
            "type": "git/worktree/list",
            "requestId": "req-wt-1",
            "cwd": root_str
        });
        let wt_resp = dispatch_session_message(&wt_req).await;
        assert_eq!(wt_resp["type"], "git/worktree/list/response");
        assert_eq!(wt_resp["success"], true);
        let wt_list = wt_resp["payload"]["worktrees"]
            .as_array()
            .expect("worktrees array");
        assert_eq!(wt_list.len(), 1);
    }

    #[tokio::test]
    async fn test_embedded_daemon_agent_and_plugins_wire_protocol() {
        // 1. List providers
        let prov_req = serde_json::json!({
            "type": "providers/list",
            "requestId": "req-prov-1"
        });
        let prov_resp = dispatch_session_message(&prov_req).await;
        assert_eq!(prov_resp["type"], "providers/list/response");
        assert_eq!(prov_resp["success"], true);
        let providers = prov_resp["payload"]["providers"]
            .as_array()
            .expect("providers array");
        assert!(providers.iter().any(|p| p["id"] == "mock"));
        assert!(providers.iter().any(|p| p["id"] == "claude"));
        assert!(providers.iter().any(|p| p["id"] == "codex"));
        assert!(providers.iter().any(|p| p["id"] == "opencode"));

        // 2. Create agent
        let create_req = serde_json::json!({
            "type": "agent/create",
            "requestId": "req-ag-1",
            "provider": "mock",
            "cwd": "/workspace/demo",
            "title": "Wire Test Agent"
        });
        let create_resp = dispatch_session_message(&create_req).await;
        assert_eq!(create_resp["type"], "agent/create/response");
        assert_eq!(create_resp["success"], true);
        let agent_id = create_resp["payload"]["id"].as_str().expect("agent id");

        // 3. Send message
        let msg_req = serde_json::json!({
            "type": "agent/message",
            "requestId": "req-msg-1",
            "id": agent_id,
            "text": "Hello Agent!"
        });
        let msg_resp = dispatch_session_message(&msg_req).await;
        assert_eq!(msg_resp["type"], "agent/message/response");
        assert_eq!(msg_resp["success"], true);

        // 4. Query timeline
        let time_req = serde_json::json!({
            "type": "agent/timeline",
            "requestId": "req-time-1",
            "id": agent_id
        });
        let time_resp = dispatch_session_message(&time_req).await;
        assert_eq!(time_resp["type"], "agent/timeline/response");
        assert_eq!(time_resp["success"], true);
        let items = time_resp["payload"]["items"]
            .as_array()
            .expect("timeline items");
        assert_eq!(items.len(), 1);

        // 5. Query plugin usage metrics
        let usage_req = serde_json::json!({
            "type": "plugins/usage",
            "requestId": "req-usage-1"
        });
        let usage_resp = dispatch_session_message(&usage_req).await;
        assert_eq!(usage_resp["type"], "plugins/usage/response");
        assert_eq!(usage_resp["success"], true);
        let metrics = usage_resp["payload"]["metrics"]
            .as_array()
            .expect("metrics array");
        assert_eq!(metrics.len(), 9);
    }

    #[tokio::test]
    async fn test_embedded_daemon_project_wire_protocol() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let project_dir = temp_dir.path().join("my-test-project");
        std::fs::create_dir_all(&project_dir).expect("create_dir");

        // 1. Add project request
        let add_req = serde_json::json!({
            "type": "project.add.request",
            "requestId": "req-proj-add-1",
            "cwd": project_dir.to_str().unwrap()
        });
        let add_resp = dispatch_session_message(&add_req).await;
        assert_eq!(add_resp["type"], "project.add.response");
        assert!(add_resp["payload"]["error"].is_null());
        let proj = &add_resp["payload"]["project"];
        assert_eq!(proj["projectDisplayName"], "my-test-project");
        assert!(proj["projectId"].as_str().unwrap().starts_with("prj_"));
        assert_eq!(proj["projectIconRevision"], "automatic:none:v1");

        // 2. List projects request
        let list_req = serde_json::json!({
            "type": "project.list.request",
            "requestId": "req-proj-list-1"
        });
        let list_resp = dispatch_session_message(&list_req).await;
        assert_eq!(list_resp["type"], "project.list.response");
        let projects = list_resp["payload"]["projects"]
            .as_array()
            .expect("projects array");
        assert!(projects
            .iter()
            .any(|p| p["projectDisplayName"] == "my-test-project"
                && p["projectIconRevision"] == "automatic:none:v1"));

        // 3. Open project request
        let open_req = serde_json::json!({
            "type": "open_project_request",
            "requestId": "req-open-1",
            "cwd": project_dir.to_str().unwrap()
        });
        let open_resp = dispatch_session_message(&open_req).await;
        assert_eq!(open_resp["type"], "open_project_response");
        assert!(open_resp["payload"]["error"].is_null());
        let ws = &open_resp["payload"]["workspace"];
        assert_eq!(ws["name"], "my-test-project");
        assert_eq!(ws["projectDisplayName"], "my-test-project");

        // 4. Workspace create request
        let ws_create_req = serde_json::json!({
            "type": "workspace.create.request",
            "requestId": "req-ws-create-1",
            "source": {
                "kind": "directory",
                "path": project_dir.to_str().unwrap()
            }
        });
        let ws_create_resp = dispatch_session_message(&ws_create_req).await;
        assert_eq!(ws_create_resp["type"], "workspace.create.response");
        assert!(ws_create_resp["payload"]["error"].is_null());
        let created_ws = &ws_create_resp["payload"]["workspace"];
        assert_eq!(created_ws["name"], "my-test-project");
        assert!(
            ws_create_resp["payload"]["agent"].is_object(),
            "agent must be returned in workspace.create.response"
        );
        assert!(ws_create_resp["payload"]["agent"]["id"]
            .as_str()
            .unwrap()
            .starts_with("ag_"));
        let ag_create_req = serde_json::json!({
            "type": "agent.create.request",
            "requestId": "req-ag-create-1",
            "workspaceId": created_ws["id"].as_str().unwrap(),
            "model": "opencode/deepseek-v4.1-flash-free",
            "initialPrompt": "Initialize test repo"
        });
        let ag_create_resp = dispatch_session_message(&ag_create_req).await;
        assert_eq!(ag_create_resp["type"], "agent.create.response");
        assert!(ag_create_resp["payload"]["error"].is_null());
        let created_ag = &ag_create_resp["payload"]["agent"];
        assert!(created_ag["id"].as_str().unwrap().starts_with("ag_"));
        assert_eq!(created_ag["provider"], "opencode");
        assert_eq!(created_ag["model"], "opencode/deepseek-v4.1-flash-free");
        assert_eq!(
            ag_create_resp["payload"]["creation"]["phase"],
            "prompt_started"
        );

        // Verify the initial prompt was recorded to the timeline
        let timeline_req = serde_json::json!({
            "type": "fetch_agent_timeline_request",
            "requestId": "req-tl-verify",
            "agentId": created_ag["id"].as_str().unwrap()
        });
        let tl_res = dispatch_session_message(&timeline_req).await;
        assert_eq!(tl_res["type"], "fetch_agent_timeline_response");
        let entries = tl_res["payload"]["entries"].as_array().unwrap();
        assert!(
            !entries.is_empty(),
            "Timeline entries must contain the initial prompt"
        );
        assert_eq!(entries[0]["item"]["text"], "Initialize test repo");
    }

    #[tokio::test]
    async fn test_embedded_daemon_subscriptions_and_workspace_wire_protocol() {
        // 1. session.events.set_subscription.request
        let sub_events_req = serde_json::json!({
            "type": "session.events.set_subscription.request",
            "requestId": "req-sub-events-1",
            "events": ["project.update", "script_status_update"]
        });
        let sub_events_resp = dispatch_session_message(&sub_events_req).await;
        assert_eq!(
            sub_events_resp["type"],
            "session.events.set_subscription.response"
        );
        assert_eq!(sub_events_resp["payload"]["requestId"], "req-sub-events-1");
        assert!(sub_events_resp["payload"]["subscriptionId"]
            .as_str()
            .unwrap()
            .starts_with("sub-events-"));

        // 2. fetch_workspaces_request (with subscribe)
        let ws_req = serde_json::json!({
            "type": "fetch_workspaces_request",
            "requestId": "req-fetch-ws-1",
            "subscribe": {}
        });
        let ws_resp = dispatch_session_message(&ws_req).await;
        assert_eq!(ws_resp["type"], "fetch_workspaces_response");
        assert_eq!(ws_resp["payload"]["requestId"], "req-fetch-ws-1");
        assert!(ws_resp["payload"]["subscriptionId"].as_str().is_some());
        assert!(ws_resp["payload"]["entries"].is_array());
        assert!(ws_resp["payload"]["emptyProjects"].is_array());
        assert_eq!(ws_resp["payload"]["pageInfo"]["hasMore"], false);

        // 3. fetch_agents_request (with subscribe)
        let agents_req = serde_json::json!({
            "type": "fetch_agents_request",
            "requestId": "req-fetch-agents-1",
            "subscribe": {}
        });
        let agents_resp = dispatch_session_message(&agents_req).await;
        assert_eq!(agents_resp["type"], "fetch_agents_response");
        assert_eq!(agents_resp["payload"]["requestId"], "req-fetch-agents-1");
        assert!(agents_resp["payload"]["subscriptionId"].as_str().is_some());
        assert!(agents_resp["payload"]["entries"].is_array());

        // 4. workspace.label.list.request
        let labels_req = serde_json::json!({
            "type": "workspace.label.list.request",
            "requestId": "req-labels-1",
            "subscribe": {}
        });
        let labels_resp = dispatch_session_message(&labels_req).await;
        assert_eq!(labels_resp["type"], "workspace.label.list.response");
        assert_eq!(labels_resp["payload"]["requestId"], "req-labels-1");
        assert!(labels_resp["payload"]["subscriptionId"].as_str().is_some());
        assert!(labels_resp["payload"]["labels"].is_array());
        assert_eq!(labels_resp["payload"]["sync"]["mode"], "snapshot");

        // 5. subscription.release.request
        let release_req = serde_json::json!({
            "type": "subscription.release.request",
            "requestId": "req-rel-1",
            "subscriptionId": "sub-events-1"
        });
        let release_resp = dispatch_session_message(&release_req).await;
        assert_eq!(release_resp["type"], "subscription.release.response");
        assert_eq!(release_resp["payload"]["requestId"], "req-rel-1");
        assert_eq!(release_resp["payload"]["subscriptionId"], "sub-events-1");

        // 6. get_daemon_config_request
        let cfg_req = serde_json::json!({
            "type": "get_daemon_config_request",
            "requestId": "req-cfg-1"
        });
        let cfg_resp = dispatch_session_message(&cfg_req).await;
        assert_eq!(cfg_resp["type"], "get_daemon_config_response");
        assert!(cfg_resp["payload"]["config"]["mcp"].is_object());

        // 7. daemon.get_status.request
        let status_req = serde_json::json!({
            "type": "daemon.get_status.request",
            "requestId": "req-stat-1"
        });
        let status_resp = dispatch_session_message(&status_req).await;
        assert_eq!(status_resp["type"], "daemon.get_status.response");
        assert_eq!(status_resp["payload"]["listen"], "127.0.0.1:6767");

        // 8. get_providers_snapshot_request
        let prov_snap_req = serde_json::json!({
            "type": "get_providers_snapshot_request",
            "requestId": "req-snap-1"
        });
        let prov_snap_resp = dispatch_session_message(&prov_snap_req).await;
        assert_eq!(prov_snap_resp["type"], "get_providers_snapshot_response");
        let entries = prov_snap_resp["payload"]["entries"]
            .as_array()
            .expect("entries array");
        assert!(!entries.is_empty());
        for entry in entries {
            let provider = entry["provider"].as_str().expect("provider id");
            for model in entry["models"].as_array().expect("models array") {
                assert_eq!(model["provider"], provider);
                assert!(!model["id"].as_str().unwrap_or_default().is_empty());
                assert!(!model["label"].as_str().unwrap_or_default().is_empty());
            }
        }

        // 9. list_available_providers_request
        let list_prov_req = serde_json::json!({
            "type": "list_available_providers_request",
            "requestId": "req-list-prov-1"
        });
        let list_prov_resp = dispatch_session_message(&list_prov_req).await;
        assert_eq!(list_prov_resp["type"], "list_available_providers_response");
        let provs = list_prov_resp["payload"]["providers"]
            .as_array()
            .expect("providers array");
        assert!(provs
            .iter()
            .any(|p| p["provider"] == "codex" && p["available"].is_boolean()));

        // 10. refresh_providers_snapshot_request
        let ref_req = serde_json::json!({
            "type": "refresh_providers_snapshot_request",
            "requestId": "req-ref-1"
        });
        let ref_resp = dispatch_session_message(&ref_req).await;
        assert_eq!(ref_resp["type"], "refresh_providers_snapshot_response");
        assert_eq!(ref_resp["payload"]["acknowledged"], true);

        // 10b. subscribe_providers_request and verify push update
        let mut push_rx = crate::kernel::server::get_provider_push_tx().subscribe();
        let sub_prov_req = serde_json::json!({
            "type": "subscribe_providers_request",
            "requestId": "req-sub-prov-1"
        });
        let sub_prov_resp = dispatch_session_message(&sub_prov_req).await;
        assert_eq!(sub_prov_resp["type"], "subscribe_providers_response");
        assert_eq!(
            sub_prov_resp["payload"]["subscriptionId"],
            "sub-providers-req-sub-prov-1"
        );

        // Wait for pushed update frame
        let parsed_update = loop {
            let pushed_frame = tokio::time::timeout(std::time::Duration::from_secs(15), push_rx.recv())
                .await
                .expect("timeout waiting for providers_snapshot_update")
                .expect("recv error");
            let parsed: serde_json::Value =
                serde_json::from_str(&pushed_frame).expect("valid json update");
            if parsed["type"] == "session" && parsed["message"]["type"] == "providers_snapshot_update" {
                break parsed;
            }
        };
        let pushed_entries = parsed_update["message"]["payload"]["entries"]
            .as_array()
            .expect("entries array");
        assert!(
            !pushed_entries.is_empty(),
            "pushed entries must not be empty"
        );
        let codex_entry = pushed_entries
            .iter()
            .find(|e| e["provider"] == "codex")
            .expect("codex entry");
        let codex_models = codex_entry["models"].as_array().expect("codex models");
        let codex_status = codex_entry["status"].as_str().unwrap_or_default();
        assert!(
            matches!(codex_status, "ready" | "unavailable"),
            "Unexpected codex provider status: {codex_status}"
        );
        if codex_status == "ready" {
            assert!(
                !codex_models.is_empty(),
                "Ready codex provider must expose discovered models"
            );
        }
        // 11. provider_diagnostic_request
        let diag_req = serde_json::json!({
            "type": "provider_diagnostic_request",
            "requestId": "req-diag-1",
            "provider": "codex"
        });
        let diag_resp = dispatch_session_message(&diag_req).await;
        assert_eq!(diag_resp["type"], "provider_diagnostic_response");
        let diag_text = diag_resp["payload"]["diagnostic"].as_str().unwrap_or_default();
        assert!(
            diag_text.contains("Provider 'OpenAI Codex'"),
            "Unexpected provider diagnostic output: {diag_text}"
        );
    }
    #[tokio::test]
    async fn test_discover_codex_models_live() {
        let entries = crate::kernel::server::get_provider_snapshot_entries(".").await;
        for e in entries {
            eprintln!(
                "PROVIDER: {} -> status={} models={}",
                e["provider"],
                e["status"],
                e["models"].as_array().map(|a| a.len()).unwrap_or(0)
            );
        }
    }

    #[test]
    fn test_provider_metadata_contains_no_model_catalog() {
        let entries = crate::kernel::server::get_provider_snapshot_metadata_entries();

        assert!(entries.iter().all(|entry| {
            entry
                .get("models")
                .and_then(serde_json::Value::as_array)
                .is_some_and(Vec::is_empty)
        }));
    }

    #[test]
    fn test_codex_catalog_uses_runtime_model_list() {
        let response = serde_json::json!({
            "result": {
                "data": [
                    {
                        "id": "runtime-model-2027",
                        "displayName": "Runtime Model",
                        "description": "Reported by Codex app-server",
                        "isDefault": true,
                        "contextWindow": 64000
                    },
                    { "id": "runtime-model-2027", "displayName": "Duplicate" },
                    { "displayName": "Missing ID" }
                ]
            }
        });

        let models = crate::kernel::server::codex_models_from_response(&response);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["id"], "runtime-model-2027");
        assert_eq!(models[0]["provider"], "codex");
        assert_eq!(models[0]["label"], "Runtime Model");
        assert_eq!(models[0]["isDefault"], true);
        assert_eq!(models[0]["contextWindowMaxTokens"], 64000);
    }

    #[test]
    fn test_codex_catalog_preserves_reported_thinking_choices_and_default() {
        let models = crate::kernel::server::codex_models_from_response(&serde_json::json!({
            "result": { "data": [
                {
                    "id": "reasoning-model",
                    "defaultReasoningEffort": "high",
                    "supportedReasoningEfforts": [
                        { "reasoningEffort": "low", "description": "Faster reasoning" },
                        { "reasoningEffort": "high", "description": "Deeper reasoning" },
                        { "reasoningEffort": "high", "description": "Duplicate" },
                        { "reasoningEffort": "" }
                    ]
                },
                { "id": "plain-model", "supportedReasoningEfforts": [] },
                { "id": "default-only", "defaultReasoningEffort": "medium" }
            ] }
        }));
        assert_eq!(
            models[0]["thinkingOptions"],
            serde_json::json!([
                { "id": "low", "label": "low", "description": "Faster reasoning", "isDefault": false },
                { "id": "high", "label": "high", "description": "Deeper reasoning", "isDefault": true }
            ])
        );
        assert_eq!(models[0]["defaultThinkingOptionId"], "high");
        assert!(models[1].get("thinkingOptions").is_none());
        assert!(models[1].get("defaultThinkingOptionId").is_none());
        assert_eq!(models[2]["thinkingOptions"][0]["id"], "medium");
        assert_eq!(models[2]["defaultThinkingOptionId"], "medium");
    }

    #[test]
    fn test_pi_catalog_derives_only_runtime_supported_thinking_levels() {
        let models = crate::kernel::server::pi_models_from_response(&serde_json::json!({
            "models": [
                {
                    "provider": "vendor", "id": "restricted", "reasoning": true,
                    "thinkingLevelMap": { "off": null, "minimal": null, "medium": null, "xhigh": "maximum" }
                },
                { "provider": "vendor", "id": "standard", "reasoning": true },
                { "provider": "vendor", "id": "plain", "reasoning": false },
                {
                    "provider": "vendor", "id": "lower-only", "reasoning": true,
                    "thinkingLevelMap": { "medium": null, "high": null }
                }
            ]
        }));
        let ids = |model: &serde_json::Value| -> Vec<String> {
            model["thinkingOptions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|option| option["id"].as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(ids(&models[0]), ["low", "high", "xhigh"]);
        assert_eq!(models[0]["defaultThinkingOptionId"], "high");
        assert_eq!(ids(&models[1]), ["off", "minimal", "low", "medium", "high"]);
        assert_eq!(models[1]["defaultThinkingOptionId"], "medium");
        assert!(models[2].get("thinkingOptions").is_none());
        assert!(models[2].get("defaultThinkingOptionId").is_none());
        assert_eq!(ids(&models[3]), ["off", "minimal", "low"]);
        assert_eq!(models[3]["defaultThinkingOptionId"], "low");
    }

    #[test]
    fn test_omp_catalog_preserves_efforts_without_inventing_missing_ladders() {
        let models = crate::kernel::server::omp_models_from_response(&serde_json::json!({
            "models": [
                {
                    "provider": "vendor", "id": "reasoning", "reasoning": true,
                    "thinking": { "mode": "effort", "efforts": ["high", "xhigh"], "defaultLevel": "xhigh" }
                },
                { "provider": "vendor", "id": "uncontrolled", "reasoning": true },
                { "provider": "vendor", "id": "empty", "reasoning": true, "thinking": { "efforts": [] } },
                { "provider": "vendor", "id": "plain", "reasoning": false, "thinking": { "efforts": ["high"] } }
            ]
        }));
        let options = models[0]["thinkingOptions"].as_array().unwrap();
        assert_eq!(
            options
                .iter()
                .map(|option| option["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["off", "auto", "high", "xhigh"]
        );
        assert_eq!(models[0]["defaultThinkingOptionId"], "xhigh");
        assert_eq!(
            options
                .iter()
                .filter(|option| option["isDefault"] == true)
                .map(|option| option["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["xhigh"]
        );
        for model in &models[1..] {
            assert!(model.get("thinkingOptions").is_none());
            assert!(model.get("defaultThinkingOptionId").is_none());
        }
    }

    #[test]
    fn test_opencode_verbose_catalog_uses_json_metadata_not_bare_ids() {
        let models = crate::kernel::server::opencode_models_from_cli_output(
            r#"
vendor/reasoning
{
  "id": "reasoning",
  "providerID": "vendor",
  "name": "Reasoning Model",
  "capabilities": { "reasoning": true },
  "limit": { "context": 123456 },
  "variants": { "default": {}, "deep": { "reasoningEffort": "high" } }
}
vendor/plain
{"id":"plain","providerID":"vendor","name":"Plain Model","variants":{}}
vendor/bare-only
"#,
        );
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "vendor/reasoning");
        assert_eq!(models[0]["label"], "Reasoning Model");
        assert_eq!(models[0]["contextWindowMaxTokens"], 123456);
        assert_eq!(
            models[0]["thinkingOptions"],
            serde_json::json!([
                { "id": "default", "label": "Default", "isDefault": true },
                { "id": "deep", "label": "deep", "isDefault": false }
            ])
        );
        assert_eq!(models[0]["defaultThinkingOptionId"], "default");
        assert!(models[1].get("thinkingOptions").is_none());
        assert!(models[1].get("defaultThinkingOptionId").is_none());
        assert!(
            crate::kernel::server::opencode_models_from_cli_output("vendor/bare-only\n").is_empty()
        );
    }

    #[test]
    fn test_opencode_plain_catalog_parses_model_lines() {
        let text = r#"
omniroute/kc/openai/gpt-5.5
opencode/deepseek-v4.1-flash-free
# comment
USAGE: opencode models
"#;
        let models = crate::kernel::server::opencode_models_from_plain_cli_output(text);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["id"], "omniroute/kc/openai/gpt-5.5");
        assert_eq!(models[0]["label"], "gpt-5.5");
        assert_eq!(models[0]["provider"], "opencode");
        assert_eq!(models[1]["id"], "opencode/deepseek-v4.1-flash-free");
        assert_eq!(models[1]["label"], "deepseek-v4.1-flash-free");
        assert_eq!(models[1]["provider"], "opencode");
    }

    #[test]
    fn test_opencode_http_catalog_preserves_actual_variant_choices() {
        let models = crate::kernel::server::opencode_models_from_response(&serde_json::json!({
            "connected": ["vendor"],
            "all": [{ "id": "vendor", "models": {
                "reasoning": { "name": "Reasoning", "variants": { "custom": { "reasoningEffort": "high" } } },
                "plain": { "name": "Plain", "variants": {} },
                "nonreasoning": {
                    "name": "Not Reasoning", "capabilities": { "reasoning": false },
                    "variants": { "custom": {} }
                }
            } }]
        }));
        let reasoning = models
            .iter()
            .find(|model| model["id"] == "vendor/reasoning")
            .unwrap();
        assert_eq!(reasoning["thinkingOptions"][1]["id"], "custom");
        assert_eq!(reasoning["defaultThinkingOptionId"], "default");
        let plain = models
            .iter()
            .find(|model| model["id"] == "vendor/plain")
            .unwrap();
        assert!(plain.get("thinkingOptions").is_none());
        let nonreasoning = models
            .iter()
            .find(|model| model["id"] == "vendor/nonreasoning")
            .unwrap();
        assert!(nonreasoning.get("thinkingOptions").is_none());
    }

    #[test]
    fn test_pi_catalog_uses_rpc_models_and_deduplicates() {
        let response = serde_json::json!({
            "models": [
                { "provider": "openai", "id": "runtime-model", "name": "Runtime Model", "contextWindow": 128000 },
                { "provider": "openai", "id": "runtime-model", "name": "Duplicate" },
                { "provider": "", "id": "invalid" },
                { "provider": "anthropic", "name": "Missing ID" }
            ]
        });

        let models = crate::kernel::server::pi_models_from_response(&response);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["provider"], "pi");
        assert_eq!(models[0]["id"], "openai/runtime-model");
        assert_eq!(models[0]["label"], "Runtime Model");
        assert_eq!(models[0]["contextWindowMaxTokens"], 128000);
    }

    #[test]
    fn test_omp_catalog_uses_runtime_models() {
        let response = serde_json::json!({
            "models": [
                { "provider": "openai", "id": "runtime-model", "name": "OMP Model" }
            ]
        });

        let models = crate::kernel::server::omp_models_from_response(&response);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["provider"], "omp");
        assert_eq!(models[0]["id"], "openai/runtime-model");
        assert_eq!(models[0]["label"], "OMP Model");
    }
    #[test]
    fn test_ollama_catalog_uses_runtime_tags() {
        let response = serde_json::json!({
            "models": [
                {
                    "name": "local-coder:latest",
                    "details": { "parameter_size": "7B", "family": "llama" }
                },
                { "name": "local-coder:latest" },
                { "name": "" }
            ]
        });

        let models = crate::kernel::server::ollama_models_from_response(&response);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0]["id"], "ollama/local-coder:latest");
        assert_eq!(models[0]["label"], "local-coder:latest (7B)");
        assert_eq!(
            models[0]["description"],
            "Local Ollama model from the llama family"
        );
        assert_eq!(models[0]["provider"], "ollama");
    }

    #[tokio::test]
    async fn test_claude_model_uses_user_settings_not_catalog_defaults() {
        let dir = tempfile::tempdir().expect("temporary Claude config");
        tokio::fs::write(
            dir.path().join("settings.json"),
            r#"{"model":"user-selected-model"}"#,
        )
        .await
        .expect("write Claude settings");

        assert_eq!(
            crate::kernel::server::read_claude_settings_model(dir.path())
                .await
                .as_deref(),
            Some("user-selected-model")
        );
    }
    #[test]
    fn test_opencode_catalog_includes_only_available_provider_models() {
        let response = serde_json::json!({
            "connected": ["connected-provider"],
            "all": [
                {
                    "id": "connected-provider",
                    "models": {
                        "runtime-a": {
                            "name": "Runtime A",
                            "default": true,
                            "limit": { "context": 96000 }
                        }
                    }
                },
                {
                    "id": "disconnected-provider",
                    "models": { "must-not-show": { "name": "Not connected" } }
                },
                {
                    "id": "api-provider",
                    "source": "api",
                    "models": { "runtime-b": { "name": "Runtime B" } }
                }
            ]
        });

        let models = crate::kernel::server::opencode_models_from_response(&response);
        assert_eq!(models.len(), 2);
        assert!(models.iter().any(|model| {
            model["id"] == "connected-provider/runtime-a"
                && model["isDefault"] == true
                && model["contextWindowMaxTokens"] == 96000
        }));
        assert!(models
            .iter()
            .any(|model| model["id"] == "api-provider/runtime-b"));
        assert!(!models.iter().any(|model| model["id"]
            .as_str()
            .unwrap_or_default()
            .contains("must-not-show")));
    }
    #[test]
    fn test_local_catalog_endpoints_accept_host_and_url_forms() {
        assert_eq!(
            crate::kernel::server::normalize_http_endpoint("localhost:11434/".to_owned()),
            "http://localhost:11434"
        );
        assert_eq!(
            crate::kernel::server::normalize_http_endpoint("https://models.local/".to_owned()),
            "https://models.local"
        );
    }

    /// Copilot's metadata entry always has `models: []`. Availability is determined by
    /// `discover_copilot_available()` at runtime, not by model count. This test verifies
    /// that the metadata structure is correct so the runtime logic can apply it.
    #[test]
    fn test_copilot_metadata_has_empty_models_and_ready_status() {
        let entries = crate::kernel::server::get_provider_snapshot_metadata_entries();
        let copilot = entries
            .iter()
            .find(|e| e.get("provider").and_then(serde_json::Value::as_str) == Some("copilot"));
        assert!(copilot.is_some(), "copilot entry must exist in metadata");
        let copilot = copilot.unwrap();
        assert_eq!(
            copilot
                .get("models")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0),
            "copilot metadata must ship with empty models; runtime fills availability"
        );
        // Metadata status starts as "ready"; get_provider_snapshot_entries() may downgrade
        // it to "unavailable" if the binary is absent. The metadata itself must not pre-set
        // "unavailable" so that binary-present installs stay visible.
        assert_eq!(
            copilot.get("status").and_then(serde_json::Value::as_str),
            Some("ready"),
        );
        // Must have at least the three standard Copilot ACP modes.
        let modes = copilot.get("modes").and_then(serde_json::Value::as_array);
        assert!(
            modes.is_some_and(|m| m.len() >= 3),
            "copilot must have >= 3 modes"
        );
    }
    #[tokio::test]
    async fn test_agent_wire_protocol_full_roundtrip() {
        use crate::kernel::server::dispatch_session_message;

        // 1. Create agent
        let create_req = serde_json::json!({
            "type": "agent.create.request",
            "requestId": "req-agent-create-1",
            "provider": "mock",
            "cwd": "/workspace/test-kronos",
            "title": "Autonomous Tester",
            "model": "gpt-4o"
        });
        let create_res = dispatch_session_message(&create_req).await;
        assert_eq!(create_res["type"], "agent.create.response");
        assert_eq!(create_res["requestId"], "req-agent-create-1");
        let agent = &create_res["payload"]["agent"];
        assert!(agent.is_object());
        let agent_id = agent["id"].as_str().expect("valid agent id");
        assert_eq!(agent["provider"], "mock");
        assert_eq!(agent["title"], "Autonomous Tester");
        assert_eq!(agent["model"], "gpt-4o");

        // 2. Fetch single agent
        let fetch_req = serde_json::json!({
            "type": "fetch_agent_request",
            "requestId": "req-agent-fetch-1",
            "agentId": agent_id
        });
        let fetch_res = dispatch_session_message(&fetch_req).await;
        assert_eq!(fetch_res["type"], "fetch_agent_response");
        assert_eq!(fetch_res["payload"]["agent"]["id"], agent_id);

        // 3. Config, mode, skills, commands acknowledgments (while idle)
        let config_req = serde_json::json!({
            "type": "agent.config.apply.request",
            "requestId": "req-config-1",
            "agentId": agent_id,
            "config": { "mode": "auto" }
        });
        assert_eq!(
            dispatch_session_message(&config_req).await["type"],
            "agent.config.apply.response"
        );

        let mode_req = serde_json::json!({
            "type": "set_agent_mode_request",
            "requestId": "req-mode-1",
            "agentId": agent_id,
            "modeId": "auto"
        });
        assert_eq!(
            dispatch_session_message(&mode_req).await["type"],
            "set_agent_mode_response"
        );

        let model_req = serde_json::json!({
            "type": "set_agent_model_request",
            "requestId": "req-model-1",
            "agentId": agent_id,
            "modelId": "claude-3-5-sonnet"
        });
        let model_res = dispatch_session_message(&model_req).await;
        assert_eq!(model_res["type"], "set_agent_model_response");
        assert_eq!(model_res["payload"]["agentId"], agent_id);
        assert_eq!(model_res["payload"]["accepted"], true);
        assert_eq!(model_res["payload"]["error"], serde_json::Value::Null);
        let verify_model_req = serde_json::json!({
            "type": "fetch_agent_request",
            "requestId": "req-verify-model",
            "agentId": agent_id
        });
        let verify_model_res = dispatch_session_message(&verify_model_req).await;
        assert_eq!(
            verify_model_res["payload"]["agent"]["model"],
            "claude-3-5-sonnet"
        );
        assert_eq!(verify_model_res["payload"]["agent"]["provider"], "claude");

        let opencode_model_req = serde_json::json!({
            "type": "set_agent_model_request",
            "requestId": "req-model-opencode",
            "agentId": agent_id,
            "modelId": "opencode/deepseek-v4.1-flash-free"
        });
        let opencode_res = dispatch_session_message(&opencode_model_req).await;
        assert_eq!(opencode_res["payload"]["accepted"], true);
        let verify_opencode = dispatch_session_message(&verify_model_req).await;
        assert_eq!(verify_opencode["payload"]["agent"]["provider"], "opencode");
        assert_eq!(
            verify_opencode["payload"]["agent"]["model"],
            "opencode/deepseek-v4.1-flash-free"
        );
        let cmd_req = serde_json::json!({
            "type": "list_commands_request",
            "requestId": "req-cmd-1",
            "agentId": agent_id
        });
        assert_eq!(
            dispatch_session_message(&cmd_req).await["type"],
            "list_commands_response"
        );

        // 4. Send message to agent
        let msg_req = serde_json::json!({
            "type": "send_agent_message_request",
            "requestId": "req-agent-msg-1",
            "agentId": agent_id,
            "text": "Hello, execute test suite"
        });
        let msg_res = dispatch_session_message(&msg_req).await;
        assert_eq!(msg_res["type"], "send_agent_message_response");
        assert_eq!(msg_res["payload"]["accepted"], true);
        assert_eq!(msg_res["payload"]["requestId"], "req-agent-msg-1");
        assert_eq!(msg_res["payload"]["agentId"], agent_id);
        // Nullable is required on the wire; indexing alone treats missing keys as null.
        assert_eq!(
            msg_res["payload"].get("error"),
            Some(&serde_json::Value::Null)
        );

        // 5. Fetch agent timeline
        let timeline_req = serde_json::json!({
            "type": "fetch_agent_timeline_request",
            "requestId": "req-agent-timeline-1",
            "agentId": agent_id
        });
        let timeline_res = dispatch_session_message(&timeline_req).await;
        assert_eq!(timeline_res["type"], "fetch_agent_timeline_response");
        let entries = timeline_res["payload"]["entries"]
            .as_array()
            .expect("entries array");
        assert_eq!(entries.len(), 1, "Timeline should have user message");
        assert_eq!(entries[0]["item"]["type"], "user_message");

        let skills_req = serde_json::json!({
            "type": "agent.skills.get_status.request",
            "requestId": "req-skills-1"
        });
        assert_eq!(
            dispatch_session_message(&skills_req).await["type"],
            "agent.skills.get_status.response"
        );

        // 6. Fetch agents list
        let list_req = serde_json::json!({
            "type": "fetch_agents_request",
            "requestId": "req-list-agents-1"
        });
        let list_res = dispatch_session_message(&list_req).await;
        assert_eq!(list_res["type"], "fetch_agents_response");
        let list_entries = list_res["payload"]["entries"]
            .as_array()
            .expect("entries array");
        assert!(list_entries.iter().any(|e| e["agent"]["id"] == agent_id));

        // 7. Cancel agent
        let cancel_req = serde_json::json!({
            "type": "cancel_agent_request",
            "requestId": "req-agent-cancel-1",
            "agentId": agent_id
        });
        let cancel_res = dispatch_session_message(&cancel_req).await;
        assert_eq!(cancel_res["type"], "cancel_agent_response");
        assert_eq!(cancel_res["payload"]["accepted"], true);

        // 8. Delete agent
        let delete_req = serde_json::json!({
            "type": "delete_agent_request",
            "requestId": "req-agent-del-1",
            "agentId": agent_id
        });
        let delete_res = dispatch_session_message(&delete_req).await;
        assert_eq!(delete_res["type"], "agent_deleted");
        assert_eq!(delete_res["payload"]["agentId"], agent_id);
    }

    #[tokio::test]
    async fn test_send_agent_message_rejects_unknown_agent() {
        let agent_id = "missing-send-message-contract-agent";
        let response = dispatch_session_message(&serde_json::json!({
            "type": "send_agent_message_request",
            "requestId": "send-missing-agent",
            "payload": {
                "agentId": agent_id,
                "text": "This message must not be stored"
            }
        }))
        .await;

        assert_eq!(response["type"], "send_agent_message_response");
        assert_eq!(response["payload"]["requestId"], "send-missing-agent");
        assert_eq!(response["payload"]["agentId"], agent_id);
        assert_eq!(response["payload"]["accepted"], false);
        assert!(!response["payload"]["error"]
            .as_str()
            .expect("rejection error string")
            .is_empty());
        assert!(crate::kernel::server::get_agent_manager()
            .get_timeline(agent_id)
            .is_empty());
    }

    #[tokio::test]
    async fn test_fetch_recent_provider_sessions_and_import() {
        let fetch_req = serde_json::json!({
            "type": "fetch_recent_provider_sessions_request",
            "requestId": "req-sessions-test-1",
            "providers": ["opencode", "codex", "pi", "omp", "claude"],
            "limit": 10
        });

        let fetch_res = dispatch_session_message(&fetch_req).await;
        assert_eq!(fetch_res["type"], "fetch_recent_provider_sessions_response");
        assert_eq!(fetch_res["requestId"], "req-sessions-test-1");
        assert_eq!(fetch_res["payload"]["requestId"], "req-sessions-test-1");
        assert!(fetch_res["payload"]["entries"].is_array());
        assert_eq!(fetch_res["payload"]["filteredAlreadyImportedCount"], 0);
        assert_eq!(
            fetch_res["payload"]["providerErrors"]
                .as_array()
                .map(|a| a.len()),
            Some(0)
        );

        // Test import_agent_request
        let import_req = serde_json::json!({
            "type": "import_agent_request",
            "requestId": "req-import-test-1",
            "providerId": "opencode",
            "providerHandleId": "ses_mock_test_123",
            "cwd": "C:/tmp/test",
            "workspaceId": "ws_test_import"
        });

        let import_res = dispatch_session_message(&import_req).await;
        assert_eq!(import_res["type"], "status");
        assert_eq!(import_res["requestId"], "req-import-test-1");
        assert_eq!(import_res["payload"]["status"], "agent_resumed");
        assert_eq!(import_res["payload"]["requestId"], "req-import-test-1");
        assert!(import_res["payload"]["agent"]["id"].is_string());
        let imported_agent_id = import_res["payload"]["agent"]["id"].as_str().unwrap();
        assert!(!imported_agent_id.is_empty());
    }

    #[tokio::test]
    async fn test_thinking_controls_update_snapshot_and_clear_setting() {
        let mgr = crate::kernel::server::get_agent_manager();
        let agent = mgr
            .create_agent(
                "codex",
                ".",
                None,
                None,
                Some(serde_json::json!({"model": "runtime-model", "thinkingOptionId": "high"})),
            )
            .unwrap();
        let initial = crate::kernel::server::agent_record_to_snapshot(&agent);
        assert_eq!(initial["thinkingOptionId"], "high");
        assert_eq!(initial["capabilities"]["supportsReasoningStream"], true);
        let mut updates = crate::kernel::server::get_provider_push_tx().subscribe();
        let response = dispatch_session_message(&serde_json::json!({
            "type": "set_agent_thinking_request", "requestId": "thinking-update",
            "agentId": agent.id, "thinkingOptionId": "low",
        }))
        .await;
        let updated = mgr.get_agent(&agent.id).unwrap();
        let stored = updated.config.as_ref().unwrap()["thinkingOptionId"].clone();
        let published = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match updates.recv().await {
                    Ok(frame) => {
                        let frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
                        if frame["message"]["type"] == "agent_update"
                            && frame["message"]["payload"]["agent"]["id"] == agent.id
                        {
                            break frame;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(error) => panic!("Thinking update channel closed: {error}"),
                }
            }
        })
        .await
        .expect("Thinking update must publish this agent's state");
        let fetched = dispatch_session_message(&serde_json::json!({
            "type": "fetch_agent_request", "requestId": "thinking-fetch", "agentId": agent.id,
        }))
        .await;
        let cleared = dispatch_session_message(&serde_json::json!({
            "type": "set_agent_thinking_request", "requestId": "thinking-clear",
            "payload": {"agentId": agent.id, "thinkingOptionId": null},
        }))
        .await;
        let clear_config = mgr.get_agent(&agent.id).unwrap().config.unwrap();
        mgr.delete_agent(&agent.id).unwrap();
        assert_eq!(response["payload"]["accepted"], true);
        assert_eq!(stored, "low");
        assert_eq!(
            published["message"]["payload"]["agent"]["thinkingOptionId"],
            "low"
        );
        assert_eq!(fetched["payload"]["agent"]["thinkingOptionId"], "low");
        assert_eq!(
            fetched["payload"].get("error"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(cleared["payload"]["accepted"], true);
        assert!(clear_config.get("thinkingOptionId").is_none());
        assert_eq!(clear_config["model"], "runtime-model");
        let rejected = dispatch_session_message(&serde_json::json!({
            "type": "set_agent_thinking_request", "requestId": "thinking-missing",
            "agentId": "missing-thinking-agent", "thinkingOptionId": "high",
        }))
        .await;
        assert_eq!(rejected["payload"]["accepted"], false);
        assert!(rejected["payload"]["error"].is_string());
    }

    #[tokio::test]
    async fn test_thinking_profile_applies_setting_to_existing_agent() {
        let mgr = crate::kernel::server::get_agent_manager();
        let agent = mgr.create_agent("pi", ".", None, None, None).unwrap();
        let response = dispatch_session_message(&serde_json::json!({
            "type": "agent.config.apply.request", "requestId": "thinking-profile",
            "agentId": agent.id, "config": {"thinkingOptionId": "medium"},
        }))
        .await;
        let snapshot =
            crate::kernel::server::agent_record_to_snapshot(&mgr.get_agent(&agent.id).unwrap());
        mgr.delete_agent(&agent.id).unwrap();
        assert_eq!(response["payload"]["accepted"], true);
        assert_eq!(snapshot["thinkingOptionId"], "medium");
    }

    #[tokio::test]
    async fn test_thinking_creation_preserves_sdk_provider_config() {
        let response = dispatch_session_message(&serde_json::json!({
            "type": "create_agent_request", "requestId": "thinking-sdk-create",
            "config": {
                "provider": "pi", "cwd": "C:/thinking-test", "model": "runtime-model",
                "thinkingOptionId": "high",
            },
        }))
        .await;
        let agent = &response["payload"]["agent"];
        let id = agent["id"].as_str().unwrap();
        crate::kernel::server::get_agent_manager()
            .delete_agent(id)
            .unwrap();
        assert_eq!(agent["provider"], "pi");
        assert_eq!(agent["cwd"], "C:/thinking-test");
        assert_eq!(agent["model"], "runtime-model");
        assert_eq!(agent["thinkingOptionId"], "high");
    }
    #[tokio::test]
    async fn test_embedded_daemon_browser_proxy_preserves_single_use_response() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let visits = Arc::new(AtomicUsize::new(0));
        let upstream = axum::Router::new().route("/once", axum::routing::get({
            let visits = visits.clone();
            move || {
                let visits = visits.clone();
                async move {
                    use axum::response::IntoResponse;
                    if visits.fetch_add(1, Ordering::SeqCst) == 0 {
                        axum::response::Html("<h1>One-use private document</h1>").into_response()
                    } else { axum::http::StatusCode::GONE.into_response() }
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move { axum::serve(listener, upstream).await.unwrap(); });
        let daemon = EmbeddedDaemon::start_test(0).await.unwrap();
        let client = reqwest::Client::new();
        let base = format!("http://127.0.0.1:{}", daemon.port);
        let issued: serde_json::Value = client.post(format!("{base}/api/browser-proxy/navigation"))
            .bearer_auth(&daemon.access.local_credential)
            .json(&serde_json::json!({"url":format!("http://127.0.0.1:{port}/once")}))
            .send().await.unwrap().json().await.unwrap();
        let rendered = client.get(format!("{base}{}", issued["url"].as_str().unwrap())).send().await.unwrap();
        assert_eq!(rendered.status(), reqwest::StatusCode::OK);
        assert!(rendered.text().await.unwrap().contains("One-use private document"));
        assert_eq!(visits.load(Ordering::SeqCst), 1);
        daemon.stop().await;
        task.abort();
    }

    #[tokio::test]
    async fn test_embedded_daemon_fetch_agent_history_request() {
        let mgr = crate::kernel::server::get_agent_manager();
        let agent = mgr.create_agent("codex", ".", Some("History Test Agent".to_string()), None, None).unwrap();

        let req = serde_json::json!({
            "type": "fetch_agent_history_request",
            "requestId": "hist-req-1",
            "sort": [{ "key": "updated_at", "direction": "desc" }],
            "page": { "limit": 20 }
        });

        let resp = dispatch_session_message(&req).await;
        assert_eq!(resp["type"], "fetch_agent_history_response");
        assert_eq!(resp["payload"]["requestId"], "hist-req-1");
        let entries = resp["payload"]["entries"].as_array().expect("entries array");
        assert!(entries.iter().any(|e| e["agent"]["id"] == agent.id));
        assert_eq!(resp["payload"]["pageInfo"]["hasMore"], false);

        // Test search matching
        let search_req = serde_json::json!({
            "type": "fetch_agent_history_request",
            "requestId": "hist-req-2",
            "search": "History Test",
            "page": { "limit": 20 }
        });
        let search_resp = dispatch_session_message(&search_req).await;
        let search_entries = search_resp["payload"]["entries"].as_array().unwrap();
        assert!(search_entries.iter().any(|e| e["agent"]["id"] == agent.id));

        // Test search non-matching
        let no_match_req = serde_json::json!({
            "type": "fetch_agent_history_request",
            "requestId": "hist-req-3",
            "search": "NonExistentTermXYZ123",
            "page": { "limit": 20 }
        });
        let no_match_resp = dispatch_session_message(&no_match_req).await;
        let no_match_entries = no_match_resp["payload"]["entries"].as_array().unwrap();
        assert_eq!(no_match_entries.len(), 0);
    }

    #[tokio::test]
    async fn test_embedded_daemon_schedule_lifecycle() {
        // 1. Initial schedule list should succeed
        let list_req = serde_json::json!({
            "type": "schedule/list",
            "requestId": "sched-test-list-1"
        });
        let list_resp = dispatch_session_message(&list_req).await;
        assert_eq!(list_resp["type"], "schedule/list/response");
        assert_eq!(list_resp["payload"]["requestId"], "sched-test-list-1");
        assert!(list_resp["payload"]["error"].is_null());

        // 2. Create schedule
        let create_req = serde_json::json!({
            "type": "schedule/create",
            "requestId": "sched-test-create-1",
            "name": "Hourly Health Check",
            "prompt": "Check all services and report status",
            "cadence": {
                "type": "every",
                "everyMs": 3600000
            },
            "target": {
                "type": "new-agent",
                "config": {
                    "provider": "codex",
                    "cwd": "."
                }
            }
        });
        let create_resp = dispatch_session_message(&create_req).await;
        assert_eq!(create_resp["type"], "schedule/create/response");
        assert!(create_resp["payload"]["error"].is_null());
        let sched = &create_resp["payload"]["schedule"];
        let sched_id = sched["id"].as_str().expect("schedule id string");
        assert_eq!(sched["status"], "active");
        assert_eq!(sched["name"], "Hourly Health Check");

        // 3. Inspect schedule
        let inspect_req = serde_json::json!({
            "type": "schedule/inspect",
            "requestId": "sched-test-inspect-1",
            "scheduleId": sched_id
        });
        let inspect_resp = dispatch_session_message(&inspect_req).await;
        assert_eq!(inspect_resp["type"], "schedule/inspect/response");
        assert_eq!(inspect_resp["payload"]["schedule"]["id"], sched_id);
        assert!(inspect_resp["payload"]["schedule"]["runs"].is_array());

        // 4. Logs
        let logs_req = serde_json::json!({
            "type": "schedule/logs",
            "requestId": "sched-test-logs-1",
            "scheduleId": sched_id
        });
        let logs_resp = dispatch_session_message(&logs_req).await;
        assert_eq!(logs_resp["type"], "schedule/logs/response");
        assert!(logs_resp["payload"]["runs"].is_array());

        // 5. Pause
        let pause_req = serde_json::json!({
            "type": "schedule/pause",
            "requestId": "sched-test-pause-1",
            "scheduleId": sched_id
        });
        let pause_resp = dispatch_session_message(&pause_req).await;
        assert_eq!(pause_resp["type"], "schedule/pause/response");
        assert_eq!(pause_resp["payload"]["schedule"]["status"], "paused");

        // 6. Resume
        let resume_req = serde_json::json!({
            "type": "schedule/resume",
            "requestId": "sched-test-resume-1",
            "scheduleId": sched_id
        });
        let resume_resp = dispatch_session_message(&resume_req).await;
        assert_eq!(resume_resp["type"], "schedule/resume/response");
        assert_eq!(resume_resp["payload"]["schedule"]["status"], "active");

        // 7. Update
        let update_req = serde_json::json!({
            "type": "schedule/update",
            "requestId": "sched-test-update-1",
            "scheduleId": sched_id,
            "name": "Updated Check Name",
            "prompt": "Updated prompt text"
        });
        let update_resp = dispatch_session_message(&update_req).await;
        assert_eq!(update_resp["type"], "schedule/update/response");
        assert_eq!(update_resp["payload"]["schedule"]["name"], "Updated Check Name");
        assert_eq!(update_resp["payload"]["schedule"]["prompt"], "Updated prompt text");

        // 8. Delete
        let delete_req = serde_json::json!({
            "type": "schedule/delete",
            "requestId": "sched-test-delete-1",
            "scheduleId": sched_id
        });
        let delete_resp = dispatch_session_message(&delete_req).await;
        assert_eq!(delete_resp["type"], "schedule/delete/response");
        assert_eq!(delete_resp["payload"]["scheduleId"], sched_id);

        // 9. Inspect deleted should return not found error
        let inspect_deleted_resp = dispatch_session_message(&inspect_req).await;
        assert!(inspect_deleted_resp["payload"]["error"].is_string());
    }

    #[tokio::test]
    async fn test_real_agent_mode_archive_and_metadata_rpc() {
        let create_req = serde_json::json!({
            "type": "agent/create",
            "requestId": "real-agent-create-1",
            "provider": "codex",
            "cwd": ".",
            "title": "Mode & Archive Check"
        });
        let created = dispatch_session_message(&create_req).await;
        let agent_id = created["agent"]["id"]
            .as_str()
            .expect("agent id returned")
            .to_string();

        let mode_resp = dispatch_session_message(&serde_json::json!({
            "type": "set_agent_mode_request",
            "requestId": "real-mode-1",
            "agentId": agent_id,
            "modeId": "architect"
        }))
        .await;
        assert_eq!(mode_resp["type"], "set_agent_mode_response");
        assert_eq!(mode_resp["payload"]["accepted"], true);
        assert!(mode_resp["payload"]["error"].is_null());

        let feat_resp = dispatch_session_message(&serde_json::json!({
            "type": "set_agent_feature_request",
            "requestId": "real-feat-1",
            "agentId": agent_id,
            "featureId": "mcp",
            "value": true
        }))
        .await;
        assert_eq!(feat_resp["type"], "set_agent_feature_response");
        assert_eq!(feat_resp["payload"]["accepted"], true);
        assert!(feat_resp["payload"]["error"].is_null());

        let meta_resp = dispatch_session_message(&serde_json::json!({
            "type": "agent.metadata.update.request",
            "requestId": "real-meta-1",
            "agentId": agent_id,
            "metadata": { "tag": "verified" }
        }))
        .await;
        assert_eq!(meta_resp["type"], "agent.metadata.update.response");
        assert_eq!(meta_resp["payload"]["accepted"], true);
        assert_eq!(meta_resp["payload"]["metadata"]["tag"], "verified");

        let archive_resp = dispatch_session_message(&serde_json::json!({
            "type": "archive_agent_request",
            "requestId": "real-archive-1",
            "agentId": agent_id
        }))
        .await;
        assert_eq!(archive_resp["type"], "agent_archived");
        assert!(archive_resp["payload"]["archivedAt"].is_string());

        let refresh_resp = dispatch_session_message(&serde_json::json!({
            "type": "refresh_agent_request",
            "requestId": "real-refresh-1",
            "agentId": agent_id
        }))
        .await;
        assert_eq!(refresh_resp["type"], "refresh_agent_response");
        assert!(refresh_resp["payload"]["agent"]["archivedAt"].is_string());
    }

    #[tokio::test]
    async fn test_real_creation_subscribe_release_and_checkout_rpc() {
        let host_resp = dispatch_session_message(&serde_json::json!({
            "type": "browser.host.register.request",
            "requestId": "real-host-reg-1",
            "hostKind": "desktop app"
        }))
        .await;
        assert_eq!(host_resp["type"], "browser.host.register.response");
        let sub_id = host_resp["payload"]["subscriptionId"]
            .as_str()
            .expect("subscription id returned")
            .to_string();

        let rel_resp = dispatch_session_message(&serde_json::json!({
            "type": "subscription.release.request",
            "requestId": "real-sub-rel-1",
            "subscriptionId": sub_id
        }))
        .await;
        assert_eq!(rel_resp["type"], "subscription.release.response");

        let val_ok = dispatch_session_message(&serde_json::json!({
            "type": "validate_branch_request",
            "requestId": "real-branch-ok",
            "cwd": ".",
            "branchName": "main"
        }))
        .await;
        assert_eq!(val_ok["type"], "validate_branch_response");
        assert!(val_ok["payload"]["exists"].is_boolean());

        let val_bad = dispatch_session_message(&serde_json::json!({
            "type": "validate_branch_request",
            "requestId": "real-branch-bad",
            "cwd": ".",
            "branchName": "nonexistent-branch-xyz-999"
        }))
        .await;
        assert_eq!(val_bad["type"], "validate_branch_response");
        assert_eq!(val_bad["payload"]["exists"], false);

        let commits_resp = dispatch_session_message(&serde_json::json!({
            "type": "checkout.commits.list.request",
            "requestId": "real-commits-1",
            "cwd": ".",
            "limit": 3
        }))
        .await;
        assert_eq!(commits_resp["type"], "checkout.commits.list.response");
        assert!(commits_resp["payload"]["commits"].is_array());
        assert!(crate::agent::sessions::inspect_omp_session("nonexistent-omp-id-xyz").is_none());
    }

    #[tokio::test]
    async fn test_voice_dictation_pairing_and_provider_controls_rpcs() {
        // 1. Dictation stream start -> chunk -> finish roundtrip
        let start_resp = dispatch_session_message(&serde_json::json!({
            "type": "dictation_stream_start",
            "payload": {
                "dictationId": "dict-test-1",
                "format": "audio/pcm;rate=16000;bits=16"
            }
        }))
        .await;
        assert_eq!(start_resp["type"], "dictation_stream_ack");
        assert_eq!(start_resp["payload"]["dictationId"], "dict-test-1");
        assert_eq!(start_resp["payload"]["ackSeq"], -1);

        // Send 32 bytes (16 silent 16-bit PCM samples) encoded as base64
        let silent_pcm_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &[0u8; 32],
        );
        let chunk_resp = dispatch_session_message(&serde_json::json!({
            "type": "dictation_stream_chunk",
            "payload": {
                "dictationId": "dict-test-1",
                "seq": 0,
                "audio": silent_pcm_b64,
                "format": "audio/pcm;rate=16000;bits=16"
            }
        }))
        .await;
        assert_eq!(chunk_resp["type"], "dictation_stream_ack");
        assert_eq!(chunk_resp["payload"]["ackSeq"], 0);

        let finish_resp = dispatch_session_message(&serde_json::json!({
            "type": "dictation_stream_finish",
            "payload": {
                "dictationId": "dict-test-1",
                "finalSeq": 0
            }
        }))
        .await;
        assert_eq!(finish_resp["type"], "dictation_stream_final");
        assert_eq!(finish_resp["payload"]["dictationId"], "dict-test-1");
        assert!(finish_resp["payload"]["text"].is_string());

        // 2. Realtime Voice Mode toggle
        let voice_resp = dispatch_session_message(&serde_json::json!({
            "type": "set_voice_mode",
            "requestId": "voice-mode-1",
            "enabled": true
        }))
        .await;
        assert_eq!(voice_resp["type"], "set_voice_mode_response");
        assert_eq!(voice_resp["payload"]["enabled"], true);
        assert!(voice_resp["payload"]["error"].is_null());

        // 3. Device Pairing Offer RPC
        let pair_resp = dispatch_session_message(&serde_json::json!({
            "type": "daemon.get_pairing_offer.request",
            "requestId": "pair-offer-1"
        }))
        .await;
        assert_eq!(pair_resp["type"], "daemon.get_pairing_offer.response");
        let pair_url = pair_resp["payload"]["url"].as_str().unwrap_or("");
        assert!(pair_url.starts_with("paseo://pair?endpoint=ws://"));
        let pair_qr = pair_resp["payload"]["qr"].as_str().unwrap_or("");
        assert!(pair_qr.contains('█'));

        // 4. Provider Features & Controls RPCs
        let feat_resp = dispatch_session_message(&serde_json::json!({
            "type": "list_provider_features_request",
            "requestId": "prov-feat-1",
            "provider": "claude"
        }))
        .await;
        assert_eq!(feat_resp["type"], "list_provider_features_response");
        assert!(!feat_resp["payload"]["features"].as_array().unwrap().is_empty());
        assert!(feat_resp["payload"]["fetchedAt"].is_string());

        let ctrl_resp = dispatch_session_message(&serde_json::json!({
            "type": "provider.controls.list.request",
            "requestId": "prov-ctrl-1",
            "provider": "codex"
        }))
        .await;
        assert_eq!(ctrl_resp["type"], "provider.controls.list.response");
        assert!(!ctrl_resp["payload"]["controls"].as_array().unwrap().is_empty());
        assert!(ctrl_resp["payload"]["fetchedAt"].is_string());
    }

    #[tokio::test]
    async fn test_vfs_rpc_diff_and_discard() {
        let test_path = "vfs_rpc_test_file.txt";

        // 1. Write virtual
        let write_resp = dispatch_session_message(&serde_json::json!({
            "type": "vfs/write_virtual",
            "requestId": "vfs-w-1",
            "payload": {
                "path": test_path,
                "content": "line1\nline2 added\n"
            }
        }))
        .await;
        assert_eq!(write_resp["type"], "vfs/write_virtual/response");
        assert_eq!(write_resp["success"], true);

        // 2. Status
        let status_resp = dispatch_session_message(&serde_json::json!({
            "type": "vfs/status",
            "requestId": "vfs-s-1"
        }))
        .await;
        assert_eq!(status_resp["type"], "vfs/status/response");
        assert_eq!(status_resp["success"], true);
        let dirty_count = status_resp["payload"]["dirtyCount"].as_u64().unwrap();
        assert!(dirty_count >= 1);

        // 3. Diff for single file
        let diff_resp = dispatch_session_message(&serde_json::json!({
            "type": "vfs/diff",
            "requestId": "vfs-d-1",
            "payload": {
                "path": test_path
            }
        }))
        .await;
        assert_eq!(diff_resp["type"], "vfs/diff/response");
        assert_eq!(diff_resp["success"], true);
        assert_eq!(diff_resp["payload"]["isVirtual"], true);
        assert_eq!(diff_resp["payload"]["hasChanges"], true);
        assert!(diff_resp["payload"]["diff"].as_str().unwrap().contains("line2 added"));

        // 4. Diff all
        let diff_all_resp = dispatch_session_message(&serde_json::json!({
            "type": "vfs/diff",
            "requestId": "vfs-d-all"
        }))
        .await;
        assert_eq!(diff_all_resp["type"], "vfs/diff/response");
        assert_eq!(diff_all_resp["success"], true);
        assert!(diff_all_resp["payload"]["totalFiles"].as_u64().unwrap() >= 1);

        // 5. Discard single file
        let discard_resp = dispatch_session_message(&serde_json::json!({
            "type": "vfs/discard",
            "requestId": "vfs-disc-1",
            "payload": {
                "path": test_path
            }
        }))
        .await;
        assert_eq!(discard_resp["type"], "vfs/discard/response");
        assert_eq!(discard_resp["success"], true);
        assert_eq!(discard_resp["payload"]["discarded"], true);

        // 6. Discard all
        let discard_all_resp = dispatch_session_message(&serde_json::json!({
            "type": "vfs/discard",
            "requestId": "vfs-disc-all",
            "payload": {
                "all": true
            }
        }))
        .await;
        assert_eq!(discard_all_resp["type"], "vfs/discard/response");
        assert_eq!(discard_all_resp["success"], true);
        assert_eq!(discard_all_resp["payload"]["dirtyCount"], 0);
    }

    #[tokio::test]
    async fn test_lsp_rpc_status_and_lifecycle() {
        // 1. LSP status check
        let status_resp = dispatch_session_message(&serde_json::json!({
            "type": "lsp/status",
            "requestId": "lsp-s-1"
        }))
        .await;
        assert_eq!(status_resp["type"], "lsp/status/response");
        assert_eq!(status_resp["success"], true);
        assert!(status_resp["payload"]["servers"].is_array());

        // 2. LSP stop non-existent
        let stop_resp = dispatch_session_message(&serde_json::json!({
            "type": "lsp/stop",
            "requestId": "lsp-stop-1",
            "payload": {
                "id": "non_existent_server"
            }
        }))
        .await;
        assert_eq!(stop_resp["type"], "lsp/stop/response");
        assert_eq!(stop_resp["success"], true);
        assert_eq!(stop_resp["payload"]["stopped"], false);

        // 3. LSP hover error without running server
        let hover_resp = dispatch_session_message(&serde_json::json!({
            "type": "lsp/hover",
            "requestId": "lsp-h-1",
            "payload": {
                "path": "src/main.rs",
                "line": 10,
                "character": 5
            }
        }))
        .await;
        assert_eq!(hover_resp["type"], "lsp/hover/response");
        assert_eq!(hover_resp["success"], false);
        assert!(hover_resp["error"].as_str().unwrap().contains("No active LSP server"));
    }
}
