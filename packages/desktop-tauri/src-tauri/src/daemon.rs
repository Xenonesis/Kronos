use crate::kernel::server::{DaemonHandle, NetworkStatus, SessionContext};
use futures_util::FutureExt;
use std::env;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "phase", rename_all = "lowercase")]
pub enum EnginePhase {
    Stopped,
    Starting,
    Ready,
    Failed { reason: String },
}

type ReadyResult = Result<Arc<SessionContext>, String>;

struct EmbeddedRun {
    cancel: Arc<AtomicBool>,
    join_handle: tokio::task::JoinHandle<()>,
    completion: tokio::sync::watch::Sender<Option<ReadyResult>>,
    #[allow(dead_code)] // read by the supervised acceptance network toggle wired in Task 4
    network_tx: tokio::sync::mpsc::Sender<NetworkRequest>,
}

struct NetworkRequest {
    enabled: bool,
    reply: tokio::sync::oneshot::Sender<Result<(), String>>,
}

struct EmbeddedDaemonState {
    phase: EnginePhase,
    context: Option<Arc<SessionContext>>,
    run: Option<EmbeddedRun>,
}

static EMBEDDED_DAEMON: Mutex<EmbeddedDaemonState> = Mutex::new(EmbeddedDaemonState {
    phase: EnginePhase::Stopped,
    context: None,
    run: None,
});

#[cfg(test)]
#[derive(Clone)]
struct StartupHook {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    fail: bool,
}
#[cfg(test)]
static STARTUP_HOOK: Mutex<Option<StartupHook>> = Mutex::new(None);

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    let threads = num_cpus::get().clamp(4, 16);
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(threads)
        .enable_all()
        .thread_name("kronos-embedded-daemon")
        .build()
        .expect("Failed to initialize EmbeddedDaemon Tokio runtime")
});

fn get_or_init_runtime() -> &'static tokio::runtime::Runtime {
    &RUNTIME
}

static ACTIVE_DAEMON_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

pub fn set_active_daemon_port(port: u16) {
    ACTIVE_DAEMON_PORT.store(port, Ordering::SeqCst);
}

pub fn get_active_daemon_port() -> u16 {
    ACTIVE_DAEMON_PORT.load(Ordering::SeqCst)
}

pub(crate) fn publish_active_daemon_port(context: &Arc<SessionContext>, port: u16) {
    let state = EMBEDDED_DAEMON.lock().unwrap_or_else(|e| e.into_inner());
    if !context.is_stopped()
        && state
            .context
            .as_ref()
            .is_none_or(|current| Arc::ptr_eq(current, context))
    {
        set_active_daemon_port(port);
    }
}

pub(crate) fn clear_active_daemon_port(port: u16) {
    let _ = ACTIVE_DAEMON_PORT.compare_exchange(port, 0, Ordering::SeqCst, Ordering::SeqCst);
}

