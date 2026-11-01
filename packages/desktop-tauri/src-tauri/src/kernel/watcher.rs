use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use notify::event::{EventKind, ModifyKind, RenameMode};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::kernel::search::WorkspaceSearchIndex;
use crate::kernel::vfs::ShadowVfs;

static NEXT_SUBSCRIPTION_ID: AtomicU64 = AtomicU64::new(1);

/// Represents the classification of a filesystem mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileChangeKind {
    Create,
    Modify,
    Delete,
    Rename,
}

/// Represents a single debounced and normalized filesystem change event.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileChangeEvent {
    pub path: String,
    pub kind: FileChangeKind,
}

/// Prioritizes event kinds according to: `Delete` > `Create` > `Rename` > `Modify`.
fn prioritize_kind(a: FileChangeKind, b: FileChangeKind) -> FileChangeKind {
    use FileChangeKind::*;
    match (a, b) {
        (Delete, _) | (_, Delete) => Delete,
        (Create, _) | (_, Create) => Create,
        (Rename, _) | (_, Rename) => Rename,
        (Modify, Modify) => Modify,
    }
}

/// Resolves `.` and `..` components lexically without requiring disk access.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => {
                normalized.push(prefix.as_os_str());
            }
            _ => normalized.push(comp.as_os_str()),
        }
    }
    normalized
}

/// Converts a path to a normalized cross-platform string representation with `/` separators
/// and strips Windows extended-length path prefix (`\\?\`).
pub fn normalize_path_str(path: impl AsRef<Path>) -> String {
    let path_ref = path.as_ref();
    let norm = normalize_path(path_ref);
    let s = norm.to_string_lossy();
    let trimmed = if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped
    } else {
        &s
    };
    trimmed.replace('\\', "/")
}

/// Strips Windows extended-length prefix (`\\?\`) and returns a clean PathBuf.
fn clean_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let trimmed = s.strip_prefix(r"\\?\").unwrap_or(&s);
    PathBuf::from(trimmed)
}

/// Derives the relative path of `path` from `root`, handling Windows case-insensitivity.
fn relative_to_root(path: &Path, root: &Path) -> PathBuf {
    let clean_p = clean_path(path);
    let clean_r = clean_path(root);

    if let Ok(rel) = clean_p.strip_prefix(&clean_r) {
        return rel.to_path_buf();
    }

    #[cfg(windows)]
    {
        let p_str = clean_p.to_string_lossy().replace('\\', "/");
        let r_str = clean_r.to_string_lossy().replace('\\', "/");
        let p_lower = p_str.to_ascii_lowercase();
        let mut r_lower = r_str.to_ascii_lowercase();
        if !r_lower.ends_with('/') {
            r_lower.push('/');
        }
        if p_lower.starts_with(&r_lower) {
            return PathBuf::from(&p_str[r_lower.len()..]);
        }
    }

    clean_p
}

/// Tests whether a relative path matches a specific pattern.
fn matches_pattern(path: &Path, pattern: &str) -> bool {
    let p = pattern.trim();
    if p.is_empty() {
        return false;
    }
    let clean_pat = p.trim_matches('/').replace('\\', "/");

    // 1. Path component check (case-insensitive for cross-platform robustness)
    for comp in path.components() {
        let comp_str = comp.as_os_str().to_string_lossy();
        if comp_str.eq_ignore_ascii_case(&clean_pat) {
            return true;
        }
    }

    let path_str = path.to_string_lossy().replace('\\', "/");
    let path_lower = path_str.to_ascii_lowercase();
    let pat_lower = clean_pat.to_ascii_lowercase();

    // 2. Exact match or folder prefix/segment match
    if path_lower == pat_lower
        || path_lower.starts_with(&format!("{pat_lower}/"))
        || path_lower.contains(&format!("/{pat_lower}/"))
        || path_lower.ends_with(&format!("/{pat_lower}"))
    {
        return true;
    }

    // 3. Glob wildcard suffix match (e.g. "*.log" or "*.tmp")
    if pat_lower.starts_with('*') {
        let suffix = &pat_lower[1..];
        if path_lower.ends_with(suffix) {
            return true;
        }
    }

    false
}

