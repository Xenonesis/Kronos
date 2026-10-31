// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod editor;

pub mod kernel;
pub mod process_job;
pub mod native_diff;
pub mod native_pty;
mod notifications;
mod tray;

use notifications::{notify_task_complete, setup_notification_listeners, show_notification};
use tray::{setup_tray, update_active_agents};
mod attachments;
mod dialogs;
use attachments::{
    copy_attachment_file, delete_attachment_file, garbage_collect_attachment_files,
    read_file_base64, write_attachment_base64, write_attachment_bytes,
};
use native_diff::compute_file_diff;
use native_pty::{
    kill_native_terminal, resize_native_terminal, spawn_native_terminal, write_native_terminal,
    NativePtyManager,
};

use daemon::{start_daemon, stop_daemon, wait_for_daemon_health, DaemonState};
use dialogs::{pick_file, pick_folder};
use editor::open_in_editor;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

#[cfg(target_os = "windows")]
pub fn apply_mica_effect<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMWA_CAPTION_COLOR, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };

    if let Ok(hwnd) = window.hwnd() {
        let hwnd_raw = hwnd.0 as windows_sys::Win32::Foundation::HWND;
        unsafe {
            // 1. Force Immersive Dark Mode for window frame & titlebar (attribute 20, TRUE)
            let dark_mode: i32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            // 2. Apply Mica Backdrop on Windows 11 (DWMSBT_MAINWINDOW = 2, attribute 38)
            let backdrop_type: u32 = DWMSBT_MAINWINDOW as u32;
            let _ = DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                &backdrop_type as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 3. Caption background: #0A0A0A (BGR format: 0x000A0A0A, attribute 35)
            let caption_color: u32 = 0x000A0A0A;
            let _ = DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_CAPTION_COLOR as u32,
                &caption_color as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 4. Caption text: #EAEAEA (BGR format: 0x00EAEAEA, attribute 36)
            let text_color: u32 = 0x00EAEAEA;
            let _ = DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_TEXT_COLOR as u32,
                &text_color as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply_mica_effect<R: tauri::Runtime>(_window: &WebviewWindow<R>) {}

pub fn apply_windows_window_decorations<R: tauri::Runtime>(win: &WebviewWindow<R>) {
    apply_mica_effect(win);
}

pub fn toggle_window<R: tauri::Runtime>(win: &WebviewWindow<R>) {
    let is_minimized = win.is_minimized().unwrap_or(false);
    let is_visible = win.is_visible().unwrap_or(false);
    let is_focused = win.is_focused().unwrap_or(false);
    if is_visible && !is_minimized && is_focused {
        let _ = win.hide();
    } else {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let _ = win.emit(
            "kronos://quick-toggle",
            serde_json::json!({
                "timestamp": now_ms,
            }),
        );
    }
}

pub fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        toggle_window(&win);
    }
}

#[derive(Clone, Default)]
pub struct UpdateState(pub std::sync::Arc<tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>>);

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct DesktopAppUpdateCheckResult {
    pub hasUpdate: bool,
    pub readyToInstall: bool,
    pub currentVersion: Option<String>,
    pub latestVersion: Option<String>,
    pub body: Option<String>,
    pub date: Option<String>,
    pub errorMessage: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[allow(non_snake_case)]
pub struct DesktopAppUpdateInstallResult {
    pub installed: bool,
    pub version: Option<String>,
    pub message: String,
}

#[tauri::command]
async fn check_app_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdateState>,
    release_channel: Option<String>,
    intent: Option<String>,
) -> Result<DesktopAppUpdateCheckResult, String> {
    let _ = (release_channel, intent);
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            return Ok(DesktopAppUpdateCheckResult {
                hasUpdate: false,
                readyToInstall: false,
                currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
                latestVersion: None,
                body: None,
                date: None,
                errorMessage: Some(format!("Updater not initialized: {e}")),
            });
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            println!(
                "[updater] Update available: v{} (current: v{})",
                update.version, update.current_version
            );
            let result = DesktopAppUpdateCheckResult {
                hasUpdate: true,
                readyToInstall: true,
                currentVersion: Some(update.current_version.clone()),
                latestVersion: Some(update.version.clone()),
                body: update.body.clone(),
                date: update.date.map(|d| d.to_string()),
                errorMessage: None,
            };
            *state.0.lock().await = Some(update);
            Ok(result)
        }
        Ok(None) => {
            println!("[updater] Application is up to date");
            Ok(DesktopAppUpdateCheckResult {
                hasUpdate: false,
                readyToInstall: false,
                currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
                latestVersion: None,
                body: None,
                date: None,
                errorMessage: None,
            })
        }
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("valid release JSON") || err_msg.contains("404") {
                println!(
                    "[updater] No remote release manifest found; running current release (v{})",
                    env!("CARGO_PKG_VERSION")
                );
                Ok(DesktopAppUpdateCheckResult {
                    hasUpdate: false,
                    readyToInstall: false,
                    currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
                    latestVersion: None,
                    body: None,
                    date: None,
                    errorMessage: None,
                })
            } else {
                eprintln!("[updater] Failed to check for updates: {e}");
                Ok(DesktopAppUpdateCheckResult {
                    hasUpdate: false,
                    readyToInstall: false,
                    currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
                    latestVersion: None,
                    body: None,
                    date: None,
                    errorMessage: Some(err_msg),
                })
            }
        }
    }
}

