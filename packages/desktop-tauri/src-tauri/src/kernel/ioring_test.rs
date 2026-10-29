use crate::kernel::ioring::IoRingBatchReader;
use std::fs;

#[test]
fn test_ioring_batch_file_read() {
    let reader = IoRingBatchReader::new(1024).expect("Failed to initialize IoRingBatchReader");
    let test_path = std::env::temp_dir().join("kronos_ioring_test_single.txt");
    fs::write(&test_path, b"kernel ioring fast read").expect("Failed to write single test file");

    let results = reader
        .read_files_batched(&[test_path.clone()])
        .expect("Batch read failed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], b"kernel ioring fast read");
    let _ = fs::remove_file(test_path);
}

#[test]
fn test_ioring_multiple_files_batch() {
    let reader = IoRingBatchReader::new(1024).expect("Failed to initialize IoRingBatchReader");
    let file_count = 16;
    let mut paths = Vec::with_capacity(file_count);
    let mut expected_contents = Vec::with_capacity(file_count);

    for i in 0..file_count {
        let path = std::env::temp_dir().join(format!("kronos_ioring_multi_test_{:04}.txt", i));
        let content = format!("kronos-ioring-chunk-payload-data-line-{:04}", i).into_bytes();
        fs::write(&path, &content).expect("Failed to write temp test file");
        paths.push(path);
        expected_contents.push(content);
    }

    let results = reader
        .read_files_batched(&paths)
        .expect("Multiple files batch read failed");
    assert_eq!(results.len(), file_count);

    for i in 0..file_count {
        assert_eq!(
            results[i], expected_contents[i],
            "Mismatch at index {} for path {}",
            i,
            paths[i].display()
        );
    }

    for path in paths {
        let _ = fs::remove_file(path);
    }
}

#[test]
fn test_ioring_missing_file_handling() {
    let reader = IoRingBatchReader::new(1024).expect("Failed to initialize IoRingBatchReader");
    let test_path_1 = std::env::temp_dir().join("kronos_ioring_valid_1.txt");
    let test_path_2 = std::env::temp_dir().join("kronos_ioring_valid_2.txt");
    let missing_path = std::env::temp_dir().join("kronos_ioring_nonexistent_definitely_missing.txt");

    fs::write(&test_path_1, b"valid file 1").unwrap();
    fs::write(&test_path_2, b"valid file 2").unwrap();
    let _ = fs::remove_file(&missing_path);

    let batch = vec![test_path_1.clone(), missing_path.clone(), test_path_2.clone()];
    let result = reader.read_files_batched(&batch);

    assert!(result.is_err(), "Expected error for batch containing missing file");
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        std::io::ErrorKind::NotFound,
        "Expected NotFound error kind, got {:?}",
        err
    );

    let _ = fs::remove_file(test_path_1);
    let _ = fs::remove_file(test_path_2);
}

#[test]
fn test_ioring_backend_detection() {
    let reader = IoRingBatchReader::new(1024).expect("Failed to initialize IoRingBatchReader");
    let is_active = reader.is_kernel_ioring_active();
    println!("Kernel IoRing active status: {}", is_active);
    assert!(reader.queue_size() >= 16);

    // Verify that reader.is_kernel_ioring_active() returns a valid boolean state
    // and that batched reads succeed regardless of whether native kernel IoRing
    // or threadpool fallback was selected by hardware capability probes/OS loader.
    let test_path = std::env::temp_dir().join("kronos_ioring_backend_detection_test.txt");
    fs::write(&test_path, b"backend detection test payload").expect("Failed to write test file");
    let results = reader
        .read_files_batched(&[test_path.clone()])
        .expect("Read should succeed regardless of active backend");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], b"backend detection test payload");
    let _ = fs::remove_file(test_path);
}

#[cfg(windows)]
#[test]
fn test_native_ioring_reset() {
    if let Some(api) = crate::kernel::ioring::windows_native::load_ioring_api() {
        if let Some(mut ring) = crate::kernel::ioring::windows_native::NativeIoRing::create(api, 64) {
            let initial_handle = ring.handle;
            assert!(!initial_handle.is_null());
            assert!(ring.reset(), "NativeIoRing reset must succeed");
            assert!(!ring.handle.is_null());
        }
    }
}

#[test]
fn test_ioring_fallback_threadpool() {
    let reader = IoRingBatchReader::new_with_fallback(64, true)
        .expect("Failed to initialize threadpool reader");
    assert!(
        !reader.is_kernel_ioring_active(),
        "Threadpool reader must not have kernel IoRing active"
    );

    let mut paths = Vec::new();
    let mut contents = Vec::new();
    for i in 0..12 {
        let path = std::env::temp_dir().join(format!("kronos_threadpool_test_{}.txt", i));
        let data = format!("threadpool test content payload {}", i).into_bytes();
        fs::write(&path, &data).unwrap();
        paths.push(path);
        contents.push(data);
    }

    let results = reader
        .read_files_batched(&paths)
        .expect("Threadpool batch read failed");
    assert_eq!(results.len(), 12);
    for i in 0..12 {
        assert_eq!(results[i], contents[i]);
    }

    for path in paths {
        let _ = fs::remove_file(path);
    }
}

#[test]
fn test_ioring_empty_file_batch() {
    let reader = IoRingBatchReader::new(1024).expect("Failed to initialize IoRingBatchReader");
    let empty_path = std::env::temp_dir().join("kronos_ioring_empty.txt");
    let regular_path = std::env::temp_dir().join("kronos_ioring_nonempty.txt");

    fs::write(&empty_path, b"").unwrap();
    fs::write(&regular_path, b"some content").unwrap();

    let results = reader
        .read_files_batched(&[empty_path.clone(), regular_path.clone()])
        .expect("Batch read with empty file failed");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], b"");
    assert_eq!(results[1], b"some content");

    let _ = fs::remove_file(empty_path);
    let _ = fs::remove_file(regular_path);
}

#[test]
fn test_ioring_chunking_small_queue() {
    // Queue size clamped to 16 min internally, so create 35 files to ensure multiple chunks
    let reader = IoRingBatchReader::new(16).expect("Failed to initialize IoRingBatchReader");
    let file_count = 35;
    let mut paths = Vec::with_capacity(file_count);
    let mut expected_contents = Vec::with_capacity(file_count);

    for i in 0..file_count {
        let path = std::env::temp_dir().join(format!("kronos_ioring_chunking_{:03}.txt", i));
        let content = format!("chunking test data block {:03}", i).into_bytes();
        fs::write(&path, &content).unwrap();
        paths.push(path);
        expected_contents.push(content);
    }

    let results = reader
        .read_files_batched(&paths)
        .expect("Chunking batch read failed");
    assert_eq!(results.len(), file_count);

    for i in 0..file_count {
        assert_eq!(results[i], expected_contents[i]);
    }

    for path in paths {
        let _ = fs::remove_file(path);
    }
}

#[test]
fn test_ioring_empty_batch() {
    let reader = IoRingBatchReader::new(1024).expect("Failed to initialize IoRingBatchReader");
    let results = reader.read_files_batched(&[]).expect("Empty batch read failed");
    assert!(results.is_empty());
}
