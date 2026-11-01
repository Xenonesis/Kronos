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

        let response = ws_stream.next().await.unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(response.to_text().unwrap()).unwrap();
        assert_eq!(parsed["type"], "session");
        assert_eq!(parsed["message"]["type"], "status");
        assert_eq!(parsed["message"]["payload"]["status"], "server_info");
        assert_eq!(parsed["message"]["payload"]["protocolVersion"], 1);
        assert_eq!(parsed["message"]["payload"]["serverId"], "kronos-embedded-daemon");

        // 2. Send Simple Ping
        let ping = serde_json::json!({ "type": "ping" });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(ping.to_string())).await.unwrap();

        let pong_resp = ws_stream.next().await.unwrap().unwrap();
        let parsed_pong: serde_json::Value = serde_json::from_str(pong_resp.to_text().unwrap()).unwrap();
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

        let tele_resp = ws_stream.next().await.unwrap().unwrap();
        let parsed_tele: serde_json::Value = serde_json::from_str(tele_resp.to_text().unwrap()).unwrap();
        assert_eq!(parsed_tele["type"], "session");
        assert_eq!(parsed_tele["message"]["type"], "kernel/telemetry/response");
        assert_eq!(parsed_tele["message"]["requestId"], "req-tele-test");
        assert!(parsed_tele["message"]["payload"]["uptime_secs"].is_number());

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

        let watch_reply = ws_stream.next().await.unwrap().unwrap();
        let parsed_reply: serde_json::Value = serde_json::from_str(watch_reply.to_text().unwrap()).unwrap();
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

        let analyze_resp = ws_stream.next().await.unwrap().unwrap();
        let parsed_analyze: serde_json::Value = serde_json::from_str(analyze_resp.to_text().unwrap()).unwrap();
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

        let symbols_resp = ws_stream.next().await.unwrap().unwrap();
        let parsed_symbols: serde_json::Value = serde_json::from_str(symbols_resp.to_text().unwrap()).unwrap();
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
}
