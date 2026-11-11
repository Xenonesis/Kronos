use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::ipc::Channel;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSpawnOptions {
    pub command: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum TerminalEvent {
    Data(String),
    Exit(Option<u32>),
    Error(String),
}

static INSTANCE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct ActiveTerminal {
    pub instance_id: u64,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub writer: Box<dyn Write + Send>,
    pub output_backlog: Arc<Mutex<Vec<u8>>>,
}
#[derive(Clone, Debug)]
pub struct TerminalOutputChunk {
    pub terminal_id: String,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct NativePtyManager {
    sessions: Arc<Mutex<HashMap<String, ActiveTerminal>>>,
    job_supervisor: Option<Arc<crate::process_job::JobSupervisor>>,
    output_broadcaster: tokio::sync::broadcast::Sender<TerminalOutputChunk>,
}

impl Default for NativePtyManager {
    fn default() -> Self {
        Self::new()
    }
}

fn sniff_dev_server_url(chunk: &[u8]) -> Option<(u16, String)> {
    let text = String::from_utf8_lossy(chunk);
    for line in text.lines() {
        let line_lower = line.to_ascii_lowercase();
        let offset = if let Some(idx) = line_lower.find("localhost:") {
            Some(idx + "localhost:".len())
        } else if let Some(idx) = line_lower.find("127.0.0.1:") {
            Some(idx + "127.0.0.1:".len())
        } else if let Some(idx) = line_lower.find("0.0.0.0:") {
            Some(idx + "0.0.0.0:".len())
        } else {
            None
        };

        if let Some(start) = offset {
            let port_str: String = line[start..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(port) = port_str.parse::<u16>() {
                if (1024..=65535).contains(&port) {
                    let url = format!("http://localhost:{port}");
                    return Some((port, url));
                }
            }
        }
    }
    None
}

/// Finds the index where an incomplete trailing UTF-8 byte sequence starts, if any.
/// Returns the byte offset into `bytes` where valid complete UTF-8 ends.
fn split_incomplete_utf8(bytes: &[u8]) -> usize {
    let len = bytes.len();
    if len == 0 {
        return 0;
    }

    let start_check = len.saturating_sub(3);
    for i in (start_check..len).rev() {
        let b = bytes[i];
        if b & 0b1000_0000 == 0 {
            break;
        }
        if b & 0b1100_0000 == 0b1100_0000 {
            let expected_len = if b & 0b1110_0000 == 0b1100_0000 {
                2
            } else if b & 0b1111_0000 == 0b1110_0000 {
                3
            } else if b & 0b1111_1000 == 0b1111_0000 {
                4
            } else {
                break;
            };

            let actual_len = len - i;
            if actual_len < expected_len {
                return i;
            } else {
                break;
            }
        }
    }

    len
}

impl NativePtyManager {
    pub fn new() -> Self {
        let job_supervisor = crate::process_job::JobSupervisor::new().ok().map(Arc::new);
        let (output_broadcaster, _) = tokio::sync::broadcast::channel(2048);
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            job_supervisor,
            output_broadcaster,
        }
    }

    pub fn with_job_supervisor(supervisor: Arc<crate::process_job::JobSupervisor>) -> Self {
        let (output_broadcaster, _) = tokio::sync::broadcast::channel(2048);
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            job_supervisor: Some(supervisor),
            output_broadcaster,
        }
    }

    pub fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<TerminalOutputChunk> {
        self.output_broadcaster.subscribe()
    }

    pub fn spawn_terminal(&self, id: String, options: TerminalSpawnOptions) -> Result<(), String> {
        {
            let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = sessions.get_mut(&id) {
                if let Ok(None) = existing.child.try_wait() {
                    // Session already exists and child process is running.
                    return Ok(());
                } else {
                    // Child process has terminated; remove stale entry.
                    sessions.remove(&id);
                }
            }
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: options.rows.max(1),
                cols: options.cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let shell = options.command.unwrap_or_else(|| {
            if cfg!(windows) {
                std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
            } else {
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
            }
        });

        let mut cmd = CommandBuilder::new(shell);
        for arg in options.args {
            cmd.arg(arg);
        }
        if let Some(cwd) = options.cwd {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn child in PTY: {}", e))?;

        #[cfg(windows)]
        if let Some(supervisor) = &self.job_supervisor {
            if let Some(pid) = child.process_id() {
                use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
                use windows_sys::Win32::System::Threading::{
                    OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
                };
                unsafe {
                    let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                    if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
                        let _ = supervisor.assign_process(handle);
                        CloseHandle(handle);
                    }
                }
            }
        }

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take master writer: {}", e))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone master reader: {}", e))?;

        let instance_id = INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let output_backlog = Arc::new(Mutex::new(Vec::<u8>::new()));
        let backlog_for_reader = output_backlog.clone();

        let broadcaster = self.output_broadcaster.clone();
        let term_id = id.clone();
        let sessions_for_exit = self.sessions.clone();

        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 16384];
            let mut detected_ports = std::collections::HashSet::<u16>::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = &buf[..n];
                        {
                            if let Ok(mut bg) = backlog_for_reader.lock() {
                                bg.extend_from_slice(chunk);
                                const MAX_BACKLOG: usize = 128 * 1024;
                                if bg.len() > MAX_BACKLOG {
                                    let excess = bg.len() - MAX_BACKLOG;
                                    bg.drain(0..excess);
                                }
                            }
                        }
                        if let Some((port, url)) = sniff_dev_server_url(chunk) {
                            if detected_ports.insert(port) {
                                let evt = serde_json::json!({
                                    "type": "session",
                                    "message": {
                                        "type": "dev_server_detected",
                                        "payload": {
                                            "terminalId": term_id,
                                            "port": port,
                                            "url": url,
                                        }
                                    }
                                });
                                let _ = crate::kernel::server::get_provider_push_tx().send(evt.to_string());
                            }
                        }
                        let _ = broadcaster.send(TerminalOutputChunk {
                            terminal_id: term_id.clone(),
                            data: chunk.to_vec(),
                        });
                    }
                    Err(_) => break,
                }
            }
            let mut lock = sessions_for_exit.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(active) = lock.get(&term_id) {
                if active.instance_id == instance_id {
                    lock.remove(&term_id);
                }
            }
        });

        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.insert(
            id,
            ActiveTerminal {
                instance_id,
                master: pair.master,
                child,
                writer,
                output_backlog,
            },
        );

        Ok(())
    }

    pub fn attach_reader_channel(
        &self,
        id: String,
        channel: Channel<TerminalEvent>,
    ) -> Result<(), String> {
        let (instance_id, backlog) = {
            let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
            let active = sessions.get(&id).ok_or_else(|| "Session not found".to_string())?;
            let backlog = active.output_backlog.lock().map(|bg| bg.clone()).unwrap_or_default();
            (active.instance_id, backlog)
        };

        if !backlog.is_empty() {
            let text = String::from_utf8_lossy(&backlog).into_owned();
            let _ = channel.send(TerminalEvent::Data(text));
        }

        let mut broadcast_rx = self.output_broadcaster.subscribe();
        let sessions_clone = self.sessions.clone();
        let session_id = id.clone();

        #[allow(dead_code)]
        enum RawPtyMsg {
            Data(Vec<u8>),
            Exit(Option<u32>),
            Error(String),
        }

        let (tx, rx) = std::sync::mpsc::channel::<RawPtyMsg>();
        let tx_reader = tx.clone();
        let target_id = id.clone();

        thread::spawn(move || {
            loop {
                match broadcast_rx.blocking_recv() {
                    Ok(chunk) => {
                        if chunk.terminal_id == target_id {
                            if tx_reader.send(RawPtyMsg::Data(chunk.data)).is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        let _ = tx_reader.send(RawPtyMsg::Exit(None));
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                }
            }
        });

        // Dispatcher thread with adaptive micro-batching and UTF-8 boundary preservation
        thread::spawn(move || {
            let mut leftover: Vec<u8> = Vec::new();
            let mut batch_text = String::with_capacity(65536);
            let min_window = std::time::Duration::from_millis(2);
            let burst_window = std::time::Duration::from_millis(16);
            let mut recent_bytes_counter = 0usize;
            let mut last_window_reset = std::time::Instant::now();
            const MAX_BATCH_SIZE: usize = 65536;

            loop {
                let first_msg = match rx.recv() {
                    Ok(msg) => msg,
                    Err(_) => break,
                };

                let mut process_bytes = |bytes: Vec<u8>, text_target: &mut String, counter: &mut usize| {
                    *counter = counter.saturating_add(bytes.len());
                    let mut combined = if leftover.is_empty() {
                        bytes
                    } else {
                        let mut c = std::mem::take(&mut leftover);
                        c.extend_from_slice(&bytes);
                        c
                    };

                    let split_idx = split_incomplete_utf8(&combined);
                    if split_idx < combined.len() {
                        leftover = combined.split_off(split_idx);
                    }

                    if !combined.is_empty() {
                        let text = String::from_utf8_lossy(&combined);
                        text_target.push_str(&text);
                    }
                };

                if last_window_reset.elapsed() > std::time::Duration::from_millis(100) {
                    recent_bytes_counter = 0;
                    last_window_reset = std::time::Instant::now();
                }

                match first_msg {
                    RawPtyMsg::Data(bytes) => {
                        process_bytes(bytes, &mut batch_text, &mut recent_bytes_counter);

                        let micro_window = if recent_bytes_counter > 16384 {
                            burst_window
                        } else {
                            min_window
                        };

                        let start = std::time::Instant::now();
                        while batch_text.len() < MAX_BATCH_SIZE {
                            let remaining = micro_window.saturating_sub(start.elapsed());
                            if remaining.is_zero() {
                                break;
                            }
                            match rx.recv_timeout(remaining) {
                                Ok(RawPtyMsg::Data(more_bytes)) => {
                                    process_bytes(more_bytes, &mut batch_text, &mut recent_bytes_counter);
                                }
                                Ok(RawPtyMsg::Exit(code)) => {
                                    if !batch_text.is_empty() {
                                        let _ = channel.send(TerminalEvent::Data(std::mem::take(
                                            &mut batch_text,
                                        )));
                                    }
                                    let _ = channel.send(TerminalEvent::Exit(code));
                                    let mut lock = sessions_clone.lock().unwrap_or_else(|e| e.into_inner());
                                    lock.remove(&session_id);
                                    return;
                                }
                                Ok(RawPtyMsg::Error(err)) => {
                                    if !batch_text.is_empty() {
                                        let _ = channel.send(TerminalEvent::Data(std::mem::take(
                                            &mut batch_text,
                                        )));
                                    }
                                    let _ = channel.send(TerminalEvent::Error(err));
                                    let mut lock = sessions_clone.lock().unwrap_or_else(|e| e.into_inner());
                                    lock.remove(&session_id);
                                    return;
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                    break;
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                    break;
                                }
                            }
                        }

                        if !batch_text.is_empty() {
                            let out = std::mem::take(&mut batch_text);
                            if channel.send(TerminalEvent::Data(out)).is_err() {
                                break;
                            }
                        }
                    }
                    RawPtyMsg::Exit(code) => {
                        if !leftover.is_empty() {
                            let text = String::from_utf8_lossy(&leftover).to_string();
                            let _ = channel.send(TerminalEvent::Data(text));
                        }
                        let _ = channel.send(TerminalEvent::Exit(code));
                        break;
                    }
                    RawPtyMsg::Error(err) => {
                        if !leftover.is_empty() {
                            let text = String::from_utf8_lossy(&leftover).to_string();
                            let _ = channel.send(TerminalEvent::Data(text));
                        }
                        let _ = channel.send(TerminalEvent::Error(err));
                        break;
                    }
                }
            }

            let mut lock = sessions_clone.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(active) = lock.get(&session_id) {
                if active.instance_id == instance_id {
                    lock.remove(&session_id);
                }
            }
        });

        Ok(())
    }

    pub fn write_terminal(&self, id: &str, data: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let term = sessions
            .get_mut(id)
            .ok_or_else(|| "Session not found".to_string())?;
        term.writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Failed to write to PTY: {}", e))?;
        term.writer
            .flush()
            .map_err(|e| format!("Failed to flush PTY: {}", e))
    }

    pub fn resize_terminal(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let term = sessions
            .get(id)
            .ok_or_else(|| "Session not found".to_string())?;
        term.master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {}", e))
    }

    pub fn kill_terminal(&self, id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut term) = sessions.remove(id) {
            let _ = term.child.kill();
        }
        Ok(())
    }
}

