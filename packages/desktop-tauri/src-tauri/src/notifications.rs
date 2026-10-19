use tauri::{AppHandle, Listener};
use tauri_plugin_notification::NotificationExt;

#[tauri::command]
pub fn show_notification(
    app: AppHandle,
    title: String,
    body: Option<String>,
) -> Result<(), String> {
    std::thread::spawn(move || {
        let mut builder = app.notification().builder().title(title);
        if let Some(b) = body {
            builder = builder.body(b);
        }
        let _ = builder.show();
    });
    Ok(())
}

#[tauri::command]
pub fn notify_task_complete(
    app: AppHandle,
    task_name: String,
    status: Option<String>,
    details: Option<String>,
) -> Result<(), String> {
    std::thread::spawn(move || {
        let title = format!("Task Complete: {}", task_name);
        let body_text = match (status, details) {
            (Some(s), Some(d)) => format!("{}: {}", s, d),
            (Some(s), None) => format!("Status: {}", s),
            (None, Some(d)) => d,
            (None, None) => "Background task has completed successfully.".to_string(),
        };
        let _ = app
            .notification()
            .builder()
            .title(title)
            .body(body_text)
            .show();
    });
    Ok(())
}

pub fn setup_notification_listeners(app: &AppHandle) {
    let app_handle = app.clone();
    app.listen("show_notification", move |event| {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(event.payload()) {
            let title = val
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Paseo Notification");
            let body = val.get("body").and_then(|v| v.as_str());

            let mut builder = app_handle.notification().builder().title(title);
            if let Some(b) = body {
                builder = builder.body(b);
            }
            let _ = builder.show();
        } else {
            // String payload fallback
            let body_str = event.payload();
            let _ = app_handle
                .notification()
                .builder()
                .title("Paseo Notification")
                .body(body_str)
                .show();
        }
    });

    let app_handle_task = app.clone();
    app.listen("task-completed", move |event| {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(event.payload()) {
            let task_name = val
                .get("taskName")
                .or_else(|| val.get("task"))
                .and_then(|v| v.as_str())
                .unwrap_or("Background Task");
            let title = format!("Task Complete: {}", task_name);
            let details = val
                .get("details")
                .or_else(|| val.get("body"))
                .or_else(|| val.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Task has finished execution.");

            let _ = app_handle_task
                .notification()
                .builder()
                .title(title)
                .body(details)
                .show();
        } else {
            let _ = app_handle_task
                .notification()
                .builder()
                .title("Task Complete")
                .body(event.payload())
                .show();
        }
    });
}
