use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use crate::kernel::search::WorkspaceSearchIndex;
use crate::kernel::vfs::ShadowVfs;
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

    let events = tokio::time::timeout(Duration::from_millis(1000), rx.recv())
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

    let timeout_res = tokio::time::timeout(Duration::from_millis(250), rx.recv()).await;
    assert!(timeout_res.is_err(), "Expected no events for ignored .git directory");

    engine.unwatch(&sub_id).expect("unwatch");
}

#[tokio::test]
async fn test_watcher_unwatch_nonexistent() {
    let engine = WatcherEngine::new(25);
    let removed = engine.unwatch("nonexistent-sub-id").expect("unwatch");
    assert!(!removed);
    assert!(!engine.is_watching("nonexistent-sub-id"));
}

#[tokio::test]
async fn test_watcher_detects_delete() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("to_delete.txt");
    fs::write(&file_path, "data").expect("write");

    let engine = WatcherEngine::new(25);
    let mut rx = engine.subscribe_events();

    let sub_id = engine.watch(dir.path(), &[]).expect("watch");

    // Wait a brief moment to ensure watcher is initialized
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Drain any leftover events from setup if any
    while rx.try_recv().is_ok() {}

    fs::remove_file(&file_path).expect("remove");

    let events = tokio::time::timeout(Duration::from_millis(1000), rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    assert!(!events.is_empty());
    assert!(events.iter().any(|e| e.kind == FileChangeKind::Delete));

    engine.unwatch(&sub_id).expect("unwatch");
}

