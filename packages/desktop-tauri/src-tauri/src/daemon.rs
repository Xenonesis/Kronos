use std::env;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use crate::kernel::server::{DaemonHandle, EmbeddedDaemon};

enum EmbeddedDaemonState {
    Stopped,
    Starting {
        cancel: Arc<AtomicBool>,
        join_handle: tokio::task::JoinHandle<()>,
    },
    Running(DaemonHandle),
}

static EMBEDDED_DAEMON: Mutex<EmbeddedDaemonState> = Mutex::new(EmbeddedDaemonState::Stopped);
static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("kronos-embedded-daemon")
        .build()
        .expect("Failed to initialize EmbeddedDaemon Tokio runtime")
});

fn get_or_init_runtime() -> &'static tokio::runtime::Runtime {
    &RUNTIME
}

pub fn wait_for_daemon_health(max_attempts: usize) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    for _ in 0..max_attempts {
        if let Ok(mut stream) = TcpStream::connect("127.0.0.1:6767") {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let req =
                b"GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:6767\r\nConnection: close\r\n\r\n";
            if stream.write_all(req).is_ok() {
                let mut buf = [0u8; 64];
                if let Ok(n) = stream.read(&mut buf) {
                    if n > 0
                        && (buf.starts_with(b"HTTP/1.1 200") || buf.starts_with(b"HTTP/1.0 200"))
                    {
                        return true;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

#[allow(dead_code)]
pub fn is_embedded_running() -> bool {
    if let Ok(lock) = EMBEDDED_DAEMON.lock() {
        matches!(*lock, EmbeddedDaemonState::Running(_))
    } else {
        false
    }
}

pub fn stop_embedded() {
    let old_state = {
        let mut lock = match EMBEDDED_DAEMON.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        std::mem::replace(&mut *lock, EmbeddedDaemonState::Stopped)
    };

    let rt = get_or_init_runtime();
    match old_state {
        EmbeddedDaemonState::Stopped => {}
        EmbeddedDaemonState::Starting { cancel, join_handle } => {
            cancel.store(true, Ordering::SeqCst);
            let abort_handle = join_handle.abort_handle();
            let (tx, rx) = std::sync::mpsc::channel();
            rt.spawn(async move {
                let _ = join_handle.await;
                let _ = tx.send(());
            });
            if rx.recv_timeout(std::time::Duration::from_millis(2500)).is_err() {
                abort_handle.abort();
            }
        }
        EmbeddedDaemonState::Running(handle) => {
            let (tx, rx) = std::sync::mpsc::channel();
            rt.spawn(async move {
                handle.stop().await;
                let _ = tx.send(());
            });
            let _ = rx.recv_timeout(std::time::Duration::from_millis(2500));
        }
    }
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
pub struct DaemonState {
    pub process: Mutex<Option<Child>>,
}
pub fn is_valid_node_executable(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }
    if let Ok(metadata) = std::fs::metadata(path) {
        if !metadata.is_file() || metadata.len() < 1024 * 1024 {
            return false;
        }
    } else {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new(path)
            .arg("-v")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new(path)
            .arg("-v")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

pub fn find_system_node() -> PathBuf {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if Command::new("node")
            .arg("-v")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return PathBuf::from("node");
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            let pi_node = PathBuf::from(&local_app_data)
                .join("pi-node")
                .join("current")
                .join("node.exe");
            if is_valid_node_executable(&pi_node) {
                return pi_node;
            }
            let user_node = PathBuf::from(&local_app_data)
                .join("Programs")
                .join("node")
                .join("node.exe");
            if is_valid_node_executable(&user_node) {
                return user_node;
            }
        }
        if let Ok(prog_files) = env::var("ProgramFiles") {
            let prog_node = PathBuf::from(&prog_files)
                .join("nodejs")
                .join("node.exe");
            if is_valid_node_executable(&prog_node) {
                return prog_node;
            }
        }
    }
    #[cfg(not(windows))]
    {
        if Command::new("node")
            .arg("-v")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return PathBuf::from("node");
        }
    }
    PathBuf::from("node")
}

fn resolve_runtime() -> Option<(PathBuf, PathBuf, PathBuf)> {
    // 1. Installed location (_up_/server-dist or resources/server-dist) takes priority
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidates = [
                parent.join("_up_").join("server-dist"),
                parent.join("server-dist"),
                parent.join("resources").join("server-dist"),
            ];
            for res_dir in &candidates {
                let res_node = res_dir.join("node.exe");
                let res_script = res_dir
                    .join("dist")
                    .join("server")
                    .join("server")
                    .join("daemon-worker.js");
                if res_script.exists() {
                    let node = if is_valid_node_executable(&res_node) {
                        res_node
                    } else {
                        find_system_node()
                    };
                    return Some((res_dir.clone(), node, res_script));
                }
            }
        }
    }

    // 2. Optional override via environment variable
    if let Ok(repo) = env::var("KRONOS_REPO_DIR").or_else(|_| env::var("PASEO_REPO_DIR")) {
        let script = PathBuf::from(repo)
            .join("packages")
            .join("server")
            .join("dist")
            .join("server")
            .join("server")
            .join("daemon-worker.js");
        if script.exists() {
            return Some((
                script.parent()?.parent()?.parent()?.parent()?.to_path_buf(),
                find_system_node(),
                script,
            ));
        }
    }
    // 3. Local workspace walking up from exe
    if let Ok(mut current) = env::current_exe() {
        for _ in 0..10 {
            if !current.pop() {
                break;
            }
            let server_worker = current
                .join("packages")
                .join("server")
                .join("dist")
                .join("server")
                .join("server")
                .join("daemon-worker.js");
            if server_worker.exists() {
                return Some((current, find_system_node(), server_worker));
            }
        }
    }

    // 4. Current dir
    if let Ok(cwd) = env::current_dir() {
        let server_worker = cwd
            .join("packages")
            .join("server")
            .join("dist")
            .join("server")
            .join("server")
            .join("daemon-worker.js");
        if server_worker.exists() {
            return Some((cwd, find_system_node(), server_worker));
        }
    }

    None
}

#[cfg(windows)]
pub fn find_pid_listening_on_port(port: u16) -> Option<u32> {
    let port_str = port.to_string();
    if let Ok(output) = Command::new("netstat")
        .args(["-aon"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5
                    && parts[1].rsplit(':').next() == Some(port_str.as_str())
                    && parts.contains(&"LISTENING")
                {
                    if let Some(pid_str) = parts.last() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            if pid != 0 {
                                return Some(pid);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn find_pid_listening_on_port(port: u16) -> Option<u32> {
    if let Ok(output) = Command::new("lsof").args(["-ti", &format!(":{}", port)]).output() {
        if let Ok(text) = String::from_utf8(output.stdout) {
            for line in text.lines() {
                if let Ok(pid) = line.trim().parse::<u32>() {
                    if pid != 0 {
                        return Some(pid);
                    }
                }
            }
        }
    }
    None
}
#[cfg(windows)]
fn ensure_port_clean() {
    let current_pid = std::process::id();
    if let Ok(output) = Command::new("netstat")
        .args(["-aon"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5
                    && parts[1].rsplit(':').next() == Some("6767")
                    && parts.contains(&"LISTENING")
                {
                    if let Some(pid_str) = parts.last() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            if pid != 0 && pid != current_pid {
                                let _ = Command::new("taskkill")
                                    .args(["/F", "/PID", &pid.to_string()])
                                    .creation_flags(CREATE_NO_WINDOW)
                                    .output();
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn ensure_port_clean() {
    let current_pid = std::process::id();
    if let Ok(output) = Command::new("lsof").args(["-ti:6767"]).output() {
        if let Ok(text) = String::from_utf8(output.stdout) {
            for line in text.lines() {
                if let Ok(pid) = line.trim().parse::<u32>() {
                    if pid != 0 && pid != current_pid {
                        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                    }
                }
            }
        }
    }
}

pub fn start_node_daemon(server_id: Option<&str>) -> Option<Child> {
    if wait_for_daemon_health(1) {
        return None;
    }
    ensure_port_clean();
    let (work_dir, node_bin, script_path) = resolve_runtime()?;

    let mut cmd = Command::new(&node_bin);
    cmd.arg("--max-old-space-size=2048");
    let refresh_timeout = env::var("KRONOS_PROVIDER_REFRESH_TIMEOUT_MS")
        .or_else(|_| env::var("PASEO_PROVIDER_REFRESH_TIMEOUT_MS"))
        .unwrap_or_else(|_| "30000".to_string());
    cmd.arg(&script_path)
        .env("PASEO_LISTEN", "127.0.0.1:6767")
        .env("PASEO_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout)
        .env("KRONOS_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout);
    if let Some(id) = server_id {
        cmd.env("PASEO_SERVER_ID", id);
    } else if let Some(mut home) = dirs::home_dir() {
        home.push(".paseo");
        home.push("server-id");
        if let Ok(content) = fs::read_to_string(&home) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                cmd.env("PASEO_SERVER_ID", trimmed);
            }
        }
    }
    // Ensure bundled server-dist/node_modules is always included in NODE_PATH
    let local_nm = work_dir.join("node_modules");
    let mut node_paths = Vec::new();
    if local_nm.exists() {
        node_paths.push(local_nm.to_string_lossy().to_string());
    }
    if let Ok(existing) = env::var("NODE_PATH") {
        if !existing.is_empty() {
            node_paths.push(existing);
        }
    }
    if let Ok(appdata) = env::var("APPDATA") {
        let global_npm = PathBuf::from(appdata).join("npm").join("node_modules");
        if global_npm.exists() {
            node_paths.push(global_npm.to_string_lossy().to_string());
        }
    }
    if !node_paths.is_empty() {
        #[cfg(windows)]
        cmd.env("NODE_PATH", node_paths.join(";"));
        #[cfg(not(windows))]
        cmd.env("NODE_PATH", node_paths.join(":"));
    }
    cmd.current_dir(&work_dir).stdin(Stdio::null());

    if let Some(mut home) = dirs::home_dir() {
        home.push(".paseo");
        let _ = std::fs::create_dir_all(&home);
        home.push("daemon.log");
        if let Ok(f) = OpenOptions::new().create(true).append(true).open(&home) {
            if let Ok(f_err) = f.try_clone() {
                cmd.stdout(Stdio::from(f));
                cmd.stderr(Stdio::from(f_err));
            }
        }
    }

    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[Daemon] Primary spawn failed with {:?}: {}", node_bin, e);
            let sys_node = find_system_node();
            if sys_node != node_bin {
                eprintln!("[Daemon] Falling back to system node {:?}", sys_node);
                let mut fallback_cmd = Command::new(&sys_node);
                fallback_cmd.arg("--max-old-space-size=2048");
                fallback_cmd.arg(&script_path)
                    .env("PASEO_LISTEN", "127.0.0.1:6767")
                    .env("PASEO_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout)
                    .env("KRONOS_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout);
                if let Some(id) = server_id {
                    fallback_cmd.env("PASEO_SERVER_ID", id);
                } else if let Some(mut home) = dirs::home_dir() {
                    home.push(".paseo");
                    home.push("server-id");
                    if let Ok(content) = fs::read_to_string(&home) {
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            fallback_cmd.env("PASEO_SERVER_ID", trimmed);
                        }
                    }
                }
                if !node_paths.is_empty() {
                    #[cfg(windows)]
                    fallback_cmd.env("NODE_PATH", node_paths.join(";"));
                    #[cfg(not(windows))]
                    fallback_cmd.env("NODE_PATH", node_paths.join(":"));
                }
                fallback_cmd.current_dir(&work_dir).stdin(Stdio::null());
                if let Some(mut home) = dirs::home_dir() {
                    home.push(".paseo");
                    let _ = std::fs::create_dir_all(&home);
                    home.push("daemon.log");
                    if let Ok(f) = OpenOptions::new().create(true).append(true).open(&home) {
                        if let Ok(f_err) = f.try_clone() {
                            fallback_cmd.stdout(Stdio::from(f));
                            fallback_cmd.stderr(Stdio::from(f_err));
                        }
                    }
                }
                #[cfg(windows)]
                {
                    fallback_cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
                }
                match fallback_cmd.spawn() {
                    Ok(c) => c,
                    Err(fallback_err) => {
                        eprintln!("[Daemon] Fallback spawn failed with {:?}: {}", sys_node, fallback_err);
                        return None;
                    }
                }
            } else {
                return None;
            }
        }
    };
    Some(child)
}
static DAEMON_SPAWN_LOCK: Mutex<()> = Mutex::new(());

pub fn start_daemon(server_id: Option<&str>) -> Option<Child> {
    if wait_for_daemon_health(1) {
        return None;
    }
    let _guard = DAEMON_SPAWN_LOCK.lock().ok()?;
    if wait_for_daemon_health(1) {
        return None;
    }
    let engine = env::var("KRONOS_DAEMON_ENGINE").unwrap_or_else(|_| "node".to_string());
    if engine.eq_ignore_ascii_case("node") {
        stop_embedded();
        start_node_daemon(server_id)
    } else {
        stop_embedded();
        ensure_port_clean();
        let rt = get_or_init_runtime();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_flag = cancel.clone();

        let mut lock = match EMBEDDED_DAEMON.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let join_handle = rt.spawn(async move {
            let result = EmbeddedDaemon::start().await;
            let handle_to_stop = match result {
                Ok(handle) => {
                    let mut lock = match EMBEDDED_DAEMON.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    if cancel_flag.load(Ordering::SeqCst) {
                        *lock = EmbeddedDaemonState::Stopped;
                        Some(handle)
                    } else {
                        *lock = EmbeddedDaemonState::Running(handle);
                        None
                    }
                }
                Err(e) => {
                    eprintln!("[EmbeddedDaemon] Failed to start: {}", e);
                    let mut lock = match EMBEDDED_DAEMON.lock() {
                        Ok(guard) => guard,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    *lock = EmbeddedDaemonState::Stopped;
                    None
                }
            };

            if let Some(handle) = handle_to_stop {
                handle.stop().await;
            }
        });

        *lock = EmbeddedDaemonState::Starting {
            cancel,
            join_handle,
        };
        drop(lock);

        None
    }
}


pub fn stop_daemon(state: &DaemonState) {
    if let Ok(mut lock) = state.process.lock() {
        if let Some(mut child) = lock.take() {
            #[cfg(windows)]
            {
                let pid = child.id();
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            let _ = child.kill();
        }
    }
    stop_embedded();
    ensure_port_clean();
}

pub fn restart_daemon(state: &DaemonState) -> bool {
    stop_daemon(state);
    let new_child = start_daemon(None);
    let engine = env::var("KRONOS_DAEMON_ENGINE").unwrap_or_else(|_| "rust".to_string());
    if engine.eq_ignore_ascii_case("node") {
        if let Ok(mut lock) = state.process.lock() {
            let success = new_child.is_some();
            *lock = new_child;
            success
        } else {
            false
        }
    } else {
        if let Ok(mut lock) = state.process.lock() {
            *lock = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_daemon_state_empty_stop() {
        let state = DaemonState {
            process: Mutex::new(None),
        };
        stop_daemon(&state);
        assert!(state.process.lock().unwrap().is_none());
    }

    #[test]
    fn test_daemon_engine_switch_node() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("KRONOS_DAEMON_ENGINE", "node");
        let state = DaemonState {
            process: Mutex::new(None),
        };
        let _ = start_daemon(None);
        stop_daemon(&state);
        std::env::remove_var("KRONOS_DAEMON_ENGINE");
    }

    #[tokio::test]
    async fn test_daemon_engine_switch_rust() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("KRONOS_DAEMON_ENGINE", "rust");
        let state = DaemonState {
            process: Mutex::new(None),
        };
        let child = start_daemon(None);
        assert!(child.is_none(), "Rust engine boots embedded daemon without external Child process");
        stop_daemon(&state);
        std::env::remove_var("KRONOS_DAEMON_ENGINE");
    }

    #[tokio::test]
    async fn test_daemon_rapid_start_stop_race() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("KRONOS_DAEMON_ENGINE", "rust");
        let state = DaemonState {
            process: Mutex::new(None),
        };
        let _ = start_daemon(None);
        stop_daemon(&state);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!is_embedded_running(), "Daemon must not remain running after immediate stop");
        std::env::remove_var("KRONOS_DAEMON_ENGINE");
    }
    #[test]
    fn test_ensure_port_clean_does_not_kill_self() {
        ensure_port_clean();
        assert_eq!(std::process::id(), std::process::id());
    }

    #[test]
    fn test_is_valid_node_executable_rejects_empty_file() {
        let temp_dir = std::env::temp_dir();
        let empty_node = temp_dir.join("test-zero-byte-node.exe");
        let _ = std::fs::write(&empty_node, b"");
        assert!(!is_valid_node_executable(&empty_node));
        let _ = std::fs::remove_file(empty_node);
    }

    #[test]
    fn test_find_system_node_returns_executable() {
        let sys_node = find_system_node();
        assert!(!sys_node.as_os_str().is_empty());
    }
}