#[tauri::command]
async fn install_app_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdateState>,
    release_channel: Option<String>,
) -> Result<DesktopAppUpdateInstallResult, String> {
    let _ = release_channel;
    let mut lock = state.0.lock().await;
    let update = if let Some(u) = lock.take() {
        u
    } else {
        let updater = app.updater().map_err(|e| format!("Updater not initialized: {e}"))?;
        match updater.check().await {
            Ok(Some(u)) => u,
            Ok(None) => {
                return Ok(DesktopAppUpdateInstallResult {
                    installed: false,
                    version: None,
                    message: "No pending update ready to install.".into(),
                });
            }
            Err(e) => {
                return Err(format!("Check before install failed: {e}"));
            }
        }
    };

    let version = update.version.clone();
    println!("[updater] Downloading and installing update v{}...", version);
    let mut downloaded = 0;
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded += chunk_length;
                println!("[updater] Downloaded {downloaded} bytes of {:?}", content_length);
            },
            || {
                println!("[updater] Download complete, installing update...");
            },
        )
        .await
        .map_err(|e| format!("Download/install failed: {e}"))?;

    Ok(DesktopAppUpdateInstallResult {
        installed: true,
        version: Some(version),
        message: "Update applied successfully. Restart Kronos to launch the new version.".into(),
    })
}


fn detect_and_configure_portable_mode() -> Option<PathBuf> {
    // Check KRONOS_PORTABLE=1 env var
    let env_portable = std::env::var("KRONOS_PORTABLE").map(|v| v == "1").unwrap_or(false);

    // Check if adjacent .kronos-data directory exists next to current executable
    let mut adjacent_data_dir = None;
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join(".kronos-data");
            if candidate.exists() && candidate.is_dir() {
                adjacent_data_dir = Some(candidate);
            }
        }
    }

    if env_portable || adjacent_data_dir.is_some() {
        let portable_dir = adjacent_data_dir.unwrap_or_else(|| {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let candidate = exe_dir.join(".kronos-data");
                    let _ = fs::create_dir_all(&candidate);
                    return candidate;
                }
            }
            PathBuf::from(".kronos-data")
        });

        let dir_str = portable_dir.to_string_lossy().to_string();
        std::env::set_var("KRONOS_HOME", &dir_str);
        std::env::set_var("PASEO_HOME", &dir_str);
        return Some(portable_dir);
    }
    None
}

pub fn get_data_dir() -> PathBuf {
    if let Ok(kronos_home) = std::env::var("KRONOS_HOME") {
        if !kronos_home.is_empty() {
            let path = PathBuf::from(kronos_home);
            let _ = fs::create_dir_all(&path);
            return path;
        }
    }
    if let Ok(paseo_home) = std::env::var("PASEO_HOME") {
        if !paseo_home.is_empty() {
            let path = PathBuf::from(paseo_home);
            let _ = fs::create_dir_all(&path);
            return path;
        }
    }
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".paseo");
    let _ = fs::create_dir_all(&home);
    home
}

fn get_or_create_server_id() -> String {
    let home = get_data_dir();
    let _ = fs::create_dir_all(&home);

    let server_id_file = home.join("server-id");
    if let Ok(content) = fs::read_to_string(&server_id_file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let id = format!("srv_{:x}{:x}", now, pid);
    let _ = fs::write(&server_id_file, format!("{}\n", id));
    id
}

fn get_daemon_status_json() -> String {
    let home = get_data_dir();

    let server_id = get_or_create_server_id();
    let mut local_credential = String::new();
    let credential_file = home.join("local-credential");
    if let Ok(content) = fs::read_to_string(&credential_file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            local_credential = trimmed.to_string();
        }
    }

    let is_healthy = wait_for_daemon_health(1);
    let listen = "127.0.0.1:6767".to_string();
    let hostname = "localhost".to_string();
    let detected_pid = if is_healthy {
        daemon::find_pid_listening_on_port(6767)
    } else {
        None
    };
    let status = if is_healthy { "running" } else { "stopped" };

    serde_json::json!({
        "serverId": server_id,
        "status": status,
        "listen": Some(listen),
        "hostname": hostname,
        "pid": detected_pid,
        "localCredential": local_credential,
        "home": home.to_string_lossy().to_string(),
        "version": env!("CARGO_PKG_VERSION"),
        "desktopManaged": true,
        "ownedByDesktop": true,
        "startedAt": if is_healthy { Some("2026-09-26T12:00:00.000Z") } else { None },
        "error": null
    })
    .to_string()
}

