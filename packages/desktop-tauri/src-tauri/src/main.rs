// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod editor;

mod tray;
mod notifications;

use tray::{setup_tray, update_active_agents};
use notifications::{notify_task_complete, setup_notification_listeners, show_notification};
mod dialogs;

use dialogs::{pick_file, pick_folder};
use daemon::{start_daemon, stop_daemon, DaemonState};
use editor::open_in_editor;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let is_visible = win.is_visible().unwrap_or(false);
        let is_focused = win.is_focused().unwrap_or(false);
        if is_visible && is_focused {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }
}
fn get_daemon_status_json() -> String {
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".paseo");
    let pid_file = home.join("paseo.pid");

    let mut server_id = "srv_AUs6tLHBB3Fq".to_string();
    let mut listen = "127.0.0.1:6767".to_string();
    let mut hostname = "localhost".to_string();
    let mut pid: u32 = 1;

    for _ in 0..6 {
        if let Ok(content) = fs::read_to_string(&pid_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(s) = val.get("serverId").and_then(|v| v.as_str()) {
                    server_id = s.to_string();
                }
                if let Some(l) = val.get("listen").and_then(|v| v.as_str()) {
                    listen = l.to_string();
                }
                if let Some(h) = val.get("hostname").and_then(|v| v.as_str()) {
                    hostname = h.to_string();
                }
                if let Some(p) = val.get("pid").and_then(|v| v.as_u64()) {
                    pid = p as u32;
                }
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    serde_json::json!({
        "serverId": server_id,
        "status": "running",
        "listen": listen,
        "hostname": hostname,
        "pid": pid,
        "home": home.to_string_lossy().to_string(),
        "version": "0.12.0",
        "desktopManaged": true,
        "ownedByDesktop": true,
        "startedAt": "2026-09-26T12:00:00.000Z",
        "error": null
    }).to_string()
}

fn main() {
    let daemon_child = start_daemon();
    let state = DaemonState {
        process: Mutex::new(daemon_child),
    };

    let status_json = get_daemon_status_json();
    let bridge_script = format!(
        r#"
try {{
  const daemonData = {status_json};
  const listenEndpoint = (daemonData.listen || 'localhost:6767').replace(/^http:\/\//, '').replace(/\/.*$/, '');
  const endpoint = listenEndpoint.startsWith('127.0.0.1') ? listenEndpoint.replace('127.0.0.1', 'localhost') : listenEndpoint;

  window.paseoDesktop = {{
    platform: 'win32',
    windowChromeMode: 'custom-windows',
    invoke: async function(command, args) {{
      if (command === 'desktop_daemon_status' || command === 'start_desktop_daemon') {{
        return daemonData;
      }}
      if (command === 'desktop_daemon_logs') {{
        return {{ logPath: daemonData.home + '/daemon.log', contents: '' }};
      }}
      if (command === 'desktop_app_logs') {{
        return {{ logPath: daemonData.home + '/app.log', contents: '' }};
      }}
      if (command === 'stop_desktop_daemon' || command === 'restart_desktop_daemon') {{
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
      return null;
    }}
  }};

  // Seed valid StoredHostProfile matching StoredHostProfileSchema
  try {{
    const REGISTRY_KEY = '@paseo:daemon-registry';
    const localHostProfile = [{{
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
    }}];
    localStorage.setItem(REGISTRY_KEY, JSON.stringify(localHostProfile));
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
        const deepLink = args.find(a => typeof a === 'string' && a.startsWith('paseo://'));
        if (deepLink) {{
          window.dispatchEvent(new CustomEvent('paseo:deep-link', {{ detail: deepLink }}));
        }}
      }});
    }}
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
        .invoke_handler(tauri::generate_handler![
            open_in_editor,
            pick_folder,
            pick_file,
            update_active_agents,
            show_notification,
            notify_task_complete
        ])
        .setup(move |app| {
            let _ = setup_tray(app.handle());
            setup_notification_listeners(app.handle());
            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Paseo")
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .initialization_script(&bridge_script);

            #[cfg(target_os = "windows")]
            let win_builder = {
                let effects = tauri::window::EffectsBuilder::new()
                    .effect(tauri::window::Effect::Mica)
                    .build();
                win_builder.effects(effects).transparent(true).shadow(true)
            };

            let win = win_builder.build();
            if win.is_err() {
                eprintln!("[window] Failed to build window with effects, falling back to standard window");
                let fallback_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                    .title("Paseo")
                    .inner_size(1280.0, 800.0)
                    .min_inner_size(800.0, 600.0)
                    .initialization_script(&bridge_script);
                let _ = fallback_builder.build();
            }

            // Register global shortcut to toggle main window visibility
            for shortcut in ["CommandOrControl+Shift+P", "Alt+Space"] {
                if let Err(err) = app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_main_window(app);
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
                                eprintln!("[updater] Failed to check for updates: {e}");
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
                if let Some(state) = app_handle.try_state::<DaemonState>() {
                    stop_daemon(&state);
                }
            }
            _ => {}
        });
}
