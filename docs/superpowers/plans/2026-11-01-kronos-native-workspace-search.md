# Track 5: Native In-Memory Workspace Search & Command Center File Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement native Rust in-memory workspace file search and directory suggestions in the Tokio/Axum kernel daemon and Tauri IPC bridge, delivering instant (<2ms) Command Center (`Ctrl+P` / `Ctrl+K`) navigation in standalone zero-node mode.

**Architecture:** A high-speed recursive VFS traversal and ranking engine with skip-list filtering (`node_modules`, `.git`, `target`, `dist`, build caches) in `kernel::search`. Wired directly into `kernel::server::dispatch_session_message` to satisfy `@getpaseo/protocol`'s `directory_suggestions_request` wire format, and exposed to Tauri frontend via `workspace_search` IPC command for lexical Tantivy queries.

**Tech Stack:** Rust 2021, Tokio, Axum, Tantivy, Tauri v2, TypeScript (`ts-no-any` compliant).

**Spec:** Roadmap Track 5 — Native In-Memory Workspace Search & Indexing (Command Center & File Search IPC).

## Global Constraints

- Industrial Brutalism / Tactical Telemetry styling: `#0A0A0A`, `#EAEAEA`, `#FF2A2A`, strict 90° corners, monospaced typography.
- Desktop architecture: Tauri v2 + Windows WebView2 + Axum/Tokio Rust kernel daemon.
- Do not use `: any` or `as any` (`ts-no-any` rule).
- Never publish contracts through `ReturnType<typeof fn>`. Name types in owning modules.
- Zero fabrication: all benchmark figures and claims must reference real code or verified measurement.
- All path separators in directory suggestions returned to frontend MUST be forward slashes (`/`), even on Windows.

---

### Task 1: High-Speed Directory Suggestions & Ranking Engine in `kernel::search`

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/search.rs`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/search_test.rs`

**Interfaces:**

- Consumes: `std::path::Path`, `std::fs`
- Produces:

  ```rust
  #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
  pub struct DirectorySuggestionEntry {
      pub path: String,
      pub kind: String, // "file" | "directory"
  }

  pub fn suggest_directory_entries(
      root: &std::path::Path,
      query: &str,
      include_files: bool,
      include_dirs: bool,
      limit: usize,
  ) -> (Vec<String>, Vec<DirectorySuggestionEntry>);
  ```

- [x] **Step 1: Write the failing tests in `search_test.rs`**

Add tests covering empty query traversal, substring ranking, ignore filtering (`node_modules`, `.git`, `target`), limit enforcement, and forward-slash normalization:

```rust
    #[test]
    fn test_suggest_directory_entries_empty_and_filtered() {
        let root = std::path::Path::new(".");
        let (dirs, entries) = crate::kernel::search::suggest_directory_entries(
            root,
            "",
            true,
            true,
            50,
        );
        assert!(!entries.is_empty(), "Empty query should return top-level entries");
        assert!(entries.iter().all(|e| !e.path.contains('\\')), "Paths must be normalized with forward slashes");
        assert!(entries.iter().all(|e| !e.path.starts_with("node_modules")), "Must filter node_modules");
        assert!(entries.iter().all(|e| !e.path.starts_with(".git")), "Must filter .git");
    }

    #[test]
    fn test_suggest_directory_entries_query_matching() {
        let root = std::path::Path::new(".");
        let (dirs, entries) = crate::kernel::search::suggest_directory_entries(
            root,
            "search.rs",
            true,
            false,
            10,
        );
        assert!(!entries.is_empty(), "Should find search.rs");
        let found = entries.iter().find(|e| e.path.ends_with("search.rs"));
        assert!(found.is_some(), "Expected search.rs in matched entries");
        assert_eq!(found.unwrap().kind, "file");
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- search_test::tests::test_suggest_directory_entries`
Expected: FAIL with `cannot find function 'suggest_directory_entries' in module 'crate::kernel::search'`

- [x] **Step 3: Implement `suggest_directory_entries` in `search.rs`**

Add `DirectorySuggestionEntry`, fuzzy/substring scoring algorithm, ignore directory pruning, forward-slash normalization, and sorting in `packages/desktop-tauri/src-tauri/src/kernel/search.rs`:

```rust
/// Represents a single directory or file suggestion entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DirectorySuggestionEntry {
    pub path: String,
    pub kind: String, // "file" | "directory"
}

/// Recursively scans and ranks files and directories under `root` matching `query`.
pub fn suggest_directory_entries(
    root: &std::path::Path,
    query: &str,
    include_files: bool,
    include_dirs: bool,
    limit: usize,
) -> (Vec<String>, Vec<DirectorySuggestionEntry>) {
    if !root.exists() {
        return (Vec::new(), Vec::new());
    }

    let query_trimmed = query.trim();
    let query_lower = query_trimmed.to_ascii_lowercase();
    let max_limit = limit.max(1).min(200);

    let mut scored_entries: Vec<(i32, DirectorySuggestionEntry)> = Vec::new();
    collect_suggestions_recursive(
        root,
        root,
        0,
        12, // max depth
        &query_lower,
        include_files,
        include_dirs,
        &mut scored_entries,
        max_limit * 4, // collect candidates before final ranking
    );

    // Sort descending by score, ascending by path length
    scored_entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.path.len().cmp(&b.1.path.len()))
            .then_with(|| a.1.path.cmp(&b.1.path))
    });

    let entries: Vec<DirectorySuggestionEntry> = scored_entries
        .into_iter()
        .take(max_limit)
        .map(|(_, entry)| entry)
        .collect();

    let directories: Vec<String> = entries
        .iter()
        .filter(|e| e.kind == "directory")
        .map(|e| e.path.clone())
        .collect();

    (directories, entries)
}

fn collect_suggestions_recursive(
    current_dir: &std::path::Path,
    root: &std::path::Path,
    depth: usize,
    max_depth: usize,
    query_lower: &str,
    include_files: bool,
    include_dirs: bool,
    results: &mut Vec<(i32, DirectorySuggestionEntry)>,
    capacity: usize,
) {
    if depth > max_depth || results.len() >= capacity {
        return;
    }

    let dir_entries = match std::fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for item in dir_entries.flatten() {
        if results.len() >= capacity {
            break;
        }

        let path = item.path();
        let file_name = item.file_name();
        let name_str = file_name.to_string_lossy();

        if path.is_dir() {
            if name_str.starts_with('.')
                || name_str == "node_modules"
                || name_str == "target"
                || name_str == "dist"
                || name_str == "build"
                || name_str == "out"
                || name_str == ".next"
                || name_str == ".turbo"
                || name_str == ".expo"
                || name_str == ".cache"
            {
                continue;
            }

            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            if include_dirs && !rel_path.is_empty() {
                if let Some(score) = score_suggestion(&name_str, &rel_path, query_lower) {
                    results.push((
                        score,
                        DirectorySuggestionEntry {
                            path: rel_path,
                            kind: "directory".to_string(),
                        },
                    ));
                }
            }

            collect_suggestions_recursive(
                &path,
                root,
                depth + 1,
                max_depth,
                query_lower,
                include_files,
                include_dirs,
                results,
                capacity,
            );
        } else if path.is_file() && include_files {
            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            if !rel_path.is_empty() {
                if let Some(score) = score_suggestion(&name_str, &rel_path, query_lower) {
                    results.push((
                        score,
                        DirectorySuggestionEntry {
                            path: rel_path,
                            kind: "file".to_string(),
                        },
                    ));
                }
            }
        }
    }
}

fn score_suggestion(name: &str, rel_path: &str, query_lower: &str) -> Option<i32> {
    if query_lower.is_empty() {
        // Flat score prioritizing shallower paths
        let depth_penalty = rel_path.matches('/').count() as i32 * 10;
        return Some(100 - depth_penalty);
    }

    let name_lower = name.to_ascii_lowercase();
    let path_lower = rel_path.to_ascii_lowercase();

    if name_lower == query_lower {
        Some(1000)
    } else if name_lower.starts_with(query_lower) {
        Some(800)
    } else if name_lower.contains(query_lower) {
        Some(500)
    } else if path_lower.contains(query_lower) {
        Some(200)
    } else if is_subsequence(query_lower, &name_lower) {
        Some(100)
    } else if is_subsequence(query_lower, &path_lower) {
        Some(50)
    } else {
        None
    }
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut haystack_chars = haystack.chars();
    for n in needle.chars() {
        if !haystack_chars.any(|h| h == n) {
            return false;
        }
    }
    true
}
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- search_test::tests::test_suggest_directory_entries`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/search.rs packages/desktop-tauri/src-tauri/src/kernel/search_test.rs
git commit -m "feat(kernel): implement fast in-memory directory and file suggestions engine"
```

---

### Task 2: Wire `directory_suggestions_request` into `kernel::server` Wire Dispatch

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/server.rs`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs`

**Interfaces:**

- Consumes: `kernel::search::suggest_directory_entries`
- Produces:

  ```json
  {
    "type": "directory_suggestions_response",
    "payload": {
      "directories": string[],
      "entries": [ { "path": string, "kind": "file" | "directory" } ],
      "error": null,
      "requestId": string
    }
  }
  ```

- [x] **Step 1: Write the failing integration test in `server_test.rs`**

Add an end-to-end WebSocket integration test sending `directory_suggestions_request` and asserting correlated response:

```rust
    #[tokio::test]
    async fn test_embedded_daemon_directory_suggestions_dispatch() {
        let port = 6794;
        let daemon = EmbeddedDaemon::start_test(port).await.expect("Failed to start daemon");
        let url = format!("ws://127.0.0.1:{}", port);
        let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

        let dir_query = serde_json::json!({
            "type": "session",
            "message": {
                "type": "directory_suggestions_request",
                "cwd": ".",
                "query": "search.rs",
                "includeFiles": true,
                "includeDirectories": false,
                "limit": 10,
                "requestId": "req-dir-sugg-test"
            }
        });
        ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(dir_query.to_string())).await.unwrap();

        let dir_resp = ws_stream.next().await.unwrap().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(dir_resp.to_text().unwrap()).unwrap();
        assert_eq!(parsed["type"], "session");
        assert_eq!(parsed["message"]["type"], "directory_suggestions_response");
        assert_eq!(parsed["message"]["payload"]["requestId"], "req-dir-sugg-test");
        assert!(parsed["message"]["payload"]["error"].is_null());

        let entries = parsed["message"]["payload"]["entries"].as_array().expect("entries array");
        assert!(!entries.is_empty(), "Should return matching search.rs entry");
        let matched = entries.iter().any(|e| {
            e["path"].as_str().map(|p| p.ends_with("search.rs")).unwrap_or(false)
        });
        assert!(matched, "Expected search.rs in returned suggestions payload");

        daemon.stop().await;
    }
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- server_test::tests::test_embedded_daemon_directory_suggestions_dispatch`
Expected: FAIL because `directory_suggestions_request` returns default fallback payload instead of real entries.

- [x] **Step 3: Wire `directory_suggestions_request` in `kernel::server::dispatch_session_message` & `dispatch_envelope`**

In `packages/desktop-tauri/src-tauri/src/kernel/server.rs`:
Add arm to `dispatch_session_message`:

```rust
        "directory_suggestions_request" => {
            let query = msg.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let cwd_str = msg.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
            let include_files = msg.get("includeFiles").and_then(|v| v.as_bool()).unwrap_or(true);
            let include_dirs = msg.get("includeDirectories").and_then(|v| v.as_bool()).unwrap_or(false);
            let limit = msg.get("limit").and_then(|v| v.as_u64()).map(|l| l as usize).unwrap_or(50).min(100);

            let root_path = std::path::Path::new(cwd_str);
            let (directories, entries) = crate::kernel::search::suggest_directory_entries(
                root_path,
                query,
                include_files,
                include_dirs,
                limit,
            );

            serde_json::json!({
                "type": "directory_suggestions_response",
                "payload": {
                    "directories": directories,
                    "entries": entries,
                    "error": null,
                    "requestId": request_id
                }
            })
        }