#[tauri::command]
fn get_live_daemon_status() -> serde_json::Value {
    serde_json::from_str(&get_daemon_status_json()).unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
fn get_live_local_credential() -> Option<String> {
    let home = get_data_dir();
    let credential_file = home.join("local-credential");
    if let Ok(content) = fs::read_to_string(&credential_file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}
fn read_tail_file(path: &std::path::Path, max_lines: usize) -> String {
    if !path.exists() {
        return String::new();
    }
    match fs::read_to_string(path) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().collect();
            if lines.len() <= max_lines {
                text
            } else {
                lines[lines.len() - max_lines..].join("\n")
            }
        }
        Err(_) => String::new(),
    }
}

#[tauri::command]
fn get_log_contents(log_type: String) -> serde_json::Value {
    let home = get_data_dir();
    let file_name = match log_type.as_str() {
        "daemon" => "daemon.log",
        "app" => "app.log",
        "crash" => "crash.log",
        _ => "app.log",
    };
    let file_path = home.join(file_name);
    let log_path = file_path.to_string_lossy().to_string();
    let contents = read_tail_file(&file_path, 100);
    serde_json::json!({
        "logPath": log_path,
        "contents": contents
    })
}

#[tauri::command]
fn log_desktop_event(level: String, message: String) {
    let home = get_data_dir();

    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entry = format!("[ts:{}] [{}] {}\n", now, level, message);

    let app_log = home.join("app.log");
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&app_log)
    {
        let _ = f.write_all(entry.as_bytes());
    }

    if level == "error" || level == "crash" || level == "panic" {
        let crash_log = home.join("crash.log");
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
        {
            let _ = f.write_all(entry.as_bytes());
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DirectoryValidationResult {
    pub exists: bool,
    pub is_dir: bool,
    pub readable: bool,
    pub error: Option<String>,
}

#[tauri::command]
fn fast_validate_directory(path: String) -> DirectoryValidationResult {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return DirectoryValidationResult {
            exists: false,
            is_dir: false,
            readable: false,
            error: Some("Directory does not exist.".to_string()),
        };
    }
    let metadata = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) => {
            return DirectoryValidationResult {
                exists: true,
                is_dir: false,
                readable: false,
                error: Some(format!("Cannot access path: {}", e)),
            }
        }
    };
    if !metadata.is_dir() {
        return DirectoryValidationResult {
            exists: true,
            is_dir: false,
            readable: false,
            error: Some("Selected path is a file, not a directory.".to_string()),
        };
    }
    match std::fs::read_dir(p) {
        Ok(_) => DirectoryValidationResult {
            exists: true,
            is_dir: true,
            readable: true,
            error: None,
        },
        Err(e) => DirectoryValidationResult {
            exists: true,
            is_dir: true,
            readable: false,
            error: Some(format!("Permission denied reading directory: {}", e)),
        },
    }
}
#[tauri::command]
fn start_desktop_daemon(state: tauri::State<DaemonState>) -> serde_json::Value {
    let mut lock = state.process.lock().unwrap();
    let is_running = if let Some(child) = lock.as_mut() {
        child.try_wait().map(|status| status.is_none()).unwrap_or(false)
    } else {
        false
    };
    if !is_running && !wait_for_daemon_health(1) {
        let server_id = get_or_create_server_id();
        let new_child = daemon::start_daemon(Some(&server_id));
        *lock = new_child;
        let _ = wait_for_daemon_health(40);
    }
    drop(lock);
    get_live_daemon_status()
}

#[tauri::command]
fn stop_desktop_daemon(state: tauri::State<DaemonState>) -> serde_json::Value {
    stop_daemon(&state);
    let mut status: serde_json::Value =
        serde_json::from_str(&get_daemon_status_json()).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = status.as_object_mut() {
        obj.insert("status".to_string(), serde_json::json!("stopped"));
        obj.insert("pid".to_string(), serde_json::Value::Null);
    }
    status
}

#[tauri::command]
fn restart_desktop_daemon(state: tauri::State<DaemonState>) -> serde_json::Value {
    let success = daemon::restart_daemon(&state);
    if success {
        let _ = wait_for_daemon_health(30);
    }
    get_live_daemon_status()
}