/// Determines whether a path should be ignored according to default and custom patterns.
fn is_path_ignored(path: &Path, root: &Path, custom_patterns: &[String]) -> bool {
    let rel_path = relative_to_root(path, root);

    // Default ignore rules for development workspaces
    const DEFAULT_IGNORES: &[&str] = &[".git", "node_modules", "target"];
    for default_pat in DEFAULT_IGNORES {
        if matches_pattern(&rel_path, default_pat) {
            return true;
        }
    }

    for custom_pat in custom_patterns {
        if matches_pattern(&rel_path, custom_pat) {
            return true;
        }
    }

    false
}

/// Attempts to read a file up to 1MB into a UTF-8 String, retrying on transient Windows sharing violations.
fn try_read_text_file(path: &Path, max_retries: usize) -> Option<String> {
    for i in 0..=max_retries {
        match std::fs::metadata(path) {
            Ok(meta) => {
                if !meta.is_file() || meta.len() >= 1_048_576 {
                    return None;
                }
                match std::fs::read(path) {
                    Ok(bytes) => {
                        if bytes.len() >= 1_048_576 {
                            return None;
                        }
                        return String::from_utf8(bytes).ok();
                    }
                    Err(e)
                        if i < max_retries
                            && (e.kind() == std::io::ErrorKind::PermissionDenied
                                || e.kind() == std::io::ErrorKind::WouldBlock) =>
                    {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => return None,
                }
            }
            Err(e)
                if i < max_retries
                    && (e.kind() == std::io::ErrorKind::PermissionDenied
                        || e.kind() == std::io::ErrorKind::WouldBlock) =>
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
    None
}

/// Internal active subscription record.
struct ActiveSubscription {
    watcher: RecommendedWatcher,
    root: PathBuf,
    ignore_patterns: Vec<String>,
}

/// Message payload passed from OS watcher callback to the background coalescer.
struct RawWatcherMessage {
    sub_id: String,
    event: notify::Event,
}

/// High-performance native filesystem watcher engine.
/// Coalesces raw OS events with sub-millisecond debouncing and broadcasts normalized batches.
pub struct WatcherEngine {
    subscriptions: Arc<Mutex<HashMap<String, ActiveSubscription>>>,
    raw_tx: std::sync::mpsc::Sender<RawWatcherMessage>,
    event_tx: tokio::sync::broadcast::Sender<Vec<FileChangeEvent>>,
    debounce_ms: u64,
    shutdown: Arc<AtomicBool>,
    search_index: Arc<RwLock<Option<Arc<WorkspaceSearchIndex>>>>,
    shadow_vfs: Arc<RwLock<Option<Arc<ShadowVfs>>>>,
}

impl WatcherEngine {
    /// Creates a new `WatcherEngine` with the specified debounce sliding window in milliseconds.
    pub fn new(debounce_ms: u64) -> Self {
        let (raw_tx, raw_rx) = std::sync::mpsc::channel();
        let (sync_tx, sync_rx) = std::sync::mpsc::channel();
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        let subscriptions = Arc::new(Mutex::new(HashMap::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let search_index = Arc::new(RwLock::new(None));
        let shadow_vfs = Arc::new(RwLock::new(None));

        let thread_subs = Arc::clone(&subscriptions);
        let thread_event_tx = event_tx.clone();
        let thread_shutdown = Arc::clone(&shutdown);

        std::thread::Builder::new()
            .name("kronos-watcher-coalescer".to_string())
            .spawn(move || {
                Self::coalescer_loop(
                    raw_rx,
                    thread_subs,
                    thread_event_tx,
                    sync_tx,
                    thread_shutdown,
                    debounce_ms,
                );
            })
            .expect("Failed to spawn watcher coalescer thread");

        let sync_subs = Arc::clone(&subscriptions);
        let sync_search_index = Arc::clone(&search_index);
        let sync_shadow_vfs = Arc::clone(&shadow_vfs);
        let sync_shutdown = Arc::clone(&shutdown);

        std::thread::Builder::new()
            .name("kronos-watcher-sync".to_string())
            .spawn(move || {
                Self::sync_worker_loop(
                    sync_rx,
                    sync_subs,
                    sync_search_index,
                    sync_shadow_vfs,
                    sync_shutdown,
                );
            })
            .expect("Failed to spawn watcher sync worker thread");

        Self {
            subscriptions,
            raw_tx,
            event_tx,
            debounce_ms,
            shutdown,
            search_index,
            shadow_vfs,
        }
    }

    /// Binds an in-memory Tantivy search index for automatic incremental updates.
    pub fn bind_search_index(&self, index: Arc<WorkspaceSearchIndex>) {
        if let Ok(mut lock) = self.search_index.write() {
            *lock = Some(index);
        }
    }

    /// Binds an in-memory Shadow VFS for automatic virtual file invalidation on deletion.
    pub fn bind_shadow_vfs(&self, vfs: Arc<ShadowVfs>) {
        if let Ok(mut lock) = self.shadow_vfs.write() {
            *lock = Some(vfs);
        }
    }

    /// Returns the configured debounce window in milliseconds.
    pub fn debounce_ms(&self) -> u64 {
        self.debounce_ms
    }

    /// Subscribes to the broadcast stream of coalesced file change event batches.
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<Vec<FileChangeEvent>> {
        self.event_tx.subscribe()
    }

    /// Watches a filesystem path recursively with the given custom ignore patterns.
    /// Returns a unique subscription ID on success.
    pub fn watch(&self, path: impl AsRef<Path>, ignore_patterns: &[String]) -> Result<String, String> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Err(format!("Path does not exist: {}", path_ref.display()));
        }

        let sub_id = format!("sub-{}", NEXT_SUBSCRIPTION_ID.fetch_add(1, Ordering::SeqCst));
        let raw_tx = self.raw_tx.clone();
        let sub_id_cb = sub_id.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let _ = raw_tx.send(RawWatcherMessage {
                        sub_id: sub_id_cb.clone(),
                        event,
                    });
                }
            },
            Config::default(),
        )
        .map_err(|e| format!("Failed to create watcher: {e}"))?;

        watcher
            .watch(path_ref, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch path {}: {e}", path_ref.display()))?;

        let mut subs = self
            .subscriptions
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        subs.insert(
            sub_id.clone(),
            ActiveSubscription {
                watcher,
                root: path_ref.to_path_buf(),
                ignore_patterns: ignore_patterns.to_vec(),
            },
        );

        Ok(sub_id)
    }

    /// Unwatches a subscription by its subscription ID.
    /// Returns `Ok(true)` if removed, `Ok(false)` if not found.
    pub fn unwatch(&self, subscription_id: &str) -> Result<bool, String> {
        let mut subs = self
            .subscriptions
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        if let Some(mut sub) = subs.remove(subscription_id) {
            let _ = sub.watcher.unwatch(&sub.root);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Checks if a subscription ID is currently active.
    pub fn is_watching(&self, subscription_id: &str) -> bool {
        match self.subscriptions.lock() {
            Ok(subs) => subs.contains_key(subscription_id),
            Err(_) => false,
        }
    }

    /// Background coalescer worker loop with sliding window debouncing.
    fn coalescer_loop(
        rx: std::sync::mpsc::Receiver<RawWatcherMessage>,
        subscriptions: Arc<Mutex<HashMap<String, ActiveSubscription>>>,
        broadcast_tx: tokio::sync::broadcast::Sender<Vec<FileChangeEvent>>,
        sync_tx: std::sync::mpsc::Sender<Vec<FileChangeEvent>>,
        shutdown: Arc<AtomicBool>,
        debounce_ms: u64,
    ) {
        let debounce_dur = Duration::from_millis(debounce_ms.max(1));
        let max_delay = debounce_dur.saturating_mul(20).max(Duration::from_millis(500));

        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            // Block waiting for the first event in a burst
            let first_msg = match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(msg) => msg,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };

            let burst_start = Instant::now();
            let mut deadline = burst_start + debounce_dur;
            let max_deadline = burst_start + max_delay;

            let mut batch = vec![first_msg];

            // Sliding window collection
            loop {
                let now = Instant::now();
                if now >= max_deadline || shutdown.load(Ordering::Relaxed) {
                    break;
                }
                let timeout = deadline
                    .saturating_duration_since(now)
                    .min(max_deadline.saturating_duration_since(now));

                match rx.recv_timeout(timeout) {
                    Ok(next_msg) => {
                        batch.push(next_msg);
                        // Slide the debounce window
                        deadline = Instant::now() + debounce_dur;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }

            // Snapshot active subscriptions
            let subs_snapshot: HashMap<String, (PathBuf, Vec<String>)> = match subscriptions.lock() {
                Ok(subs) => subs
                    .iter()
                    .map(|(k, v)| (k.clone(), (v.root.clone(), v.ignore_patterns.clone())))
                    .collect(),
                Err(_) => break,
            };

            let mut path_order: Vec<String> = Vec::new();
            let mut coalesced_map: HashMap<String, FileChangeKind> = HashMap::new();

            for msg in batch {
                let Some((root, ignore_patterns)) = subs_snapshot.get(&msg.sub_id) else {
                    continue;
                };

                let norm_root_str = normalize_path_str(root);
                let raw_changes = Self::extract_raw_changes(msg.event);

                for (path, kind) in raw_changes {
                    let norm_path_str = normalize_path_str(&path);

                    // Skip events on the root folder itself
                    if norm_path_str == norm_root_str {
                        continue;
                    }

                    // Check ignore rules
                    if is_path_ignored(&path, root, ignore_patterns) {
                        continue;
                    }

                    if let Some(existing) = coalesced_map.get_mut(&norm_path_str) {
                        *existing = prioritize_kind(*existing, kind);
                    } else {
                        path_order.push(norm_path_str.clone());
                        coalesced_map.insert(norm_path_str, kind);
                    }
                }
            }

            if !path_order.is_empty() {
                let events: Vec<FileChangeEvent> = path_order
                    .into_iter()
                    .filter_map(|p| {
                        coalesced_map.remove(&p).map(|kind| FileChangeEvent {
                            path: p,
                            kind,
                        })
                    })
                    .collect();

                if !events.is_empty() {
                    let _ = broadcast_tx.send(events.clone());
                    let _ = sync_tx.send(events);
                }
            }
        }
    }

    /// Background worker loop processing debounced events for search index sync and shadow VFS invalidation.
    fn sync_worker_loop(
        rx: std::sync::mpsc::Receiver<Vec<FileChangeEvent>>,
        subscriptions: Arc<Mutex<HashMap<String, ActiveSubscription>>>,
        search_index: Arc<RwLock<Option<Arc<WorkspaceSearchIndex>>>>,
        shadow_vfs: Arc<RwLock<Option<Arc<ShadowVfs>>>>,
        shutdown: Arc<AtomicBool>,
    ) {
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            let events = match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(events) => events,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };

            let current_index = search_index.read().ok().and_then(|guard| guard.clone());
            let current_vfs = shadow_vfs.read().ok().and_then(|guard| guard.clone());

            if current_index.is_none() && current_vfs.is_none() {
                continue;
            }

            let roots: Vec<PathBuf> = match subscriptions.lock() {
                Ok(subs) => subs.values().map(|s| s.root.clone()).collect(),
                Err(_) => Vec::new(),
            };

            for event in events {
                match event.kind {
                    FileChangeKind::Create | FileChangeKind::Modify => {
                        if let Some(index) = &current_index {
                            let path = Path::new(&event.path);
                            if let Some(content) = try_read_text_file(path, 3) {
                                let _ = index.update_document(&event.path, &content);
                            }
                        }
                    }
                    FileChangeKind::Delete => {
                        if let Some(index) = &current_index {
                            let _ = index.delete_document(&event.path);
                            let path = Path::new(&event.path);
                            for root in &roots {
                                let rel = relative_to_root(path, root);
                                if rel != path {
                                    let rel_str = normalize_path_str(&rel);
                                    let _ = index.delete_document(&rel_str);
                                }
                            }
                        }
                        if let Some(vfs) = &current_vfs {
                            let _ = vfs.remove_virtual(&event.path);
                            let path = Path::new(&event.path);
                            let _ = vfs.remove_virtual(path);
                            for root in &roots {
                                let rel = relative_to_root(path, root);
                                if rel != path {
                                    let _ = vfs.remove_virtual(&rel);
                                    let _ = vfs.remove_virtual(normalize_path_str(&rel));
                                }
                            }
                        }
                    }
                    FileChangeKind::Rename => {
                        let path = Path::new(&event.path);
                        if path.exists() {
                            if let Some(index) = &current_index {
                                if let Some(content) = try_read_text_file(path, 3) {
                                    let _ = index.update_document(&event.path, &content);
                                }
                            }
                        } else {
                            if let Some(index) = &current_index {
                                let _ = index.delete_document(&event.path);
                                for root in &roots {
                                    let rel = relative_to_root(path, root);
                                    if rel != path {
                                        let rel_str = normalize_path_str(&rel);
                                        let _ = index.delete_document(&rel_str);
                                    }
                                }
                            }
                            if let Some(vfs) = &current_vfs {
                                let _ = vfs.remove_virtual(&event.path);
                                let _ = vfs.remove_virtual(path);
                                for root in &roots {
                                    let rel = relative_to_root(path, root);
                                    if rel != path {
                                        let _ = vfs.remove_virtual(&rel);
                                        let _ = vfs.remove_virtual(normalize_path_str(&rel));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Maps `notify::Event` into explicit `(PathBuf, FileChangeKind)` pairs.
    fn extract_raw_changes(event: notify::Event) -> Vec<(PathBuf, FileChangeKind)> {
        let mut changes = Vec::new();
        match event.kind {
            EventKind::Create(_) => {
                for p in event.paths {
                    changes.push((p, FileChangeKind::Create));
                }
            }
            EventKind::Remove(_) => {
                for p in event.paths {
                    changes.push((p, FileChangeKind::Delete));
                }
            }
            EventKind::Modify(ModifyKind::Name(mode)) => match mode {
                RenameMode::From => {
                    for p in event.paths {
                        changes.push((p, FileChangeKind::Delete));
                    }
                }
                RenameMode::To => {
                    for p in event.paths {
                        changes.push((p, FileChangeKind::Create));
                    }
                }
                _ => {
                    if event.paths.len() >= 2 {
                        changes.push((event.paths[0].clone(), FileChangeKind::Delete));
                        changes.push((event.paths[1].clone(), FileChangeKind::Create));
                    } else {
                        for p in event.paths {
                            changes.push((p, FileChangeKind::Rename));
                        }
                    }
                }
            },
            EventKind::Modify(_) => {
                for p in event.paths {
                    changes.push((p, FileChangeKind::Modify));
                }
            }
            EventKind::Any | EventKind::Other => {
                for p in event.paths {
                    changes.push((p, FileChangeKind::Modify));
                }
            }
            EventKind::Access(_) => {}
        }
        changes
    }
}

impl Drop for WatcherEngine {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}
