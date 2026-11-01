# Track 6: Native File Watcher & Instant VFS Stream Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a high-performance native filesystem watcher in the Rust desktop kernel (`kernel::watcher`) using `notify` and Win32 IOCP (`ReadDirectoryChangesW`), with sub-50ms debouncing, ignore-rule filtering, instant Tantivy incremental search synchronization, and real-time WebSocket & Tauri IPC event streaming.

**Architecture:** Embed a cross-platform recursive filesystem watcher in `kernel::watcher` powered by `notify` 6.1. Raw OS filesystem events are debounced and coalesced over a 25ms sliding window, filtered against ignore rules (`.git/`, `node_modules/`, `target/`, custom patterns), and dispatched concurrently to: (1) `WorkspaceSearchIndex` for real-time incremental re-indexing / term deletion without full rescans, (2) `ShadowVfs` for virtual overlay invalidation, and (3) `kernel::server` / Tauri IPC for streaming live file changes to the frontend.

**Tech Stack:** Rust 2021, Tokio, `notify` 6.1, Win32 IOCP (`ReadDirectoryChangesW`), Tantivy 0.22, Tauri v2 IPC, Axum WebSocket.

**Spec:** Roadmap Track 6 — Native File Watcher & Instant VFS Stream (`notify` / Win32 IOCP).

## Global Constraints

- **Architecture:** Rust 2021, Tokio async runtime, `notify` 6.1, Tantivy 0.22, Axum 0.8.
- **Industrial Brutalism:** Zero unnecessary abstractions, predictable memory footprint, minimal lock contention.
- **Strict Types:** No `: any` or `as any` across TypeScript packages.
- **Non-blocking:** Filesystem event processing and Tantivy document updating must run in background threads without blocking the main Tauri UI thread or WebSocket server.
- **All Rust Tests Passing:** Every task must pass with 0 warnings and 0 failures.

---

### Task 1: Core Native Watcher Engine with Debouncing & Ignore Rules (`kernel::watcher`)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml:40-46`
- Create: `packages/desktop-tauri/src-tauri/src/kernel/watcher.rs`
- Create: `packages/desktop-tauri/src-tauri/src/kernel/watcher_test.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/mod.rs:1-40`

**Interfaces:**

- Produces:

  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub enum FileChangeKind {
      Create,
      Modify,
      Delete,
      Rename,
  }

  #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct FileChangeEvent {
      pub path: String,
      pub kind: FileChangeKind,
  }

  pub struct WatcherEngine { ... }
  impl WatcherEngine {
      pub fn new(debounce_ms: u64) -> Self;
      pub fn watch(&self, path: impl AsRef<Path>, ignore_patterns: &[String]) -> Result<String, String>;
      pub fn unwatch(&self, subscription_id: &str) -> Result<bool, String>;
      pub fn is_watching(&self, subscription_id: &str) -> bool;
      pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<Vec<FileChangeEvent>>;
  }
  ```

- [ ] **Step 1: Add `notify = "6.1.1"` dependency in `Cargo.toml`**

In `packages/desktop-tauri/src-tauri/Cargo.toml`:

```toml
notify = "6.1.1"
```

- [ ] **Step 2: Write failing unit tests in `packages/desktop-tauri/src-tauri/src/kernel/watcher_test.rs`**

```rust
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;
use crate::kernel::watcher::{FileChangeKind, WatcherEngine};

#[tokio::test]
async fn test_watcher_detects_create_and_modify() {
    let dir = tempdir().expect("tempdir");
    let engine = WatcherEngine::new(25);
    let mut rx = engine.subscribe_events();

    let sub_id = engine.watch(dir.path(), &[]).expect("watch");
    assert!(engine.is_watching(&sub_id));

    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "initial content").expect("write");

    let events = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    assert!(!events.is_empty());
    assert!(events.iter().any(|e| e.kind == FileChangeKind::Create || e.kind == FileChangeKind::Modify));

    engine.unwatch(&sub_id).expect("unwatch");
    assert!(!engine.is_watching(&sub_id));
}

