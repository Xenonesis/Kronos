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

pub struct ActiveTerminal {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub writer: Box<dyn Write + Send>,
}

#[derive(Clone)]
pub struct NativePtyManager {
    sessions: Arc<Mutex<HashMap<String, ActiveTerminal>>>,
    job_supervisor: Option<Arc<crate::process_job::JobSupervisor>>,
}

impl Default for NativePtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePtyManager {
    pub fn new() -> Self {
        let job_supervisor = crate::process_job::JobSupervisor::new().ok().map(Arc::new);
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            job_supervisor,
        }
    }

    pub fn with_job_supervisor(supervisor: Arc<crate::process_job::JobSupervisor>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            job_supervisor: Some(supervisor),
        }
    }

    pub fn spawn_terminal(
        &self,
        id: String,
        options: TerminalSpawnOptions,
    ) -> Result<(), String> {
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

        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(
            id,
            ActiveTerminal {
                master: pair.master,
                child,
                writer,
            },
        );

        Ok(())
    }

    pub fn attach_reader_channel(
        &self,
        id: String,
        channel: Channel<TerminalEvent>,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        let term = sessions.get_mut(&id).ok_or_else(|| "Session not found".to_string())?;

        let mut reader = term
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {}", e))?;

        let sessions_clone = self.sessions.clone();
        let session_id = id.clone();

        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = channel.send(TerminalEvent::Exit(None));
                        break;
                    }
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        if channel.send(TerminalEvent::Data(text)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::BrokenPipe {
                            let _ = channel.send(TerminalEvent::Exit(None));
                        } else {
                            let _ = channel.send(TerminalEvent::Error(e.to_string()));
                        }
                        break;
                    }
                }
            }
            let mut lock = sessions_clone.lock().unwrap();
            lock.remove(&session_id);
        });

        Ok(())
    }

    pub fn write_terminal(&self, id: &str, data: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        let term = sessions.get_mut(id).ok_or_else(|| "Session not found".to_string())?;
        term.writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Failed to write to PTY: {}", e))?;
        term.writer.flush().map_err(|e| format!("Failed to flush PTY: {}", e))
    }

    pub fn resize_terminal(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock().unwrap();
        let term = sessions.get(id).ok_or_else(|| "Session not found".to_string())?;
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
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(mut term) = sessions.remove(id) {
            let _ = term.child.kill();
        }
        Ok(())
    }
}

#[tauri::command]
pub fn spawn_native_terminal(
    manager: tauri::State<NativePtyManager>,
    id: String,
    options: TerminalSpawnOptions,
    on_event: Channel<TerminalEvent>,
) -> Result<(), String> {
    manager.spawn_terminal(id.clone(), options)?;
    manager.attach_reader_channel(id, on_event)
}

#[tauri::command]
pub fn write_native_terminal(
    manager: tauri::State<NativePtyManager>,
    id: String,
    data: String,
) -> Result<(), String> {
    manager.write_terminal(&id, &data)
}

#[tauri::command]
pub fn resize_native_terminal(
    manager: tauri::State<NativePtyManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    manager.resize_terminal(&id, cols, rows)
}

#[tauri::command]
pub fn kill_native_terminal(
    manager: tauri::State<NativePtyManager>,
    id: String,
) -> Result<(), String> {
    manager.kill_terminal(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(received.is_ok(), "Should receive terminal output event from channel");

        assert!(manager.kill_terminal(&session_id).is_ok());
    }
}
