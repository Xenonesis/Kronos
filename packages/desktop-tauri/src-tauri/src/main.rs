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
mod browser_automation;
mod dialogs;
use browser_automation::{
    browser_automation_execute, browser_clear_profile, browser_focus_tab,
    browser_register_attached, browser_unregister_tab,
};
use attachments::{
    copy_attachment_file, delete_attachment_file, garbage_collect_attachment_files,
    read_file_base64, write_attachment_base64, write_attachment_bytes,
};
use native_diff::{apply_diff_hunk, compute_file_diff};
use native_pty::{
    kill_native_terminal, resize_native_terminal, spawn_native_terminal, write_native_terminal,
};

use daemon::{start_daemon, stop_daemon, wait_for_daemon_health, DaemonState};
use dialogs::{ask_confirmation_dialog, pick_file, pick_folder};
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
async fn web_search_query(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<crate::agent::web_search::SearchResult>, String> {
    crate::agent::web_search::search_web(&query, limit.unwrap_or(8)).await
}

#[tauri::command]
async fn web_scrape_url(
    url: String,
    max_chars: Option<usize>,
) -> Result<crate::agent::web_search::WebFetchResult, String> {
    crate::agent::web_search::fetch_url(&url, max_chars).await
}

#[tauri::command]
async fn get_repo_map(cwd: Option<String>) -> Result<String, String> {
    let target_dir = cwd.unwrap_or_else(|| ".".to_string());
    let path = std::path::PathBuf::from(target_dir);
    crate::kernel::repo_map::generate_repo_map(&path)
        .ok_or_else(|| "No eligible source symbols found in workspace".to_string())
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
async fn list_turn_checkpoints(
    repo_path: String,
) -> Result<Vec<crate::kernel::git::CheckpointInfo>, String> {
    crate::kernel::git::GitEngine::list_checkpoints(std::path::Path::new(&repo_path)).await
}

#[tauri::command]
async fn get_workspace_git_diff_files(
    repo_path: String,
) -> Result<Vec<crate::kernel::git::WorkspaceDiffFile>, String> {
    crate::kernel::git::GitEngine::list_diff_files(std::path::Path::new(&repo_path)).await
}

#[tauri::command]
async fn revert_workspace_files(repo_path: String, paths: Vec<String>) -> Result<usize, String> {
    crate::kernel::git::GitEngine::revert_files(std::path::Path::new(&repo_path), &paths).await
}

#[tauri::command]
async fn suggest_semantic_commit(
    repo_path: String,
) -> Result<crate::kernel::git::SemanticCommitSuggestion, String> {
    crate::kernel::git::GitEngine::suggest_semantic_commit(std::path::Path::new(&repo_path)).await
}

#[tauri::command]
async fn execute_semantic_commit(repo_path: String, message: String) -> Result<String, String> {
    let repo = std::path::Path::new(&repo_path);
    let _ = crate::kernel::git::GitEngine::git_exec(repo, &["add", "-A"]).await?;
    crate::kernel::git::GitEngine::commit(repo, message.trim()).await
}

#[tauri::command]
async fn mcp_list_tools() -> Result<Vec<crate::kernel::mcp::McpToolDefinition>, String> {
    Ok(crate::kernel::server::get_mcp_hub().list_tools())
}

#[tauri::command]
async fn mcp_call_tool(tool_name: String, args: serde_json::Value) -> Result<String, String> {
    crate::kernel::server::get_mcp_hub().call_tool(&tool_name, args)
}

#[tauri::command]
async fn scan_kronos_workspace_docs(
    cwd: String,
) -> Result<Vec<crate::kernel::workspace_tools::KronosWorkspaceDoc>, String> {
    Ok(crate::kernel::workspace_tools::scan_kronos_docs(
        std::path::Path::new(&cwd),
    ))
}

#[tauri::command]
async fn save_kronos_workspace_doc(
    cwd: String,
    relative_path: String,
    content: String,
) -> Result<String, String> {
    crate::kernel::workspace_tools::save_kronos_doc(
        std::path::Path::new(&cwd),
        &relative_path,
        &content,
    )
}

#[tauri::command]
async fn get_dependency_graph(
    cwd: String,
) -> Result<crate::kernel::repo_map::DependencyGraphResult, String> {
    crate::kernel::repo_map::generate_dependency_graph(std::path::Path::new(&cwd))
        .ok_or_else(|| "No source dependencies found in workspace".to_string())
}

#[tauri::command]
async fn discover_workspace_databases(cwd: String) -> Result<Vec<String>, String> {
    Ok(crate::kernel::workspace_tools::discover_databases(
        std::path::Path::new(&cwd),
    ))
}

#[tauri::command]
async fn execute_sqlite_query(
    cwd: String,
    db_path: String,
    sql: String,
    read_only: bool,
) -> Result<crate::kernel::workspace_tools::DatabaseQueryResult, String> {
    let resolved = if std::path::Path::new(&db_path).is_absolute() {
        std::path::PathBuf::from(&db_path)
    } else {
        std::path::Path::new(&cwd).join(&db_path)
    };
    crate::kernel::workspace_tools::run_sqlite_query(&resolved, &sql, read_only).await
}

#[tauri::command]
async fn workspace_text_search(
    req: crate::agent::ripgrep_search::WorkspaceSearchQuery,
) -> Result<Vec<crate::agent::ripgrep_search::FileMatchesGroup>, String> {
    tokio::task::spawn_blocking(move || crate::agent::ripgrep_search::search_workspace(&req))
        .await
        .map_err(|e| format!("Search task panicked: {e}"))?
}

#[tauri::command]
async fn workspace_batch_replace(
    req: crate::agent::ripgrep_search::WorkspaceBatchReplaceRequest,
) -> Result<crate::agent::ripgrep_search::WorkspaceBatchReplaceResult, String> {
    tokio::task::spawn_blocking(move || crate::agent::ripgrep_search::batch_replace_workspace(&req))
        .await
        .map_err(|e| format!("Replace task panicked: {e}"))?
}

#[tauri::command]
async fn get_github_pr_draft(cwd: String) -> Result<crate::agent::github_ops::PrDraftInfo, String> {
    tokio::task::spawn_blocking(move || {
        crate::agent::github_ops::get_github_pr_draft(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|e| format!("PR draft task panicked: {e}"))?
}

#[tauri::command]
async fn create_github_pr(
    req: crate::agent::github_ops::CreatePrRequest,
) -> Result<crate::agent::github_ops::CreatePrResult, String> {
    tokio::task::spawn_blocking(move || crate::agent::github_ops::execute_create_pr(&req))
        .await
        .map_err(|e| format!("PR creation task panicked: {e}"))?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudProjectDetection {
    framework: String,
    output_dir: String,
}

#[tauri::command]
async fn detect_cloud_project(cwd: String) -> Result<CloudProjectDetection, String> {
    let path = std::path::PathBuf::from(cwd);
    let (framework, output_dir) = crate::agent::cloud_deploy::detect_framework_output(&path);
    Ok(CloudProjectDetection {
        framework: framework.to_string(),
        output_dir: output_dir.to_string(),
    })
}

#[tauri::command]
async fn deploy_cloud_project(
    req: crate::agent::cloud_deploy::CloudDeployRequest,
) -> Result<crate::agent::cloud_deploy::CloudDeployResult, String> {
    tokio::task::spawn_blocking(move || crate::agent::cloud_deploy::deploy_to_cloud(&req))
        .await
        .map_err(|e| format!("Cloud deploy task panicked: {e}"))?
}

#[tauri::command]
async fn semantic_search_codebase(
    req: crate::agent::semantic_index::SemanticSearchRequest,
) -> Result<Vec<crate::agent::semantic_index::SemanticCodeChunk>, String> {
    crate::agent::semantic_index::search_codebase_semantic_hybrid(req).await
}

#[tauri::command]
async fn verify_web_ui_url(
    url: String,
) -> Result<crate::agent::browser_runner::BrowserVerificationReport, String> {
    crate::agent::browser_runner::verify_web_ui(&url).await
}

#[tauri::command]
async fn run_codebase_bug_audit(
    cwd: String,
) -> Result<crate::agent::bug_hunter::BugAuditReport, String> {
    tokio::task::spawn_blocking(move || {
        crate::agent::bug_hunter::BugHunter::scan_workspace(std::path::Path::new(&cwd))
    })
    .await
    .map_err(|e| format!("Bug audit task panicked: {e}"))?
}

#[tauri::command]
async fn detect_workspace_preview_url(cwd: String) -> String {
    tokio::task::spawn_blocking(move || {
        crate::kernel::workspace_tools::detect_workspace_preview_url(std::path::Path::new(&cwd))
    })
    .await
    .unwrap_or_else(|_| "http://localhost:5173".to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceEnvPayload {
    file_name: String,
    variables: Vec<crate::kernel::workspace_tools::EnvVarEntry>,
}

#[tauri::command]
async fn load_workspace_env_vars(cwd: String) -> Result<WorkspaceEnvPayload, String> {
    let (file_name, variables) =
        crate::kernel::workspace_tools::load_workspace_env_vars(std::path::Path::new(&cwd))?;
    Ok(WorkspaceEnvPayload {
        file_name,
        variables,
    })
}

#[tauri::command]
async fn save_workspace_env_vars(
    cwd: String,
    vars: Vec<crate::kernel::workspace_tools::EnvVarEntry>,
) -> Result<String, String> {
    crate::kernel::workspace_tools::save_workspace_env_vars(std::path::Path::new(&cwd), &vars)
}

#[tauri::command]
async fn set_permission_policy(
    policy: crate::agent::guardrails::PermissionPolicy,
) -> Result<(), String> {
    crate::agent::guardrails::set_permission_policy(policy);
    Ok(())
}

#[tauri::command]
async fn get_permission_policy() -> Result<crate::agent::guardrails::PermissionPolicy, String> {
    Ok(crate::agent::guardrails::get_permission_policy())
}

#[tauri::command]
async fn set_budget_policy(policy: crate::agent::guardrails::BudgetPolicy) -> Result<(), String> {
    crate::agent::guardrails::set_budget_policy(policy);
    Ok(())
}

#[tauri::command]
async fn get_budget_policy() -> Result<crate::agent::guardrails::BudgetPolicy, String> {
    Ok(crate::agent::guardrails::get_budget_policy())
}

#[tauri::command]
async fn reset_agent_budget_usage(agent_id: String) -> Result<(), String> {
    crate::agent::guardrails::reset_agent_usage(&agent_id);
    Ok(())
}

#[tauri::command]
async fn get_desktop_settings(
) -> Result<crate::kernel::config_store::DesktopSettingsConfig, String> {
    Ok(crate::kernel::config_store::get_config_store().get_desktop_settings())
}

#[tauri::command]
async fn patch_desktop_settings(
    patch: Option<serde_json::Value>,
) -> Result<crate::kernel::config_store::DesktopSettingsConfig, String> {
    crate::kernel::config_store::get_config_store()
        .patch_desktop_settings(patch.unwrap_or_else(|| serde_json::json!({})))
        .map_err(|e| format!("Failed persisting desktop settings: {e}"))
}

fn kronos_cli_bin_dir() -> PathBuf {
    get_data_dir().join("bin")
}

fn cli_shim_paths() -> (PathBuf, PathBuf) {
    let dir = kronos_cli_bin_dir();
    if cfg!(windows) {
        (dir.join("kronos.cmd"), dir.join("paseo.cmd"))
    } else {
        (dir.join("kronos"), dir.join("paseo"))
    }
}

fn cli_is_installed_on_system() -> bool {
    let (k_shim, p_shim) = cli_shim_paths();
    k_shim.is_file()
        || p_shim.is_file()
        || crate::agent::registry::resolve_command("paseo").is_some()
        || crate::agent::registry::resolve_command("kronos").is_some()
}

#[tauri::command]
async fn get_cli_install_status() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({ "installed": cli_is_installed_on_system() }))
}

#[tauri::command]
async fn install_cli() -> Result<serde_json::Value, String> {
    let dir = kronos_cli_bin_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed creating CLI bin directory: {e}"))?;
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed resolving executable path: {e}"))?;
    let exe_str = exe_path.to_string_lossy();
    let (k_shim, p_shim) = cli_shim_paths();

    #[cfg(windows)]
    let script = format!("@echo off\r\n\"{exe_str}\" --cli %*\r\n");
    #[cfg(not(windows))]
    let script = format!("#!/bin/sh\nexec \"{exe_str}\" --cli \"$@\"\n");

    for path in [&k_shim, &p_shim] {
        fs::write(path, script.as_bytes())
            .map_err(|e| format!("Failed writing CLI shim {}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)
                .map_err(|e| format!("Failed reading permissions: {e}"))?
                .permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(path, perms);
        }
    }

    Ok(serde_json::json!({ "installed": true, "binDir": dir.to_string_lossy() }))
}

#[tauri::command]
async fn uninstall_cli() -> Result<serde_json::Value, String> {
    let (k_shim, p_shim) = cli_shim_paths();
    for path in [&k_shim, &p_shim] {
        if path.is_file() {
            let _ = fs::remove_file(path);
        }
    }
    let still_on_path = crate::agent::registry::resolve_command("paseo").is_some()
        || crate::agent::registry::resolve_command("kronos").is_some();
    Ok(serde_json::json!({ "installed": still_on_path }))
}

fn read_diagnostic_file_entry(path: PathBuf) -> serde_json::Value {
    let exists = path.is_file();
    let modified_at = if exists {
        fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
    } else {
        None
    };
    let (contents, error) = if exists {
        match fs::read_to_string(&path) {
            Ok(c) => (c, serde_json::Value::Null),
            Err(e) => (String::new(), serde_json::Value::String(e.to_string())),
        }
    } else {
        (String::new(), serde_json::Value::Null)
    };
    serde_json::json!({
        "path": path.to_string_lossy(),
        "exists": exists,
        "modifiedAt": modified_at,
        "contents": contents,
        "error": error
    })
}

#[tauri::command]
async fn desktop_update_diagnostics(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdateState>,
) -> Result<serde_json::Value, String> {
    let data_dir = get_data_dir();
    let logs_dir = data_dir.join("logs");
    let configured = updater_configured(&app);
    let pending = state.0.lock().await;
    let target_version = pending.as_ref().map(|u| u.version.clone());
    let target_error = if !configured {
        Some("Automatic updates unavailable in unsigned local build (no updater pubkey configured).".to_string())
    } else {
        None
    };

    Ok(serde_json::json!({
        "platform": if cfg!(windows) { "win32" } else if cfg!(target_os = "macos") { "darwin" } else { "linux" },
        "currentVersion": env!("CARGO_PKG_VERSION"),
        "targetVersion": target_version,
        "targetVersionError": target_error,
        "shipItDirectory": data_dir.to_string_lossy(),
        "state": read_diagnostic_file_entry(data_dir.join("updater-state.json")),
        "stdout": read_diagnostic_file_entry(logs_dir.join("updater.log")),
        "stderr": read_diagnostic_file_entry(logs_dir.join("updater-error.log"))
    }))
}

#[tauri::command]
async fn desktop_sandbox_diagnostics() -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        match crate::process_job::JobSupervisor::new() {
            Ok(_job) => Ok(serde_json::json!({
                "enabled": true,
                "reason": "Windows Job Object process-tree isolation active (JobSupervisor kill-on-close)."
            })),
            Err(e) => Ok(serde_json::json!({
                "enabled": false,
                "reason": format!("Windows Job Object allocation failed: {e}")
            })),
        }
    }
    #[cfg(unix)]
    {
        Ok(serde_json::json!({
            "enabled": true,
            "reason": "POSIX process-group isolation active for spawned agent turns."
        }))
    }
    #[cfg(not(any(windows, unix)))]
    {
        Ok(serde_json::json!({
            "enabled": false,
            "reason": "No OS process sandbox backend available on this platform."
        }))
    }
}

#[tauri::command]
async fn mcp_list_servers() -> Result<Vec<crate::kernel::mcp::ExternalMcpServerConfig>, String> {
    Ok(crate::kernel::mcp::McpHub::load_external_servers())
}

#[tauri::command]
async fn mcp_save_servers(
    servers: Vec<crate::kernel::mcp::ExternalMcpServerConfig>,
) -> Result<(), String> {
    crate::kernel::mcp::McpHub::save_external_servers(&servers)
}

#[tauri::command]
async fn mcp_probe_server(
    server: crate::kernel::mcp::ExternalMcpServerConfig,
) -> Result<crate::kernel::mcp::ExternalMcpServerStatus, String> {
    tokio::task::spawn_blocking(move || {
        Ok(crate::kernel::server::get_mcp_hub().probe_and_sync_external_server(&server))
    })
    .await
    .map_err(|e| format!("MCP probe panicked: {e}"))?
}

#[tauri::command]
async fn mcp_sync_all_servers() -> Result<Vec<crate::kernel::mcp::ExternalMcpServerStatus>, String> {
    tokio::task::spawn_blocking(move || {
        let servers = crate::kernel::mcp::McpHub::load_external_servers();
        let hub = crate::kernel::server::get_mcp_hub();
        Ok(servers
            .iter()
            .map(|srv| hub.probe_and_sync_external_server(srv))
            .collect())
    })
    .await
    .map_err(|e| format!("MCP sync panicked: {e}"))?
}

#[tauri::command]
async fn list_available_ai_models() -> Vec<crate::agent::quick_llm::AvailableModelOption> {
    crate::agent::quick_llm::list_available_models().await
}

#[tauri::command]
async fn run_inline_code_edit(
    filename: String,
    selected_code: String,
    instruction: String,
    cwd: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<crate::agent::quick_llm::QuickLlmResponse, String> {
    crate::agent::quick_llm::run_inline_code_edit(
        &filename,
        &selected_code,
        &instruction,
        cwd.as_deref().map(std::path::Path::new),
        provider.as_deref(),
        model.as_deref(),
    )
    .await
}

#[tauri::command]
async fn synthesize_terminal_command(
    prompt: String,
    cwd: Option<String>,
    provider: Option<String>,
    model: Option<String>,
) -> Result<crate::agent::quick_llm::TerminalCommandSynthesis, String> {
    crate::agent::quick_llm::synthesize_terminal_command(
        &prompt,
        cwd.as_deref().map(std::path::Path::new),
        provider.as_deref(),
        model.as_deref(),
    )
    .await
}

#[tauri::command]
async fn run_inline_completion(
    filename: String,
    prefix: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<String, String> {
    crate::agent::quick_llm::run_inline_completion(
        &filename,
        &prefix,
        provider.as_deref(),
        model.as_deref(),
    )
    .await
}

#[tauri::command]
async fn generate_ai_semantic_commit(
    cwd: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<String, String> {
    crate::agent::quick_llm::generate_ai_semantic_commit(
        std::path::Path::new(&cwd),
        provider.as_deref(),
        model.as_deref(),
    )
    .await
}

#[tauri::command]
async fn run_model_arena_comparison(
    prompt: String,
    cwd: Option<String>,
    candidates: Vec<crate::agent::quick_llm::ArenaCandidateInput>,
) -> Vec<crate::agent::quick_llm::ArenaCandidateResult> {
    crate::agent::quick_llm::run_model_arena_comparison(
        &prompt,
        cwd.as_deref().map(std::path::Path::new),
        candidates,
    )
    .await
}

#[tauri::command]
async fn synthesize_consensus_patch(
    task_prompt: String,
    proposals: Vec<crate::agent::quick_llm::ConsensusProposalInput>,
    cwd: Option<String>,
) -> Result<crate::agent::quick_llm::QuickLlmResponse, String> {
    crate::agent::quick_llm::synthesize_consensus_patch(
        &task_prompt,
        &proposals,
        cwd.as_deref().map(std::path::Path::new),
    )
    .await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeechStatusPayload {
    loaded: bool,
    backend: String,
}

#[tauri::command]
async fn get_speech_status() -> SpeechStatusPayload {
    let lock = crate::kernel::server::get_speech_engine();
    let engine = lock.read().await;
    let loaded = engine.is_model_loaded();
    SpeechStatusPayload {
        loaded,
        backend: if loaded { "whisper" } else { "browser_or_unloaded" }.to_string(),
    }
}

#[tauri::command]
async fn transcribe_voice_pcm(
    samples: Vec<f32>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
) -> Result<String, String> {
    let lock = crate::kernel::server::get_speech_engine();
    let engine = lock.read().await;
    if !engine.is_model_loaded() {
        return Err("No local Whisper GGML model loaded (~/.paseo/models/ggml-*.bin)".to_string());
    }
    let cfg = crate::kernel::speech::SpeechAudioConfig::new(
        sample_rate.unwrap_or(16_000),
        channels.unwrap_or(1),
    );
    engine.transcribe_with_config(&samples, cfg)
}

#[tauri::command]
async fn download_whisper_model(model_name: Option<String>) -> Result<SpeechStatusPayload, String> {
    let variant = match model_name.as_deref().unwrap_or("tiny.en") {
        "base.en" => "ggml-base.en.bin",
        _ => "ggml-tiny.en.bin",
    };
    let models_dir = get_data_dir().join("models");
    fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed creating Whisper models directory: {e}"))?;
    let target_path = models_dir.join(variant);

    if !target_path.is_file() {
        let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{variant}");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| format!("Failed creating download HTTP client: {e}"))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed downloading {variant}: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "Whisper model download returned HTTP {}",
                resp.status()
            ));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed reading downloaded Whisper model bytes: {e}"))?;
        let path_clone = target_path.clone();
        tokio::task::spawn_blocking(move || {
            crate::kernel::config_store::atomic_write(&path_clone, &bytes)
        })
        .await
        .map_err(|e| format!("Failed joining model write task: {e}"))?
        .map_err(|e| format!("Failed writing Whisper model to disk: {e}"))?;
    }

    let load_path = target_path.clone();
    let new_engine = tokio::task::spawn_blocking(move || {
        crate::kernel::speech::SpeechEngine::new(Some(&load_path))
    })
    .await
    .map_err(|e| format!("Failed joining Whisper loader task: {e}"))??;

    let loaded = new_engine.is_model_loaded();
    let lock = crate::kernel::server::get_speech_engine();
    *lock.write().await = new_engine;

    Ok(SpeechStatusPayload {
        loaded,
        backend: if loaded { "whisper" } else { "browser_or_unloaded" }.to_string(),
    })
}

#[tauri::command]
async fn start_preview_tunnel(
    local_port: u16,
    provider: crate::kernel::tunnel::TunnelProvider,
) -> Result<crate::kernel::tunnel::TunnelState, String> {
    tokio::task::spawn_blocking(move || {
        let mut mgr = crate::kernel::tunnel::get_tunnel_manager()
            .lock()
            .map_err(|_| "Tunnel manager mutex poisoned".to_string())?;
        mgr.start(local_port, provider)
    })
    .await
    .map_err(|e| format!("Preview tunnel task panicked: {e}"))?
}

#[tauri::command]
async fn stop_preview_tunnel() -> crate::kernel::tunnel::TunnelState {
    if let Ok(mut mgr) = crate::kernel::tunnel::get_tunnel_manager().lock() {
        mgr.stop();
        mgr.state().clone()
    } else {
        crate::kernel::tunnel::TunnelState::Inactive
    }
}

#[tauri::command]
async fn get_preview_tunnel_state() -> crate::kernel::tunnel::TunnelState {
    if let Ok(mgr) = crate::kernel::tunnel::get_tunnel_manager().lock() {
        mgr.state().clone()
    } else {
        crate::kernel::tunnel::TunnelState::Inactive
    }
}

#[tauri::command]
async fn scaffold_project_template(
    parent_dir: String,
    project_name: String,
    command: String,
) -> Result<String, String> {
    let clean_name = project_name.trim();
    if clean_name.is_empty()
        || clean_name.contains("..")
        || clean_name.contains('/')
        || clean_name.contains('\\')
    {
        return Err("Invalid project name; use a simple folder name without path separators.".into());
    }
    let parent = std::path::PathBuf::from(&parent_dir);
    if !parent.is_dir() {
        return Err(format!("Target directory not found: {parent_dir}"));
    }
    let final_cmd = command.replace("my-app", clean_name);
    let target_dir = parent.join(clean_name);
    let parent_clone = parent.clone();
    let target_clone = target_dir.clone();

    tokio::task::spawn_blocking(move || -> Result<String, String> {
        let mut cmd = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", &final_cmd]);
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", &final_cmd]);
            c
        };
        cmd.current_dir(&parent_clone);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("Failed launching template scaffolder: {e}"))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            let out = String::from_utf8_lossy(&output.stdout);
            return Err(format!(
                "Template scaffold command failed ({}): {} {}",
                output.status,
                err.trim(),
                out.trim()
            ));
        }
        if !target_clone.exists() {
            fs::create_dir_all(&target_clone)
                .map_err(|e| format!("Failed creating target directory: {e}"))?;
        }
        Ok(target_clone.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("Scaffold task panicked: {e}"))?
}

#[tauri::command]
async fn load_workspace_memories(
    cwd: String,
) -> Result<Vec<crate::agent::memories::MemoryEntry>, String> {
    tokio::task::spawn_blocking(move || {
        let dir = std::path::Path::new(&cwd);
        if !dir.is_dir() {
            return Err(format!("Workspace directory not found: {cwd}"));
        }
        Ok(crate::agent::memories::load_workspace_memories(dir))
    })
    .await
    .map_err(|e| format!("Load memories task panicked: {e}"))?
}

#[tauri::command]
async fn save_workspace_memories(
    cwd: String,
    entries: Vec<crate::agent::memories::MemoryEntry>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let dir = std::path::Path::new(&cwd);
        if !dir.is_dir() {
            return Err(format!("Workspace directory not found: {cwd}"));
        }
        crate::agent::memories::save_workspace_memories(dir, &entries)
            .map_err(|e| format!("Failed to save .kronos/memories.md: {e}"))
    })
    .await
    .map_err(|e| format!("Save memories task panicked: {e}"))?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TestCommandRunResult {
    exit_code: i32,
    passed: bool,
    stdout: String,
    stderr: String,
    combined: String,
}

#[tauri::command]
async fn run_workspace_test_command(
    cwd: String,
    command: String,
) -> Result<TestCommandRunResult, String> {
    tokio::task::spawn_blocking(move || {
        let dir = std::path::Path::new(&cwd);
        if !dir.is_dir() {
            return Err(format!("Workspace directory not found: {cwd}"));
        }
        let policy = crate::agent::guardrails::get_permission_policy();
        let risk = crate::agent::guardrails::evaluate_command_risk(&command);
        if policy.execution_mode != "yolo" && risk.is_destructive() {
            return Err("Destructive command blocked by Guarded Execution Policy".to_string());
        }

        let mut cmd = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd.exe");
            c.args(["/C", &command]);
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
            c
        } else {
            let mut c = std::process::Command::new("sh");
            c.args(["-c", &command]);
            c
        };
        cmd.current_dir(dir);
        let output = cmd
            .output()
            .map_err(|e| format!("Failed executing test command: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{stdout}\n{stderr}");
        Ok(TestCommandRunResult {
            exit_code: output.status.code().unwrap_or(-1),
            passed: output.status.success(),
            stdout,
            stderr,
            combined,
        })
    })
    .await
    .map_err(|e| format!("Test runner panicked: {e}"))?
}

async fn forward_rpc_to_external_daemon_if_detached(message: serde_json::Value) {
    if crate::daemon::is_embedded_running() {
        return;
    }
    let port = crate::daemon::get_active_daemon_port();
    let ws_url = format!("ws://127.0.0.1:{port}/ws");
    let token_path = get_data_dir().join("local-credential");
    let token = std::fs::read_to_string(token_path)
        .unwrap_or_default()
        .trim()
        .to_string();
    let Ok((mut ws, _)) = tokio_tungstenite::connect_async(&ws_url).await else {
        return;
    };
    use futures_util::SinkExt;
    let hello = if token.is_empty() {
        serde_json::json!({ "type": "hello", "protocolVersion": 1 })
    } else {
        serde_json::json!({
            "type": "hello",
            "protocolVersion": 1,
            "auth": { "kind": "localCredential", "token": token }
        })
    };
    if ws
        .send(tokio_tungstenite::tungstenite::Message::Text(
            hello.to_string().into(),
        ))
        .await
        .is_ok()
    {
        let _ = ws
            .send(tokio_tungstenite::tungstenite::Message::Text(
                message.to_string().into(),
            ))
            .await;
    }
    let _ = ws.close(None).await;
}

#[tauri::command]
async fn set_agent_execution_mode(agent_id: String, mode_id: String) -> Result<(), String> {
    let mgr = crate::kernel::server::get_agent_manager();
    if let Some(mut agent) = mgr.get_agent(&agent_id) {
        let mut cfg = agent.config.unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert("modeId".to_string(), serde_json::json!(mode_id));
            obj.insert("persona".to_string(), serde_json::json!(mode_id));
        }
        agent.config = Some(cfg);
        mgr.storage().save_record(agent).map_err(|e| e.to_string())?;
        crate::agent::runner::broadcast_agent_status_update(&agent_id);
    } else if crate::daemon::is_embedded_running() {
        return Err(format!("Agent '{agent_id}' not found"));
    }
    forward_rpc_to_external_daemon_if_detached(serde_json::json!({
        "type": "set_agent_mode_request",
        "requestId": format!("tauri-mode-{agent_id}"),
        "agentId": agent_id,
        "modeId": mode_id,
    }))
    .await;
    Ok(())
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

static DAEMON_PROCESS_STARTED_AT: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| chrono::Utc::now().to_rfc3339());

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
        "startedAt": if is_healthy { Some(DAEMON_PROCESS_STARTED_AT.as_str()) } else { None },
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

fn is_running_under_arm64_translation() -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = std::process::Command::new("sysctl")
            .args(["-in", "sysctl.proc_translated"])
            .output()
        {
            return String::from_utf8_lossy(&out.stdout).trim() == "1";
        }
    }
    #[cfg(windows)]
    {
        if std::env::var("PROCESSOR_ARCHITECTURE")
            .map(|a| a.eq_ignore_ascii_case("x86"))
            .unwrap_or(false)
            && std::env::var("PROCESSOR_ARCHITEW6432")
                .map(|a| a.to_uppercase().contains("ARM64"))
                .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

#[tauri::command]
fn desktop_get_runtime_info() -> serde_json::Value {
    serde_json::json!({
        "appVersion": env!("CARGO_PKG_VERSION"),
        "runningUnderARM64Translation": is_running_under_arm64_translation(),
    })
}

#[tauri::command]
async fn get_local_daemon_version() -> serde_json::Value {
    let port = daemon::get_active_daemon_port();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();
    if let Ok(client) = client {
        let url = format!("http://127.0.0.1:{port}/api/version");
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(ver) = json.get("version").and_then(|v| v.as_str()) {
                    return serde_json::json!({
                        "version": ver,
                        "error": serde_json::Value::Null,
                    });
                }
            }
        }
    }
    if wait_for_daemon_health(1) {
        return serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "error": serde_json::Value::Null,
        });
    }
    serde_json::json!({
        "version": serde_json::Value::Null,
        "error": "Local daemon is not reachable on loopback.",
    })
}