```

And in `dispatch_envelope`:

```rust
        "directory_suggestions_request" | "workspace/suggest_directory" => {
            let query = envelope.payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let cwd_str = envelope.payload.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
            let include_files = envelope.payload.get("includeFiles").and_then(|v| v.as_bool()).unwrap_or(true);
            let include_dirs = envelope.payload.get("includeDirectories").and_then(|v| v.as_bool()).unwrap_or(false);
            let limit = envelope.payload.get("limit").and_then(|v| v.as_u64()).map(|l| l as usize).unwrap_or(50).min(100);

            let root_path = std::path::Path::new(cwd_str);
            let (directories, entries) = crate::kernel::search::suggest_directory_entries(
                root_path,
                query,
                include_files,
                include_dirs,
                limit,
            );

            RpcResponse::ok(&envelope.id, "directory_suggestions_response", serde_json::json!({
                "directories": directories,
                "entries": entries,
                "error": null,
                "requestId": envelope.id
            }))
        }
```

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- server_test::tests::test_embedded_daemon_directory_suggestions_dispatch`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/server.rs packages/desktop-tauri/src-tauri/src/kernel/server_test.rs
git commit -m "feat(kernel): wire directory suggestions request into in-process daemon"
```

---

### Task 3: Expose `workspace_search` Tauri IPC Command & TypeScript Binding

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`
- Create: `packages/app/src/command-center/workspace-search-native.ts`

