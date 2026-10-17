use std::env;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

    // 2. Known local repo path on development workstation
    let repo_candidates = [
        PathBuf::from(r"C:\Users\Acer\Desktop\paseo"),
    ];
    for repo in &repo_candidates {
        let script = repo.join("packages").join("server").join("dist").join("server").join("server").join("daemon-worker.js");
        if script.exists() {
            return Some((repo.clone(), PathBuf::from("node"), script));
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

pub fn start_daemon() -> Option<Child> {
    let (work_dir, node_bin, script_path) = resolve_runtime()?;

    let mut cmd = Command::new(&node_bin);
    cmd.arg(&script_path)
        .env("PASEO_LISTEN", "127.0.0.1:6767");

    // Inherit NODE_PATH from global/user node environment if running installed
    if let Ok(appdata) = env::var("APPDATA") {
        let global_npm = PathBuf::from(appdata).join("npm").join("node_modules");
        if global_npm.exists() {
            cmd.env("NODE_PATH", global_npm);
        }
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
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd.spawn().ok()?;
    std::thread::sleep(Duration::from_millis(2000));
    Some(child)
}

pub fn stop_daemon(state: &DaemonState) {
    if let Ok(mut lock) = state.process.lock() {
        if let Some(mut child) = lock.take() {
            let _ = child.kill();
        }
    }
}

pub fn restart_daemon(state: &DaemonState) -> bool {
    stop_daemon(state);
    let new_child = start_daemon();
    if let Ok(mut lock) = state.process.lock() {
        let success = new_child.is_some();
        *lock = new_child;
        success
    } else {
        false
    }
}
