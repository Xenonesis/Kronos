#[cfg(test)]
mod tests {
    use crate::kernel::server::EmbeddedDaemon;
    use tokio_tungstenite::connect_async;
    use futures_util::{SinkExt, StreamExt};
    use crate::kernel::protocol::RpcEnvelope;
    use crate::kernel::server::{dispatch_envelope, dispatch_session_message};

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
        let inner_str = resp["payload"]["response"].as_str().expect("inner response string");
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
        let daemon = EmbeddedDaemon::start_test(0).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", daemon.port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(msg.to_string())).await.unwrap();

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
        let daemon = EmbeddedDaemon::start_test(6799).await.expect("Failed to start daemon");
        let url = "ws://127.0.0.1:6799";
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

        let msg = serde_json::json!({
            "id": "ping-1",
            "type": "ping",
            "payload": {}
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(msg.to_string())).await.unwrap();

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
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

        // 1. Send Canonical Hello Handshake
        let hello = serde_json::json!({
            "type": "hello",
            "clientId": "cid-integration-test",
            "clientType": "cli",
            "protocolVersion": 1
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(hello.to_string())).await.unwrap();

        let mut parsed: serde_json::Value = serde_json::Value::Null;
        for _ in 0..10 {
            if let Ok(Some(Ok(msg))) = tokio::time::timeout(std::time::Duration::from_millis(1000), ws_stream.next()).await {
                if let Ok(text) = msg.to_text() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
                        if val.get("message").and_then(|m| m.get("payload")).and_then(|p| p.get("serverId")).and_then(|v| v.as_str()) == Some("kronos-embedded-daemon") {
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
        assert_eq!(parsed["message"]["payload"]["serverId"], "kronos-embedded-daemon");

        // 2. Send Simple Ping
        let ping = serde_json::json!({ "type": "ping" });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(ping.to_string())).await.unwrap();
        let mut parsed_pong: serde_json::Value = serde_json::Value::Null;
        for _ in 0..10 {
            if let Ok(Some(Ok(msg))) = tokio::time::timeout(std::time::Duration::from_millis(500), ws_stream.next()).await {
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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(tele_query.to_string())).await.unwrap();

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
        let daemon = EmbeddedDaemon::start_test(6797).await.expect("Failed to start daemon");
        let url = "ws://127.0.0.1:6797";
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

        let msg = serde_json::json!({
            "id": "req-unknown-1",
            "type": "workspace.unimplemented",
            "payload": {}
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(msg.to_string())).await.unwrap();

        let response = ws_stream.next().await.unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(parsed["id"], "req-unknown-1");
        assert_eq!(parsed["success"], false);
        assert!(parsed["error"].is_string());
        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_directory_suggestions_dispatch() {
        let port = 6794;
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(dir_query.to_string())).await.unwrap();

        let dir_resp = ws_stream.next().await.unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(dir_resp.to_text().unwrap()).unwrap();
        assert_eq!(parsed["type"], "session");
        assert_eq!(parsed["message"]["type"], "directory_suggestions_response");
        assert_eq!(parsed["message"]["payload"]["requestId"], "req-dir-sugg-test");
        assert!(parsed["message"]["payload"]["error"].is_null());

        let entries = parsed["message"]["payload"]["entries"].as_array().expect("entries array");
        assert!(!entries.is_empty(), "Should return matching search.rs entry");
        let matched = entries.iter().any(|e| {
            e["path"].as_str().map(|p| p.ends_with("search.rs")).unwrap_or(false)
        });
        assert!(matched, "Expected search.rs in returned suggestions payload");

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
            e["path"].as_str().map(|p| p.ends_with("search.rs")).unwrap_or(false)
        });
        assert!(matched, "Expected search.rs in returned suggestions payload");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_embedded_daemon_named_pipe_ipc() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::windows::named_pipe::ClientOptions;

        let port = 6796;
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let pipe_name = format!(r"\\.\pipe\kronos-ipc-{}", port);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = ClientOptions::new().open(&pipe_name).expect("Failed to open pipe");
        let (reader, mut writer) = tokio::io::split(client);
        let mut lines = BufReader::new(reader).lines();

        let msg = serde_json::json!({
            "id": "pipe-ping-1",
            "type": "ping",
            "payload": {}
        });
        writer.write_all(format!("{}\n", msg).as_bytes()).await.unwrap();

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
        let sub_id = watch_res.data.unwrap()["subscriptionId"].as_str().unwrap().to_string();
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
        let daemon = EmbeddedDaemon::start_test(0).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", daemon.port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

        let temp = tempfile::tempdir().expect("tempdir");
        let watch_msg = serde_json::json!({
            "id": "ws-watch-1",
            "type": "vfs/watch",
            "payload": {
                "path": temp.path().to_str().unwrap()
            }
        });

        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(watch_msg.to_string())).await.unwrap();

        let parsed_reply = wait_for_rpc_response(&mut ws_stream, "ws-watch-1").await;
        assert_eq!(parsed_reply["id"], "ws-watch-1");
        assert_eq!(parsed_reply["success"], true);
        let sub_id = parsed_reply["data"]["subscriptionId"].as_str().unwrap().to_string();
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

        assert!(received_event, "Expected to receive vfs/change_events frame over WebSocket");

        // Unwatch
        let unwatch_msg = serde_json::json!({
            "id": "ws-unwatch-1",
            "type": "vfs/unwatch",
            "payload": {
                "subscriptionId": sub_id
            }
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(unwatch_msg.to_string())).await.unwrap();

        let unwatch_reply = ws_stream.next().await.unwrap().unwrap();
        let parsed_unwatch: serde_json::Value = serde_json::from_str(unwatch_reply.to_text().unwrap()).unwrap();
        assert_eq!(parsed_unwatch["id"], "ws-unwatch-1");
        assert_eq!(parsed_unwatch["success"], true);

        daemon.stop().await;
    }

    async fn wait_for_rpc_response<S>(ws_stream: &mut S, expected_id: &str) -> serde_json::Value
    where
        S: futures_util::StreamExt<Item = Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    {
        while let Some(Ok(frame)) = ws_stream.next().await {
            if let Ok(text) = frame.to_text() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                    let id_opt = json.get("id")
                        .or_else(|| json.get("requestId"))
                        .or_else(|| json.get("message").and_then(|m| m.get("id").or_else(|| m.get("requestId"))))
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
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(analyze_envelope.to_string())).await.unwrap();

        let parsed_analyze = wait_for_rpc_response(&mut ws_stream, "ast-analyze-1").await;
        assert_eq!(parsed_analyze["id"], "ast-analyze-1");
        assert_eq!(parsed_analyze["success"], true);
        let symbols = parsed_analyze.get("data")
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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(symbols_envelope.to_string())).await.unwrap();

        let parsed_symbols = wait_for_rpc_response(&mut ws_stream, "ast-symbols-1").await;
        assert_eq!(parsed_symbols["id"], "ast-symbols-1");
        assert_eq!(parsed_symbols["success"], true);
        let symbols_list = parsed_symbols.get("data")
            .or_else(|| parsed_symbols.get("payload"))
            .and_then(|v| v.as_array())
            .expect("symbols payload array");
        assert_eq!(symbols_list.len(), 2);

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_crdt_realtime_collaboration() {
        let port = 6789;
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);

        // 1. Connect Client 1 (Human Editor)
        let (mut client1, _) = connect_async(&url).await.expect("Client 1 connect");

        // 2. Connect Client 2 (Agent A)
        let (mut client2, _) = connect_async(&url).await.expect("Client 2 connect");

        let room_id = "test-collab-room";

        // Client 1 joins the room
        let join_msg = serde_json::json!({
            "id": "crdt-join-1",
            "type": "crdt/join",
            "payload": {
                "room": room_id
            }
        });
        client1.send(tokio_tungstenite::tungstenite::Message::Text(join_msg.to_string())).await.unwrap();
        let join_reply = client1.next().await.unwrap().unwrap();
        let parsed_join: serde_json::Value = serde_json::from_str(join_reply.to_text().unwrap()).unwrap();
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
        client2.send(tokio_tungstenite::tungstenite::Message::Text(edit_msg.to_string())).await.unwrap();
        let edit_reply = client2.next().await.unwrap().unwrap();
        let parsed_edit: serde_json::Value = serde_json::from_str(edit_reply.to_text().unwrap()).unwrap();
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
        assert!(received_crdt_event, "Client 1 should receive real-time CRDT update event from Agent A");

        // Client 1 queries room state
        let state_msg = serde_json::json!({
            "id": "crdt-state-1",
            "type": "crdt/state",
            "payload": {
                "room": room_id
            }
        });
        client1.send(tokio_tungstenite::tungstenite::Message::Text(state_msg.to_string())).await.unwrap();
        let state_reply = client1.next().await.unwrap().unwrap();
        let parsed_state: serde_json::Value = serde_json::from_str(state_reply.to_text().unwrap()).unwrap();
        assert_eq!(parsed_state["id"], "crdt-state-1");
        assert_eq!(parsed_state["success"], true);
        let state_data = parsed_state.get("data").or_else(|| parsed_state.get("payload")).unwrap();
        assert_eq!(state_data["text"], "const kernelStatus = 'READY';");

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_ioring_envelopes() {
        let port = 6788;
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(&url).await.expect("connect");

        // 1. Test ioring/status
        let status_msg = serde_json::json!({
            "id": "ioring-status-1",
            "type": "ioring/status",
            "payload": {}
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(status_msg.to_string())).await.unwrap();
        let parsed_status = wait_for_rpc_response(&mut ws_stream, "ioring-status-1").await;
        assert_eq!(parsed_status["success"], true);
        let status_data = parsed_status.get("data").or_else(|| parsed_status.get("payload")).unwrap();
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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(read_batch_msg.to_string())).await.unwrap();
        let parsed_read = wait_for_rpc_response(&mut ws_stream, "ioring-read-1").await;
        assert_eq!(parsed_read["success"], true);
        let files = parsed_read.get("data")
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
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(&url).await.expect("connect");

        // 1. Test speech/status
        let status_msg = serde_json::json!({
            "id": "speech-status-1",
            "type": "speech/status",
            "payload": {}
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(status_msg.to_string())).await.unwrap();
        let parsed_status = wait_for_rpc_response(&mut ws_stream, "speech-status-1").await;
        assert_eq!(parsed_status["success"], true);
        let status_data = parsed_status.get("data").or_else(|| parsed_status.get("payload")).unwrap();
        assert_eq!(status_data["targetSampleRate"], 16000);
        assert_eq!(status_data["targetChannels"], 1);

        // 2. Test speech/transcribe with synthetic voiced samples
        let synthetic_samples: Vec<f64> = (0..16000)
            .map(|i| (i as f64 * 0.05).sin() * 0.75)
            .collect();

        let transcribe_msg = serde_json::json!({
            "id": "speech-transcribe-1",
            "type": "speech/transcribe",
            "payload": {
                "samples": synthetic_samples,
                "sampleRate": 16000,
                "channels": 1
            }
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(transcribe_msg.to_string())).await.unwrap();
        let parsed_trans = wait_for_rpc_response(&mut ws_stream, "speech-transcribe-1").await;
        assert_eq!(parsed_trans["success"], true);
        let trans_data = parsed_trans.get("data").or_else(|| parsed_trans.get("payload")).unwrap();
        assert!(!trans_data["transcript"].as_str().unwrap().is_empty());
        assert_eq!(trans_data["samplesCount"], 16000);

        daemon.stop().await;
    }

    #[tokio::test]
    async fn test_embedded_daemon_shm_envelopes() {
        let port = 6786;
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(&url).await.expect("connect");

        // 1. Test shm/status
        let status_msg = serde_json::json!({
            "id": "shm-status-1",
            "type": "shm/status",
            "payload": {}
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(status_msg.to_string())).await.unwrap();
        let parsed_status = wait_for_rpc_response(&mut ws_stream, "shm-status-1").await;
        assert_eq!(parsed_status["success"], true);
        let status_data = parsed_status.get("data").or_else(|| parsed_status.get("payload")).unwrap();
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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(push_msg.to_string())).await.unwrap();
        let parsed_push = wait_for_rpc_response(&mut ws_stream, "shm-push-1").await;
        assert_eq!(parsed_push["success"], true);
        let push_data = parsed_push.get("data").or_else(|| parsed_push.get("payload")).unwrap();
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
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(poll_msg.to_string())).await.unwrap();
        let parsed_poll = wait_for_rpc_response(&mut ws_stream, "shm-poll-1").await;
        assert_eq!(parsed_poll["success"], true);
        let records = parsed_poll.get("data")
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
        assert_eq!(resp["payload"]["requestType"], "project.unknown_unsupported.request");
        assert_eq!(resp["payload"]["code"], "not_implemented");
    }

    #[tokio::test]
    async fn test_embedded_daemon_truthful_features() {
        let server_info = crate::kernel::protocol::build_server_info_payload();
        let features = &server_info["features"];
        // Unimplemented features must be explicitly false until ported
        assert_eq!(features["connectedHosts"], false);
        assert_eq!(features["projectAdd"], false);
        assert_eq!(features["workspaceGithubRepositorySearch"], false);
        // Kernel modules must be true
        assert_eq!(features["kernelTelemetry"], true);
        assert_eq!(features["kernelAst"], true);
        assert_eq!(features["kernelCrdt"], true);
        assert_eq!(features["kernelIoRing"], true);
        assert_eq!(features["kernelShm"], true);
        assert_eq!(features["kernelSpeech"], true);
    }

    #[tokio::test]
    async fn test_embedded_daemon_browser_proxy_endpoints() {
        let port = 6792;
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");

        let client = reqwest::Client::new();

        // 1. Test POST /api/browser-proxy/cookies
        let cookies_resp = client
            .post(format!("http://127.0.0.1:{port}/api/browser-proxy/cookies"))
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
            .send()
            .await
            .expect("Failed to call browser proxy");
        assert_eq!(missing_url_resp.status(), reqwest::StatusCode::BAD_REQUEST);

        daemon.stop().await;
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
        assert_eq!(std::fs::read_to_string(root.join("hello.txt")).unwrap(), "Rust Kernel VFS File Service");

        // 3. File explorer request
        let explore_req = serde_json::json!({
            "type": "file_explorer_request",
            "requestId": "req-explore-1",
            "cwd": root_str
        });
        let explore_resp = dispatch_session_message(&explore_req).await;
        assert_eq!(explore_resp["type"], "file_explorer_response");
        assert_eq!(explore_resp["success"], true);
        let entries = explore_resp["payload"]["entries"].as_array().expect("entries array");
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
        let _ = tokio::process::Command::new("git").args(&["config", "user.name", "Kronos Test"]).current_dir(root).output().await;
        let _ = tokio::process::Command::new("git").args(&["config", "user.email", "test@kronos.local"]).current_dir(root).output().await;

        // Create and commit a file
        std::fs::write(root.join("file1.txt"), b"line1\n").unwrap();
        let _ = tokio::process::Command::new("git").args(&["add", "file1.txt"]).current_dir(root).output().await;
        let _ = tokio::process::Command::new("git").args(&["commit", "-m", "init commit"]).current_dir(root).output().await;

        // 1. Test git/branches wire call
        let branches_req = serde_json::json!({
            "type": "git/branches",
            "requestId": "req-branches-1",
            "cwd": root_str
        });
        let branches_resp = dispatch_session_message(&branches_req).await;
        assert_eq!(branches_resp["type"], "git/branches/response");
        assert_eq!(branches_resp["success"], true);
        let branches = branches_resp["payload"]["branches"].as_array().expect("branches array");
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
        let wt_list = wt_resp["payload"]["worktrees"].as_array().expect("worktrees array");
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
        let providers = prov_resp["payload"]["providers"].as_array().expect("providers array");
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
        let items = time_resp["payload"]["items"].as_array().expect("timeline items");
        assert_eq!(items.len(), 1);

        // 5. Query plugin usage metrics
        let usage_req = serde_json::json!({
            "type": "plugins/usage",
            "requestId": "req-usage-1"
        });
        let usage_resp = dispatch_session_message(&usage_req).await;
        assert_eq!(usage_resp["type"], "plugins/usage/response");
        assert_eq!(usage_resp["success"], true);
        let metrics = usage_resp["payload"]["metrics"].as_array().expect("metrics array");
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
        let projects = list_resp["payload"]["projects"].as_array().expect("projects array");
        assert!(projects.iter().any(|p| p["projectDisplayName"] == "my-test-project" && p["projectIconRevision"] == "automatic:none:v1"));

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

        // 5. Agent create request
        let ag_create_req = serde_json::json!({
            "type": "agent.create.request",
            "requestId": "req-ag-create-1",
            "workspaceId": created_ws["id"].as_str().unwrap()
        });
        let ag_create_resp = dispatch_session_message(&ag_create_req).await;
        assert_eq!(ag_create_resp["type"], "agent.create.response");
        assert!(ag_create_resp["payload"]["error"].is_null());
        let created_ag = &ag_create_resp["payload"]["agent"];
        assert!(created_ag["id"].as_str().unwrap().starts_with("ag_"));
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
        assert_eq!(sub_events_resp["type"], "session.events.set_subscription.response");
        assert_eq!(sub_events_resp["payload"]["requestId"], "req-sub-events-1");
        assert!(sub_events_resp["payload"]["subscriptionId"].as_str().unwrap().starts_with("sub-events-"));

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
    }
}