#[cfg(windows)]
fn get_system_idle_time_ms() -> u64 {
    use std::mem;
    #[repr(C)]
    #[allow(non_camel_case_types)]
    #[allow(clippy::upper_case_acronyms)]
    struct LASTINPUTINFO {
        cb_size: u32,
        dw_time: u32,
    }
    extern "system" {
        fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
        fn GetTickCount() -> u32;
    }
    unsafe {
        let mut lii = LASTINPUTINFO {
            cb_size: mem::size_of::<LASTINPUTINFO>() as u32,
            dw_time: 0,
        };
        if GetLastInputInfo(&mut lii) != 0 {
            let now = GetTickCount();
            return now.wrapping_sub(lii.dw_time) as u64;
        }
    }
    0
}

#[cfg(not(windows))]
fn get_system_idle_time_ms() -> u64 {
    0
}

#[tauri::command]
fn get_system_idle_time() -> u64 {
    get_system_idle_time_ms()
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathInspectResult {
    pub exists: bool,
    pub is_dir: bool,
    pub is_file: bool,
    pub byte_size: u64,
    pub modified_ms: u64,
    pub readable: bool,
}

#[tauri::command]
fn fast_inspect_path(path: String) -> PathInspectResult {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return PathInspectResult {
            exists: false,
            is_dir: false,
            is_file: false,
            byte_size: 0,
            modified_ms: 0,
            readable: false,
        };
    }
    match std::fs::metadata(p) {
        Ok(meta) => {
            let is_dir = meta.is_dir();
            let is_file = meta.is_file();
            let byte_size = meta.len();
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            PathInspectResult {
                exists: true,
                is_dir,
                is_file,
                byte_size,
                modified_ms,
                readable: true,
            }
        }
        Err(_) => PathInspectResult {
            exists: true,
            is_dir: false,
            is_file: false,
            byte_size: 0,
            modified_ms: 0,
            readable: false,
        },
    }
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let home = get_data_dir();
        let crash_log = home.join("crash.log");

        use std::io::Write;
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<dyn Any>".to_string()
        };

        let entry = format!(
            "[ts:{}] [panic] Thread '{}' panicked at '{}' ({})\n",
            now,
            std::thread::current().name().unwrap_or("unnamed"),
            payload,
            location
        );

        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_log)
        {
            let _ = f.write_all(entry.as_bytes());
        }
        eprintln!("{}", entry);
    }));
}

fn check_and_log_crashpad_dumps() {
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let reports_dir = PathBuf::from(local_appdata)
            .join("sh.paseo.desktop.tauri")
            .join("EBWebView")
            .join("Crashpad")
            .join("reports");
        if reports_dir.exists() {
            if let Ok(entries) = fs::read_dir(&reports_dir) {
                let mut dumps = Vec::new();
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("dmp") {
                        if let Ok(metadata) = entry.metadata() {
                            dumps.push((path.display().to_string(), metadata.len()));
                        }
                    }
                }
                if !dumps.is_empty() {
                    let home = get_data_dir();
                    let crash_log = home.join("crash.log");
                    use std::io::Write;
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let msg = format!(
                        "[ts:{}] [crashpad] Found {} WebView2 crashpad dump(s) in reports folder: {:?}\n",
                        now,
                        dumps.len(),
                        dumps
                    );
                    if let Ok(mut f) = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&crash_log)
                    {
                        let _ = f.write_all(msg.as_bytes());
                    }
                }
            }
        }
    }
}

