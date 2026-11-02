use std::process::Command;

#[tauri::command]
pub fn open_in_editor(editor: String, file_path: String, line: Option<u32>) -> Result<(), String> {
    let mut cmd = Command::new(&editor);
    if let Some(l) = line {
        if editor.contains("code") || editor.contains("cursor") {
            cmd.arg("-g").arg(format!("{}:{}", file_path, l));
        } else {
            cmd.arg(&file_path);
        }
    } else {
        cmd.arg(&file_path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}
