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
fn wait_for_daemon_health(max_attempts: usize) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    for _ in 0..max_attempts {
        if let Ok(mut stream) = TcpStream::connect("127.0.0.1:6767") {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let req = b"GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:6767\r\nConnection: close\r\n\r\n";
            if stream.write_all(req).is_ok() {
                let mut buf = [0u8; 64];
                if let Ok(n) = stream.read(&mut buf) {
                    if n > 0 && (buf.starts_with(b"HTTP/1.1 200") || buf.starts_with(b"HTTP/1.0 200")) {
                        return true;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

fn get_or_create_server_id() -> String {
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".paseo");
    let _ = fs::create_dir_all(&home);

    let server_id_file = home.join("server-id");
    if let Ok(content) = fs::read_to_string(&server_id_file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let pid = std::process::id();
    let id = format!("srv_{:x}{:x}", now, pid);
    let _ = fs::write(&server_id_file, format!("{}\n", id));
    id
}

fn get_daemon_status_json() -> String {
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".paseo");

    let server_id = get_or_create_server_id();
    let mut local_credential = String::new();
    let credential_file = home.join("local-credential");
    if let Ok(content) = fs::read_to_string(&credential_file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            local_credential = trimmed.to_string();
        }
    }

    let listen = "127.0.0.1:6767".to_string();
    let hostname = "localhost".to_string();
    let pid: u32 = std::process::id();

    serde_json::json!({
        "serverId": server_id,
        "status": "running",
        "listen": listen,
        "hostname": hostname,
        "pid": pid,
        "localCredential": local_credential,
        "home": home.to_string_lossy().to_string(),
        "version": env!("CARGO_PKG_VERSION"),
        "desktopManaged": true,
        "ownedByDesktop": true,
        "startedAt": "2026-09-26T12:00:00.000Z",
        "error": null
    }).to_string()
}

#[tauri::command]
fn get_live_daemon_status() -> serde_json::Value {
    serde_json::from_str(&get_daemon_status_json()).unwrap_or(serde_json::Value::Null)
}

#[tauri::command]
fn get_live_local_credential() -> Option<String> {
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".paseo");
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
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".paseo");
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
    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.push(".paseo");
    let _ = fs::create_dir_all(&home);

    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    let entry = format!("[ts:{}] [{}] {}\n", now, level, message);

    let app_log = home.join("app.log");
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&app_log) {
        let _ = f.write_all(entry.as_bytes());
    }

    if level == "error" || level == "crash" || level == "panic" {
        let crash_log = home.join("crash.log");
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&crash_log) {
            let _ = f.write_all(entry.as_bytes());
        }
    }
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.push(".paseo");
        let _ = fs::create_dir_all(&home);
        let crash_log = home.join("crash.log");

        use std::io::Write;
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown".to_string());
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

        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&crash_log) {
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
                    let mut home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                    home.push(".paseo");
                    let _ = fs::create_dir_all(&home);
                    let crash_log = home.join("crash.log");
                    use std::io::Write;
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    let msg = format!(
                        "[ts:{}] [crashpad] Found {} WebView2 crashpad dump(s) in reports folder: {:?}\n",
                        now,
                        dumps.len(),
                        dumps
                    );
                    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&crash_log) {
                        let _ = f.write_all(msg.as_bytes());
                    }
                }
            }
        }
    }
}

fn main() {
    setup_panic_hook();
    check_and_log_crashpad_dumps();

    #[cfg(target_os = "windows")]
    {
        // Prevent DWM occlusion freezes while allowing Chromium GPU watchdog and driver compatibility protections
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--disable-features=CalculateNativeWinOcclusion",
        );
    }

    let server_id = get_or_create_server_id();
    let daemon_child = start_daemon(Some(&server_id));
    let state = DaemonState {
        process: Mutex::new(daemon_child),
    };
    // Background thread probes daemon health non-blockingly so window renders immediately
    std::thread::spawn(|| {
        let is_healthy = wait_for_daemon_health(50);
        if is_healthy {
            println!("[daemon] Host daemon confirmed healthy on 127.0.0.1:6767");
        } else {
            eprintln!("[daemon] Host daemon not responding after startup timeout");
        }
    });


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
    invoke: async function(command, args) {{
      if (command === 'desktop_daemon_status' || command === 'start_desktop_daemon') {{
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
        return 0;
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
            log_desktop_event
        ])
        .setup(move |app| {
            let _ = setup_tray(app.handle());
            setup_notification_listeners(app.handle());
            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Kronos")
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .shadow(true)
                .visible(true)
                .initialization_script(&bridge_script);

            let win = win_builder.build().ok();
            if let Some(w) = win {
                let _ = w.show();
                let _ = w.set_focus();
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
