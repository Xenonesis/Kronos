use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::native_diff::{compute_file_diff_sync, DiffHunk};

/// Represents diff information between physical disk file and in-memory virtual overlay.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualFileDiff {
    pub path: String,
    pub is_virtual: bool,
    pub has_changes: bool,
    pub additions: usize,
    pub deletions: usize,
    pub diff: String,
    pub hunks: Vec<DiffHunk>,
}

/// Normalizes a path by resolving `.` and `..` components lexically
/// without requiring the path to exist on disk.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(comp.as_os_str()),
        }
    }
    normalized
}

/// Thread-safe in-memory virtual filesystem for Kronos Hyper-Native Kernel.
/// Provides Copy-on-Write (CoW) shadow virtual overlays on top of physical storage.
#[derive(Debug, Clone, Default)]
pub struct ShadowVfs {
    entries: Arc<RwLock<HashMap<PathBuf, Vec<u8>>>>,
}

impl ShadowVfs {
    /// Create a new empty ShadowVfs instance.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Write in-memory bytes to a virtual path.
    pub fn write_virtual(&self, path: impl AsRef<Path>, data: &[u8]) -> std::io::Result<()> {
        let norm = normalize_path(path.as_ref());
        let mut map = self.entries.write().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("ShadowVfs lock poisoned: {e}"),
            )
        })?;
        map.insert(norm, data.to_vec());
        Ok(())
    }

    /// Read virtual bytes for a given path if it exists in the shadow overlay.
    pub fn read_virtual(&self, path: impl AsRef<Path>) -> Option<Vec<u8>> {
        let norm = normalize_path(path.as_ref());
        let map = self.entries.read().ok()?;
        map.get(&norm).cloned()
    }

    /// Check if a path currently exists in the virtual overlay.
    pub fn is_virtual(&self, path: impl AsRef<Path>) -> bool {
        let norm = normalize_path(path.as_ref());
        match self.entries.read() {
            Ok(map) => map.contains_key(&norm),
            Err(_) => false,
        }
    }

    /// Flush in-memory virtual file contents to a target physical disk path.
    /// Creates parent directories automatically if they do not exist.
    pub fn flush_to_disk(
        &self,
        path: impl AsRef<Path>,
        target_path: impl AsRef<Path>,
    ) -> std::io::Result<()> {
        let data = self.read_virtual(path.as_ref()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "Virtual file not found in ShadowVfs: {}",
                    path.as_ref().display()
                ),
            )
        })?;

        if let Some(parent) = target_path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(target_path.as_ref(), &data)?;
        Ok(())
    }

    /// Read file with Copy-on-Write semantics:
    /// Returns virtual file content if present in shadow VFS;
    /// otherwise falls back to reading from physical disk.
    pub fn read_file_cow(&self, path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
        if let Some(data) = self.read_virtual(path.as_ref()) {
            return Ok(data);
        }
        if let Ok(canonical) = std::fs::canonicalize(path.as_ref()) {
            if let Some(data) = self.read_virtual(&canonical) {
                return Ok(data);
            }
        }
        std::fs::read(path.as_ref())
    }

    /// Remove a virtual file from the shadow overlay.
    pub fn remove_virtual(&self, path: impl AsRef<Path>) -> std::io::Result<Option<Vec<u8>>> {
        let norm = normalize_path(path.as_ref());
        let mut map = self.entries.write().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("ShadowVfs lock poisoned: {e}"),
            )
        })?;
        if let Some(removed) = map.remove(&norm) {
            return Ok(Some(removed));
        }
        if let Ok(canonical) = std::fs::canonicalize(path.as_ref()) {
            let norm_c = normalize_path(&canonical);
            if let Some(removed) = map.remove(&norm_c) {
                return Ok(Some(removed));
            }
        }
        Ok(None)
    }

    /// Computes unified diff and structured hunks between physical disk version and virtual overlay.
    /// Returns None if the path is not present in the virtual overlay.
    pub fn diff_virtual_file(&self, path: impl AsRef<Path>) -> Option<VirtualFileDiff> {
        let norm = normalize_path(path.as_ref());
        let virt_bytes = self.read_virtual(&norm).or_else(|| {
            if let Ok(canonical) = std::fs::canonicalize(path.as_ref()) {
                self.read_virtual(&canonical)
            } else {
                None
            }
        })?;
        let virt_text = String::from_utf8_lossy(&virt_bytes);

        // Attempt reading physical disk version
        let disk_text = if norm.exists() && norm.is_file() {
            std::fs::read(&norm)
                .map(|b| String::from_utf8_lossy(&b).to_string())
                .unwrap_or_default()
        } else if path.as_ref().exists() && path.as_ref().is_file() {
            std::fs::read(path.as_ref())
                .map(|b| String::from_utf8_lossy(&b).to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

        let file_diff = compute_file_diff_sync(&disk_text, &virt_text, 3);
        let header_old = format!("a/{}", path.as_ref().display()).replace('\\', "/");
        let header_new = format!("b/{}", path.as_ref().display()).replace('\\', "/");
        let unified_patch = TextDiff::from_lines(disk_text.as_str(), &virt_text)
            .unified_diff()
            .context_radius(3)
            .header(&header_old, &header_new)
            .to_string();

        Some(VirtualFileDiff {
            path: path.as_ref().to_string_lossy().to_string(),
            is_virtual: true,
            has_changes: disk_text != virt_text,
            additions: file_diff.additions,
            deletions: file_diff.deletions,
            diff: unified_patch,
            hunks: file_diff.hunks,
        })
    }

    /// Computes diffs for all files currently modified in the virtual overlay.
    pub fn diff_all_virtual(&self) -> Vec<VirtualFileDiff> {
        let paths = self.list_virtual();
        let mut results = Vec::with_capacity(paths.len());
        for p in paths {
            if let Some(diff) = self.diff_virtual_file(&p) {
                results.push(diff);
            }
        }
        results
    }

    /// Returns a list of all normalized virtual paths currently in the shadow overlay.
    pub fn list_virtual(&self) -> Vec<PathBuf> {
        match self.entries.read() {
            Ok(map) => map.keys().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Returns number of virtual entries currently in the overlay.
    pub fn len(&self) -> usize {
        match self.entries.read() {
            Ok(map) => map.len(),
            Err(_) => 0,
        }
    }

    /// Returns true if the virtual overlay is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all virtual entries from the shadow overlay.
    pub fn clear(&self) -> std::io::Result<()> {
        let mut map = self.entries.write().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("ShadowVfs lock poisoned: {e}"),
            )
        })?;
        map.clear();
        Ok(())
    }
}

static GLOBAL_SHADOW_VFS: std::sync::OnceLock<Arc<ShadowVfs>> = std::sync::OnceLock::new();

/// Returns the shared process-wide Copy-on-Write ShadowVfs instance.
pub fn get_global_shadow_vfs() -> Arc<ShadowVfs> {
    GLOBAL_SHADOW_VFS
        .get_or_init(|| Arc::new(ShadowVfs::new()))
        .clone()
}