#[tauri::command]
fn run_local_daemon_update(state: tauri::State<DaemonState>) -> serde_json::Value {
    let engine = std::env::var("KRONOS_DAEMON_ENGINE").unwrap_or_else(|_| "rust".to_string());
    if engine.eq_ignore_ascii_case("node") {
        let bin = if cfg!(windows) { "npm.cmd" } else { "npm" };
        if let Ok(out) = std::process::Command::new(bin)
            .args(["install", "-g", "@getpaseo/cli@latest"])
            .output()
        {
            let code = out.status.code().unwrap_or(1);
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if code == 0 {
                let _ = daemon::restart_daemon(&state);
            }
            return serde_json::json!({
                "exitCode": code,
                "stdout": stdout,
                "stderr": stderr,
            });
        }
    }

    let restarted = daemon::restart_daemon(&state);
    let healthy = if restarted { wait_for_daemon_health(20) } else { wait_for_daemon_health(1) };
    serde_json::json!({
        "exitCode": if healthy { 0 } else { 1 },
        "stdout": if healthy {
            format!("Embedded Kronos kernel daemon is active on v{} (port {}).", env!("CARGO_PKG_VERSION"), daemon::get_active_daemon_port())
        } else {
            String::new()
        },
        "stderr": if healthy {
            String::new()
        } else {
            "Embedded daemon did not respond healthy after restart.".to_string()
        },
    })
}

