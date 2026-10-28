use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

pub struct TrayState {
    pub active_agents_item: MenuItem<tauri::Wry>,
    pub last_count: std::sync::atomic::AtomicUsize,
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let active_agents_i = MenuItem::with_id(
        app,
        "active_agents",
        "Active Agents: 0",
        false,
        None::<&str>,
    )?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let new_chat_i = MenuItem::with_id(app, "new_chat", "New Agent Chat", true, None::<&str>)?;
    let toggle_window_i =
        MenuItem::with_id(app, "toggle_window", "Toggle Window", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let restart_daemon_i =
        MenuItem::with_id(app, "restart_daemon", "Restart Daemon", true, None::<&str>)?;
    let show_logs_i = MenuItem::with_id(app, "show_logs", "Show Logs", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit Paseo", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &active_agents_i,
            &sep1,
            &new_chat_i,
            &toggle_window_i,
            &sep2,
            &restart_daemon_i,
            &show_logs_i,
            &sep3,
            &quit_i,
        ],
    )?;

    app.manage(TrayState {
        active_agents_item: active_agents_i,
        last_count: std::sync::atomic::AtomicUsize::new(usize::MAX),
    });

    let _tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Kronos")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "new_chat" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                    let _ = window.emit(
                        "navigate",
                        serde_json::json!({ "route": "/chat", "action": "new_chat" }),
                    );
                    let _ = window.emit(
                        "paseo:new-chat",
                        serde_json::json!({ "action": "new_chat" }),
                    );
                    let _ = window.emit("open-agent", serde_json::json!({ "new": true }));
                }
            }
            "toggle_window" => {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
            "restart_daemon" => {
                if let Some(state) = app.try_state::<crate::daemon::DaemonState>() {
                    let restarted = crate::daemon::restart_daemon(&state);
                    let _ = app
                        .notification()
                        .builder()
                        .title("Paseo Daemon")
                        .body(if restarted {
                            "Daemon restarted successfully."
                        } else {
                            "Failed to restart daemon."
                        })
                        .show();
                }
                let _ = app.emit("daemon:restarted", ());
            }
            "show_logs" => {
                if let Some(mut home) = dirs::home_dir() {
                    home.push(".paseo");
                    let log_file = home.join("daemon.log");
                    let target = if log_file.exists() { log_file } else { home };
                    let _ = app
                        .opener()
                        .open_path(target.to_string_lossy(), None::<&str>);
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[tauri::command]
pub fn update_active_agents(app: AppHandle, count: usize) -> Result<(), String> {
    if let Some(state) = app.try_state::<TrayState>() {
        let previous = state
            .last_count
            .swap(count, std::sync::atomic::Ordering::SeqCst);
        if previous == count {
            return Ok(());
        }
        let text = format!("Active Agents: {count}");
        let _ = state.active_agents_item.set_text(text);
    }
    if let Some(tray) = app.tray_by_id("main") {
        let tooltip = if count == 0 {
            "Kronos".to_string()
        } else if count == 1 {
            "Kronos (1 active agent)".to_string()
        } else {
            format!("Kronos ({count} active agents)")
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
    Ok(())
}
