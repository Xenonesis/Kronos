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