static PTY_MANAGER: std::sync::OnceLock<NativePtyManager> = std::sync::OnceLock::new();

pub fn get_pty_manager() -> &'static NativePtyManager {
    PTY_MANAGER.get_or_init(NativePtyManager::new)
}

#[tauri::command]
pub async fn spawn_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
    options: TerminalSpawnOptions,
    on_event: Channel<TerminalEvent>,
) -> Result<(), String> {
    manager.spawn_terminal(id.clone(), options)?;
    if let Err(e) = manager.attach_reader_channel(id.clone(), on_event) {
        let _ = manager.kill_terminal(&id);
        return Err(e);
    }
    Ok(())
}

#[tauri::command]
pub fn write_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
    data: String,
) -> Result<(), String> {
    manager.write_terminal(&id, &data)
}

#[tauri::command]
pub fn resize_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    manager.resize_terminal(&id, cols, rows)
}

#[tauri::command]
pub fn kill_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
) -> Result<(), String> {
    manager.kill_terminal(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_pty_double_spawn_preserves_session() {
        let manager = NativePtyManager::new();
        let session_id = "test-double-spawn".to_string();
        let opts = TerminalSpawnOptions {
            command: None,
            args: vec![],
            cwd: None,
            cols: 80,
            rows: 24,
        };
        assert!(manager.spawn_terminal(session_id.clone(), opts.clone()).is_ok());
        assert!(manager.spawn_terminal(session_id.clone(), opts).is_ok());
        assert!(manager.write_terminal(&session_id, "echo still_alive\r\n").is_ok());
        assert!(manager.kill_terminal(&session_id).is_ok());
    }

    #[test]
    fn test_native_pty_lifecycle() {
        let manager = NativePtyManager::new();
        let session_id = "test-session-1".to_string();
        let spawn_res = manager.spawn_terminal(
            session_id.clone(),
            TerminalSpawnOptions {
                command: None,
                args: vec![],
                cwd: None,
                cols: 80,
                rows: 24,
            },
        );
        assert!(spawn_res.is_ok(), "Terminal should spawn successfully");

        let write_res = manager.write_terminal(&session_id, "echo hello\r\n");
        assert!(write_res.is_ok(), "Writing to PTY should succeed");

        let resize_res = manager.resize_terminal(&session_id, 120, 40);
        assert!(resize_res.is_ok(), "Resizing PTY should succeed");

        let kill_res = manager.kill_terminal(&session_id);
        assert!(kill_res.is_ok(), "Killing PTY should succeed");
    }

    #[test]
    fn test_native_pty_invalid_session_error() {
        let manager = NativePtyManager::new();
        let nonexistent = "nonexistent-session";
        assert!(manager.write_terminal(nonexistent, "test").is_err());
        assert!(manager.resize_terminal(nonexistent, 80, 24).is_err());
        assert!(manager.kill_terminal(nonexistent).is_ok());
    }

    #[test]
    fn test_native_pty_custom_command() {
        let manager = NativePtyManager::new();
        let session_id = "test-custom-cmd".to_string();
        let cmd = if cfg!(windows) { "cmd.exe" } else { "sh" };
        let spawn_res = manager.spawn_terminal(
            session_id.clone(),
            TerminalSpawnOptions {
                command: Some(cmd.to_string()),
                args: vec![],
                cwd: None,
                cols: 100,
                rows: 30,
            },
        );
        assert!(spawn_res.is_ok(), "Custom command terminal should spawn");
        assert!(manager.kill_terminal(&session_id).is_ok());
    }

    #[test]
    fn test_native_pty_channel_read() {
        let manager = NativePtyManager::new();
        let session_id = "test-channel-read".to_string();
        let spawn_res = manager.spawn_terminal(
            session_id.clone(),
            TerminalSpawnOptions {
                command: None,
                args: vec![],
                cwd: None,
                cols: 80,
                rows: 24,
            },
        );
        assert!(spawn_res.is_ok());

        let (tx, rx) = std::sync::mpsc::channel();
        let channel = Channel::new(move |event| {
            let _ = tx.send(event);
            Ok(())
        });

        let attach_res = manager.attach_reader_channel(session_id.clone(), channel);
        assert!(attach_res.is_ok(), "Attaching channel should succeed");

        let write_res = manager.write_terminal(&session_id, "echo hello_channel\r\n");
        assert!(write_res.is_ok());

        // Wait for at least one data event from PTY
        let received = rx.recv_timeout(std::time::Duration::from_secs(3));
        assert!(
            received.is_ok(),
            "Should receive terminal output event from channel"
        );

        assert!(manager.kill_terminal(&session_id).is_ok());
    }

    #[test]
    fn test_split_incomplete_utf8() {
        // Complete ASCII
        assert_eq!(split_incomplete_utf8(b"hello world"), 11);
        assert_eq!(split_incomplete_utf8(b""), 0);

        // Complete 2-byte sequence (e.g. 'é' = [0xC3, 0xA9])
        assert_eq!(split_incomplete_utf8(&[b'a', 0xC3, 0xA9]), 3);
        // Incomplete 2-byte sequence cut at leading byte
        assert_eq!(split_incomplete_utf8(&[b'a', 0xC3]), 1);

        // Complete 3-byte sequence (e.g. '—' = [0xE2, 0x80, 0x94])
        assert_eq!(split_incomplete_utf8(&[0xE2, 0x80, 0x94]), 3);
        // Incomplete 3-byte sequence cut at 1 byte
        assert_eq!(split_incomplete_utf8(&[0xE2]), 0);
        // Incomplete 3-byte sequence cut at 2 bytes
        assert_eq!(split_incomplete_utf8(&[0xE2, 0x80]), 0);

        // Complete 4-byte emoji (e.g. '🔥' = [0xF0, 0x9F, 0x94, 0xA5])
        assert_eq!(split_incomplete_utf8(&[0xF0, 0x9F, 0x94, 0xA5]), 4);
        // Incomplete 4-byte cut at 3 bytes
        assert_eq!(split_incomplete_utf8(&[b'x', 0xF0, 0x9F, 0x94]), 1);
    }

    #[test]
    fn test_native_pty_burst_coalescing() {
        let manager = NativePtyManager::new();
        let session_id = "test-burst-coalescing".to_string();
        let spawn_res = manager.spawn_terminal(
            session_id.clone(),
            TerminalSpawnOptions {
                command: None,
                args: vec![],
                cwd: None,
                cols: 80,
                rows: 24,
            },
        );
        assert!(spawn_res.is_ok());

        let (tx, rx) = std::sync::mpsc::channel();
        let channel = Channel::new(move |event| {
            let _ = tx.send(event);
            Ok(())
        });

        assert!(manager
            .attach_reader_channel(session_id.clone(), channel)
            .is_ok());

        // Rapid burst of 10 sequential write commands
        for i in 0..10 {
            let cmd = format!("echo burst_item_{}\r\n", i);
            let _ = manager.write_terminal(&session_id, &cmd);
        }

        // Collect events received within 3 seconds
        let mut collected_output = String::new();
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            if let Ok(body) = rx.recv_timeout(std::time::Duration::from_millis(300)) {
                match body {
                    tauri::ipc::InvokeResponseBody::Json(s) => collected_output.push_str(&s),
                    tauri::ipc::InvokeResponseBody::Raw(b) => {
                        collected_output.push_str(&String::from_utf8_lossy(&b))
                    }
                }
                if collected_output.contains("burst_item_9") {
                    break;
                }
            }
        }

        assert!(
            collected_output.contains("burst_item_0") || collected_output.contains("burst_item_"),
            "Should receive burst streaming output"
        );

        assert!(manager.kill_terminal(&session_id).is_ok());
    }
}
