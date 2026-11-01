use rkyv::ser::serializers::AllocSerializer;
use rkyv::ser::Serializer;
use rkyv::{AlignedVec, Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug, PartialEq, Eq))]
pub struct TelemetryChunk {
    pub sequence: u64,
    pub stream_id: String,
    pub bytes_len: u32,
    pub payload: Vec<u8>,
}

impl TelemetryChunk {
    pub fn new(sequence: u64, stream_id: impl Into<String>, payload: Vec<u8>) -> Self {
        let bytes_len = payload.len() as u32;
        Self {
            sequence,
            stream_id: stream_id.into(),
            bytes_len,
            payload,
        }
    }

    pub fn serialize_to_aligned_bytes(&self) -> Result<AlignedVec, String> {
        let mut serializer = AllocSerializer::<4096>::default();
        serializer
            .serialize_value(self)
            .map_err(|e| e.to_string())?;
        Ok(serializer.into_serializer().into_inner())
    }

    pub fn access_archived<'a>(bytes: &'a [u8]) -> Result<&'a ArchivedTelemetryChunk, String> {
        rkyv::check_archived_root::<TelemetryChunk>(bytes).map_err(|e| e.to_string())
    }

    pub fn deserialize_archived(archived: &ArchivedTelemetryChunk) -> Result<Self, String> {
        archived
            .deserialize(&mut rkyv::Infallible)
            .map_err(|e| format!("{:?}", e))
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[archive(check_bytes)]
#[archive_attr(derive(Debug, PartialEq, Eq))]
pub struct DiffHunkChunk {
    pub file_path: String,
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<String>,
}

impl DiffHunkChunk {
    pub fn new(
        file_path: impl Into<String>,
        old_start: u32,
        new_start: u32,
        lines: Vec<String>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            old_start,
            new_start,
            lines,
        }
    }

    pub fn serialize_to_aligned_bytes(&self) -> Result<AlignedVec, String> {
        let mut serializer = AllocSerializer::<4096>::default();
        serializer
            .serialize_value(self)
            .map_err(|e| e.to_string())?;
        Ok(serializer.into_serializer().into_inner())
    }

    pub fn access_archived<'a>(bytes: &'a [u8]) -> Result<&'a ArchivedDiffHunkChunk, String> {
        rkyv::check_archived_root::<DiffHunkChunk>(bytes).map_err(|e| e.to_string())
    }

    pub fn deserialize_archived(archived: &ArchivedDiffHunkChunk) -> Result<Self, String> {
        archived
            .deserialize(&mut rkyv::Infallible)
            .map_err(|e| format!("{:?}", e))
    }
}

use crate::kernel::shm::ShmRingBuffer;

/// Writes an rkyv-serialized `TelemetryChunk` into an SMRB ring buffer.
pub fn write_rkyv_chunk_to_shm(
    ring: &ShmRingBuffer,
    sequence: u64,
    stream_id: &str,
    payload: Vec<u8>,
) -> Result<u64, String> {
    let chunk = TelemetryChunk::new(sequence, stream_id, payload);
    let bytes = chunk.serialize_to_aligned_bytes()?;
    ring.write(&bytes).map_err(|e| format!("Failed to write rkyv chunk to SHM: {e}"))
}

/// Reads all rkyv-serialized `TelemetryChunk` records written to the SMRB ring buffer since `since_seq`.
pub fn read_rkyv_chunks_since(
    ring: &ShmRingBuffer,
    since_seq: u64,
) -> Result<Vec<(u64, TelemetryChunk)>, String> {
    let raw_records = ring.read_since(since_seq)
        .map_err(|e| format!("Failed to read SHM records: {e}"))?;

    let mut chunks = Vec::with_capacity(raw_records.len());
    for (seq, bytes) in raw_records {
        let archived = TelemetryChunk::access_archived(&bytes)
            .map_err(|e| format!("Corrupt rkyv chunk in SHM: {e}"))?;
        let chunk = TelemetryChunk::deserialize_archived(archived)?;
        chunks.push((seq, chunk));
    }
    Ok(chunks)
}
