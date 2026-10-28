use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const ATTACHMENTS_DIRNAME: &str = "desktop-attachments";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentFileResult {
    pub path: String,
    pub byte_size: u64,
}

fn get_attachments_dir() -> Result<PathBuf, String> {
    let mut dir =
        dirs::home_dir().ok_or_else(|| "Could not determine user home directory".to_string())?;
    dir.push(".paseo");
    dir.push(ATTACHMENTS_DIRNAME);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create attachments directory: {}", e))?;
    Ok(dir)
}

fn validate_attachment_id(id: &str) -> Result<String, String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("Attachment id is required.".to_string());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("Invalid attachment id: {}", trimmed));
    }
    Ok(trimmed.to_string())
}

fn normalize_extension(ext: Option<&str>) -> Result<String, String> {
    let raw = match ext {
        Some(s) => s.trim(),
        None => "",
    };
    if raw.is_empty() {
        return Ok(".bin".to_string());
    }
    let lower = raw.to_lowercase();
    let normalized = if lower.starts_with('.') {
        lower
    } else {
        format!(".{}", lower)
    };
    let chars_after_dot = &normalized[1..];
    if chars_after_dot.is_empty()
        || chars_after_dot.len() > 16
        || !chars_after_dot.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return Err(format!("Invalid attachment extension: {}", raw));
    }
    Ok(normalized)
}

fn build_managed_attachment_path(
    attachment_id: &str,
    extension: Option<&str>,
) -> Result<PathBuf, String> {
    let dir = get_attachments_dir()?;
    let valid_id = validate_attachment_id(attachment_id)?;
    let ext = normalize_extension(extension)?;
    Ok(dir.join(format!("{}{}", valid_id, ext)))
}

fn clean_path_str(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

fn resolve_managed_attachment_path(input_path: &str) -> Result<PathBuf, String> {
    let trimmed = input_path.trim();
    if trimmed.is_empty() {
        return Err("Attachment path is required.".to_string());
    }

    let dir = get_attachments_dir()?;
    let canonical_dir = dir
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize attachments directory: {}", e))?;

    let path = PathBuf::from(trimmed);
    let target = if path.exists() {
        path.canonicalize()
            .map_err(|e| format!("Invalid attachment path: {}", e))?
    } else {
        // If file does not exist (e.g. for delete), verify parent
        let parent = path
            .parent()
            .ok_or_else(|| "Invalid attachment path: no parent".to_string())?;
        if parent.exists() {
            let canonical_parent = parent
                .canonicalize()
                .map_err(|e| format!("Invalid parent directory: {}", e))?;
            canonical_parent.join(
                path.file_name()
                    .ok_or_else(|| "Invalid file name".to_string())?,
            )
        } else {
            return Err("Attachment path directory does not exist.".to_string());
        }
    };

    let dir_clean = clean_path_str(&canonical_dir);
    let target_clean = clean_path_str(&target);

    let is_inside = if target_clean.starts_with(&dir_clean) {
        let rest = &target_clean[dir_clean.len()..];
        rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\')
    } else {
        false
    };

    if !is_inside {
        return Err("Attachment path must stay within desktop-managed storage.".to_string());
    }

    Ok(target)
}

#[tauri::command]
pub fn write_attachment_base64(
    attachment_id: String,
    base64: String,
    extension: Option<String>,
) -> Result<AttachmentFileResult, String> {
    let raw_b64 = base64.trim();
    if raw_b64.is_empty() {
        return Err("Attachment base64 payload is required.".to_string());
    }

    let bytes = BASE64_STANDARD
        .decode(raw_b64)
        .map_err(|e| format!("Invalid base64 payload: {}", e))?;

    let target_path = build_managed_attachment_path(&attachment_id, extension.as_deref())?;
    fs::write(&target_path, &bytes)
        .map_err(|e| format!("Failed to write attachment file: {}", e))?;

    let byte_size = fs::metadata(&target_path)
        .map(|m| m.len())
        .unwrap_or(bytes.len() as u64);

    Ok(AttachmentFileResult {
        path: clean_path_str(&target_path),
        byte_size,
    })
}

#[tauri::command]
pub fn write_attachment_bytes(
    attachment_id: String,
    bytes: Vec<u8>,
    extension: Option<String>,
) -> Result<AttachmentFileResult, String> {
    let target_path = build_managed_attachment_path(&attachment_id, extension.as_deref())?;
    fs::write(&target_path, &bytes)
        .map_err(|e| format!("Failed to write attachment file: {}", e))?;

    let byte_size = fs::metadata(&target_path)
        .map(|m| m.len())
        .unwrap_or(bytes.len() as u64);

    Ok(AttachmentFileResult {
        path: clean_path_str(&target_path),
        byte_size,
    })
}

#[tauri::command]
pub fn copy_attachment_file(
    attachment_id: String,
    source_path: String,
    extension: Option<String>,
) -> Result<AttachmentFileResult, String> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        return Err("Attachment source path is required.".to_string());
    }
    let src = PathBuf::from(trimmed);
    if !src.exists() {
        return Err(format!(
            "Source attachment file does not exist: {}",
            trimmed
        ));
    }

    let target_path = build_managed_attachment_path(&attachment_id, extension.as_deref())?;

    if src != target_path {
        fs::copy(&src, &target_path)
            .map_err(|e| format!("Failed to copy attachment file: {}", e))?;
    }

    let byte_size = fs::metadata(&target_path)
        .map(|m| m.len())
        .map_err(|e| format!("Failed to read attachment file metadata: {}", e))?;

    Ok(AttachmentFileResult {
        path: clean_path_str(&target_path),
        byte_size,
    })
}

