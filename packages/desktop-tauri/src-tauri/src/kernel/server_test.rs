#[cfg(test)]
mod tests {
    use crate::kernel::server::EmbeddedDaemon;
    use tokio_tungstenite::connect_async;
    use futures_util::{SinkExt, StreamExt};

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
}
