use tauri_plugin_dialog::DialogExt;

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
