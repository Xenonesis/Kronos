use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

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
        Ok(map.remove(&norm))
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
