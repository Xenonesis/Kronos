use std::path::Path;
use super::git::GitEngine;
use super::vfs::ShadowVfs;

#[test]
fn test_vfs_virtual_creation_reading_flushing() {
    let vfs = ShadowVfs::new();
    assert!(vfs.is_empty());
    assert_eq!(vfs.len(), 0);

    let virtual_path = "test/virtual_shadow_file.txt";
    let data = b"Hello Kronos Shadow VFS";

    // Write virtual
    vfs.write_virtual(virtual_path, data).expect("write_virtual failed");
    assert!(vfs.is_virtual(virtual_path));
    assert_eq!(vfs.len(), 1);

    // Read virtual
    let read_back = vfs.read_virtual(virtual_path).expect("read_virtual failed");
    assert_eq!(read_back, data);

    // Test lexical normalization (e.g. redundant components)
    let non_normalized = "test/sub/../virtual_shadow_file.txt";
    assert!(vfs.is_virtual(non_normalized));
    let read_norm = vfs.read_virtual(non_normalized).expect("read_norm failed");
    assert_eq!(read_norm, data);

    // Flush to disk
    let temp_dir = std::env::temp_dir().join(format!(
        "kronos_vfs_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let target_disk_path = temp_dir.join("flushed_file.txt");

    vfs.flush_to_disk(virtual_path, &target_disk_path)
        .expect("flush_to_disk failed");

    assert!(target_disk_path.exists());
    let disk_content = std::fs::read(&target_disk_path).expect("read disk failed");
    assert_eq!(disk_content, data);

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_vfs_cow_semantics() {
    let vfs = ShadowVfs::new();

    let temp_dir = std::env::temp_dir().join(format!(
        "kronos_cow_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create_dir_all failed");
    let physical_path = temp_dir.join("physical_base.txt");

    let physical_data = b"Initial disk content";
    std::fs::write(&physical_path, physical_data).expect("write physical file failed");

    // Before virtual write: read_file_cow should read physical content from disk
    let read_initial = vfs.read_file_cow(&physical_path).expect("cow initial read failed");
    assert_eq!(read_initial, physical_data);
    assert!(!vfs.is_virtual(&physical_path));

    // Write virtual overlay at same path
    let shadow_data = b"Overridden in-memory virtual content";
    vfs.write_virtual(&physical_path, shadow_data).expect("write virtual failed");
    assert!(vfs.is_virtual(&physical_path));

    // CoW read returns shadow overlay
    let read_cow = vfs.read_file_cow(&physical_path).expect("cow shadow read failed");
    assert_eq!(read_cow, shadow_data);

    // Physical disk file must remain untouched before flush
    let disk_untouched = std::fs::read(&physical_path).expect("read disk failed");
    assert_eq!(disk_untouched, physical_data);

    // Flush virtual to physical path
    vfs.flush_to_disk(&physical_path, &physical_path).expect("flush failed");
    let disk_flushed = std::fs::read(&physical_path).expect("read disk after flush failed");
    assert_eq!(disk_flushed, shadow_data);

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_git_engine_repo_and_branch_discovery() {
    // Current manifest dir is packages/desktop-tauri/src-tauri
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .expect("desktop-tauri parent")
        .parent()
        .expect("repo root");

    // Discover repository
    assert!(GitEngine::is_repo(repo_root));
    assert!(GitEngine::is_repo(manifest_dir));

    let repo = GitEngine::discover(repo_root).expect("git discover failed");

    // Discover current branch
    let branch_via_path = GitEngine::current_branch(repo_root).expect("branch via path failed");
    let branch_via_repo = GitEngine::current_branch_for_repo(&repo).expect("branch via repo failed");
    let branch_via_ref = GitEngine::current_branch(&repo).expect("branch via ref failed");

    assert_eq!(branch_via_path, branch_via_repo);
    assert_eq!(branch_via_repo, branch_via_ref);
    assert!(
        branch_via_path.is_some(),
        "Expected repository to have a branch or ref: {:?}",
        branch_via_path
    );

    // Check status summary and is_dirty
    let status_summary = GitEngine::status_summary(repo_root).expect("status_summary failed");
    assert_eq!(status_summary.current_branch, branch_via_path);

    let is_dirty = GitEngine::is_dirty(repo_root);
    assert!(is_dirty.is_ok());

    let is_dirty_repo = GitEngine::is_dirty(&repo);
    assert!(is_dirty_repo.is_ok());
}