#[tokio::test]
async fn test_watcher_ignores_git_and_target_directories() {
    let dir = tempdir().expect("tempdir");
    let engine = WatcherEngine::new(25);
    let mut rx = engine.subscribe_events();

    let sub_id = engine.watch(dir.path(), &[".git".to_string(), "node_modules".to_string()]).expect("watch");

    let git_dir = dir.path().join(".git");
    fs::create_dir_all(&git_dir).expect("mkdir");
    fs::write(git_dir.join("config"), "core.bare=false").expect("write");

    let timeout_res = tokio::time::timeout(Duration::from_millis(150), rx.recv()).await;
    assert!(timeout_res.is_err(), "Expected no events for ignored .git directory");

    engine.unwatch(&sub_id).expect("unwatch");
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml watcher_test`
Expected: FAIL (module not found)

- [ ] **Step 4: Implement `WatcherEngine` in `packages/desktop-tauri/src-tauri/src/kernel/watcher.rs`**

Implement:

1. `FileChangeKind` and `FileChangeEvent` with serde derivation.
2. `WatcherEngine` with an internal `Arc<Mutex<HashMap<String, notify::RecommendedWatcher>>>`.
3. Background event collector thread utilizing Tokio broadcast channel (`tokio::sync::broadcast::Sender<Vec<FileChangeEvent>>`).
4. Debounce coalescer: gathers raw events across a 25ms sliding window, deduplicating events by canonical path and prioritizing `Delete` > `Create` > `Modify`.
5. Ignore rule checker: filters out paths matching `.git/`, `node_modules/`, `target/`, or custom user patterns.

- [ ] **Step 5: Register module in `packages/desktop-tauri/src-tauri/src/kernel/mod.rs`**

```rust
pub mod watcher;
#[cfg(test)]
pub mod watcher_test;
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml watcher_test`
Expected: PASS

- [ ] **Step 7: Commit Task 1**

```bash
git add packages/desktop-tauri/src-tauri/Cargo.toml packages/desktop-tauri/src-tauri/Cargo.lock packages/desktop-tauri/src-tauri/src/kernel/watcher.rs packages/desktop-tauri/src-tauri/src/kernel/watcher_test.rs packages/desktop-tauri/src-tauri/src/kernel/mod.rs
git commit -m "feat(kernel): implement native file watcher engine with debouncing and ignore rules"
```

---

### Task 2: Instant Tantivy Search Incremental Synchronization & Shadow VFS Invalidation

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/search.rs:25-120`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/search_test.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/watcher.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/watcher_test.rs`

**Interfaces:**

- Consumes: `WatcherEngine`, `WorkspaceSearchIndex`, `ShadowVfs`.
- Produces:
  - `WorkspaceSearchIndex::delete_document(&self, path: &str) -> Result<(), String>`
  - `WorkspaceSearchIndex::update_document(&self, path: &str, text: &str) -> Result<(), String>`
  - `WatcherEngine::bind_search_index(&self, index: Arc<WorkspaceSearchIndex>)`
  - `WatcherEngine::bind_shadow_vfs(&self, vfs: Arc<ShadowVfs>)`

- [ ] **Step 1: Write failing tests for Tantivy term deletion in `packages/desktop-tauri/src-tauri/src/kernel/search_test.rs`**

```rust
#[test]
fn test_tantivy_incremental_document_deletion_and_update() {
    let index = WorkspaceSearchIndex::new_in_memory().expect("create index");
    index.index_document("src/main.rs", "fn main() { println!(\"hello\"); }").expect("index");

    let res = index.search("println").expect("search");
    assert_eq!(res.len(), 1);

    // Delete document
    index.delete_document("src/main.rs").expect("delete");
    let res_after = index.search("println").expect("search");
    assert_eq!(res_after.len(), 0);

    // Update document
    index.update_document("src/main.rs", "fn main() { eprintln!(\"error\"); }").expect("update");
    let res_new = index.search("eprintln").expect("search");
    assert_eq!(res_new.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_tantivy_incremental_document_deletion`
Expected: FAIL (method not found)

- [ ] **Step 3: Implement `delete_document` and `update_document` in `kernel/search.rs`**

1. Modify `path_field` creation in `new_in_memory`:
   Ensure `path_field` uses `STRING | STORED` (or `raw` tokenizer) so `tantivy::Term::from_field_text(self.path_field, path)` matches the exact path term.
2. Implement `delete_document(&self, path: &str) -> Result<(), String>`:
   ```rust
   let term = Term::from_field_text(self.path_field, path);
   let mut writer = self.writer.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
   writer.delete_term(term);
   writer.commit().map_err(|e| format!("Failed to commit delete: {e}"))?;
   self.reader.reload().map_err(|e| format!("Failed to reload reader: {e}"))?;
   ```
3. Implement `update_document(&self, path: &str, text: &str) -> Result<(), String>`:
   Deletes existing term, inserts new document, commits, and reloads reader in one atomic batch.

- [ ] **Step 4: Bind `WatcherEngine` to `WorkspaceSearchIndex` and `ShadowVfs`**

In `kernel/watcher.rs`:
Add background event processing loop that receives debounced events:

- On `Create` / `Modify`: If file size < 1MB and UTF-8 readable, calls `index.update_document(&path_str, &content)`.
- On `Delete`: Calls `index.delete_document(&path_str)`. Invalidate `vfs.remove_virtual(&path_str)`.

- [ ] **Step 5: Write and run integration test in `kernel/watcher_test.rs`**

Verify that editing a physical file on disk automatically updates search results in the bound `WorkspaceSearchIndex` within 100ms.

- [ ] **Step 6: Run tests to verify all pass**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml search_test watcher_test`
Expected: PASS

- [ ] **Step 7: Commit Task 2**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/search.rs packages/desktop-tauri/src-tauri/src/kernel/search_test.rs packages/desktop-tauri/src-tauri/src/kernel/watcher.rs packages/desktop-tauri/src-tauri/src/kernel/watcher_test.rs
git commit -m "feat(kernel): connect file watcher to tantivy incremental search index and shadow vfs"
```

---

### Task 3: In-Process Daemon Wire Protocol Envelopes (`kernel::server`)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/server.rs:60-250`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs`

**Interfaces:**

- Consumes: `WatcherEngine`.
- Produces:
  - Envelope `"vfs/watch"`: Starts watching directory, responds with subscription ID.
  - Envelope `"vfs/unwatch"`: Stops watching directory.
  - Envelope `"vfs/events"` / broadcast event: Pushes batches of `FileChangeEvent` over WebSocket connection.

- [ ] **Step 1: Write failing integration test in `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs`**

```rust
#[tokio::test]
async fn test_daemon_vfs_watch_and_unwatch_envelope() {
    let temp = tempfile::tempdir().expect("tempdir");
    let watch_req = RpcEnvelope {
        id: "watch-1".to_string(),
        msg_type: "vfs/watch".to_string(),
        payload: serde_json::json!({
            "path": temp.path().to_str().unwrap(),
            "ignore": [".git", "target"]
        }),
    };

    let watch_res = dispatch_envelope(watch_req).await;
    assert!(watch_res.success);
    let sub_id = watch_res.data.unwrap()["subscriptionId"].as_str().unwrap().to_string();
    assert!(!sub_id.is_empty());

    let unwatch_req = RpcEnvelope {
        id: "unwatch-1".to_string(),
        msg_type: "vfs/unwatch".to_string(),
        payload: serde_json::json!({
            "subscriptionId": sub_id
        }),
    };
    let unwatch_res = dispatch_envelope(unwatch_req).await;
    assert!(unwatch_res.success);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_daemon_vfs_watch_and_unwatch_envelope`
Expected: FAIL (Method 'vfs/watch' not implemented)

- [ ] **Step 3: Wire `vfs/watch` and `vfs/unwatch` into `dispatch_envelope` and `dispatch_session_message`**

In `packages/desktop-tauri/src-tauri/src/kernel/server.rs`:

1. Initialize a lazy singleton `static WATCHER_ENGINE: OnceLock<WatcherEngine> = OnceLock::new();` with accessor `pub fn get_watcher_engine() -> &'static WatcherEngine`.
2. In `dispatch_envelope`:
   - Match `"vfs/watch" | "vfs/watch_subscribe"`: extracts `path` and `ignore`, calls `engine.watch(path, &ignore)`, responds with `subscriptionId`.
   - Match `"vfs/unwatch" | "vfs/watch_unsubscribe"`: extracts `subscriptionId`, calls `engine.unwatch(sub_id)`, responds with `{ "unwatched": true }`.
3. In `dispatch_session_message`:
   - Add matching handlers for `"vfs/watch"` and `"vfs/unwatch"`.
4. In WebSocket connection loop:
   - Forward subscribed `FileChangeEvent` batches as WebSocket frames: `{ "type": "vfs/change_events", "payload": events }`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_daemon_vfs_watch_and_unwatch_envelope`
Expected: PASS

- [ ] **Step 5: Commit Task 3**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/server.rs packages/desktop-tauri/src-tauri/src/kernel/server_test.rs
git commit -m "feat(server): wire vfs watch and unwatch wire protocol envelopes into in-process daemon"
```

---

### Task 4: Desktop Tauri IPC Commands & Native Event Dispatch (`main.rs`)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:1350-1420`
- Create: `packages/app/src/command-center/vfs-watcher-native.ts`
- Create: `packages/app/src/command-center/vfs-watcher-native.test.ts`

**Interfaces:**

- Produces:
  - Tauri command `watch_directory(path: String, ignore: Option<Vec<String>>) -> Result<String, String>`
  - Tauri command `unwatch_directory(subscription_id: String) -> Result<bool, String>`
  - TypeScript bridge: `subscribeVfsWatch(path: string, ignore?: string[]): Promise<string>`
  - TypeScript bridge: `unsubscribeVfsWatch(subscriptionId: string): Promise<boolean>`

- [ ] **Step 1: Implement `watch_directory` and `unwatch_directory` commands in `src-tauri/src/main.rs`**

```rust
#[tauri::command]
async fn watch_directory(path: String, ignore: Option<Vec<String>>) -> Result<String, String> {
    let engine = crate::kernel::server::get_watcher_engine();
    let ignore_patterns = ignore.unwrap_or_default();
    engine.watch(&path, &ignore_patterns)
}

#[tauri::command]
async fn unwatch_directory(subscription_id: String) -> Result<bool, String> {
    let engine = crate::kernel::server::get_watcher_engine();
    engine.unwatch(&subscription_id)
}
```

- [ ] **Step 2: Register commands in `tauri::generate_handler![..., watch_directory, unwatch_directory]`**

Register in `generate_handler!` in `main.rs`.

- [ ] **Step 3: Setup Tauri background event emitter**

In `tauri::Builder::setup`:
Subscribe to `get_watcher_engine().subscribe_events()`, and for every batch, invoke `app_handle.emit("vfs-file-change", events)` so the web frontend receives instant updates.

- [ ] **Step 4: Implement TypeScript wrapper in `packages/app/src/command-center/vfs-watcher-native.ts`**

Implement strictly typed `watchDirectoryNative` and `unwatchDirectoryNative` using the project's standard `window.paseoDesktop?.invoke` bridge pattern (no uninstalled `@tauri-apps/api/core` imports, strictly adhering to `ts-no-any`).

- [ ] **Step 5: Write unit tests in `packages/app/src/command-center/vfs-watcher-native.test.ts`**

Verify invocation, fallback behavior when desktop bridge is unavailable, and error handling.

- [ ] **Step 6: Run frontend tests**

Run: `npm test packages/app/src/command-center/vfs-watcher-native.test.ts`
Expected: PASS

- [ ] **Step 7: Commit Task 4**

```bash
git add packages/desktop-tauri/src-tauri/src/main.rs packages/app/src/command-center/vfs-watcher-native.ts packages/app/src/command-center/vfs-watcher-native.test.ts
git commit -m "feat(desktop): expose native vfs file watcher ipc commands and frontend bridge"
```

---

### Task 5: Full Suite Verification & Performance Smoke Testing

**Files:**

- Test: All Rust test suites across `packages/desktop-tauri/src-tauri`
- Test: Monorepo typecheck (`npm run typecheck`)

- [ ] **Step 1: Run complete Cargo test suite**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: 130+ tests passing, 0 failures.

- [ ] **Step 2: Run monorepo typecheck**

Run: `npm run typecheck`
Expected: 0 errors across all 12 TypeScript packages.

- [ ] **Step 3: Run burst write performance benchmark**

Verify in `kernel/watcher_test.rs` that 100 concurrent rapid writes in a test workspace are debounced and processed in `<50ms` without dropping events or crashing.

- [ ] **Step 4: Final verification commit**

```bash
git commit --allow-empty -m "chore(release): complete Track 6 native file watcher and instant vfs stream verification"
```
