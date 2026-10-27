use std::env;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
pub struct DaemonState {
    pub process: Mutex<Option<Child>>,
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
                let res_script = res_dir.join("dist").join("server").join("server").join("daemon-worker.js");
                if res_script.exists() {
                    let node = if res_node.exists() { res_node } else { PathBuf::from("node") };
                    return Some((res_dir.clone(), node, res_script));
                }
            }
        }
    }

    // 2. Optional override via environment variable
    if let Ok(repo) = env::var("KRONOS_REPO_DIR").or_else(|_| env::var("PASEO_REPO_DIR")) {
        let script = PathBuf::from(repo).join("packages").join("server").join("dist").join("server").join("server").join("daemon-worker.js");
        if script.exists() {
            return Some((script.parent()?.parent()?.parent()?.parent()?.to_path_buf(), PathBuf::from("node"), script));
        }
    }
    // 3. Local workspace walking up from exe
    if let Ok(mut current) = env::current_exe() {
        for _ in 0..10 {
            if !current.pop() {
                break;
            }
            let server_worker = current.join("packages").join("server").join("dist").join("server").join("server").join("daemon-worker.js");
            if server_worker.exists() {
                return Some((current, PathBuf::from("node"), server_worker));
            }
        }
    }

    // 4. Current dir
    if let Ok(cwd) = env::current_dir() {
        let server_worker = cwd.join("packages").join("server").join("dist").join("server").join("server").join("daemon-worker.js");
        if server_worker.exists() {
            return Some((cwd, PathBuf::from("node"), server_worker));
        }
    }

    None
}
#[cfg(windows)]
fn ensure_port_clean() {
    let _ = Command::new("cmd")
        .args(["/C", "for /f \"tokens=5\" %a in ('netstat -aon ^| findstr \":6767 \" ^| findstr \"LISTENING\"') do taskkill /f /pid %a"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

#[cfg(not(windows))]
fn ensure_port_clean() {
    let _ = Command::new("sh")
        .args(["-c", "lsof -ti:6767 | xargs -r kill -9 2>/dev/null || true"])
        .output();
}


pub fn start_daemon(server_id: Option<&str>) -> Option<Child> {
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
    cmd.current_dir(&work_dir)
        .stdin(Stdio::null());

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
    let child = cmd.spawn().ok()?;
    // Daemon process spawned immediately without blocking thread sleep
    Some(child)
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
    ensure_port_clean();
}

pub fn restart_daemon(state: &DaemonState) -> bool {
    stop_daemon(state);
    let new_child = start_daemon(None);
    if let Ok(mut lock) = state.process.lock() {
        let success = new_child.is_some();
        *lock = new_child;
        success
    } else {
        false
    }
}