fn main() {
    detect_and_configure_portable_mode();

    if let Some(code) = kernel::cli::parse_and_dispatch() {
        std::process::exit(code);
    }

    setup_panic_hook();
    check_and_log_crashpad_dumps();

    #[cfg(target_os = "windows")]
    {
        // Prevent DWM occlusion freezes and Intel GPU driver watchdog timeouts
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--disable-features=CalculateNativeWinOcclusion --disable-gpu-watchdog",
        );
    }

    let server_id = get_or_create_server_id();
    let daemon_child = start_daemon(Some(&server_id));
    let state = DaemonState {
        process: Mutex::new(daemon_child),
    };
    let daemon_server_id = server_id.clone();
    let _ = wait_for_daemon_health(2);
    let status_json = get_daemon_status_json();
    let bridge_script = format!(
        r#"
try {{
  const daemonData = {status_json};
  const listenEndpoint = (daemonData.listen || '127.0.0.1:6767').replace(/^http:\/\//, '').replace(/\/.*$/, '');
  const endpoint = listenEndpoint;

  window.paseoDesktop = {{
    platform: 'win32',
    windowChromeMode: 'custom-windows',
    validateDirectory: async function(path) {{
      try {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('fast_validate_directory', {{ path: path }});
        }}
      }} catch (e) {{
        console.error('[TauriBridge] validateDirectory failed:', e);
      }}
      return null;
    }},
    events: {{
      on: function(event, callback) {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.listen) {{
            let unlistenPromise = window.__TAURI_INTERNALS__.listen(event, function(ev) {{
              try {{
                callback(ev && ev.payload !== undefined ? ev.payload : ev);
              }} catch (err) {{
                console.error('[TauriBridge] Event callback failed:', err);
              }}
            }});
            return async function() {{
              try {{
                let unlisten = await unlistenPromise;
                if (typeof unlisten === 'function') unlisten();
              }} catch (_) {{}}
            }};
          }}
        }} catch (e) {{
          console.error('[TauriBridge] events.on failed:', e);
        }}
        return function() {{}};
      }}
    }},
    dialog: {{
      open: async function(options) {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            if (options && options.directory) {{
              return await window.__TAURI_INTERNALS__.invoke('pick_folder');
            }}
            return await window.__TAURI_INTERNALS__.invoke('pick_file');
          }}
        }} catch (e) {{
          console.error('[TauriBridge] Dialog open failed:', e);
        }}
        return null;
      }},
      ask: async function(_message, _options) {{
        return true;
      }},
      askWithCheckbox: async function(_message, _options) {{
        return {{ confirmed: true, dontAskAgain: false }};
      }}
    }},
    opener: {{
      openUrl: async function(url) {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke('plugin:opener|open_url', {{ url: url }});
          }}
        }} catch (e) {{
          window.open(url, '_blank');
        }}
      }}
    }},
    nativeTerminal: {{
      spawn: async function(id, options, onEvent) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          const channel = new window.__TAURI_INTERNALS__.Channel();
          channel.onmessage = onEvent;
          return await window.__TAURI_INTERNALS__.invoke('spawn_native_terminal', {{ id, options, onEvent: channel }});
        }}
        throw new Error('Native terminal unavailable');
      }},
      write: async function(id, data) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('write_native_terminal', {{ id, data }});
        }}
      }},
      resize: async function(id, cols, rows) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('resize_native_terminal', {{ id, cols, rows }});
        }}
      }},
      kill: async function(id) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('kill_native_terminal', {{ id }});
        }}
      }}
    }},
    nativeDiff: {{
      compute: async function(oldText, newText, contextLines) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('compute_file_diff', {{ oldText, newText, contextLines }});
        }}
        return null;
      }}
    }},
    invoke: async function(command, args) {{
      if (command === 'start_desktop_daemon') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const res = await window.__TAURI_INTERNALS__.invoke('start_desktop_daemon');
            if (res) return res;
          }}
        }} catch (e) {{}}
        return daemonData;
      }}
      if (command === 'desktop_daemon_status') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const live = await window.__TAURI_INTERNALS__.invoke('get_live_daemon_status');
            if (live) return live;
          }}
        }} catch (e) {{}}
        return daemonData;
      }}
      if (command === 'desktop_daemon_logs') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const res = await window.__TAURI_INTERNALS__.invoke('get_log_contents', {{ logType: 'daemon', log_type: 'daemon' }});
            if (res) return res;
          }}
        }} catch (e) {{}}
        return {{ logPath: daemonData.home + '/daemon.log', contents: '' }};
      }}
      if (command === 'desktop_app_logs') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const res = await window.__TAURI_INTERNALS__.invoke('get_log_contents', {{ logType: 'app', log_type: 'app' }});
            if (res) return res;
          }}
        }} catch (e) {{}}
        return {{ logPath: daemonData.home + '/app.log', contents: '' }};
      }}
      if (command === 'desktop_crash_logs') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const res = await window.__TAURI_INTERNALS__.invoke('get_log_contents', {{ logType: 'crash', log_type: 'crash' }});
            if (res) return res;
          }}
        }} catch (e) {{}}
        return {{ logPath: daemonData.home + '/crash.log', contents: '' }};
      }}
      if (command === 'stop_desktop_daemon') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const res = await window.__TAURI_INTERNALS__.invoke('stop_desktop_daemon');
            if (res) return res;
          }}
        }} catch (e) {{}}
        return daemonData;
      }}
      if (command === 'restart_desktop_daemon') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const res = await window.__TAURI_INTERNALS__.invoke('restart_desktop_daemon');
            if (res) return res;
          }}
        }} catch (e) {{}}
        return daemonData;
      }}
      if (command === 'get_cli_install_status' || command === 'install_cli') {{
        return {{ status: 'installed' }};
      }}
      if (command === 'desktop_update_diagnostics') {{
        return {{ platform: 'win32', currentVersion: daemonData.version, targetVersion: null, targetVersionError: null, files: [] }};
      }}
      if (command === 'desktop_sandbox_diagnostics') {{
        return {{ enabled: false, available: false, error: null }};
      }}
      if (command === 'read_legacy_skill_selection' || command === 'delete_legacy_skill_selection') {{
        return null;
      }}
      if (command === 'migrate_legacy_desktop_settings') {{
        return null;
      }}
      if (command === 'open_directory_dialog' || command === 'pick_folder') {{
        return await window.__TAURI_INTERNALS__.invoke('pick_folder');
      }}
      if (command === 'open_file_dialog' || command === 'pick_file') {{
        return await window.__TAURI_INTERNALS__.invoke('pick_file');
      }}
      if (command === 'show_notification') {{
        return await window.__TAURI_INTERNALS__.invoke('show_notification', args || {{}});
      }}
      if (command === 'notify_task_complete') {{
        return await window.__TAURI_INTERNALS__.invoke('notify_task_complete', args || {{}});
      }}
      if (command === 'update_active_agents') {{
        return await window.__TAURI_INTERNALS__.invoke('update_active_agents', args || {{}});
      }}
      if (command === 'desktop_local_credential') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const token = await window.__TAURI_INTERNALS__.invoke('get_live_local_credential');
            if (token) return token;
          }}
        }} catch (e) {{}}
        return daemonData.localCredential || null;
      }}
      if (command === 'get_desktop_settings' || command === 'patch_desktop_settings') {{
        return {{
          releaseChannel: 'stable',
          notifications: {{ playSound: true }},
          daemon: {{ manageBuiltInDaemon: true, keepRunningAfterQuit: false }}
        }};
      }}
      if (command === 'desktop_get_system_idle_time') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke('get_system_idle_time');
          }}
        }} catch (e) {{}}
        return 0;
      }}
      if (command === 'fast_validate_directory') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke('fast_validate_directory', args || {{}});
          }}
        }} catch (e) {{
          return {{ exists: false, is_dir: false, readable: false, error: String(e) }};
        }}
        return null;
      }}
      if (command === 'fast_inspect_path') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke('fast_inspect_path', args || {{}});
          }}
        }} catch (e) {{}}
        return null;
      }}
      if (
        command === 'write_attachment_base64' ||
        command === 'write_attachment_bytes' ||
        command === 'copy_attachment_file' ||
        command === 'read_file_base64' ||
        command === 'delete_attachment_file' ||
        command === 'garbage_collect_attachment_files'
      ) {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke(command, args || {{}});
          }}
        }} catch (e) {{
          console.error('[TauriBridge] Attachment command failed:', command, e);
          throw e;
        }}
        return null;
      }}
      return null;
    }}
  }};

  // Seed/reconcile valid StoredHostProfile matching StoredHostProfileSchema
  try {{
    const REGISTRY_KEY = '@paseo:daemon-registry';
    let registry = [];
    try {{
      const raw = localStorage.getItem(REGISTRY_KEY);
      if (raw) registry = JSON.parse(raw) || [];
    }} catch (e) {{}}

    // Remove any stale loopback entries that do not match the current daemon's serverId
    registry = registry.filter(h => {{
      if (!h || !h.connections) return true;
      const isLoopback = h.connections.some(c =>
        c.type === 'directTcp' && (c.endpoint.includes('127.0.0.1') || c.endpoint.includes('localhost'))
      );
      return !isLoopback || h.serverId === daemonData.serverId;
    }});

    const existing = registry.find(h => h && h.serverId === daemonData.serverId);
    if (!existing) {{
      registry.unshift({{
        serverId: daemonData.serverId,
        label: daemonData.hostname || 'Localhost',
        appearance: {{ color: 'none', badgeDisplay: null }},
        lifecycle: {{}},
        connections: [{{
          id: 'direct:' + endpoint,
          type: 'directTcp',
          endpoint: endpoint,
          useTls: false
        }}],
        preferredConnectionId: 'direct:' + endpoint,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString()
      }});
    }} else {{
      const conn = existing.connections && existing.connections.find(c => c.type === 'directTcp');
      if (conn) conn.endpoint = endpoint;
    }}
    localStorage.setItem(REGISTRY_KEY, JSON.stringify(registry));
  }} catch (e) {{}}
  // Migrate or sync last-workspace-route-selection so it points to the active serverId
  try {{
    const LAST_WS_KEY = 'paseo:last-workspace-route-selection';
    const rawWs = localStorage.getItem(LAST_WS_KEY);
    if (rawWs) {{
      const parsed = JSON.parse(rawWs);
      if (parsed && parsed.serverId && parsed.serverId !== daemonData.serverId) {{
        parsed.serverId = daemonData.serverId;
        localStorage.setItem(LAST_WS_KEY, JSON.stringify(parsed));
      }}
    }}
  }} catch (e) {{}}
  // Reset launchTarget to chat so New workspace composer opens in Chat mode
  try {{
    const PREF_KEY = '@paseo:create-agent-preferences';
    const raw = localStorage.getItem(PREF_KEY);
    if (raw) {{
      const parsed = JSON.parse(raw);
      if (parsed.launchTarget && parsed.launchTarget.kind !== 'chat') {{
        parsed.launchTarget = {{ kind: 'chat' }};
        localStorage.setItem(PREF_KEY, JSON.stringify(parsed));
      }}
    }}
  }} catch (e) {{}}
  // Setup deep-link listener from Tauri runtime
  try {{
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.listen) {{
      window.__TAURI_INTERNALS__.listen('single-instance-deep-link', (event) => {{
        const args = event.payload || [];
        const deepLink = args.find(a => typeof a === 'string' && (a.startsWith('kronos://') || a.startsWith('paseo://')));
        if (deepLink) {{
          window.dispatchEvent(new CustomEvent('paseo:deep-link', {{ detail: deepLink }}));
        }}
      }});
    }}
  }} catch (e) {{}}
  // Active Error & Crash Reporting Listeners
  try {{
    window.addEventListener('error', (event) => {{
      try {{
        const loc = (event.filename || 'unknown') + ':' + event.lineno + ':' + event.colno;
        const errStack = event.error && event.error.stack ? '\n' + event.error.stack : '';
        const msg = 'Uncaught Error: ' + (event.message || '') + ' at ' + loc + errStack;
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          window.__TAURI_INTERNALS__.invoke('log_desktop_event', {{ level: 'error', message: msg }}).catch(() => {{}});
        }}
      }} catch (_) {{}}
    }});
    window.addEventListener('unhandledrejection', (event) => {{
      try {{
        const reason = event.reason;
        const stack = reason && reason.stack ? reason.stack : (typeof reason === 'object' ? JSON.stringify(reason) : String(reason));
        const msg = 'Unhandled Promise Rejection: ' + (reason && reason.message ? reason.message : reason) + '\nStack: ' + stack;
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          window.__TAURI_INTERNALS__.invoke('log_desktop_event', {{ level: 'error', message: msg }}).catch(() => {{}});
        }}
      }} catch (_) {{}}
    }});
  }} catch (e) {{}}
}} catch (e) {{}}
"#
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
                let _ = win.emit("single-instance-deep-link", argv);
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .manage(std::sync::Arc::new(crate::process_job::JobSupervisor::new().ok()))
        .manage(NativePtyManager::new())
        .manage(UpdateState::default())
        .invoke_handler(tauri::generate_handler![
            open_in_editor,
            pick_folder,
            pick_file,
            update_active_agents,
            show_notification,
            notify_task_complete,
            get_live_daemon_status,
            get_live_local_credential,
            get_log_contents,
            log_desktop_event,
            fast_validate_directory,
            start_desktop_daemon,
            stop_desktop_daemon,
            restart_desktop_daemon,
            get_system_idle_time,
            fast_inspect_path,
            write_attachment_base64,
            write_attachment_bytes,
            copy_attachment_file,
            read_file_base64,
            delete_attachment_file,
            garbage_collect_attachment_files,
            spawn_native_terminal,
            write_native_terminal,
            resize_native_terminal,
            kill_native_terminal,
            compute_file_diff,
            check_app_update,
            install_app_update
        ])
        .setup(move |app| {
            let _ = setup_tray(app.handle());
            setup_notification_listeners(app.handle());
            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Kronos")
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .shadow(false)
                .visible(true)
                .initialization_script(&bridge_script);

            let win = win_builder.build().ok();
            if let Some(main_window) = &win {
                apply_mica_effect(main_window);
                let _ = main_window.show();
                let _ = main_window.set_focus();
            }

            let handle_for_daemon = app.handle().clone();
            let daemon_id = daemon_server_id.clone();
            std::thread::spawn(move || {
                let is_healthy = wait_for_daemon_health(50);
                if is_healthy {
                    println!("[daemon] Host daemon confirmed healthy on 127.0.0.1:6767");
                    if let Some(w) = handle_for_daemon.get_webview_window("main") {
                        let eval_script = format!(
                            "window.dispatchEvent(new CustomEvent('kronos:daemon-ready', {{ detail: {{ serverId: '{}' }} }}));",
                            daemon_id
                        );
                        let _ = w.eval(&eval_script);
                    }
                } else {
                    eprintln!("[daemon] Host daemon not responding after startup timeout");
                }
            });

            // Register global shortcuts to toggle main window visibility
            let window_clone = win.clone();
            for shortcut in ["CommandOrControl+Shift+K", "CommandOrControl+Shift+P"] {
                let window_clone = window_clone.clone();
                if let Err(err) = app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(window_clone) = &window_clone {
                            toggle_window(window_clone);
                        } else if let Some(window_clone) = app.get_webview_window("main") {
                            toggle_window(&window_clone);
                        }
                    }
                }) {
                    eprintln!("[shortcut] Failed to register global shortcut '{shortcut}': {err}");
                }
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match handle.updater() {
                    Ok(updater) => {
                        println!("[updater] Checking for desktop updates...");
                        match updater.check().await {
                            Ok(Some(update)) => {
                                println!(
                                    "[updater] Update available: v{} (current: v{})",
                                    update.version, update.current_version
                                );
                                let _ = handle.emit(
                                    "tauri://update-available",
                                    serde_json::json!({
                                        "version": update.version,
                                        "currentVersion": update.current_version,
                                        "body": update.body,
                                    }),
                                );
                            }
                            Ok(None) => {
                                println!("[updater] Application is up to date");
                            }
                            Err(e) => {
                                let err_msg = e.to_string();
                                if err_msg.contains("valid release JSON") || err_msg.contains("404") {
                                    println!(
                                        "[updater] No remote release manifest found; running current release (v{})",
                                        env!("CARGO_PKG_VERSION")
                                    );
                                } else {
                                    eprintln!("[updater] Failed to check for updates: {e}");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[updater] Failed to initialize updater: {e}");
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. } => {
                let should_kill = std::env::var("KRONOS_STOP_ON_EXIT").map(|v| v == "1" || v == "true").unwrap_or(false);
                if should_kill {
                    if let Some(state) = app_handle.try_state::<DaemonState>() {
                        stop_daemon(&state);
                    }
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_validate_directory_nonexistent() {
        let res =
            fast_validate_directory("this_path_definitely_does_not_exist_123456789".to_string());
        assert!(!res.exists);
        assert!(!res.is_dir);
        assert!(!res.readable);
        assert_eq!(res.error, Some("Directory does not exist.".to_string()));
    }

    #[test]
    fn test_fast_validate_directory_on_file() {
        let file_path = std::env::current_dir().unwrap().join("Cargo.toml");
        let res = fast_validate_directory(file_path.to_string_lossy().to_string());
        assert!(res.exists);
        assert!(!res.is_dir);
        assert!(!res.readable);
        assert_eq!(
            res.error,
            Some("Selected path is a file, not a directory.".to_string())
        );
    }

    #[test]
    fn test_fast_validate_directory_valid() {
        let temp_dir = std::env::temp_dir();
        let res = fast_validate_directory(temp_dir.to_string_lossy().to_string());
        assert!(res.exists);
        assert!(res.is_dir);
        assert!(res.readable);
        assert!(res.error.is_none());
    }

    #[test]
    fn test_fast_inspect_path_nonexistent() {
        let res = fast_inspect_path("definitely_nonexistent_file_path_12345".to_string());
        assert!(!res.exists);
        assert!(!res.is_dir);
        assert!(!res.is_file);
        assert_eq!(res.byte_size, 0);
        assert_eq!(res.modified_ms, 0);
        assert!(!res.readable);
    }

    #[test]
    fn test_fast_inspect_path_file() {
        let file_path = std::env::current_dir().unwrap().join("Cargo.toml");
        let res = fast_inspect_path(file_path.to_string_lossy().to_string());
        assert!(res.exists);
        assert!(!res.is_dir);
        assert!(res.is_file);
        assert!(res.byte_size > 0);
        assert!(res.modified_ms > 0);
        assert!(res.readable);
    }

    #[test]
    fn test_fast_inspect_path_dir() {
        let temp_dir = std::env::temp_dir();
        let res = fast_inspect_path(temp_dir.to_string_lossy().to_string());
        assert!(res.exists);
        assert!(res.is_dir);
        assert!(!res.is_file);
        assert!(res.readable);
    }

    #[test]
    fn test_get_system_idle_time() {
        let idle = get_system_idle_time();
        // Idle time in milliseconds should be a valid non-negative u64 value
        assert!(idle < 100_000_000_000);
    }

    #[test]
    fn test_portable_mode_detection_env() {
        let temp_dir = std::env::temp_dir().join("kronos_portable_test_dir");
        let _ = std::fs::create_dir_all(&temp_dir);
        std::env::set_var("KRONOS_PORTABLE", "1");
        std::env::set_var("KRONOS_HOME", temp_dir.to_string_lossy().to_string());
        let data_dir = get_data_dir();
        assert_eq!(data_dir, temp_dir);
        std::env::remove_var("KRONOS_PORTABLE");
        std::env::remove_var("KRONOS_HOME");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