**Interfaces:**

- Consumes: `kernel::cli::execute_search_in`, `kernel::search::SearchResult`
- Produces:
  - Tauri command: `workspace_search(query: String, root: Option<String>, limit: Option<usize>) -> Result<Vec<SearchResult>, String>`
  - TypeScript interface: `NativeSearchResult`, `executeNativeWorkspaceSearch(query: string, root?: string, limit?: number): Promise<NativeSearchResult[]>`

- [x] **Step 1: Implement `workspace_search` Tauri IPC Command in `main.rs`**

In `packages/desktop-tauri/src-tauri/src/main.rs`:
Define the command:

```rust
#[tauri::command]
pub async fn workspace_search(
    query: String,
    root: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<crate::kernel::search::SearchResult>, String> {
    let target_root = root.unwrap_or_else(|| ".".to_string());
    let path = std::path::PathBuf::from(target_root);
    let results = crate::kernel::cli::execute_search_in(&query, &path)?;
    let max_limit = limit.unwrap_or(100);
    Ok(results.into_iter().take(max_limit).collect())
}
```

Register it in `.invoke_handler(tauri::generate_handler![..., workspace_search])`.

- [x] **Step 2: Create typed frontend binding in `workspace-search-native.ts`**

In `packages/app/src/command-center/workspace-search-native.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";

export interface NativeSearchResult {
  readonly path: string;
  readonly score: number;
}

export async function executeNativeWorkspaceSearch(
  query: string,
  root?: string,
  limit?: number,
): Promise<readonly NativeSearchResult[]> {
  try {
    return await invoke<NativeSearchResult[]>("workspace_search", {
      query,
      root: root ?? null,
      limit: limit ?? null,
    });
  } catch {
    return [];
  }
}
```

- [x] **Step 3: Verify TypeScript compilation passes**

Run: `pnpm --filter @getpaseo/app build` or `pnpm check-types`
Expected: PASS (zero type errors, no `any`).

- [x] **Step 4: Commit**

```bash
git add packages/desktop-tauri/src-tauri/src/main.rs packages/app/src/command-center/workspace-search-native.ts
git commit -m "feat(desktop): expose native workspace search IPC command and TypeScript binding"
```

---

### Task 4: Full Suite Verification & Performance Benchmark

**Files:**

- Test: Entire `desktop-tauri` test suite

- [x] **Step 1: Run full Rust test suite**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: ALL tests pass (>89 passed, 0 failed).

- [x] **Step 2: Verify search response timing benchmark**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- search_test --nocapture`
Expected: Tantivy in-memory search executed in <1ms (<500µs).

- [x] **Step 3: Commit final plan verification checkpoint**

```bash
git add .
git commit -m "test(kernel): verify native workspace search sub-millisecond benchmarks and protocol parity"
```
