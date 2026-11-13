use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let folder = app.dialog().file().blocking_pick_folder();

    Ok(folder.and_then(|p| {
        p.into_path()
            .ok()
            .map(|path| path.to_string_lossy().to_string())
    }))
}

#[tauri::command]
pub async fn pick_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app.dialog().file().blocking_pick_file();

    Ok(file.and_then(|p| {
        p.into_path()
            .ok()
            .map(|path| path.to_string_lossy().to_string())
    }))
}

#[tauri::command]
pub async fn ask_confirmation_dialog(
    app: tauri::AppHandle,
    message: String,
    title: Option<String>,
    ok_label: Option<String>,
    cancel_label: Option<String>,
    kind: Option<String>,
) -> Result<bool, String> {
    let mut builder = app.dialog().message(message);
    if let Some(ref t) = title {
        if !t.trim().is_empty() {
            builder = builder.title(t);
        }
    }
    let dialog_kind = match kind.as_deref() {
        Some("warning") => MessageDialogKind::Warning,
        Some("error") => MessageDialogKind::Error,
        _ => MessageDialogKind::Info,
    };
    builder = builder.kind(dialog_kind);
    let ok = ok_label
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Confirm".to_string());
    let cancel = cancel_label
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Cancel".to_string());
    builder = builder.buttons(MessageDialogButtons::OkCancelCustom(ok, cancel));
    let confirmed = tokio::task::spawn_blocking(move || builder.blocking_show())
        .await
        .map_err(|e| format!("Confirmation dialog task failed: {e}"))?;
    Ok(confirmed)
}
