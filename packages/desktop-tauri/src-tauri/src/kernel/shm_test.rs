#[cfg(test)]
mod tests {
    use crate::kernel::shm::{ShmRingBuffer, HEADER_SIZE, MAGIC, RECORD_HEADER_SIZE, WRAP_SENTINEL};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_shm_ring_buffer_write_read_roundtrip() {
        let ring = ShmRingBuffer::new("kronos_test_shm", 1024 * 1024).unwrap();
        let payload = b"AI token chunk output telemetry";
        let seq = ring.write(payload).unwrap();
        assert!(seq > 0);

        let read = ring.read_latest().unwrap();
        assert_eq!(read, payload);
    }

    #[test]
    fn test_shm_sequential_writes_and_read_since() {
        let name = format!("kronos_test_seq_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let ring = ShmRingBuffer::new(&name, 1024 * 1024).unwrap();

        let s1 = ring.write(b"chunk_alpha").unwrap();
        let s2 = ring.write(b"chunk_beta").unwrap();
        let s3 = ring.write(b"chunk_gamma").unwrap();
        let s4 = ring.write(b"chunk_delta").unwrap();

        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        assert_eq!(s3, 3);
        assert_eq!(s4, 4);

        // read_since(0) should return all 4 records
        let all = ring.read_since(0).unwrap();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0], (1, b"chunk_alpha".to_vec()));
        assert_eq!(all[1], (2, b"chunk_beta".to_vec()));
        assert_eq!(all[2], (3, b"chunk_gamma".to_vec()));
        assert_eq!(all[3], (4, b"chunk_delta".to_vec()));

        // read_since(2) should return chunk 3 and 4
        let since_2 = ring.read_since(2).unwrap();
        assert_eq!(since_2.len(), 2);
        assert_eq!(since_2[0], (3, b"chunk_gamma".to_vec()));
        assert_eq!(since_2[1], (4, b"chunk_delta".to_vec()));

        // read_since(4) should return empty
        let since_4 = ring.read_since(4).unwrap();
        assert!(since_4.is_empty());

        // read_latest should return chunk 4
        let latest = ring.read_latest().unwrap();
        assert_eq!(latest, b"chunk_delta");
    }

    #[test]
    fn test_shm_ring_wraparound_boundary_handling() {
        // Use small capacity: 128 bytes body
        let name = format!("kronos_test_wrap_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let capacity = 128;
        let ring = ShmRingBuffer::new(&name, capacity).unwrap();

        // Each record: 12 bytes header + 28 bytes payload = 40 bytes.
        // Record 1: 0..40 (40 bytes total)
        let s1 = ring.write(&[1u8; 28]).unwrap();
        assert_eq!(s1, 1);
        // Record 2: 40..80 (80 bytes total)
        let s2 = ring.write(&[2u8; 28]).unwrap();
        assert_eq!(s2, 2);
        // Record 3: 80..120 (120 bytes total)
        let s3 = ring.write(&[3u8; 28]).unwrap();
        assert_eq!(s3, 3);

        // Record 4: 120 + 40 = 160 > 128 bytes.
        // It must WRAP to offset 0, writing sentinel at 120..128 and overwriting Record 1!
        let s4 = ring.write(&[4u8; 28]).unwrap();
        assert_eq!(s4, 4);

        // Record 1 was overwritten, so active records should be Record 2, Record 3, Record 4
        let records = ring.read_since(0).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].0, 2);
        assert_eq!(records[0].1, vec![2u8; 28]);
        assert_eq!(records[1].0, 3);
        assert_eq!(records[1].1, vec![3u8; 28]);
        assert_eq!(records[2].0, 4);
        assert_eq!(records[2].1, vec![4u8; 28]);

        // read_latest should be Record 4
        let latest = ring.read_latest().unwrap();
        assert_eq!(latest, vec![4u8; 28]);

        // Record 5: written at 40..80, overwriting Record 2!
        let s5 = ring.write(&[5u8; 28]).unwrap();
        assert_eq!(s5, 5);

        let records2 = ring.read_since(0).unwrap();
        assert_eq!(records2.len(), 3);
        assert_eq!(records2[0].0, 3);
        assert_eq!(records2[1].0, 4);
        assert_eq!(records2[2].0, 5);

        // Verify wrap sentinel byte was written in memory at tail
        let body = ring.body_slice();
        let sentinel = u32::from_le_bytes(body[120..124].try_into().unwrap());
        assert_eq!(sentinel, WRAP_SENTINEL);
    }

    #[test]
    fn test_shm_payload_exceeding_capacity() {
        let name = format!("kronos_test_toobig_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let capacity = 100;
        let ring = ShmRingBuffer::new(&name, capacity).unwrap();

        // Payload of 90 bytes + 12 byte header = 102 bytes > 100
        let big_payload = vec![0xAA; 90];
        let res = ring.write(&big_payload);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_shm_empty_buffer() {
        let name = format!("kronos_test_empty_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let ring = ShmRingBuffer::new(&name, 1024).unwrap();

        assert_eq!(ring.sequence(), 0);
        assert_eq!(ring.write_head(), 0);

        let res = ring.read_latest();
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::NotFound);

        let since = ring.read_since(0).unwrap();
        assert!(since.is_empty());
    }

    #[test]
    fn test_shm_with_latest_zero_copy() {
        let name = format!("kronos_test_zc_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let ring = ShmRingBuffer::new(&name, 1024).unwrap();
        ring.write(b"zero copy verification frame").unwrap();

        let len = ring.with_latest(|slice| {
            assert_eq!(slice, b"zero copy verification frame");
            slice.len()
        }).unwrap();
        assert_eq!(len, 28);
    }

    #[test]
    fn test_shm_header_layout_integrity() {
        let name = format!("kronos_test_hdr_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let ring = ShmRingBuffer::new(&name, 512).unwrap();
        ring.write(b"frame 1").unwrap();

        let raw = ring.as_slice();
        assert_eq!(raw.len(), HEADER_SIZE + 512);

        // Magic
        let magic = u64::from_le_bytes(raw[0..8].try_into().unwrap());
        assert_eq!(magic, MAGIC);

        // Sequence
        let seq = u64::from_le_bytes(raw[8..16].try_into().unwrap());
        assert_eq!(seq, 1);

        // Write head
        let head = u64::from_le_bytes(raw[16..24].try_into().unwrap());
        assert_eq!(head, (RECORD_HEADER_SIZE + b"frame 1".len()) as u64);

        // Capacity
        let cap = u64::from_le_bytes(raw[24..32].try_into().unwrap());
        assert_eq!(cap, 512);
    }

    #[test]
    fn test_shm_concurrent_threads() {
        let name = format!("kronos_test_threads_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let ring = Arc::new(ShmRingBuffer::new(&name, 64 * 1024).unwrap());

        let mut handles = Vec::new();
        for t in 0..4 {
            let ring_clone = Arc::clone(&ring);
            handles.push(thread::spawn(move || {
                for i in 0..25 {
                    let msg = format!("t{}_msg{}", t, i);
                    ring_clone.write(msg.as_bytes()).unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(ring.sequence(), 100);
        let all = ring.read_since(0).unwrap();
        assert_eq!(all.len(), 100);
    }
}
