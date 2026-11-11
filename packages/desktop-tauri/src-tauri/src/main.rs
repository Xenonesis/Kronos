// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_export]
macro_rules! println {
    () => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout());
    }};
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), $($arg)*);
    }};
}

#[macro_export]
macro_rules! eprintln {
    () => {{
        use std::io::Write;
        let _ = writeln!(std::io::stderr());
    }};
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

#[cfg(target_os = "windows")]
fn sanitize_windows_stdio() {
    use windows_sys::Win32::Foundation::{
        GetLastError, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, WriteFile, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Console::{
        GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };

    let nul_name: Vec<u16> = "NUL\0".encode_utf16().collect();
    let open_nul = || unsafe {
        CreateFileW(
            nul_name.as_ptr(),
            GENERIC_WRITE | GENERIC_READ,
            FILE_SHARE_WRITE | FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    let check_and_fix = |std_handle_id: u32| {
        unsafe {
            let handle = GetStdHandle(std_handle_id);
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                let nul = open_nul();
                if nul != INVALID_HANDLE_VALUE {
                    SetStdHandle(std_handle_id, nul);
                }
            } else {
                let mut written: u32 = 0;
                let ok = WriteFile(
                    handle,
                    std::ptr::null(),
                    0,
                    &mut written,
                    std::ptr::null_mut(),
                );
                if ok == 0 {
                    let err = GetLastError();
                    // 109 = ERROR_BROKEN_PIPE, 232 = ERROR_NO_DATA, 6 = ERROR_INVALID_HANDLE
                    if err == 109 || err == 232 || err == 6 {
                        let nul = open_nul();
                        if nul != INVALID_HANDLE_VALUE {
                            SetStdHandle(std_handle_id, nul);
                        }
                    }
                }
            }
        }
    };

    check_and_fix(STD_OUTPUT_HANDLE);
    check_and_fix(STD_ERROR_HANDLE);
}

#[cfg(not(target_os = "windows"))]
fn sanitize_windows_stdio() {}

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod daemon;
mod editor;

pub mod agent;
pub mod kernel;
pub mod native_diff;
pub mod native_pty;
mod notifications;
pub mod process_job;
mod tray;

use notifications::{notify_task_complete, setup_notification_listeners, show_notification};
use tray::{setup_tray, update_active_agents};
mod attachments;
mod dialogs;
use attachments::{
    copy_attachment_file, delete_attachment_file, garbage_collect_attachment_files,
    read_file_base64, write_attachment_base64, write_attachment_bytes,
};
use native_diff::{apply_diff_hunk, compute_file_diff};
use native_pty::{
    kill_native_terminal, resize_native_terminal, spawn_native_terminal, write_native_terminal,
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

#[cfg(target_os = "windows")]
pub fn trim_process_memory() {
    unsafe {
        use windows_sys::Win32::System::ProcessStatus::EmptyWorkingSet;
        use windows_sys::Win32::System::Threading::GetCurrentProcess;
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

#[cfg(not(target_os = "windows"))]
pub fn trim_process_memory() {}

pub fn toggle_window<R: tauri::Runtime>(win: &WebviewWindow<R>) {
    let is_minimized = win.is_minimized().unwrap_or(false);
    let is_visible = win.is_visible().unwrap_or(false);
    let is_focused = win.is_focused().unwrap_or(false);
    if is_visible && !is_minimized && is_focused {
        let _ = win.hide();
        trim_process_memory();
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
pub struct UpdateState(
    pub std::sync::Arc<tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>>,
);

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

fn updater_configured(app: &tauri::AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|config| config.get("pubkey"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|key| !key.trim().is_empty())
}

#[tauri::command]
async fn check_app_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdateState>,
    release_channel: Option<String>,
    intent: Option<String>,
) -> Result<DesktopAppUpdateCheckResult, String> {
    let _ = (release_channel, intent);
    if !updater_configured(&app) {
        *state.0.lock().await = None;
        return Ok(DesktopAppUpdateCheckResult {
            hasUpdate: false, readyToInstall: false,
            currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
            latestVersion: None, body: None, date: None,
            errorMessage: Some("Automatic updates are unavailable in this unsigned local build. Install an owner-signed release to enable them.".into()),
        });
    }
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
    if !updater_configured(&app) {
        return Err(
            "Automatic installation is disabled: this build has no updater verification key."
                .into(),
        );
    }
    let mut lock = state.0.lock().await;
    let update = if let Some(u) = lock.take() {
        u
    } else {
        let updater = app
            .updater()
            .map_err(|e| format!("Updater not initialized: {e}"))?;
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
    println!(
        "[updater] Downloading and installing update v{}...",
        version
    );
    let mut downloaded = 0;
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded += chunk_length;
                println!(
                    "[updater] Downloaded {downloaded} bytes of {:?}",
                    content_length
                );
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

#[tauri::command]
fn get_kernel_telemetry() -> crate::kernel::telemetry::KernelTelemetry {
    crate::kernel::telemetry::get_kernel_telemetry()
}

#[tauri::command]
async fn mark_first_paint(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}

#[tauri::command]
async fn workspace_search(
    query: String,
    root: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<crate::kernel::search::SearchResult>, String> {
    let target_root = root.unwrap_or_else(|| ".".to_string());
    let path = std::path::PathBuf::from(target_root);
    let results = tokio::task::spawn_blocking(move || {
        crate::kernel::cli::execute_cached_search_in(&query, &path)
    })
    .await
    .map_err(|e| format!("Search task failed: {e}"))??;
    let max_limit = limit.unwrap_or(100);
    Ok(results.into_iter().take(max_limit).collect())
}

#[tauri::command]
async fn create_turn_checkpoint(repo_path: String, turn_id: String) -> Result<String, String> {
    crate::kernel::git::GitEngine::create_checkpoint(std::path::Path::new(&repo_path), &turn_id).await
}

#[tauri::command]
async fn restore_turn_checkpoint(repo_path: String, turn_id: String) -> Result<(), String> {
    crate::kernel::git::GitEngine::restore_checkpoint(std::path::Path::new(&repo_path), &turn_id).await
}

#[tauri::command]
async fn watch_directory(path: String, ignore: Option<Vec<String>>) -> Result<String, String> {
    let engine = crate::kernel::server::get_watcher_engine();
    let ignore_patterns = ignore.unwrap_or_default();
    engine.watch(&path, &ignore_patterns)
}

#[tauri::command]
async fn unwatch_directory(subscription_id: String) -> Result<bool, String> {
    let engine = crate::kernel::server::get_watcher_engine();
    engine.unwatch(&subscription_id)
}

#[tauri::command]
async fn analyze_ast(
    path: Option<String>,
    source: Option<String>,
    ext: Option<String>,
) -> Result<crate::kernel::ast::AstAnalysisResult, String> {
    let (src, file_ext) = if let Some(s) = source {
        let e = ext
            .or_else(|| {
                path.as_ref().and_then(|p| {
                    std::path::Path::new(p)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(String::from)
                })
            })
            .unwrap_or_else(|| "ts".to_string());
        (s, e)
    } else if let Some(p) = path {
        let content =
            std::fs::read_to_string(&p).map_err(|e| format!("Failed to read file '{p}': {e}"))?;
        let e = ext
            .or_else(|| {
                std::path::Path::new(&p)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "ts".to_string());
        (content, e)
    } else {
        return Err("Missing 'source' or 'path' argument for analyze_ast".to_string());
    };

    let engine = crate::kernel::ast::AstEngine::new();
    engine.analyze(&src, &file_ext)
}

#[tauri::command]
async fn crdt_doc_action(
    room: String,
    action: String,
    index: Option<u32>,
    text: Option<String>,
    length: Option<u32>,
    update_base64: Option<String>,
    source_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let engine = crate::kernel::server::get_crdt_engine();
    match action.as_str() {
        "get_text" | "state" => {
            let doc = engine.get_or_create_room(&room);
            let content = doc.to_string();
            let len = doc.len();
            let sv = doc.export_state_vector();
            let sv_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &sv);
            Ok(serde_json::json!({
                "room": room,
                "text": content,
                "length": len,
                "stateVector": sv_b64,
            }))
        }
        "insert" | "delete" => {
            let idx = index.unwrap_or(0);
            let b64 =
                engine.apply_text_edit(&room, &action, idx, text.as_deref(), length, source_id)?;
            let doc = engine.get_or_create_room(&room);
            Ok(serde_json::json!({
                "room": room,
                "text": doc.to_string(),
                "length": doc.len(),
                "update": b64,
            }))
        }
        "apply_update" => {
            let b64 =
                update_base64.ok_or_else(|| "Missing 'update_base64' parameter".to_string())?;
            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)
                .map_err(|e| format!("Invalid base64: {e}"))?;
            let broadcasted_b64 = engine.apply_update(&room, &bytes, source_id)?;
            let doc = engine.get_or_create_room(&room);
            Ok(serde_json::json!({
                "room": room,
                "text": doc.to_string(),
                "length": doc.len(),
                "update": broadcasted_b64,
            }))
        }
        "export_update" => {
            let doc = engine.get_or_create_room(&room);
            let bytes = doc.export_update();
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
            Ok(serde_json::json!({
                "room": room,
                "update": b64,
            }))
        }
        "list_rooms" => {
            let rooms = engine.list_rooms();
            Ok(serde_json::json!({
                "rooms": rooms
            }))
        }
        other => Err(format!("Unsupported CRDT action: '{other}'")),
    }
}

#[tauri::command]
async fn get_ioring_status() -> Result<serde_json::Value, String> {
    let reader = crate::kernel::ioring::IoRingBatchReader::new(1024)
        .map_err(|e| format!("Failed to initialize IoRing: {e}"))?;
    Ok(serde_json::json!({
        "is_kernel_active": reader.is_kernel_ioring_active(),
        "queue_size": reader.queue_size(),
        "platform": std::env::consts::OS,
    }))
}

#[tauri::command]
async fn transcribe_audio_chunk(
    samples: Vec<f32>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
) -> Result<serde_json::Value, String> {
    let rate = sample_rate.unwrap_or(16000);
    let ch = channels.unwrap_or(1);
    let config = crate::kernel::speech::SpeechAudioConfig::new(rate, ch);

    let engine_lock = crate::kernel::server::get_speech_engine();
    let engine = engine_lock.read().await;

    let transcript = engine.transcribe_with_config(&samples, config)?;
    let duration_secs = samples.len() as f32 / (rate as f32 * ch as f32);

    Ok(serde_json::json!({
        "transcript": transcript,
        "samples_count": samples.len(),
        "duration_secs": duration_secs,
    }))
}

#[tauri::command]
async fn get_speech_engine_status() -> Result<serde_json::Value, String> {
    let engine_lock = crate::kernel::server::get_speech_engine();
    let engine = engine_lock.read().await;
    Ok(serde_json::json!({
        "is_model_loaded": engine.is_model_loaded(),
        "target_sample_rate": 16000,
        "target_channels": 1,
    }))
}

#[tauri::command]
async fn get_shm_status() -> Result<serde_json::Value, String> {
    let ring_lock = crate::kernel::server::get_shm_ring();
    let ring = ring_lock.lock().await;
    let cap = ring.capacity();
    let head = ring.write_head();
    let seq = ring.sequence();
    Ok(serde_json::json!({
        "capacity": cap,
        "head_offset": head,
        "sequence": seq,
    }))
}

#[tauri::command]
async fn push_shm_chunk(stream_id: String, payload: String) -> Result<u64, String> {
    let ring_lock = crate::kernel::server::get_shm_ring();
    let ring = ring_lock.lock().await;
    let next_seq = ring.sequence() + 1;
    crate::kernel::rkyv_shm::write_rkyv_chunk_to_shm(
        &ring,
        next_seq,
        &stream_id,
        payload.into_bytes(),
    )
}

fn detect_and_configure_portable_mode() -> Option<PathBuf> {
    // Check KRONOS_PORTABLE=1 env var
    let env_portable = std::env::var("KRONOS_PORTABLE")
        .map(|v| v == "1")
        .unwrap_or(false);

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

pub fn get_or_create_server_id() -> String {
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

    let port = daemon::get_active_daemon_port();
    let is_healthy = wait_for_daemon_health(1);
    let listen = format!("127.0.0.1:{}", port);
    let hostname = "Local (This Device)".to_string();
    let detected_pid = if is_healthy {
        daemon::find_pid_listening_on_port(port)
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
fn get_live_local_credential(listen: String) -> Option<String> {
    if !kernel::access::is_managed_target(&listen) {
        return None;
    }
    kernel::auth::read_local_credential(&get_data_dir())
}

static DAEMON_BROWSER_CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Create daemon browser client")
    });

async fn browser_daemon_request(
    path: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let credential = kernel::auth::read_local_credential(&get_data_dir())
        .ok_or("The listening daemon has no readable local credential")?;
    let port = daemon::get_active_daemon_port();
    let response = DAEMON_BROWSER_CLIENT
        .post(format!("http://127.0.0.1:{port}{path}"))
        .bearer_auth(credential)
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Browser operation rejected by the listening daemon: {}",
            response.status()
        ));
    }
    response.json().await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn browser_proxy_url(url: String) -> Result<String, String> {
    let response = browser_daemon_request(
        "/api/browser-proxy/navigation",
        serde_json::json!({"url":url}),
    )
    .await?;
    let path = response["url"]
        .as_str()
        .filter(|path| path.starts_with("/api/browser-proxy?"))
        .ok_or("The daemon returned an invalid browser navigation capability")?;
    let port = daemon::get_active_daemon_port();
    Ok(format!("http://127.0.0.1:{port}{path}"))
}

#[tauri::command]
async fn browser_proxy_cookies(host: String, cookies: String) -> Result<(), String> {
    browser_daemon_request(
        "/api/browser-proxy/cookies",
        serde_json::json!({"host":host,"cookies":cookies}),
    )
    .await?;
    Ok(())
}

struct LocalTransportSession {
    sender: tokio::sync::mpsc::UnboundedSender<tokio_tungstenite::tungstenite::Message>,
}

static LOCAL_TRANSPORT_SESSIONS: std::sync::LazyLock<
    tokio::sync::Mutex<std::collections::HashMap<String, LocalTransportSession>>,
> = std::sync::LazyLock::new(|| tokio::sync::Mutex::new(std::collections::HashMap::new()));

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenTransportInput {
    session_id: String,
    #[allow(dead_code)]
    target: serde_json::Value,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendTransportInput {
    session_id: String,
    text: Option<String>,
    binary_base64: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloseTransportInput {
    session_id: String,
}

#[tauri::command]
async fn open_local_daemon_transport(
    app: tauri::AppHandle,
    input: OpenTransportInput,
) -> Result<(), String> {
    use futures_util::{SinkExt, StreamExt};
    use tauri::Emitter;

    let session_id = input.session_id;
    let port = daemon::get_active_daemon_port();
    let ws_url = format!("ws://127.0.0.1:{}/ws", port);

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| format!("Failed to connect to local daemon WebSocket: {}", e))?;

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (out_tx, mut out_rx) =
        tokio::sync::mpsc::unbounded_channel::<tokio_tungstenite::tungstenite::Message>();

    {
        let mut sessions = LOCAL_TRANSPORT_SESSIONS.lock().await;
        sessions.insert(session_id.clone(), LocalTransportSession { sender: out_tx });
    }

    let _ = app.emit(
        "local-daemon-transport-event",
        serde_json::json!({
            "sessionId": session_id,
            "kind": "open"
        }),
    );

    let sid_out = session_id.clone();
    let sid_in = session_id.clone();
    let app_handle = app.clone();

    // Outgoing pump
    tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
        let _ = ws_write.close().await;
        let mut sessions = LOCAL_TRANSPORT_SESSIONS.lock().await;
        sessions.remove(&sid_out);
    });

    // Incoming pump
    tokio::spawn(async move {
        while let Some(msg_res) = ws_read.next().await {
            match msg_res {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    let _ = app_handle.emit(
                        "local-daemon-transport-event",
                        serde_json::json!({
                            "sessionId": sid_in,
                            "kind": "message",
                            "text": text
                        }),
                    );
                }
                Ok(tokio_tungstenite::tungstenite::Message::Binary(bin)) => {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bin);
                    let _ = app_handle.emit(
                        "local-daemon-transport-event",
                        serde_json::json!({
                            "sessionId": sid_in,
                            "kind": "message",
                            "binaryBase64": b64
                        }),
                    );
                }
                Ok(tokio_tungstenite::tungstenite::Message::Close(frame)) => {
                    let code = frame.as_ref().map(|f| u16::from(f.code));
                    let reason = frame.as_ref().map(|f| f.reason.to_string());
                    let _ = app_handle.emit(
                        "local-daemon-transport-event",
                        serde_json::json!({
                            "sessionId": sid_in,
                            "kind": "close",
                            "code": code,
                            "reason": reason
                        }),
                    );
                    break;
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "local-daemon-transport-event",
                        serde_json::json!({
                            "sessionId": sid_in,
                            "kind": "error",
                            "error": e.to_string()
                        }),
                    );
                    break;
                }
                _ => {}
            }
        }

        let mut sessions = LOCAL_TRANSPORT_SESSIONS.lock().await;
        sessions.remove(&sid_in);
    });

    Ok(())
}

#[tauri::command]
async fn send_local_daemon_transport_message(
    input: SendTransportInput,
) -> Result<(), String> {
    let sessions = LOCAL_TRANSPORT_SESSIONS.lock().await;
    if let Some(session) = sessions.get(&input.session_id) {
        if let Some(text) = input.text {
            let _ = session
                .sender
                .send(tokio_tungstenite::tungstenite::Message::Text(text.into()));
        } else if let Some(b64) = input.binary_base64 {
            use base64::Engine;
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                let _ = session
                    .sender
                    .send(tokio_tungstenite::tungstenite::Message::Binary(bytes.into()));
            }
        }
        Ok(())
    } else {
        Err(format!("Transport session {} not found", input.session_id))
    }
}

#[tauri::command]
async fn close_local_daemon_transport(
    input: CloseTransportInput,
) -> Result<(), String> {
    let mut sessions = LOCAL_TRANSPORT_SESSIONS.lock().await;
    if let Some(session) = sessions.remove(&input.session_id) {
        let _ = session
            .sender
            .send(tokio_tungstenite::tungstenite::Message::Close(None));
    }
    Ok(())
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
    let mut lock = state.process.lock().unwrap_or_else(|e| e.into_inner());
    let is_running = if let Some(child) = lock.as_mut() {
        child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
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

#[tauri::command]
fn desktop_window_minimize(window: WebviewWindow) -> Result<(), String> {
    let res = window.minimize().map_err(|e| e.to_string());
    trim_process_memory();
    res
}

#[tauri::command]
fn trim_memory() -> Result<(), String> {
    trim_process_memory();
    Ok(())
}

#[tauri::command]
fn ping_watchdog() -> Result<(), String> {
    crate::kernel::watchdog::record_heartbeat();
    Ok(())
}

#[tauri::command]
fn set_window_progress(
    window: WebviewWindow,
    progress: Option<u64>,
    state: Option<String>,
) -> Result<(), String> {
    use tauri::window::{ProgressBarState, ProgressBarStatus};
    let status = match state.as_deref() {
        Some("indeterminate") => Some(ProgressBarStatus::Indeterminate),
        Some("error") => Some(ProgressBarStatus::Error),
        Some("paused") => Some(ProgressBarStatus::Paused),
        Some("none") => Some(ProgressBarStatus::None),
        Some("normal") | _ => Some(ProgressBarStatus::Normal),
    };
    window
        .set_progress_bar(ProgressBarState {
            progress,
            status,
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn desktop_window_toggle_maximize(window: WebviewWindow) -> Result<(), String> {
    if window.is_maximized().unwrap_or(false) {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocalProviderDiscovery {
    pub provider: String,
    pub is_running: bool,
    pub endpoint: String,
    pub models: Vec<String>,
}

#[tauri::command]
async fn discover_local_ai_providers() -> Vec<LocalProviderDiscovery> {
    let mut results = Vec::new();

    // 1. Check Ollama (127.0.0.1:11434)
    let ollama = crate::agent::providers::ollama::OllamaClient::default_local();
    if ollama.check_health().await {
        let models = ollama.list_models().await.unwrap_or_default();
        results.push(LocalProviderDiscovery {
            provider: "ollama".to_string(),
            is_running: true,
            endpoint: "http://127.0.0.1:11434".to_string(),
            models,
        });
    }

    // 2. Check LM Studio (127.0.0.1:1234)
    let lmstudio = crate::agent::providers::lmstudio::LmStudioClient::default_local();
    if lmstudio.check_health().await {
        let models = lmstudio.list_models().await.unwrap_or_default();
        results.push(LocalProviderDiscovery {
            provider: "lmstudio".to_string(),
            is_running: true,
            endpoint: "http://127.0.0.1:1234".to_string(),
            models,
        });
    }

    results
}

#[tauri::command]
fn desktop_window_close(window: WebviewWindow) -> Result<(), String> {
    let res = window.hide().map_err(|e| e.to_string());
    trim_process_memory();
    res
}

#[tauri::command]
fn desktop_window_is_maximized(window: WebviewWindow) -> bool {
    window.is_maximized().unwrap_or(false)
}

#[tauri::command]
fn desktop_window_is_fullscreen(window: WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

#[tauri::command]
fn desktop_window_set_fullscreen(window: WebviewWindow, fullscreen: bool) -> Result<(), String> {
    window.set_fullscreen(fullscreen).map_err(|e| e.to_string())
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
        let _ = writeln!(std::io::stderr(), "{}", entry);
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
    let startup_trace = std::env::var("KRONOS_STARTUP_TRACE")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    let start_instant = std::time::Instant::now();
    crate::kernel::telemetry::init_tracing(startup_trace);
    tracing::info!(target: "startup", "Kronos desktop process started");
    if startup_trace {
        println!("[trace:startup] Process started");
    }

    sanitize_windows_stdio();
    detect_and_configure_portable_mode();

    if let Some(code) = kernel::cli::parse_and_dispatch() {
        std::process::exit(code);
    }

    setup_panic_hook();
    check_and_log_crashpad_dumps();

    #[cfg(target_os = "windows")]
    {
        // Prevent DWM occlusion freezes, Intel GPU driver watchdog timeouts, and memory/disk bloat
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--disable-features=CalculateNativeWinOcclusion --disable-gpu-watchdog --disk-cache-size=104857600 --renderer-process-limit=4",
        );
    }

    let server_id = get_or_create_server_id();
    let daemon_child = start_daemon(Some(&server_id));
    let state = DaemonState {
        process: Mutex::new(daemon_child),
    };
    let daemon_server_id = server_id.clone();
    let _ = wait_for_daemon_health(2);
    let mut public_status: serde_json::Value =
        serde_json::from_str(&get_daemon_status_json()).unwrap_or_default();
    if let Some(status) = public_status.as_object_mut() {
        status.remove("localCredential");
    }
    let status_json = public_status.to_string();
    let bridge_script = format!(
        r#"
(() => {{
try {{
  if (window.top !== window) return;
  const location = window.location;
  const trustedAsset = (location.protocol === 'tauri:' && location.hostname === 'localhost')
    || ((location.protocol === 'http:' || location.protocol === 'https:')
      && location.hostname === 'tauri.localhost' && location.port === '');
  if (!trustedAsset) return;
  const daemonData = {status_json};
  const listenEndpoint = (daemonData.listen || '127.0.0.1:6767').replace(/^http:\/\//, '').replace(/\/.*$/, '');
  const endpoint = listenEndpoint;

  class TauriBridgeChannel {{
    constructor(onmessage) {{
      this.onmessage = onmessage || (() => {{}});
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.transformCallback === 'function') {{
        this.id = window.__TAURI_INTERNALS__.transformCallback((raw) => {{
          if (raw && typeof raw === 'object' && 'message' in raw) {{
            this.onmessage(raw.message);
          }} else {{
            this.onmessage(raw);
          }}
        }});
      }} else {{
        this.id = Math.floor(Math.random() * 1000000);
      }}
    }}
    toJSON() {{
      return '__CHANNEL__:' + this.id;
    }}
    __TAURI_TO_IPC_KEY__() {{
      return '__CHANNEL__:' + this.id;
    }}
  }}

  window.paseoDesktop = {{
    platform: 'win32',
    windowChromeMode: 'native-windows',
    window: {{
      getCurrentWindow: function() {{
        return {{
          minimize: async function() {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                await window.__TAURI_INTERNALS__.invoke('desktop_window_minimize');
              }}
            }} catch (e) {{
              console.error('[TauriBridge] minimize failed:', e);
            }}
          }},
          toggleMaximize: async function() {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                await window.__TAURI_INTERNALS__.invoke('desktop_window_toggle_maximize');
              }}
            }} catch (e) {{
              console.error('[TauriBridge] toggleMaximize failed:', e);
            }}
          }},
          close: async function() {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                await window.__TAURI_INTERNALS__.invoke('desktop_window_close');
              }}
            }} catch (e) {{
              console.error('[TauriBridge] close failed:', e);
            }}
          }},
          isMaximized: async function() {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                return await window.__TAURI_INTERNALS__.invoke('desktop_window_is_maximized');
              }}
            }} catch (e) {{
              console.error('[TauriBridge] isMaximized failed:', e);
            }}
            return false;
          }},
          isFullscreen: async function() {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                return await window.__TAURI_INTERNALS__.invoke('desktop_window_is_fullscreen');
              }}
            }} catch (e) {{
              console.error('[TauriBridge] isFullscreen failed:', e);
            }}
            return false;
          }},
          setFullscreen: async function(fullscreen) {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                await window.__TAURI_INTERNALS__.invoke('desktop_window_set_fullscreen', {{ fullscreen: fullscreen }});
              }}
            }} catch (e) {{
              console.error('[TauriBridge] setFullscreen failed:', e);
            }}
          }},
          setProgress: async function(progress, state) {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                await window.__TAURI_INTERNALS__.invoke('set_window_progress', {{ progress: progress, state: state }});
              }}
            }} catch (e) {{
              console.error('[TauriBridge] setProgress failed:', e);
            }}
          }},
          trimMemory: async function() {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                await window.__TAURI_INTERNALS__.invoke('trim_memory');
              }}
            }} catch (e) {{}}
          }},
          onResized: function(callback) {{
            try {{
              if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.listen) {{
                let unlistenPromise = window.__TAURI_INTERNALS__.listen('tauri://resize', function(ev) {{
                  try {{
                    callback(ev && ev.payload !== undefined ? ev.payload : ev);
                  }} catch (err) {{
                    console.error('[TauriBridge] Resize callback failed:', err);
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
              console.error('[TauriBridge] onResized failed:', e);
            }}
            return function() {{}};
          }}
        }};
      }}
    }},
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
    discoverLocalAiProviders: async function() {{
      try {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('discover_local_ai_providers');
        }}
      }} catch (e) {{
        console.error('[TauriBridge] discoverLocalAiProviders failed:', e);
      }}
      return [];
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
          const ChannelCtor = (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.Channel) ? window.__TAURI__.core.Channel : TauriBridgeChannel;
          const channel = new ChannelCtor(onEvent);
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
      }},
      applyHunk: async function(filePath, hunk, reverse) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('apply_diff_hunk', {{ filePath, hunk, reverse }});
        }}
        return null;
      }}
    }},
    nativeBrowser: {{
      resolveUrl: (url) => window.__TAURI_INTERNALS__.invoke('browser_proxy_url', {{ url }}),
      importCookies: (host, cookies) => window.__TAURI_INTERNALS__.invoke('browser_proxy_cookies', {{ host, cookies }})
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
      if (command === 'apply_diff_hunk') {{
        return await window.__TAURI_INTERNALS__.invoke('apply_diff_hunk', args || {{}});
      }}
      if (command === 'create_turn_checkpoint') {{
        return await window.__TAURI_INTERNALS__.invoke('create_turn_checkpoint', args || {{}});
      }}
      if (command === 'restore_turn_checkpoint') {{
        return await window.__TAURI_INTERNALS__.invoke('restore_turn_checkpoint', args || {{}});
      }}
      if (command === 'desktop_local_credential') {{
        return await window.__TAURI_INTERNALS__.invoke('get_live_local_credential', {{ listen: args && args.listen || '' }});
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
      if (command === 'workspace_search') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke('workspace_search', args || {{}});
          }}
        }} catch (e) {{
          console.error('[TauriBridge] workspace_search failed:', e);
          throw e;
        }}
        return null;
      }}
      if (command === 'mark_first_paint' || command === 'watch_directory' || command === 'unwatch_directory' || command === 'analyze_ast' || command === 'crdt_doc_action' || command === 'get_ioring_status' || command === 'transcribe_audio_chunk' || command === 'get_speech_engine_status' || command === 'get_shm_status' || command === 'push_shm_chunk') {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke(command, args || {{}});
          }}
        }} catch (e) {{
          console.error('[TauriBridge] ' + command + ' failed:', e);
          throw e;
        }}
        return null;
      }}
      if (
        command === 'open_local_daemon_transport' ||
        command === 'send_local_daemon_transport_message' ||
        command === 'close_local_daemon_transport'
      ) {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            return await window.__TAURI_INTERNALS__.invoke(command, args || {{}});
          }}
        }} catch (e) {{
          console.error('[TauriBridge] ' + command + ' failed:', e);
          throw e;
        }}
        return null;
      }}
      return null;
    }},
    getKernelTelemetry: async function() {{
      try {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('get_kernel_telemetry');
        }}
      }} catch (e) {{
        console.error('[TauriBridge] getKernelTelemetry failed:', e);
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
      const isLoopback = h.label === 'Local (This Device)' || h.connections.some(c =>
        (c.type === 'directTcp' && (c.endpoint.includes('127.0.0.1') || c.endpoint.includes('localhost')))
        || c.type === 'directPipe'
        || c.type === 'directSocket'
      );
      return !isLoopback || h.serverId === daemonData.serverId;
    }});

    const isWin = typeof navigator !== 'undefined' && navigator.platform && navigator.platform.toLowerCase().includes('win');
    const localConn = isWin
      ? {{ id: 'pipe:\\\\.\\pipe\\kronos-ipc', type: 'directPipe', path: '\\\\.\\pipe\\kronos-ipc' }}
      : {{ id: 'socket:/tmp/kronos-ipc.sock', type: 'directSocket', path: '/tmp/kronos-ipc.sock' }};
    const tcpConn = {{ id: 'direct:' + endpoint, type: 'directTcp', endpoint: endpoint, useTls: false }};

    const existing = registry.find(h => h && h.serverId === daemonData.serverId);
    if (!existing) {{
      registry.unshift({{
        serverId: daemonData.serverId,
        label: 'Local (This Device)',
        appearance: {{ color: 'none', badgeDisplay: null }},
        lifecycle: {{}},
        connections: [localConn, tcpConn],
        preferredConnectionId: localConn.id,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString()
      }});
    }} else {{
      existing.label = 'Local (This Device)';
      if (!existing.connections) existing.connections = [];
      const hasLocalConn = existing.connections.some(c => c.type === 'directPipe' || c.type === 'directSocket');
      if (!hasLocalConn) {{
        existing.connections.unshift(localConn);
      }}
      existing.preferredConnectionId = localConn.id;
      const conn = existing.connections.find(c => c.type === 'directTcp');
      if (conn) {{
        conn.endpoint = endpoint;
      }} else {{
        existing.connections.push(tcpConn);
      }}
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
  // Active UI Watchdog Heartbeat
  try {{
    setInterval(() => {{
      try {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          window.__TAURI_INTERNALS__.invoke('ping_watchdog').catch(() => {{}});
        }}
      }} catch (_) {{}}
    }}, 4000);
  }} catch (_) {{}}
}} catch (e) {{}}
}})();
"#
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                if win.is_minimized().unwrap_or(false) {
                    let _ = win.unminimize();
                }
                let _ = win.show();
                let _ = win.set_focus();

                #[cfg(target_os = "windows")]
                {
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        BringWindowToTop, SetForegroundWindow, ShowWindow, SW_RESTORE,
                    };
                    if let Ok(hwnd) = win.hwnd() {
                        unsafe {
                            let raw_hwnd = hwnd.0 as windows_sys::Win32::Foundation::HWND;
                            ShowWindow(raw_hwnd, SW_RESTORE);
                            BringWindowToTop(raw_hwnd);
                            SetForegroundWindow(raw_hwnd);
                        }
                    }
                }

                let _ = win.emit("single-instance-deep-link", argv.clone());
                if argv.len() > 1 {
                    for arg in &argv[1..] {
                        if !arg.starts_with('-') {
                            let _ = win.emit("kronos://cli-open", arg);
                        }
                    }
                }
            } else {
                app.exit(0);
            }
        }))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                trim_process_memory();
                api.prevent_close();
            }
        })
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .manage(std::sync::Arc::new(crate::process_job::JobSupervisor::new().ok()))
        .manage(crate::native_pty::get_pty_manager().clone())
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
            browser_proxy_url,
            browser_proxy_cookies,
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
            apply_diff_hunk,
            check_app_update,
            install_app_update,
            get_kernel_telemetry,
            mark_first_paint,
            workspace_search,
            create_turn_checkpoint,
            restore_turn_checkpoint,
            watch_directory,
            unwatch_directory,
            analyze_ast,
            crdt_doc_action,
            get_ioring_status,
            transcribe_audio_chunk,
            get_speech_engine_status,
            get_shm_status,
            push_shm_chunk,
            open_local_daemon_transport,
            send_local_daemon_transport_message,
            close_local_daemon_transport,
            desktop_window_minimize,
            desktop_window_toggle_maximize,
            desktop_window_close,
            desktop_window_is_maximized,
            desktop_window_is_fullscreen,
            desktop_window_set_fullscreen,
            trim_memory,
            set_window_progress,
            ping_watchdog,
            discover_local_ai_providers
        ])
        .setup(move |app| {
            crate::kernel::watchdog::start_watchdog(app.handle().clone());
            let _ = setup_tray(app.handle());
            setup_notification_listeners(app.handle());
            #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(err) = app.deep_link().register_all() {
                    eprintln!("[deep-link] Failed to register schemes: {err}");
                }
            }
            let page_trace = startup_trace;
            let page_start = start_instant;
            let window_icon = app.default_window_icon().cloned().or_else(|| {
                tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png")).ok()
            });
            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Kronos")
                .theme(Some(tauri::Theme::Dark))
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .shadow(false)
                .visible(false)
                .initialization_script(&bridge_script);

            let win_builder = if let Some(ref icon) = window_icon {
                match win_builder.icon(icon.clone()) {
                    Ok(b) => b,
                    Err(err) => {
                        eprintln!("[window] Failed to apply icon to builder: {err}");
                        WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                            .title("Kronos")
                            .theme(Some(tauri::Theme::Dark))
                            .inner_size(1280.0, 800.0)
                            .min_inner_size(800.0, 600.0)
                            .shadow(false)
                            .visible(false)
                            .initialization_script(&bridge_script)
                    }
                }
            } else {
                win_builder
            };

            let win_builder = win_builder.on_page_load(move |_win, payload| {
                if page_trace {
                    if let tauri::webview::PageLoadEvent::Finished = payload.event() {
                        println!(
                            "[trace:startup] Webview page loaded (Finished) after {:?}",
                            page_start.elapsed()
                        );
                    }
                }
            });

            let win = win_builder.build().ok();
            if let Some(main_window) = &win {
                if let Some(ref icon) = window_icon {
                    let _ = main_window.set_icon(icon.clone());
                }
                apply_mica_effect(main_window);
            }

            let fallback_handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                if let Some(win) = fallback_handle.get_webview_window("main") {
                    if !win.is_visible().unwrap_or(true) {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            });

            let handle_for_daemon = app.handle().clone();
            let daemon_id = daemon_server_id.clone();
            let daemon_trace = startup_trace;
            let daemon_start = start_instant;
            std::thread::spawn(move || {
                let is_healthy = wait_for_daemon_health(50);
                if is_healthy {
                    if daemon_trace {
                        println!(
                            "[trace:startup] Host daemon confirmed healthy after {:?}",
                            daemon_start.elapsed()
                        );
                    }
                    println!("[daemon] Host daemon confirmed healthy on 127.0.0.1:{}", daemon::get_active_daemon_port());
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

            // Windows Taskbar Progress dynamic sync with Agent Turn states
            let handle_for_taskbar = app.handle().clone();
            std::thread::spawn(move || {
                let mut rx = crate::kernel::server::get_provider_push_tx().subscribe();
                while let Ok(msg_str) = rx.blocking_recv() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg_str) {
                        if let Some(event_type) = val.pointer("/message/payload/event/type").and_then(|v| v.as_str()) {
                            if let Some(win) = handle_for_taskbar.get_webview_window("main") {
                                match event_type {
                                    "turn_started" => {
                                        let _ = win.set_progress_bar(tauri::window::ProgressBarState {
                                            progress: None,
                                            status: Some(tauri::window::ProgressBarStatus::Indeterminate),
                                        });
                                    }
                                    "turn_completed" | "turn_canceled" => {
                                        let _ = win.set_progress_bar(tauri::window::ProgressBarState {
                                            progress: None,
                                            status: Some(tauri::window::ProgressBarStatus::None),
                                        });
                                    }
                                    "turn_failed" => {
                                        let _ = win.set_progress_bar(tauri::window::ProgressBarState {
                                            progress: Some(100),
                                            status: Some(tauri::window::ProgressBarStatus::Error),
                                        });
                                        let win_err = win.clone();
                                        std::thread::spawn(move || {
                                            std::thread::sleep(std::time::Duration::from_secs(3));
                                            let _ = win_err.set_progress_bar(tauri::window::ProgressBarState {
                                                progress: None,
                                                status: Some(tauri::window::ProgressBarStatus::None),
                                            });
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            });

            // Register global shortcuts to toggle main window visibility
            let window_clone = win.clone();
            for shortcut in ["Alt+Space", "CommandOrControl+Shift+K", "CommandOrControl+Shift+P"] {
                let window_clone = window_clone.clone();
                if let Err(err) = app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(window_clone) = &window_clone {
                            toggle_window(window_clone);
                        } else if let Some(window_clone) = app.get_webview_window("main") {
                            toggle_window(&window_clone);
                        }
                        let now_ms = chrono::Utc::now().timestamp_millis();
                        let _ = app.emit(
                            "kronos://quick-toggle",
                            serde_json::json!({ "timestamp": now_ms }),
                        );
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

            let handle_for_watcher = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut watcher_rx = crate::kernel::server::get_watcher_engine().subscribe_events();
                loop {
                    match watcher_rx.recv().await {
                        Ok(events) => {
                            if let Err(e) = handle_for_watcher.emit("vfs-file-change", events) {
                                eprintln!("[watcher] Failed to emit vfs-file-change event: {e}");
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            eprintln!("[watcher] Event stream lagged by {skipped} messages");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. } => {
                let should_kill = std::env::var("KRONOS_STOP_ON_EXIT").map(|v| v == "1" || v == "true").unwrap_or(true);
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
    fn test_sanitize_windows_stdio_and_safe_print_macros() {
        sanitize_windows_stdio();
        println!("Safe println test: integer={}, string={}", 42, "hello");
        eprintln!("Safe eprintln test: flag={}", true);

        #[cfg(target_os = "windows")]
        unsafe {
            use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
            use windows_sys::Win32::System::Console::{
                GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
            };
            let orig_out = GetStdHandle(STD_OUTPUT_HANDLE);
            let orig_err = GetStdHandle(STD_ERROR_HANDLE);
            // Temporarily set handles to invalid to verify no panic
            SetStdHandle(STD_OUTPUT_HANDLE, INVALID_HANDLE_VALUE);
            SetStdHandle(STD_ERROR_HANDLE, INVALID_HANDLE_VALUE);
            println!("This should never panic even with invalid handle");
            eprintln!("This should never panic even with invalid handle");
            // Sanitize again recovers valid NUL handle
            sanitize_windows_stdio();
            println!("Recovered handle via sanitize_windows_stdio");
            // Restore original handles so the test runner can write results
            SetStdHandle(STD_OUTPUT_HANDLE, orig_out);
            SetStdHandle(STD_ERROR_HANDLE, orig_err);
        }
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

    #[tokio::test]
    async fn test_workspace_search_command() {
        let res = workspace_search(
            "WorkspaceSearchIndex".to_string(),
            Some(".".to_string()),
            Some(10),
        )
        .await;
        assert!(res.is_ok());
        let matches = res.unwrap();
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.path.contains("search.rs")));
    }

    #[tokio::test]
    async fn test_watch_directory_and_unwatch_commands() {
        let temp_dir =
            std::env::temp_dir().join(format!("kronos_watch_cmd_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let watch_res = watch_directory(
            temp_dir.to_string_lossy().to_string(),
            Some(vec!["target/**".to_string()]),
        )
        .await;
        assert!(
            watch_res.is_ok(),
            "watch_directory should succeed: {:?}",
            watch_res.err()
        );
        let sub_id = watch_res.unwrap();
        assert!(!sub_id.is_empty(), "subscription id should not be empty");

        let unwatch_res = unwatch_directory(sub_id.clone()).await;
        assert!(
            unwatch_res.is_ok(),
            "unwatch_directory should succeed: {:?}",
            unwatch_res.err()
        );
        assert!(
            unwatch_res.unwrap(),
            "unwatch_directory should return true for active subscription"
        );

        // Unwatching an already removed subscription should return Ok(false)
        let unwatch_again = unwatch_directory(sub_id).await;
        assert!(unwatch_again.is_ok());
        assert!(
            !unwatch_again.unwrap(),
            "unwatch_directory should return false when subscription does not exist"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_watch_directory_nonexistent_path() {
        let invalid_path = std::env::temp_dir()
            .join(format!("kronos_nonexistent_dir_{}", std::process::id()))
            .to_string_lossy()
            .to_string();
        let res = watch_directory(invalid_path, None).await;
        assert!(
            res.is_err(),
            "watch_directory should fail on non-existent path"
        );
    }

    #[tokio::test]
    async fn test_analyze_ast_command() {
        let source = r#"
            export interface IConfig {
                timeoutMs: number;
            }

            export function initConfig(): IConfig {
                return { timeoutMs: 5000 };
            }
        "#;

        let res = analyze_ast(None, Some(source.to_string()), Some("ts".to_string())).await;
        assert!(res.is_ok(), "analyze_ast should succeed: {:?}", res.err());
        let analysis = res.unwrap();
        assert_eq!(analysis.root_kind, "program");
        assert!(analysis.symbols.iter().any(|s| s.name == "IConfig"));
        assert!(analysis.symbols.iter().any(|s| s.name == "initConfig"));
    }

    #[tokio::test]
    async fn test_crdt_doc_action_command() {
        let room = format!("test_room_{}", std::process::id());

        // 1. Insert text
        let res1 = crdt_doc_action(
            room.clone(),
            "insert".to_string(),
            Some(0),
            Some("Initial CRDT Buffer".to_string()),
            None,
            None,
            Some("test-peer".to_string()),
        )
        .await;
        assert!(res1.is_ok(), "insert should succeed: {:?}", res1.err());
        let json1 = res1.unwrap();
        assert_eq!(json1["text"], "Initial CRDT Buffer");
        assert_eq!(json1["length"], 19);
        assert!(!json1["update"].as_str().unwrap().is_empty());

        // 2. Query state
        let res2 = crdt_doc_action(
            room.clone(),
            "state".to_string(),
            None,
            None,
            None,
            None,
            None,
        )
        .await;
        assert!(res2.is_ok());
        let json2 = res2.unwrap();
        assert_eq!(json2["text"], "Initial CRDT Buffer");
        assert!(json2["stateVector"].is_string());

        // 3. Delete text: remove " Buffer" (length 7 starting at 12)
        let res3 = crdt_doc_action(
            room.clone(),
            "delete".to_string(),
            Some(12),
            None,
            Some(7),
            None,
            None,
        )
        .await;
        assert!(res3.is_ok());
        let json3 = res3.unwrap();
        assert_eq!(json3["text"], "Initial CRDT");
        assert_eq!(json3["length"], 12);
    }

    #[tokio::test]
    async fn test_get_ioring_status_command() {
        let res = get_ioring_status().await;
        assert!(
            res.is_ok(),
            "get_ioring_status should succeed: {:?}",
            res.err()
        );
        let val = res.unwrap();
        assert!(val["is_kernel_active"].is_boolean());
        assert!(val["queue_size"].as_u64().unwrap() >= 16);
        assert!(val["platform"].is_string());
    }

    #[tokio::test]
    async fn test_transcribe_audio_chunk_command() {
        let synthetic_samples: Vec<f32> =
            (0..16000).map(|i| (i as f32 * 0.05).sin() * 0.75).collect();

        let res = transcribe_audio_chunk(synthetic_samples, Some(16000), Some(1)).await;
        assert!(
            res.is_ok(),
            "transcribe_audio_chunk should succeed: {:?}",
            res.err()
        );
        let val = res.unwrap();
        assert!(val["transcript"].is_string());
        assert!(!val["transcript"].as_str().unwrap().is_empty());
        assert_eq!(val["samples_count"], 16000);

        let status = get_speech_engine_status().await;
        assert!(status.is_ok());
        let sval = status.unwrap();
        assert_eq!(sval["target_sample_rate"], 16000);
    }

    #[tokio::test]
    async fn test_shm_commands() {
        let status = get_shm_status().await;
        assert!(status.is_ok(), "get_shm_status should succeed");
        let sval = status.unwrap();
        assert!(sval["capacity"].as_u64().unwrap() > 0);

        let push_res = push_shm_chunk(
            "stream-unit-test".to_string(),
            "telemetry payload data".to_string(),
        )
        .await;
        assert!(
            push_res.is_ok(),
            "push_shm_chunk should succeed: {:?}",
            push_res.err()
        );
        assert!(push_res.unwrap() > 0);
    }

    #[tokio::test]
    async fn test_discover_local_ai_providers() {
        let res = discover_local_ai_providers().await;
        // Should complete cleanly and safely
        for p in &res {
            assert!(!p.provider.is_empty());
            assert!(!p.endpoint.is_empty());
        }
    }

    #[test]
    fn test_trim_process_memory() {
        trim_process_memory();
    }
}
