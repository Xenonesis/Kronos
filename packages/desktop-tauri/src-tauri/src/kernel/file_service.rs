use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileExplorerEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
    pub modified_ms: u64,
}

/// Lists immediate children of a directory, returning structured file explorer entries.
pub fn list_directory(cwd: &Path, subpath: Option<&str>) -> std::io::Result<Vec<FileExplorerEntry>> {
    let target_dir = if let Some(sub) = subpath {
        if sub.trim().is_empty() {
            cwd.to_path_buf()
        } else {
            cwd.join(sub)
        }
    } else {
        cwd.to_path_buf()
    };

    if !target_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", target_dir.display()),
        ));
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(&target_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata()?;
        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let rel_path = match entry.path().strip_prefix(cwd) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => name.clone(),
        };

        entries.push(FileExplorerEntry {
            name,
            path: rel_path,
            is_directory: is_dir,
            size,
            modified_ms,
        });
    }

    // Sort: directories first, then alphabetical
    entries.sort_by(|a, b| {
        match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(entries)
}

/// Writes bytes to a relative file path inside cwd, creating directories if needed.
pub fn write_file(cwd: &Path, rel_path: &str, content: &[u8]) -> std::io::Result<()> {
    let target = cwd.join(rel_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target, content)
}

/// Creates a new empty file or directory.
pub fn create_entry(cwd: &Path, parent: &str, name: &str, is_dir: bool) -> std::io::Result<String> {
    let parent_dir = if parent.is_empty() {
        cwd.to_path_buf()
    } else {
        cwd.join(parent)
    };

    fs::create_dir_all(&parent_dir)?;
    let target = parent_dir.join(name);

    if is_dir {
        fs::create_dir_all(&target)?;
    } else {
        if !target.exists() {
            fs::write(&target, b"")?;
        }
    }

    let rel_path = target
        .strip_prefix(cwd)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| name.to_string());

    Ok(rel_path)
}

/// Renames a file or directory within cwd.
pub fn rename_entry(cwd: &Path, rel_path: &str, new_name: &str) -> std::io::Result<String> {
    let source = cwd.join(rel_path);
    if !source.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path not found: {}", source.display()),
        ));
    }

    let parent = source.parent().unwrap_or(cwd);
    let target = parent.join(new_name);

    fs::rename(&source, &target)?;

    let rel_new = target
        .strip_prefix(cwd)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| new_name.to_string());

    Ok(rel_new)
}

/// Duplicates a file by copying it with a `.copy` suffix.
pub fn duplicate_entry(cwd: &Path, rel_path: &str) -> std::io::Result<String> {
    let source = cwd.join(rel_path);
    if !source.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path not found: {}", source.display()),
        ));
    }

    let parent = source.parent().unwrap_or(cwd);
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "copy".to_string());
    let ext = source
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    let mut copy_name = format!("{stem}.copy{ext}");
    let mut target = parent.join(&copy_name);
    let mut counter = 2;

    while target.exists() {
        copy_name = format!("{stem}.copy{counter}{ext}");
        target = parent.join(&copy_name);
        counter += 1;
    }

    if source.is_dir() {
        copy_dir_recursive(&source, &target)?;
    } else {
        fs::copy(&source, &target)?;
    }

    let rel_new = target
        .strip_prefix(cwd)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| copy_name);

    Ok(rel_new)
}

/// Deletes a file or directory recursively.
pub fn delete_entry(cwd: &Path, rel_path: &str) -> std::io::Result<()> {
    let target = cwd.join(rel_path);
    if !target.exists() {
        return Ok(()); // idempotent deletion
    }

    if target.is_dir() {
        fs::remove_dir_all(target)
    } else {
        fs::remove_file(target)
    }
}

/// Recursively copies a directory to a destination.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Chunked temporary upload manager.
pub struct FileUploadManager;

impl FileUploadManager {
    pub fn sanitize_file_name(name: &str) -> String {
        let raw = Path::new(name)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "upload".to_string());

