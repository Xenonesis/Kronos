use std::fs;
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
    while let Ok(_) = rx.try_recv() {}

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