#[tauri::command]
pub fn read_file_base64(path: String) -> Result<String, String> {
    let resolved = resolve_managed_attachment_path(&path)?;
    let bytes =
        fs::read(&resolved).map_err(|e| format!("Failed to read attachment file: {}", e))?;
    Ok(BASE64_STANDARD.encode(&bytes))
}

#[tauri::command]
pub fn delete_attachment_file(path: String) -> Result<bool, String> {
    let resolved = resolve_managed_attachment_path(&path)?;
    if resolved.exists() {
        let _ = fs::remove_file(&resolved);
    }
    Ok(true)
}

#[tauri::command]
pub fn garbage_collect_attachment_files(referenced_ids: Vec<String>) -> Result<usize, String> {
    let dir = get_attachments_dir()?;
    let valid_refs: HashSet<String> = referenced_ids
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| validate_attachment_id(s).is_ok())
        .collect();

    let mut deleted_count = 0;
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !valid_refs.contains(stem) && fs::remove_file(&path).is_ok() {
                        deleted_count += 1;
                    }
                }
            }
        }
    }

    Ok(deleted_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_attachment_id() {
        assert!(validate_attachment_id("att_12345-abc").is_ok());
        assert!(validate_attachment_id("").is_err());
        assert!(validate_attachment_id("bad/id").is_err());
        assert!(validate_attachment_id("../escaped").is_err());
    }

    #[test]
    fn test_normalize_extension() {
        assert_eq!(normalize_extension(None).unwrap(), ".bin");
        assert_eq!(normalize_extension(Some("")).unwrap(), ".bin");
        assert_eq!(normalize_extension(Some("png")).unwrap(), ".png");
        assert_eq!(normalize_extension(Some(".JPEG")).unwrap(), ".jpeg");
        assert!(normalize_extension(Some("waytoolongextensionnamethatexceeds16chars")).is_err());
        assert!(normalize_extension(Some(".bad/ext")).is_err());
    }

    #[test]
    fn test_write_and_read_attachment_base64() {
        let id = format!(
            "test_att_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let payload = "SGVsbG8gS3Jvbm9z"; // "Hello Kronos"
        let res = write_attachment_base64(id.clone(), payload.to_string(), Some("txt".to_string()))
            .expect("write failed");
        assert_eq!(res.byte_size, 12);
        assert!(res.path.ends_with(".txt"));

        let read_back = read_file_base64(res.path.clone()).expect("read failed");
        assert_eq!(read_back, payload);

        let deleted = delete_attachment_file(res.path.clone()).expect("delete failed");
        assert!(deleted);
    }

    #[test]
    fn test_path_traversal_refusal() {
        let bad_path = if cfg!(windows) {
            "C:\\Windows\\System32\\drivers\\etc\\hosts"
        } else {
            "/etc/passwd"
        };
        let res = read_file_base64(bad_path.to_string());
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("desktop-managed storage"));
    }
}