        let clean: String = raw
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        if clean.is_empty() || clean == "." || clean == ".." {
            "upload".to_string()
        } else {
            clean
        }
    }

    pub fn start_upload(
        home: &Path,
        upload_id: &str,
        file_name: &str,
    ) -> std::io::Result<PathBuf> {
        let clean_name = Self::sanitize_file_name(file_name);
        let upload_dir = home.join("temp-uploads").join(upload_id);
        fs::create_dir_all(&upload_dir)?;
        let file_path = upload_dir.join(clean_name);
        fs::write(&file_path, b"")?;
        Ok(file_path)
    }

    pub fn append_chunk(file_path: &Path, chunk: &[u8]) -> std::io::Result<u64> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new().append(true).open(file_path)?;
        file.write_all(chunk)?;
        Ok(file.metadata()?.len())
    }

    pub fn delete_upload(home: &Path, upload_id: &str) -> std::io::Result<()> {
        let upload_dir = home.join("temp-uploads").join(upload_id);
        if upload_dir.exists() {
            fs::remove_dir_all(upload_dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_file_explorer_listing_and_sorting() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir(root.join("z_dir")).unwrap();
        fs::create_dir(root.join("a_dir")).unwrap();
        fs::write(root.join("b_file.txt"), b"12345").unwrap();
        fs::write(root.join("a_file.txt"), b"1").unwrap();

        let entries = list_directory(root, None).expect("List directory");
        assert_eq!(entries.len(), 4);

        // Directories first, alphabetical
        assert_eq!(entries[0].name, "a_dir");
        assert!(entries[0].is_directory);
        assert_eq!(entries[1].name, "z_dir");
        assert!(entries[1].is_directory);

        // Files second, alphabetical
        assert_eq!(entries[2].name, "a_file.txt");
        assert!(!entries[2].is_directory);
        assert_eq!(entries[2].size, 1);
        assert_eq!(entries[3].name, "b_file.txt");
        assert_eq!(entries[3].size, 5);
    }

    #[test]
    fn test_create_rename_duplicate_delete_lifecycle() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // 1. Create file
        let rel_file = create_entry(root, "", "test.ts", false).expect("Create file");
        assert_eq!(rel_file, "test.ts");
        assert!(root.join("test.ts").exists());

        // 2. Write content
        write_file(root, "test.ts", b"console.log('hello');").expect("Write file");
        assert_eq!(fs::read(root.join("test.ts")).unwrap(), b"console.log('hello');");

        // 3. Rename
        let renamed = rename_entry(root, "test.ts", "renamed.ts").expect("Rename file");
        assert_eq!(renamed, "renamed.ts");
        assert!(!root.join("test.ts").exists());
        assert!(root.join("renamed.ts").exists());

        // 4. Duplicate
        let dup = duplicate_entry(root, "renamed.ts").expect("Duplicate file");
        assert_eq!(dup, "renamed.copy.ts");
        assert!(root.join("renamed.copy.ts").exists());
        assert_eq!(fs::read(root.join("renamed.copy.ts")).unwrap(), b"console.log('hello');");

        // 5. Delete
        delete_entry(root, "renamed.ts").expect("Delete file");
        assert!(!root.join("renamed.ts").exists());
    }

    #[test]
    fn test_chunked_upload_manager() {
        let dir = tempdir().unwrap();
        let home = dir.path();

        let upload_id = "upl_test123";
        let target_file = FileUploadManager::start_upload(home, upload_id, "my data.json")
            .expect("Start upload");

        assert_eq!(target_file.file_name().unwrap(), "my_data.json");

        let size1 = FileUploadManager::append_chunk(&target_file, b"chunk-1,").expect("Append 1");
        assert_eq!(size1, 8);

        let size2 = FileUploadManager::append_chunk(&target_file, b"chunk-2").expect("Append 2");
        assert_eq!(size2, 15);

        assert_eq!(fs::read(&target_file).unwrap(), b"chunk-1,chunk-2");

        FileUploadManager::delete_upload(home, upload_id).expect("Delete upload");
        assert!(!target_file.exists());
    }
}