#[tokio::test]
async fn test_watcher_coalescing_debounces_rapid_writes() {
    let dir = tempdir().expect("tempdir");
    let engine = WatcherEngine::new(50);
    let mut rx = engine.subscribe_events();

    let sub_id = engine.watch(dir.path(), &[]).expect("watch");
    let file_path = dir.path().join("coalesce.txt");

    // Rapid writes in a tight loop
    for i in 0..5 {
        fs::write(&file_path, format!("write #{i}")).expect("write");
    }

    let events = tokio::time::timeout(Duration::from_millis(1000), rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    assert!(!events.is_empty());
    // For this single file, after debouncing and coalescing within the window,
    // there should be at most 1 coalesced event in this batch for `coalesce.txt`.
    let file_events: Vec<_> = events.iter().filter(|e| e.path.ends_with("coalesce.txt")).collect();
    assert_eq!(file_events.len(), 1, "Expected exactly 1 coalesced event for coalesce.txt in batch, got {:?}", file_events);

    engine.unwatch(&sub_id).expect("unwatch");
}

#[test]
fn test_normalize_path_str_formats() {
    use std::path::Path;
    use crate::kernel::watcher::normalize_path_str;

    assert_eq!(normalize_path_str(Path::new("src/main.rs")), "src/main.rs");
    assert_eq!(normalize_path_str(Path::new("src\\kernel\\watcher.rs")), "src/kernel/watcher.rs");
    assert_eq!(normalize_path_str(Path::new("foo/./bar/../baz.txt")), "foo/baz.txt");
    assert_eq!(normalize_path_str(Path::new(r"\\?\C:\Projects\app\file.rs")), "C:/Projects/app/file.rs");
}

#[tokio::test]
async fn test_watcher_multiple_subscribers() {
    let dir = tempdir().expect("tempdir");
    let engine = WatcherEngine::new(25);
    let mut rx1 = engine.subscribe_events();
    let mut rx2 = engine.subscribe_events();

    let sub_id = engine.watch(dir.path(), &[]).expect("watch");
    let file_path = dir.path().join("broadcast_test.txt");
    fs::write(&file_path, "broadcast").expect("write");

    let events1 = tokio::time::timeout(Duration::from_millis(1000), rx1.recv())
        .await
        .expect("timeout rx1")
        .expect("recv rx1");
    let events2 = tokio::time::timeout(Duration::from_millis(1000), rx2.recv())
        .await
        .expect("timeout rx2")
        .expect("recv rx2");

    assert_eq!(events1, events2);
    engine.unwatch(&sub_id).expect("unwatch");
}

#[tokio::test]
async fn test_watcher_custom_glob_and_folder_ignore() {
    let dir = tempdir().expect("tempdir");
    let engine = WatcherEngine::new(25);
    let mut rx = engine.subscribe_events();

    let sub_id = engine
        .watch(dir.path(), &["*.log".to_string(), "build/".to_string()])
        .expect("watch");

    // Create ignored files
    let log_file = dir.path().join("app.log");
    fs::write(&log_file, "log content").expect("write log");

    let build_dir = dir.path().join("build");
    fs::create_dir_all(&build_dir).expect("create build dir");
    fs::write(build_dir.join("output.bin"), "binary").expect("write build");

    // Create valid file
    let valid_file = dir.path().join("valid.txt");
    fs::write(&valid_file, "valid content").expect("write valid");

    let events = tokio::time::timeout(Duration::from_millis(1000), rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    // Should only contain valid.txt, not app.log or build/output.bin
    assert!(!events.is_empty());
    for e in &events {
        assert!(
            !e.path.ends_with("app.log"),
            "app.log should be ignored, found: {}",
            e.path
        );
        assert!(
            !e.path.contains("/build/"),
            "build/ should be ignored, found: {}",
            e.path
        );
    }
    assert!(events.iter().any(|e| e.path.ends_with("valid.txt")));

    engine.unwatch(&sub_id).expect("unwatch");
}

#[tokio::test]
async fn test_watcher_custom_glob_wildcard_directory_ignores() {
    let dir = tempdir().expect("tempdir");
    let engine = WatcherEngine::new(25);
    let mut rx = engine.subscribe_events();

    let sub_id = engine
        .watch(
            dir.path(),
            &[
                "build/**".to_string(),
                "dist/*".to_string(),
            ],
        )
        .expect("watch");

    // Create ignored files inside build/** and dist/*
    let build_sub_dir = dir.path().join("build").join("nested");
    fs::create_dir_all(&build_sub_dir).expect("create build nested dir");
    fs::write(build_sub_dir.join("bundle.js"), "console.log('build')").expect("write build");

    let dist_dir = dir.path().join("dist");
    fs::create_dir_all(&dist_dir).expect("create dist dir");
    fs::write(dist_dir.join("output.min.js"), "minified").expect("write dist");

    // Create a normal valid file
    let valid_file = dir.path().join("src.rs");
    fs::write(&valid_file, "fn main() {}").expect("write valid");

    let events = tokio::time::timeout(Duration::from_millis(1000), rx.recv())
        .await
        .expect("timeout")
        .expect("recv");

    assert!(!events.is_empty());
    for e in &events {
        assert!(
            !e.path.contains("/build/"),
            "build/** should be ignored, found: {}",
            e.path
        );
        assert!(
            !e.path.contains("/dist/"),
            "dist/* should be ignored, found: {}",
            e.path
        );
    }
    assert!(events.iter().any(|e| e.path.ends_with("src.rs")));

    engine.unwatch(&sub_id).expect("unwatch");
}

#[tokio::test]
async fn test_watcher_deduplicates_relative_search_index_entries() {
    let dir = tempdir().expect("tempdir");
    let search_index = Arc::new(WorkspaceSearchIndex::new_in_memory().expect("search index"));
    let engine = WatcherEngine::new(20);
    engine.bind_search_index(Arc::clone(&search_index));

    // Pre-populate index with a relative path variant
    let rel_path = "nested/component.rs";
    search_index
        .update_document(rel_path, "pub fn legacy_relative_impl() {}")
        .expect("populate relative");
    assert!(!search_index.search("legacy_relative_impl").unwrap().is_empty());

    let _sub_id = engine.watch(dir.path(), &[]).expect("watch");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Now write the file on disk at the corresponding absolute path
    let file_path = dir.path().join("nested").join("component.rs");
    fs::create_dir_all(file_path.parent().unwrap()).expect("create dir");
    fs::write(&file_path, "pub fn upgraded_absolute_impl() {}").expect("write file");

    let deduplicated = wait_for_condition(
        || {
            let old_rel_found = search_index
                .search("legacy_relative_impl")
                .map(|r| r.iter().any(|doc| doc.path == rel_path))
                .unwrap_or(false);
            let new_abs_found = search_index
                .search("upgraded_absolute_impl")
                .map(|r| !r.is_empty())
                .unwrap_or(false);
            !old_rel_found && new_abs_found
        },
        1500,
    )
    .await;

    assert!(deduplicated, "Relative entry must be purged when absolute file is updated");
}

async fn wait_for_condition<F>(mut check: F, timeout_ms: u64) -> bool
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    let max = Duration::from_millis(timeout_ms);
    while start.elapsed() < max {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    check()
}

#[tokio::test]
async fn test_watcher_bind_search_index_updates_and_deletes() {
    let dir = tempdir().expect("tempdir");
    let search_index = Arc::new(WorkspaceSearchIndex::new_in_memory().expect("search index"));
    let engine = WatcherEngine::new(20);
    engine.bind_search_index(Arc::clone(&search_index));

    let _sub_id = engine.watch(dir.path(), &[]).expect("watch");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let file_path = dir.path().join("service.rs");
    fs::write(&file_path, "pub fn start_kronos_microservice() -> bool { true }").expect("write");

    let found = wait_for_condition(
        || {
            search_index
                .search("start_kronos_microservice")
                .map(|r| !r.is_empty())
                .unwrap_or(false)
        },
        1500,
    )
    .await;
    assert!(found, "Search index should contain updated content for created file");

    // Modify file
    fs::write(&file_path, "pub fn stop_kronos_microservice() -> bool { false }").expect("modify");

    let found_modified = wait_for_condition(
        || {
            let old_empty = search_index
                .search("start_kronos_microservice")
                .map(|r| r.is_empty())
                .unwrap_or(false);
            let new_found = search_index
                .search("stop_kronos_microservice")
                .map(|r| !r.is_empty())
                .unwrap_or(false);
            old_empty && new_found
        },
        1500,
    )
    .await;
    assert!(found_modified, "Search index should reflect modified content and remove old tokens");

    // Delete file
    fs::remove_file(&file_path).expect("delete");

    let deleted = wait_for_condition(
        || {
            search_index
                .search("stop_kronos_microservice")
                .map(|r| r.is_empty())
                .unwrap_or(false)
        },
        1500,
    )
    .await;
    assert!(deleted, "Search index should delete document when file is deleted");
}

#[tokio::test]
async fn test_watcher_bind_shadow_vfs_invalidation() {
    let dir = tempdir().expect("tempdir");
    let vfs = Arc::new(ShadowVfs::new());
    let engine = WatcherEngine::new(20);
    engine.bind_shadow_vfs(Arc::clone(&vfs));

    let _sub_id = engine.watch(dir.path(), &[]).expect("watch");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let file_path = dir.path().join("draft.md");
    fs::write(&file_path, "initial draft on disk").expect("write");

    // Store virtual overlay in ShadowVfs
    vfs.write_virtual(&file_path, b"shadow overlay content").expect("write virtual");
    assert!(vfs.is_virtual(&file_path));

    // When physical file is deleted, shadow VFS overlay for that path is invalidated
    fs::remove_file(&file_path).expect("remove file");

    let invalidated = wait_for_condition(
        || !vfs.is_virtual(&file_path),
        1500,
    )
    .await;
    assert!(invalidated, "Shadow VFS virtual entry should be invalidated on file deletion");
}

#[tokio::test]
async fn test_watcher_ignores_files_over_1mb_and_binary() {
    let dir = tempdir().expect("tempdir");
    let search_index = Arc::new(WorkspaceSearchIndex::new_in_memory().expect("search index"));
    let engine = WatcherEngine::new(20);
    engine.bind_search_index(Arc::clone(&search_index));

    let _sub_id = engine.watch(dir.path(), &[]).expect("watch");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 1. Binary file with non-UTF-8 bytes
    let bin_path = dir.path().join("blob.bin");
    let non_utf8_data = vec![0xFF, 0xFE, 0xFD, 0xFC, 0x00, 0x01];
    fs::write(&bin_path, &non_utf8_data).expect("write bin");

    // 2. Large file >= 1MB
    let large_path = dir.path().join("large.txt");
    let large_data = vec![b'A'; 1_048_576 + 100];
    fs::write(&large_path, &large_data).expect("write large");

    // 3. Normal file checkpoint
    let norm_path = dir.path().join("checkpoint.txt");
    fs::write(&norm_path, "checkpoint_unique_symbol").expect("write checkpoint");

    let checkpoint_indexed = wait_for_condition(
        || {
            search_index
                .search("checkpoint_unique_symbol")
                .map(|r| !r.is_empty())
                .unwrap_or(false)
        },
        2000,
    )
    .await;
    assert!(checkpoint_indexed, "Checkpoint file must be indexed");

    let norm_large_str = crate::kernel::watcher::normalize_path_str(&large_path);
    let norm_bin_str = crate::kernel::watcher::normalize_path_str(&bin_path);

    let results = search_index.search("A").unwrap_or_default();
    assert!(!results.iter().any(|r| r.path == norm_large_str), "File > 1MB must not be indexed");
    assert!(!results.iter().any(|r| r.path == norm_bin_str), "Binary file must not be indexed");
}

#[tokio::test]
async fn test_watcher_burst_write_performance_benchmark() {
    let dir = tempdir().expect("tempdir");
    let engine = WatcherEngine::new(20);
    let mut rx = engine.subscribe_events();

    let sub_id = engine.watch(dir.path(), &[]).expect("watch");
    tokio::time::sleep(Duration::from_millis(50)).await;
    while rx.try_recv().is_ok() {}

    let write_start = Instant::now();
    let count = 100;
    let mut handles = Vec::with_capacity(count);
    for i in 0..count {
        let file_path = dir.path().join(format!("burst_bench_{i:03}.txt"));
        handles.push(tokio::spawn(async move {
            tokio::fs::write(&file_path, format!("benchmark payload content #{i}"))
                .await
                .expect("write burst file");
        }));
    }
    for h in handles {
        h.await.expect("join burst write task");
    }
    let write_elapsed = write_start.elapsed();

    let recv_start = Instant::now();
    let mut observed_files = HashSet::new();
    let mut batch_count = 0;
    let timeout = Duration::from_millis(2000);

    while observed_files.len() < count {
        let batch = tokio::time::timeout(timeout, rx.recv())
            .await
            .expect("timeout waiting for debounced batch")
            .expect("receive debounced events");
        batch_count += 1;
        for event in batch {
            if let Some(name) = std::path::Path::new(&event.path).file_name() {
                let name_str = name.to_string_lossy().to_string();
                if name_str.starts_with("burst_bench_") {
                    observed_files.insert(name_str);
                }
            }
        }
    }
    let debounce_and_process_elapsed = recv_start.elapsed();

    println!(
        "[BENCH] 100 concurrent writes took: {:?}, debounce & event delivery: {:?} across {} batch(es)",
        write_elapsed, debounce_and_process_elapsed, batch_count
    );

    assert_eq!(
        observed_files.len(),
        count,
        "All 100 burst writes must be detected without dropping events"
    );

    // Reasonable test threshold (<1500ms) ensuring non-blocking debounced delivery
    assert!(
        debounce_and_process_elapsed < Duration::from_millis(1500),
        "Debounce and process latency exceeded threshold: {:?}",
        debounce_and_process_elapsed
    );

    engine.unwatch(&sub_id).expect("unwatch");
}