pub fn wait_for_daemon_health(max_attempts: usize) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    // Wait up to 15 seconds for the embedded daemon to assign a port (port starts as 0)
    let mut port = get_active_daemon_port();
    if port == 0 {
        for _ in 0..60 {
            std::thread::sleep(Duration::from_millis(250));
            port = get_active_daemon_port();
            if port != 0 {
                break;
            }
        }
    }
    if port == 0 {
        return false;
    }
    let addr_str = format!("127.0.0.1:{}", port);

    for _ in 0..max_attempts {
        if let Ok(mut stream) = TcpStream::connect(&addr_str) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let req = format!(
                "GET /api/health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                addr_str
            );
            if stream.write_all(req.as_bytes()).is_ok() {
                let mut response = Vec::new();
                if stream.take(64 * 1024).read_to_end(&mut response).is_ok()
                    && (response.starts_with(b"HTTP/1.1 200")
                        || response.starts_with(b"HTTP/1.0 200"))
                {
                    if let Some(body) = response.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        if let Ok(payload) =
                            serde_json::from_slice::<serde_json::Value>(&response[body + 4..])
                        {
                            let native = payload["engine"] == "kronos-rust-kernel";
                            let node = env::var("KRONOS_DAEMON_ENGINE")
                                .is_ok_and(|engine| engine.eq_ignore_ascii_case("node"))
                                && payload["ready"] == true
                                && payload["timestamp"].is_string();
                            if payload["status"] == "ok" && (native || node) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

pub fn embedded_phase() -> EnginePhase {
    EMBEDDED_DAEMON
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .phase
        .clone()
}

pub fn embedded_network_status() -> NetworkStatus {
    EMBEDDED_DAEMON
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .context
        .as_ref()
        .map(|context| context.network_status())
        .unwrap_or_default()
}

pub fn is_embedded_running() -> bool {
    embedded_phase() == EnginePhase::Ready
}

fn request_embedded_start(
    retry_failed: bool,
) -> Result<tokio::sync::watch::Receiver<Option<ReadyResult>>, String> {
    let mut state = EMBEDDED_DAEMON.lock().unwrap_or_else(|e| e.into_inner());
    if matches!(state.phase, EnginePhase::Starting | EnginePhase::Ready) {
        return Ok(state
            .run
            .as_ref()
            .expect("active engine owns startup")
            .completion
            .subscribe());
    }
    if let EnginePhase::Failed { reason } = &state.phase {
        if !retry_failed {
            return Err(reason.clone());
        }
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let (completion, receiver) = tokio::sync::watch::channel(None);
    let (network_tx, network_rx) = tokio::sync::mpsc::channel(4);
    let worker_cancel = cancel.clone();
    let worker_completion = completion.clone();
    let join_handle = get_or_init_runtime().spawn(async move {
        run_embedded(worker_cancel, worker_completion, network_rx).await;
    });
    state.phase = EnginePhase::Starting;
    state.context = None;
    state.run = Some(EmbeddedRun {
        cancel,
        join_handle,
        completion,
        network_tx,
    });
    Ok(receiver)
}

pub async fn ensure_embedded_ready() -> ReadyResult {
    if env::var("KRONOS_DAEMON_ENGINE").is_ok_and(|engine| engine.eq_ignore_ascii_case("node")) {
        return Err("Embedded IPC is unavailable in Node engine mode".into());
    }
    let mut receiver = request_embedded_start(false)?;
    loop {
        if let Some(result) = receiver.borrow_and_update().clone() {
            return result;
        }
        receiver
            .changed()
            .await
            .map_err(|_| "Embedded startup ended without readiness".to_string())?;
    }
}

/// Toggles the network listener owned by the current embedded run task.
/// Never creates an engine: it fails when the core is not ready and when the
/// run is stopped or superseded before completion replies.
#[cfg(feature = "ipc-acceptance")]
#[allow(dead_code)] // invoked by the supervised acceptance startup built in Task 4
pub async fn acceptance_set_network(enabled: bool) -> Result<(), String> {
    let (reply, reply_rx) = tokio::sync::oneshot::channel();
    let (sender, run_cancel) = {
        let state = EMBEDDED_DAEMON.lock().unwrap_or_else(|e| e.into_inner());
        if state.phase != EnginePhase::Ready {
            return Err("Embedded core is not ready for a network toggle".into());
        }
        let run = state
            .run
            .as_ref()
            .ok_or("Embedded core has no owning run task")?;
        (run.network_tx.clone(), run.cancel.clone())
    };
    sender
        .send(NetworkRequest { enabled, reply })
        .await
        .map_err(|_| "Embedded core stopped before the network toggle".to_string())?;
    reply_rx
        .await
        .map_err(|_| "Network toggle was superseded".to_string())??;
    let current = {
        let state = EMBEDDED_DAEMON.lock().unwrap_or_else(|e| e.into_inner());
        state.phase == EnginePhase::Ready
            && state
                .run
                .as_ref()
                .is_some_and(|run| Arc::ptr_eq(&run.cancel, &run_cancel))
    };
    if !current {
        return Err("Network toggle was superseded by a new embedded core".into());
    }
    Ok(())
}

/// Acceptance listeners bind only the run-owned pipe; ordinary startup keeps the
/// canonical listener unchanged.
fn embedded_pipe() -> Option<String> {
    #[cfg(all(windows, feature = "ipc-acceptance"))]
    {
        crate::desktop_acceptance::acceptance_pipe_name()
    }
    #[cfg(not(all(windows, feature = "ipc-acceptance")))]
    {
        None
    }
}

async fn run_embedded(
    cancel: Arc<AtomicBool>,
    completion: tokio::sync::watch::Sender<Option<ReadyResult>>,
    mut network_rx: tokio::sync::mpsc::Receiver<NetworkRequest>,
) {
    let initialized = std::panic::AssertUnwindSafe(async {
        #[cfg(test)]
        {
            let hook = STARTUP_HOOK.lock().unwrap().clone();
            if let Some(hook) = hook {
                hook.entered.notify_one();
                hook.release.notified().await;
                if hook.fail {
                    return Err("injected core startup failure".to_string());
                }
            }
        }
        SessionContext::initialize().map_err(|error| error.to_string())
    })
    .catch_unwind()
    .await
    .unwrap_or_else(|_| Err("Embedded engine startup panicked".into()));

    let context = {
        let mut state = EMBEDDED_DAEMON.lock().unwrap_or_else(|e| e.into_inner());
        let current = state
            .run
            .as_ref()
            .is_some_and(|run| Arc::ptr_eq(&run.cancel, &cancel));
        if !current || cancel.load(Ordering::SeqCst) {
            if let Ok(context) = initialized {
                context.stop();
            }
            return;
        }
        match initialized {
            Ok(context) => {
                state.phase = EnginePhase::Ready;
                state.context = Some(context.clone());
                completion.send_replace(Some(Ok(context.clone())));
                context
            }
            Err(reason) => {
                eprintln!("[EmbeddedDaemon] Failed to initialize core: {reason}");
                state.phase = EnginePhase::Failed {
                    reason: reason.clone(),
                };
                completion.send_replace(Some(Err(reason)));
                return;
            }
        }
    };
    let mut shutdown = context.subscribe_shutdown();
    let mut handle =
        match DaemonHandle::start_network(context.clone(), 0, true, embedded_pipe()).await {
            Ok(handle) => Some(handle),
            Err(error) => {
                context.network_failed(error.to_string());
                None
            }
        };
    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            request = network_rx.recv() => {
                let Some(request) = request else { break };
                let result = if request.enabled {
                    if handle.is_some() {
                        Ok(())
                    } else {
                        match DaemonHandle::start_network(context.clone(), 0, true, embedded_pipe()).await {
                            Ok(started) => {
                                handle = Some(started);
                                Ok(())
                            }
                            Err(error) => {
                                context.network_failed(error.to_string());
                                Err(error.to_string())
                            }
                        }
                    }
                } else {
                    if let Some(existing) = handle.take() {
                        existing.stop_network().await;
                    }
                    Ok(())
                };
                let _ = request.reply.send(result);
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
        }
    }
    if let Some(handle) = handle {
        handle.stop_network().await;
    }
}

pub fn stop_embedded() {
    let (context, run) = {
        let mut state = EMBEDDED_DAEMON.lock().unwrap_or_else(|e| e.into_inner());
        state.phase = EnginePhase::Stopped;
        (state.context.take(), state.run.take())
    };
    if let Some(context) = context {
        if let Some(port) = context.network_status().port {
            clear_active_daemon_port(port);
        }
        context.stop();
    }
    if let Some(run) = run {
        run.cancel.store(true, Ordering::SeqCst);
        run.completion
            .send_replace(Some(Err("Embedded engine stopped during startup".into())));
        let abort_handle = run.join_handle.abort_handle();
        let (tx, rx) = std::sync::mpsc::channel();
        get_or_init_runtime().spawn(async move {
            let _ = run.join_handle.await;
            let _ = tx.send(());
        });
        if rx
            .recv_timeout(std::time::Duration::from_millis(2500))
            .is_err()
        {
            abort_handle.abort();
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
            let prog_node = PathBuf::from(&prog_files).join("nodejs").join("node.exe");
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
                let res_node = if cfg!(windows) {
                    res_dir.join("node.exe")
                } else {
                    res_dir.join("node")
                };
                let script_candidates = [
                    res_dir
                        .join("dist")
                        .join("server")
                        .join("server")
                        .join("daemon-worker.js"),
                    res_dir.join("dist").join("server").join("daemon-worker.js"),
                    res_dir
                        .join("server")
                        .join("server")
                        .join("daemon-worker.js"),
                ];
                for res_script in &script_candidates {
                    if res_script.exists() {
                        let node = if is_valid_node_executable(&res_node) {
                            res_node
                        } else {
                            find_system_node()
                        };
                        return Some((res_dir.clone(), node, res_script.clone()));
                    }
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
    if let Ok(output) = Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
    {
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
fn is_kronos_process(pid: u32) -> bool {
    let pid_str = pid.to_string();
    // Use PowerShell Get-CimInstance to inspect the process name and executable path
    let script = format!(
        "$p = Get-CimInstance Win32_Process -Filter \"ProcessId = {}\"; if ($p) {{ Write-Output ($p.Name + '|' + $p.ExecutablePath + '|' + $p.CommandLine) }}",
        pid_str
    );
    if let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            let lower = text.to_lowercase();
            if lower.contains("kronos") || lower.contains("paseo") || lower.contains("daemon-worker") {
                return true;
            }
        }
    }
    false
}

#[cfg(not(windows))]
fn is_kronos_process(pid: u32) -> bool {
    if let Ok(output) = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            let lower = text.to_lowercase();
            if lower.contains("kronos") || lower.contains("paseo") || lower.contains("daemon-worker") {
                return true;
            }
        }
    }
    false
}

fn kill_process_by_pid(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output();
    }
    true
}

fn ensure_port_available() -> bool {
    // 1. If 6767 is already free, great!
    if let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:6767") {
        drop(listener);
        return true;
    }

    // 2. Port 6767 is occupied. Identify the process holding it.
    let current_pid = std::process::id();
    if let Some(pid) = find_pid_listening_on_port(6767) {
        if pid != current_pid && is_kronos_process(pid) {
            eprintln!(
                "[Daemon] Port 6767 is held by stale Kronos daemon (PID {pid}). Auto-terminating to reclaim default port..."
            );
            kill_process_by_pid(pid);

            // Wait up to 1 second for socket to fully release
            for _ in 0..10 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Ok(listener) = std::net::TcpListener::bind("127.0.0.1:6767") {
                    drop(listener);
                    eprintln!("[Daemon] Port 6767 successfully reclaimed after terminating stale PID {pid}.");
                    return true;
                }
            }
            eprintln!("[Daemon] Port 6767 was not immediately freed after terminating PID {pid}.");
        } else if pid == current_pid {
            // Already owned by this exact process
            return true;
        } else {
            eprintln!(
                "[Daemon] Port 6767 is occupied by a non-Kronos process (PID {pid}). Will fallback dynamically to an OS ephemeral port."
            );
        }
    } else {
        eprintln!("[Daemon] Port 6767 is occupied by an unknown process; will fallback dynamically.");
    }

    // Return true so start_daemon/EmbeddedDaemon can proceed and safely bind to ephemeral port (port 0)
    true
}
#[cfg(windows)]
fn sanitize_windows_path(extra_dirs: &[PathBuf]) -> Option<String> {
    let path_var = env::var("PATH").unwrap_or_default();
    let system_root = env::var("SystemRoot")
        .or_else(|_| env::var("windir"))
        .unwrap_or_else(|_| "C:\\Windows".to_string());
    let sys32 = format!("{}\\System32", system_root);
    let wbem = format!("{}\\System32\\Wbem", system_root);
    let powershell = format!("{}\\System32\\WindowsPowerShell\\v1.0", system_root);

    let parts: Vec<&str> = path_var
        .split(';')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .filter(|p| {
            !p.contains("whisper-rs-sys")
                && !p.contains("CMakeScratch")
                && !p.contains("CompilerId")
        })
        .collect();

    let mut deduplicated: Vec<String> = Vec::new();

    // 1. Extra bundled dirs (e.g. bundled node.exe and node_modules/.bin)
    for extra in extra_dirs {
        if extra.exists() {
            let s = extra.to_string_lossy().to_string();
            if !deduplicated.iter().any(|d| d.eq_ignore_ascii_case(&s)) {
                deduplicated.push(s);
            }
        }
    }

    // 2. Core Windows system directories
    for sys_dir in [&sys32, &system_root, &wbem, &powershell] {
        if !deduplicated.iter().any(|d| d.eq_ignore_ascii_case(sys_dir)) {
            deduplicated.push(sys_dir.to_string());
        }
    }

    // 3. Known Git directories (supports PCs where Git was installed without adding to system PATH)
    let mut git_candidates = Vec::new();
    if let Ok(pf) = env::var("ProgramFiles") {
        git_candidates.push(format!("{}\\Git\\cmd", pf));
        git_candidates.push(format!("{}\\Git\\bin", pf));
        git_candidates.push(format!("{}\\Git\\usr\\bin", pf));
    }
    if let Ok(pf86) = env::var("ProgramFiles(x86)") {
        git_candidates.push(format!("{}\\Git\\cmd", pf86));
        git_candidates.push(format!("{}\\Git\\bin", pf86));
        git_candidates.push(format!("{}\\Git\\usr\\bin", pf86));
    }
    if let Ok(local_app) = env::var("LOCALAPPDATA") {
        git_candidates.push(format!("{}\\Programs\\Git\\cmd", local_app));
        git_candidates.push(format!("{}\\GitHubDesktop\\bin", local_app));
    }
    for git_dir in git_candidates {
        if std::path::Path::new(&git_dir).exists()
            && !deduplicated
                .iter()
                .any(|d| d.eq_ignore_ascii_case(&git_dir))
        {
            deduplicated.push(git_dir);
        }
    }

    // 4. Existing environment PATH entries
    for p in parts {
        if !deduplicated.iter().any(|d| d.eq_ignore_ascii_case(p)) {
            deduplicated.push(p.to_string());
        }
    }
    Some(deduplicated.join(";"))
}

pub fn start_node_daemon(server_id: Option<&str>) -> Option<Child> {
    #[cfg(windows)]
    {
        // Cleanup orphaned opencode/omp processes from previous runs that may consume 100% CPU
        let _ = std::process::Command::new("taskkill")
            .args(&["/F", "/IM", "opencode.exe"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
    }

    if wait_for_daemon_health(1) {
        return None;
    }
    if !ensure_port_available() {
        return None;
    }
    let (work_dir, node_bin, script_path) = resolve_runtime()?;

    let mut cmd = Command::new(&node_bin);
    cmd.arg("--max-old-space-size=2048");
    let refresh_timeout = env::var("KRONOS_PROVIDER_REFRESH_TIMEOUT_MS")
        .or_else(|_| env::var("PASEO_PROVIDER_REFRESH_TIMEOUT_MS"))
        .unwrap_or_else(|_| "30000".to_string());
    cmd.arg(&script_path)
        .env("PASEO_LISTEN", "127.0.0.1:6767")
        .env("PASEO_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout)
        .env("KRONOS_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout)
        .env("UV_THREADPOOL_SIZE", "16");
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
    #[cfg(windows)]
    if let Some(clean_path) = sanitize_windows_path(&[work_dir.clone(), local_nm.join(".bin")]) {
        cmd.env("PATH", &clean_path);
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
                fallback_cmd
                    .arg(&script_path)
                    .env("PASEO_LISTEN", "127.0.0.1:6767")
                    .env("PASEO_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout)
                    .env("KRONOS_PROVIDER_REFRESH_TIMEOUT_MS", &refresh_timeout)
                    .env("UV_THREADPOOL_SIZE", "16");
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
                #[cfg(windows)]
                if let Some(clean_path) =
                    sanitize_windows_path(&[work_dir.clone(), local_nm.join(".bin")])
                {
                    fallback_cmd.env("PATH", &clean_path);
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
                        eprintln!(
                            "[Daemon] Fallback spawn failed with {:?}: {}",
                            sys_node, fallback_err
                        );
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
    let _guard = DAEMON_SPAWN_LOCK.lock().ok()?;
    let engine = env::var("KRONOS_DAEMON_ENGINE").unwrap_or_else(|_| "rust".to_string());
    if engine.eq_ignore_ascii_case("node") {
        ensure_port_available();
        if wait_for_daemon_health(1) {
            return None;
        }
        stop_embedded();
        if let Some(child) = start_node_daemon(server_id) {
            return Some(child);
        }
        eprintln!("[Daemon] Node daemon requested (KRONOS_DAEMON_ENGINE=node) but failed to start; refusing to hijack port with protocol-incompatible EmbeddedDaemon");
        return None;
    }

    let _ = request_embedded_start(true);
    None
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
pub(crate) mod tests {
    use super::*;
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) async fn lifecycle_child(name: &str) -> bool {
        const MARKER: &str = "KRONOS_LIFECYCLE_TEST_CHILD";
        if std::env::var(MARKER).as_deref() == Ok(name) {
            let home = PathBuf::from(std::env::var("KRONOS_HOME").unwrap());
            assert_eq!(std::env::var("PASEO_HOME").unwrap(), home.to_str().unwrap());
            assert!(home.starts_with(std::env::temp_dir()));
            assert!(home
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("kronos-lifecycle-"));
            assert_eq!(crate::get_data_dir(), home);
            return true;
        }
        let inherited = (
            std::env::var_os("KRONOS_HOME"),
            std::env::var_os("PASEO_HOME"),
        );
        let home = tempfile::Builder::new()
            .prefix("kronos-lifecycle-")
            .tempdir()
            .unwrap();
        let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
            .args([name, "--exact", "--test-threads=1", "--nocapture"])
            .env(MARKER, name)
            .env("KRONOS_HOME", home.path())
            .env("PASEO_HOME", home.path())
            .env(
                "KRONOS_DAEMON_ENGINE",
                if name.ends_with("node_mode_rejects_ipc") {
                    "node"
                } else {
                    "rust"
                },
            )
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(20), child.wait()).await;
        let status = match result {
            Ok(Ok(status)) => status,
            _ => {
                let _ = child.start_kill();
                match tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await {
                    Ok(Ok(_)) => {
                        home.close().unwrap();
                        panic!("lifecycle child failed or exceeded deadline; reaped=true owned_home_removed=true");
                    }
                    _ => {
                        let preserved = home.keep();
                        panic!(
                            "lifecycle child exit unconfirmed; preserved home {}",
                            preserved.display()
                        );
                    }
                }
            }
        };
        home.close().unwrap();
        assert_eq!(
            inherited,
            (
                std::env::var_os("KRONOS_HOME"),
                std::env::var_os("PASEO_HOME")
            )
        );
        eprintln!("lifecycle_child reaped=true owned_home_removed=true parent_env_unchanged=true");
        assert!(status.success(), "lifecycle child failed: {status}");
        false
    }

    async fn running_access() -> Arc<crate::kernel::access::AccessControl> {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Some(access) = {
                    let state = EMBEDDED_DAEMON.lock().unwrap();
                    state.context.as_ref().map(|context| context.access.clone())
                } {
                    return access;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("embedded engine did not become ready")
    }

    #[tokio::test]
    async fn test_embedded_ensure_ready_is_idempotent() {
        if !lifecycle_child("daemon::tests::test_embedded_ensure_ready_is_idempotent").await {
            return;
        }
        start_daemon(None);
        let first = running_access().await;
        start_daemon(None);
        let second = running_access().await;
        stop_embedded();
        assert!(
            Arc::ptr_eq(&first, &second),
            "attaching twice replaced the existing engine's admission authority"
        );
    }

    #[tokio::test]
    async fn test_embedded_ready_when_network_bind_fails() {
        if !lifecycle_child("daemon::tests::test_embedded_ready_when_network_bind_fails").await {
            return;
        }
        crate::kernel::server::TEST_NETWORK_BIND_FAILURE.store(true, Ordering::SeqCst);
        start_daemon(None);
        let core = ensure_embedded_ready().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while core.network_status().error.is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            core.network_status().error.as_deref(),
            Some("injected listener failure")
        );
        assert_eq!(
            get_active_daemon_port(),
            0,
            "failed listener must not advertise an endpoint"
        );
        assert_eq!(embedded_phase(), EnginePhase::Ready);
        assert!(!core.is_stopped());
        let status: serde_json::Value =
            serde_json::from_str(&crate::get_daemon_status_json()).unwrap();
        assert_eq!(status["enginePhase"], serde_json::json!({"phase":"ready"}));
        assert_eq!(status["status"], "errored");
        assert_eq!(status["networkAvailable"], false);
        assert_eq!(status["listen"], serde_json::Value::Null);
        assert_eq!(status["error"], "injected listener failure");
        stop_embedded();
    }

    fn startup_hook(fail: bool) -> StartupHook {
        let hook = StartupHook {
            entered: Arc::new(tokio::sync::Notify::new()),
            release: Arc::new(tokio::sync::Notify::new()),
            fail,
        };
        *STARTUP_HOOK.lock().unwrap() = Some(hook.clone());
        hook
    }

    #[tokio::test]
    async fn test_embedded_concurrent_ensure_shares_start() {
        if !lifecycle_child("daemon::tests::test_embedded_concurrent_ensure_shares_start").await {
            return;
        }
        let hook = startup_hook(false);
        let first = tokio::spawn(ensure_embedded_ready());
        tokio::time::timeout(std::time::Duration::from_secs(3), hook.entered.notified())
            .await
            .unwrap();
        let second = tokio::spawn(ensure_embedded_ready());
        assert_eq!(embedded_phase(), EnginePhase::Starting);
        hook.release.notify_one();
        let a = tokio::time::timeout(std::time::Duration::from_secs(3), first)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let b = tokio::time::timeout(std::time::Duration::from_secs(3), second)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(
            Arc::ptr_eq(&a, &b),
            "concurrent ensures created different engines"
        );
        assert_eq!(embedded_phase(), EnginePhase::Ready);
        stop_embedded();
    }

    #[tokio::test]
    async fn test_embedded_start_failure_releases_waiters() {
        if !lifecycle_child("daemon::tests::test_embedded_start_failure_releases_waiters").await {
            return;
        }
        let hook = startup_hook(true);
        let first = tokio::spawn(ensure_embedded_ready());
        tokio::time::timeout(std::time::Duration::from_secs(3), hook.entered.notified())
            .await
            .unwrap();
        let second = tokio::spawn(ensure_embedded_ready());
        hook.release.notify_one();
        for waiter in [first, second] {
            let result = tokio::time::timeout(std::time::Duration::from_secs(3), waiter)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                result.err().as_deref(),
                Some("injected core startup failure")
            );
        }
        assert_eq!(
            embedded_phase(),
            EnginePhase::Failed {
                reason: "injected core startup failure".into()
            }
        );
        assert_eq!(get_active_daemon_port(), 0);
        let status: serde_json::Value =
            serde_json::from_str(&crate::get_daemon_status_json()).unwrap();
        assert_eq!(status["status"], "errored");
        assert_eq!(
            status["enginePhase"],
            serde_json::json!({"phase":"failed","reason":"injected core startup failure"})
        );
        assert_eq!(status["error"], "injected core startup failure");
        *STARTUP_HOOK.lock().unwrap() = None;
        start_daemon(None);
        assert!(
            ensure_embedded_ready().await.is_ok(),
            "explicit start should retry failed initialization"
        );
        stop_embedded();
    }

    #[tokio::test]
    async fn test_embedded_stop_during_start_releases_waiters() {
        if !lifecycle_child("daemon::tests::test_embedded_stop_during_start_releases_waiters").await
        {
            return;
        }
        let hook = startup_hook(false);
        let waiter = tokio::spawn(ensure_embedded_ready());
        tokio::time::timeout(std::time::Duration::from_secs(3), hook.entered.notified())
            .await
            .unwrap();
        stop_embedded();
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), waiter)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.err().as_deref(),
            Some("Embedded engine stopped during startup")
        );
        assert_eq!(embedded_phase(), EnginePhase::Stopped);
        *STARTUP_HOOK.lock().unwrap() = None;
        let new_core = ensure_embedded_ready().await.unwrap();
        hook.release.notify_one();
        assert!(Arc::ptr_eq(
            &new_core,
            &ensure_embedded_ready().await.unwrap()
        ));
        assert_eq!(embedded_phase(), EnginePhase::Ready);
        stop_embedded();
    }

    #[tokio::test]
    async fn test_embedded_ui_detach_preserves_engine() {
        if !lifecycle_child("daemon::tests::test_embedded_ui_detach_preserves_engine").await {
            return;
        }
        let first = ensure_embedded_ready().await.unwrap();
        let detached_transport_subscription = first.subscribe_shutdown();
        drop(detached_transport_subscription);
        let second = ensure_embedded_ready().await.unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(
            !second.is_stopped(),
            "dropping UI transport subscription stopped engine"
        );
        assert_eq!(embedded_phase(), EnginePhase::Ready);
        stop_embedded();
        assert!(second.is_stopped());
        assert_eq!(get_active_daemon_port(), 0);
    }

    #[tokio::test]
    async fn test_embedded_node_mode_rejects_ipc() {
        if !lifecycle_child("daemon::tests::test_embedded_node_mode_rejects_ipc").await {
            return;
        }
        assert_eq!(
            ensure_embedded_ready().await.err().as_deref(),
            Some("Embedded IPC is unavailable in Node engine mode")
        );
        assert_eq!(embedded_phase(), EnginePhase::Stopped);
        assert_eq!(get_active_daemon_port(), 0);
    }

    #[tokio::test]
    async fn test_embedded_network_shutdown_preserves_core() {
        if !lifecycle_child("daemon::tests::test_embedded_network_shutdown_preserves_core").await {
            return;
        }
        let context = SessionContext::initialize().unwrap();
        let network = DaemonHandle::start_network(context.clone(), 0, false, None)
            .await
            .unwrap();
        assert_eq!(context.network_status().port, Some(network.port));
        network.stop_network().await;
        assert!(!context.is_stopped());
        assert_eq!(context.network_status().port, None);
        assert_eq!(get_active_daemon_port(), 0);
        let hello = serde_json::json!({"type":"hello","protocolVersion":1,
            "auth":{"kind":"localCredential","token":context.access.local_credential}});
        assert_eq!(context.access.authenticate_hello(&hello).await, Ok(()));
        network.stop().await;
        assert!(context.is_stopped());
    }

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
        assert!(
            !is_embedded_running(),
            "Node engine mode must never silently fall back to EmbeddedDaemon on port 6767"
        );
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
        assert!(
            child.is_none(),
            "Rust engine boots embedded daemon without external Child process"
        );
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
        assert!(
            !is_embedded_running(),
            "Daemon must not remain running after immediate stop"
        );
        std::env::remove_var("KRONOS_DAEMON_ENGINE");
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
