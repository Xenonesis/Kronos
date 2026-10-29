use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    routing::get,
    Router,
};
use tokio::sync::{broadcast, Mutex};
use crate::kernel::protocol::{RpcEnvelope, RpcResponse};

pub async fn health_check() -> &'static str {
    "OK"
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
        other => RpcResponse::err(
            envelope.id,
            other,
            format!("Method '{}' not implemented", other),
        ),
    }
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
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<RpcEnvelope>(&text) {
                            Ok(envelope) => {
                                let resp = dispatch_envelope(envelope).await;
                                if let Ok(json) = serde_json::to_string(&resp) {
                                    if socket.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Err(err) => {
                                let err_resp = RpcResponse::err("unknown", "error", format!("Invalid RPC envelope: {}", err));
                                if let Ok(json) = serde_json::to_string(&err_resp) {
                                    let _ = socket.send(Message::Text(json.into())).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            if let Ok(envelope) = serde_json::from_str::<RpcEnvelope>(text) {
                                let resp = dispatch_envelope(envelope).await;
                                if let Ok(json) = serde_json::to_string(&resp) {
                                    if socket.send(Message::Text(json.into())).await.is_err() {
                                        break;
                                    }
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
}

#[cfg(not(windows))]
async fn run_unix_socket(sock_path: String, mut shutdown_rx: broadcast::Receiver<()>) {
    use tokio::net::UnixListener;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let _ = std::fs::remove_file(&sock_path);
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
    let _ = std::fs::remove_file(&sock_path);
}