#[tauri::command]
fn cli_daemon_status() -> String {
    let live: serde_json::Value =
        serde_json::from_str(&get_daemon_status_json()).unwrap_or(serde_json::Value::Null);
    let status = live["status"].as_str().unwrap_or("unknown");
    let listen = live["listen"].as_str().unwrap_or("unknown");
    let pid = live["pid"]
        .as_u64()
        .map(|p| p.to_string())
        .unwrap_or_else(|| "none".to_string());
    let home = live["home"].as_str().unwrap_or("unknown");
    let version = live["version"].as_str().unwrap_or(env!("CARGO_PKG_VERSION"));
    let started = live["startedAt"].as_str().unwrap_or("n/a");
    let agents = crate::kernel::server::get_agent_manager().list_agents();
    let providers = crate::agent::registry::ProviderRegistry::new().list();
    let ready_providers: Vec<String> = providers
        .into_iter()
        .filter(|p| p.is_installed)
        .map(|p| p.id)
        .collect();
    let mcp_count = crate::kernel::mcp::McpHub::load_external_servers().len();
    let policy = crate::agent::guardrails::get_permission_policy();
    let budget = crate::agent::guardrails::get_budget_policy();

    format!(
        "Kronos Kernel Daemon Status\n\
         ---------------------------\n\
         State:            {}\n\
         Version:          {}\n\
         Listen:           {}\n\
         PID:              {}\n\
         Started At:       {}\n\
         Data Home:        {}\n\
         Active Agents:    {}\n\
         Ready Providers:  {}\n\
         External MCPs:    {}\n\
         Guardrails Mode:  {}\n\
         Cost Limit (USD): {:?}",
        status,
        version,
        listen,
        pid,
        started,
        home,
        agents.len(),
        if ready_providers.is_empty() {
            "none".to_string()
        } else {
            ready_providers.join(", ")
        },
        mcp_count,
        policy.execution_mode,
        budget.max_cost_usd
    )
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

#[cfg(target_os = "macos")]
fn get_system_idle_time_ms() -> u64 {
    std::process::Command::new("ioreg")
        .args(["-c", "IOHIDSystem"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                if line.contains("HIDIdleTime") {
                    if let Some((_, val)) = line.split_once('=') {
                        if let Ok(ns) = val.trim().parse::<u64>() {
                            return Some(ns / 1_000_000);
                        }
                    }
                }
            }
            None
        })
        .unwrap_or(0)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn get_system_idle_time_ms() -> u64 {
    if let Ok(out) = std::process::Command::new("xprintidle").output() {
        if out.status.success() {
            if let Ok(ms) = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>() {
                return ms;
            }
        }
    }
    if let Ok(out) = std::process::Command::new("dbus-send")
        .args([
            "--print-reply=literal",
            "--dest=org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
            "org.freedesktop.ScreenSaver.GetSessionIdleTime",
        ])
        .output()
    {
        if out.status.success() {
            let txt = String::from_utf8_lossy(&out.stdout);
            if let Some(num) = txt.split_whitespace().last().and_then(|s| s.parse::<u64>().ok()) {
                return num;
            }
        }
    }
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
      ask: async function(message, options) {{
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const opts = options || {{}};
            return await window.__TAURI_INTERNALS__.invoke('ask_confirmation_dialog', {{
              message: String(message || ''),
              title: opts.title || null,
              okLabel: opts.okLabel || null,
              cancelLabel: opts.cancelLabel || null,
              kind: opts.kind || null
            }});
          }}
        }} catch (e) {{
          console.error('[TauriBridge] ask dialog failed:', e);
        }}
        return typeof window.confirm === 'function' ? window.confirm(String(message || '')) : false;
      }},
      askWithCheckbox: async function(message, options) {{
        const opts = options || {{}};
        if (opts.checkboxLabel && typeof document !== 'undefined' && document.body) {{
          return await new Promise((resolve) => {{
            const overlay = document.createElement('div');
            overlay.style.cssText = 'position:fixed;inset:0;z-index:2147483647;background:rgba(0,0,0,0.55);display:flex;align-items:center;justify-content:center;font-family:system-ui,sans-serif;';
            const box = document.createElement('div');
            box.style.cssText = 'background:#18181b;color:#f4f4f5;border:1px solid #3f3f46;border-radius:10px;padding:18px 20px;max-width:440px;width:90%;box-shadow:0 20px 50px rgba(0,0,0,0.45);';
            const titleEl = document.createElement('div');
            titleEl.style.cssText = 'font-weight:600;font-size:15px;margin-bottom:8px;';
            titleEl.textContent = opts.title || 'Confirm';
            const msgEl = document.createElement('div');
            msgEl.style.cssText = 'font-size:13px;line-height:1.45;color:#d4d4d8;margin-bottom:14px;word-break:break-word;';
            msgEl.textContent = String(message || '');
            const labelEl = document.createElement('label');
            labelEl.style.cssText = 'display:flex;align-items:center;gap:8px;font-size:12px;color:#a1a1aa;margin-bottom:16px;cursor:pointer;user-select:none;';
            const cb = document.createElement('input');
            cb.type = 'checkbox';
            const cbText = document.createElement('span');
            cbText.textContent = String(opts.checkboxLabel);
            labelEl.appendChild(cb);
            labelEl.appendChild(cbText);
            const actions = document.createElement('div');
            actions.style.cssText = 'display:flex;justify-content:flex-end;gap:8px;';
            const cancelBtn = document.createElement('button');
            cancelBtn.textContent = opts.cancelLabel || 'Cancel';
            cancelBtn.style.cssText = 'padding:6px 12px;border-radius:6px;border:1px solid #3f3f46;background:#27272a;color:#e4e4e7;cursor:pointer;font-size:13px;';
            const okBtn = document.createElement('button');
            okBtn.textContent = opts.okLabel || 'Confirm';
            okBtn.style.cssText = 'padding:6px 14px;border-radius:6px;border:none;background:#3b82f6;color:#ffffff;font-weight:600;cursor:pointer;font-size:13px;';
            const cleanup = (confirmed) => {{
              const dontAskAgain = !!cb.checked;
              window.removeEventListener('keydown', onKey);
              overlay.remove();
              resolve({{ confirmed, dontAskAgain }});
            }};
            const onKey = (ev) => {{
              if (ev.key === 'Escape') cleanup(false);
            }};
            window.addEventListener('keydown', onKey);
            cancelBtn.onclick = () => cleanup(false);
            okBtn.onclick = () => cleanup(true);
            actions.appendChild(cancelBtn);
            actions.appendChild(okBtn);
            box.appendChild(titleEl);
            box.appendChild(msgEl);
            box.appendChild(labelEl);
            box.appendChild(actions);
            overlay.appendChild(box);
            document.body.appendChild(overlay);
          }});
        }}
        try {{
          if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
            const confirmed = await window.__TAURI_INTERNALS__.invoke('ask_confirmation_dialog', {{
              message: String(message || ''),
              title: opts.title || null,
              okLabel: opts.okLabel || null,
              cancelLabel: opts.cancelLabel || null,
              kind: opts.kind || null
            }});
            return {{ confirmed: !!confirmed, dontAskAgain: false }};
          }}
        }} catch (e) {{
          console.error('[TauriBridge] askWithCheckbox failed:', e);
        }}
        const fallback = typeof window.confirm === 'function' ? window.confirm(String(message || '')) : false;
        return {{ confirmed: fallback, dontAskAgain: false }};
      }}
    }},
    browser: {{
      profilePartition: 'persist:kronos',
      executeAutomationCommand: async function(request) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('browser_automation_execute', {{ request: request }});
        }}
        throw new Error('Tauri IPC unavailable');
      }},
      registerAttachedBrowser: async function(input) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          await window.__TAURI_INTERNALS__.invoke('browser_register_attached', {{
            browserId: input && input.browserId,
            workspaceId: (input && input.workspaceId) || 'default',
            url: (input && input.url) || null
          }});
        }}
      }},
      unregisterWorkspaceBrowser: async function(browserId) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          await window.__TAURI_INTERNALS__.invoke('browser_unregister_tab', {{ browserId: browserId }});
        }}
      }},
      focus: async function(browserId) {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          return await window.__TAURI_INTERNALS__.invoke('browser_focus_tab', {{ browserId: browserId }});
        }}
        return false;
      }},
      clearProfile: async function() {{
        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
          await window.__TAURI_INTERNALS__.invoke('browser_clear_profile');
        }}
      }},
      setShortcutPolicy: async function() {{}},
      setWorkspaceActiveBrowser: async function() {{}}
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
      if (command === 'get_cli_install_status') {{
        return await window.__TAURI_INTERNALS__.invoke('get_cli_install_status');
      }}
      if (command === 'install_cli') {{
        return await window.__TAURI_INTERNALS__.invoke('install_cli');
      }}
      if (command === 'uninstall_cli') {{
        return await window.__TAURI_INTERNALS__.invoke('uninstall_cli');
      }}
      if (command === 'desktop_update_diagnostics') {{
        return await window.__TAURI_INTERNALS__.invoke('desktop_update_diagnostics');
      }}
      if (command === 'desktop_sandbox_diagnostics') {{
        return await window.__TAURI_INTERNALS__.invoke('desktop_sandbox_diagnostics');
      }}
      if (command === 'check_app_update') {{
        return await window.__TAURI_INTERNALS__.invoke('check_app_update', args || {{}});
      }}
      if (command === 'install_app_update') {{
        return await window.__TAURI_INTERNALS__.invoke('install_app_update', args || {{}});
      }}
      if (command === 'desktop_get_runtime_info') {{
        return await window.__TAURI_INTERNALS__.invoke('desktop_get_runtime_info');
      }}
      if (command === 'get_local_daemon_version') {{
        return await window.__TAURI_INTERNALS__.invoke('get_local_daemon_version');
      }}
      if (command === 'run_local_daemon_update') {{
        return await window.__TAURI_INTERNALS__.invoke('run_local_daemon_update');
      }}
      if (command === 'cli_daemon_status') {{
        return await window.__TAURI_INTERNALS__.invoke('cli_daemon_status');
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
      if (command === 'get_desktop_settings') {{
        return await window.__TAURI_INTERNALS__.invoke('get_desktop_settings');
      }}
      if (command === 'patch_desktop_settings') {{
        return await window.__TAURI_INTERNALS__.invoke('patch_desktop_settings', {{ patch: args || {{}} }});
      }}
      if (command === 'set_budget_policy') {{
        return await window.__TAURI_INTERNALS__.invoke('set_budget_policy', args || {{}});
      }}
      if (command === 'get_budget_policy') {{
        return await window.__TAURI_INTERNALS__.invoke('get_budget_policy');
      }}
      if (command === 'reset_agent_budget_usage') {{
        return await window.__TAURI_INTERNALS__.invoke('reset_agent_budget_usage', args || {{}});
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
      if (command === 'mark_first_paint' || command === 'watch_directory' || command === 'unwatch_directory' || command === 'analyze_ast' || command === 'crdt_doc_action' || command === 'get_ioring_status' || command === 'transcribe_audio_chunk' || command === 'get_speech_engine_status' || command === 'get_shm_status' || command === 'push_shm_chunk' || command === 'web_search_query' || command === 'web_scrape_url' || command === 'get_repo_map' || command === 'synthesize_terminal_command') {{
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
            discover_local_ai_providers,
            web_search_query,
            web_scrape_url,
            get_repo_map,
            list_turn_checkpoints,
            get_workspace_git_diff_files,
            revert_workspace_files,
            suggest_semantic_commit,
            execute_semantic_commit,
            mcp_list_tools,
            mcp_call_tool,
            scan_kronos_workspace_docs,
            save_kronos_workspace_doc,
            get_dependency_graph,
            discover_workspace_databases,
            execute_sqlite_query,
            workspace_text_search,
            workspace_batch_replace,
            get_github_pr_draft,
            create_github_pr,
            detect_cloud_project,
            deploy_cloud_project,
            semantic_search_codebase,
            verify_web_ui_url,
            run_codebase_bug_audit,
            detect_workspace_preview_url,
            load_workspace_env_vars,
            save_workspace_env_vars,
            set_permission_policy,
            get_permission_policy,
            set_budget_policy,
            get_budget_policy,
            reset_agent_budget_usage,
            ask_confirmation_dialog,
            get_desktop_settings,
            patch_desktop_settings,
            get_cli_install_status,
            install_cli,
            uninstall_cli,
            desktop_update_diagnostics,
            desktop_sandbox_diagnostics,
            desktop_get_runtime_info,
            get_local_daemon_version,
            run_local_daemon_update,
            cli_daemon_status,
            mcp_list_servers,
            mcp_save_servers,
            mcp_probe_server,
            mcp_sync_all_servers,
            list_available_ai_models,
            run_inline_code_edit,
            synthesize_terminal_command,
            run_inline_completion,
            generate_ai_semantic_commit,
            run_model_arena_comparison,
            synthesize_consensus_patch,
            get_speech_status,
            download_whisper_model,
            transcribe_voice_pcm,
            run_workspace_test_command,
            set_agent_execution_mode,
            start_preview_tunnel,
            stop_preview_tunnel,
            get_preview_tunnel_state,
            scaffold_project_template,
            load_workspace_memories,
            save_workspace_memories,
            browser_automation_execute,
            browser_register_attached,
            browser_unregister_tab,
            browser_focus_tab,
            browser_clear_profile
        ])
        .setup(move |app| {
            crate::browser_automation::init_app_handle(app.handle().clone());
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
        let status = get_speech_engine_status().await;
        assert!(status.is_ok());
        let sval = status.unwrap();
        assert_eq!(sval["target_sample_rate"], 16000);

        let synthetic_samples: Vec<f32> =
            (0..16000).map(|i| (i as f32 * 0.05).sin() * 0.75).collect();

        let res = transcribe_audio_chunk(synthetic_samples, Some(16000), Some(1)).await;
        if sval["is_model_loaded"].as_bool().unwrap_or(false) {
            assert!(
                res.is_ok(),
                "transcribe_audio_chunk should succeed when model is loaded: {:?}",
                res.err()
            );
            let val = res.unwrap();
            assert!(val["transcript"].is_string());
            assert_eq!(val["samples_count"], 16000);
        } else {
            assert!(
                res.is_err(),
                "transcribe_audio_chunk must reject when no Whisper model exists instead of faking a transcript"
            );
            assert!(res.unwrap_err().contains("ggml-*.bin"));
        }
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
