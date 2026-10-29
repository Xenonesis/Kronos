use super::rkyv_shm::{DiffHunkChunk, TelemetryChunk};
use rkyv::Deserialize;

#[test]
fn test_rkyv_zero_allocation_read() {
    let raw_payload = vec![1, 2, 3, 4, 5, 42, 99, 128, 255];
    let chunk = TelemetryChunk::new(1001, "stream-hnk-alpha", raw_payload.clone());

    let aligned_bytes = chunk
        .serialize_to_aligned_bytes()
        .expect("serialization should succeed");

    // Zero-allocation access via rkyv::check_archived_root
    let archived = TelemetryChunk::access_archived(&aligned_bytes)
        .expect("zero-copy access should succeed");

    // Verify fields in-place
    assert_eq!(archived.sequence, 1001);
    assert_eq!(archived.stream_id.as_str(), "stream-hnk-alpha");
    assert_eq!(archived.bytes_len, raw_payload.len() as u32);
    assert_eq!(archived.payload.as_slice(), raw_payload.as_slice());

    // Proof of zero-allocation: the payload pointer is located within the memory of aligned_bytes
    let buf_start = aligned_bytes.as_ptr() as usize;
    let buf_end = buf_start + aligned_bytes.len();
    let payload_ptr = archived.payload.as_slice().as_ptr() as usize;
    assert!(
        payload_ptr >= buf_start && payload_ptr < buf_end,
        "payload bytes must point directly inside the archived buffer (zero copy)"
    );

    // Also verify stream_id points within the buffer
    let stream_id_ptr = archived.stream_id.as_str().as_ptr() as usize;
    assert!(
        stream_id_ptr >= buf_start && stream_id_ptr < buf_end,
        "stream_id bytes must point directly inside the archived buffer (zero copy)"
    );
}

#[test]
fn test_rkyv_diff_hunk_archive() {
    let lines = vec![
        "- const old_val = 100;".to_string(),
        "+ const new_val = 200;".to_string(),
        "  println!(\"unchanged\");".to_string(),
    ];
    let hunk = DiffHunkChunk::new("packages/core/src/kernel.ts", 45, 45, lines.clone());

    let aligned_bytes = hunk
        .serialize_to_aligned_bytes()
        .expect("serialization of diff hunk should succeed");

    // Zero-allocation access
    let archived_hunk = DiffHunkChunk::access_archived(&aligned_bytes)
        .expect("zero-copy access to diff hunk should succeed");

    assert_eq!(
        archived_hunk.file_path.as_str(),
        "packages/core/src/kernel.ts"
    );
    assert_eq!(archived_hunk.old_start, 45);
    assert_eq!(archived_hunk.new_start, 45);
    assert_eq!(archived_hunk.lines.len(), 3);
    assert_eq!(archived_hunk.lines[0].as_str(), "- const old_val = 100;");
    assert_eq!(archived_hunk.lines[1].as_str(), "+ const new_val = 200;");
    assert_eq!(
        archived_hunk.lines[2].as_str(),
        "  println!(\"unchanged\");"
    );

    // Verify zero-copy memory pointers
    let buf_start = aligned_bytes.as_ptr() as usize;
    let buf_end = buf_start + aligned_bytes.len();
    let file_path_ptr = archived_hunk.file_path.as_str().as_ptr() as usize;
    assert!(
        file_path_ptr >= buf_start && file_path_ptr < buf_end,
        "file_path must point directly inside the archived buffer"
    );
}

#[test]
fn test_rkyv_deserialize_back() {
    // Roundtrip TelemetryChunk
    let original_telemetry = TelemetryChunk::new(
        8888,
        "stream-back",
        vec![11, 22, 33, 44, 55, 66, 77, 88, 99],
    );
    let bytes_telemetry = original_telemetry
        .serialize_to_aligned_bytes()
        .expect("serialize telemetry");
    let archived_telemetry =
        TelemetryChunk::access_archived(&bytes_telemetry).expect("access archived telemetry");

    // Deserialize back via helper
    let deserialized_telemetry = TelemetryChunk::deserialize_archived(archived_telemetry)
        .expect("deserialize archived telemetry via helper");
    assert_eq!(deserialized_telemetry, original_telemetry);

    // Deserialize back via rkyv trait directly
    let direct_deserialized: TelemetryChunk = archived_telemetry
        .deserialize(&mut rkyv::Infallible)
        .expect("direct deserialize");
    assert_eq!(direct_deserialized, original_telemetry);

    // Roundtrip DiffHunkChunk
    let original_hunk = DiffHunkChunk::new(
        "src/main.rs",
        1,
        1,
        vec![
            "fn main() {".to_string(),
            "    // Hello Kronos".to_string(),
            "}".to_string(),
        ],
    );
    let bytes_hunk = original_hunk
        .serialize_to_aligned_bytes()
        .expect("serialize diff hunk");
    let archived_hunk =
        DiffHunkChunk::access_archived(&bytes_hunk).expect("access archived diff hunk");

    let deserialized_hunk = DiffHunkChunk::deserialize_archived(archived_hunk)
        .expect("deserialize archived hunk via helper");
    assert_eq!(deserialized_hunk, original_hunk);

    let direct_deserialized_hunk: DiffHunkChunk = archived_hunk
        .deserialize(&mut rkyv::Infallible)
        .expect("direct deserialize hunk");
    assert_eq!(direct_deserialized_hunk, original_hunk);
}
