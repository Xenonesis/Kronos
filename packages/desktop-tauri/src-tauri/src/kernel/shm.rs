//! Zero-Copy Shared Memory Ring Buffer (SMRB) for High-Throughput Streaming
//!
//! Provides inter-thread and inter-process lock-free reading and safe synchronized
//! writing into a memory-mapped circular ring buffer.
//!
//! Memory Layout:
//! - Header (32 bytes):
//!   - 0..8:   Magic `0x4B524F4E4F53534D` ("KRONOSSM")
//!   - 8..16:  Atomic write sequence (u64, little-endian)
//!   - 16..24: Atomic write head offset (u64, little-endian)
//!   - 24..32: Capacity of the ring buffer body (u64, little-endian)
//! - Ring buffer body:
//!   - Circular payload records with `[length: u32, sequence: u64, payload: [u8]]`

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const MAGIC: u64 = 0x4B524F4E4F53534D; // "KRONOSSM"
pub const HEADER_SIZE: usize = 32;
pub const RECORD_HEADER_SIZE: usize = 12;
pub const WRAP_SENTINEL: u32 = 0xFFFF_FFFF;

pub const MAGIC_OFFSET: usize = 0;
pub const SEQ_OFFSET: usize = 8;
pub const HEAD_OFFSET: usize = 16;
pub const CAP_OFFSET: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordMeta {
    pub sequence: u64,
    pub offset: usize,
    pub length: usize,
}

#[derive(Debug)]
struct RingState {
    records: VecDeque<RecordMeta>,
    write_head: usize,
    sequence: u64,
}

struct ShmMappings {
    ptr: *mut u8,
    total_size: usize,
    capacity: usize,
    #[cfg(windows)]
    file_mapping_handle: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(windows)]
    mutex_handle: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(not(windows))]
    _layout: Option<std::alloc::Layout>,
}

unsafe impl Send for ShmMappings {}
unsafe impl Sync for ShmMappings {}

#[cfg(windows)]
impl Drop for ShmMappings {
    fn drop(&mut self) {
        unsafe {
            use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
            use windows_sys::Win32::System::Memory::{UnmapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS};

            if !self.ptr.is_null() {
                let view = MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.ptr as *mut core::ffi::c_void,
                };
                let _ = UnmapViewOfFile(view);
                self.ptr = core::ptr::null_mut();
            }

            if !self.file_mapping_handle.is_null()
                && self.file_mapping_handle != INVALID_HANDLE_VALUE
            {
                let _ = CloseHandle(self.file_mapping_handle);
                self.file_mapping_handle = core::ptr::null_mut();
            }

            if !self.mutex_handle.is_null() && self.mutex_handle != INVALID_HANDLE_VALUE {
                let _ = CloseHandle(self.mutex_handle);
                self.mutex_handle = core::ptr::null_mut();
            }
        }
    }
}

#[cfg(not(windows))]
impl Drop for ShmMappings {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            if let Some(layout) = self._layout {
                unsafe {
                    std::alloc::dealloc(self.ptr, layout);
                }
                self.ptr = core::ptr::null_mut();
            }
        }
    }
}

#[cfg(windows)]
struct WinMutexGuard(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl WinMutexGuard {
    fn acquire(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<Self> {
        use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0};
        use windows_sys::Win32::System::Threading::{WaitForSingleObject, INFINITE};

        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return None;
        }
        unsafe {
            let wait_res = WaitForSingleObject(handle, INFINITE);
            if wait_res == WAIT_OBJECT_0 || wait_res == WAIT_ABANDONED {
                Some(WinMutexGuard(handle))
            } else {
                None
            }
        }
    }
}

#[cfg(windows)]
impl Drop for WinMutexGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows_sys::Win32::System::Threading::ReleaseMutex(self.0);
        }
    }
}

fn recover_ring_state(
    body_ptr: *const u8,
    capacity: usize,
    current_seq: u64,
    write_head: usize,
) -> RingState {
    let mut records = VecDeque::new();
    if current_seq == 0 || capacity < RECORD_HEADER_SIZE {
        return RingState {
            records,
            write_head: 0,
            sequence: 0,
        };
    }

    let mut p = 0;
    while p + RECORD_HEADER_SIZE <= capacity {
        let len = unsafe { u32::from_le((body_ptr.add(p) as *const u32).read_unaligned()) };
        if len == WRAP_SENTINEL {
            break;
        }
        let seq = unsafe { u64::from_le((body_ptr.add(p + 4) as *const u64).read_unaligned()) };
        if len == 0 && seq == 0 {
            p += 4;
            continue;
        }
        if p + RECORD_HEADER_SIZE + (len as usize) > capacity {
            break;
        }
        if seq > 0 && seq <= current_seq {
            records.push_back(RecordMeta {
                sequence: seq,
                offset: p,
                length: len as usize,
            });
        }
        p += RECORD_HEADER_SIZE + (len as usize);
    }

    records.make_contiguous().sort_by_key(|r| r.sequence);

    RingState {
        records,
        write_head: write_head % capacity,
        sequence: current_seq,
    }
}

#[cfg(windows)]
fn create_or_open_mapping(name: &str, capacity: usize) -> std::io::Result<(ShmMappings, bool)> {
    use std::io::{Error, ErrorKind};
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, MapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
    };
    use windows_sys::Win32::System::Threading::CreateMutexW;

    if capacity < 64 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Shared memory capacity must be at least 64 bytes",
        ));
    }

    let total_size = HEADER_SIZE + capacity;
    let shm_name = if name.starts_with("Local\\") || name.starts_with("Global\\") {
        name.to_string()
    } else {
        format!("Local\\{}", name)
    };
    let wide_name: Vec<u16> = shm_name.encode_utf16().chain(std::iter::once(0)).collect();

    let high = (total_size >> 32) as u32;
    let low = (total_size & 0xFFFF_FFFF) as u32;

    let file_mapping_handle = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            core::ptr::null(),
            PAGE_READWRITE,
            high,
            low,
            wide_name.as_ptr(),
        )
    };

    if file_mapping_handle.is_null() || file_mapping_handle == INVALID_HANDLE_VALUE {
        return Err(Error::last_os_error());
    }

    let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;

    let view = unsafe { MapViewOfFile(file_mapping_handle, FILE_MAP_ALL_ACCESS, 0, 0, total_size) };

    if view.Value.is_null() {
        let err = Error::last_os_error();
        unsafe {
            CloseHandle(file_mapping_handle);
        }
        return Err(err);
    }

    let ptr = view.Value as *mut u8;

    let mtx_name = format!("{}_mtx", shm_name);
    let wide_mtx: Vec<u16> = mtx_name.encode_utf16().chain(std::iter::once(0)).collect();
    let mutex_handle = unsafe { CreateMutexW(core::ptr::null(), 0, wide_mtx.as_ptr()) };

    Ok((
        ShmMappings {
            ptr,
            total_size,
            capacity,
            file_mapping_handle,
            mutex_handle,
        },
        already_exists,
    ))
}

#[cfg(not(windows))]
fn create_or_open_mapping(_name: &str, capacity: usize) -> std::io::Result<(ShmMappings, bool)> {
    use std::io::{Error, ErrorKind};
    if capacity < 64 {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "Shared memory capacity must be at least 64 bytes",
        ));
    }
    let total_size = HEADER_SIZE + capacity;
    let layout = std::alloc::Layout::from_size_align(total_size, 8)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    if ptr.is_null() {
        return Err(Error::new(
            ErrorKind::OutOfMemory,
            "Failed to allocate memory buffer",
        ));
    }
    Ok((
        ShmMappings {
            ptr,
            total_size,
            capacity,
            _layout: Some(layout),
        },
        false,
    ))
}

/// Zero-Copy Shared Memory Ring Buffer
pub struct ShmRingBuffer {
    name: String,
    mapping: Arc<ShmMappings>,
    state: Mutex<RingState>,
}

impl ShmRingBuffer {
    /// Creates or opens a named shared memory ring buffer with the specified body capacity.
    pub fn new(name: &str, capacity: usize) -> std::io::Result<Self> {
        let (mapping, already_exists) = create_or_open_mapping(name, capacity)?;
        let mapping = Arc::new(mapping);

        if already_exists {
            unsafe {
                let magic =
                    u64::from_le((mapping.ptr.add(MAGIC_OFFSET) as *const u64).read_unaligned());
                if magic != MAGIC {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Invalid SHM magic: 0x{:016X}", magic),
                    ));
                }
                let seq =
                    (&*(mapping.ptr.add(SEQ_OFFSET) as *const AtomicU64)).load(Ordering::Acquire);
                let head =
                    (&*(mapping.ptr.add(HEAD_OFFSET) as *const AtomicU64)).load(Ordering::Acquire)
                        as usize;
                let cap =
                    u64::from_le((mapping.ptr.add(CAP_OFFSET) as *const u64).read_unaligned())
                        as usize;
                let body_ptr = mapping.ptr.add(HEADER_SIZE);
                let state = recover_ring_state(body_ptr, cap, seq, head);
                Ok(Self {
                    name: name.to_string(),
                    mapping,
                    state: Mutex::new(state),
                })
            }
        } else {
            unsafe {
                (mapping.ptr.add(MAGIC_OFFSET) as *mut u64).write_unaligned(MAGIC.to_le());
                (&*(mapping.ptr.add(SEQ_OFFSET) as *const AtomicU64)).store(0, Ordering::Release);
                (&*(mapping.ptr.add(HEAD_OFFSET) as *const AtomicU64)).store(0, Ordering::Release);
                (mapping.ptr.add(CAP_OFFSET) as *mut u64)
                    .write_unaligned((capacity as u64).to_le());
            }
            Ok(Self {
                name: name.to_string(),
                mapping,
                state: Mutex::new(RingState {
                    records: VecDeque::new(),
                    write_head: 0,
                    sequence: 0,
                }),
            })
        }
    }

    /// Appends a new payload record into the ring buffer.
    /// Returns the assigned sequence ID.
    pub fn write(&self, data: &[u8]) -> std::io::Result<u64> {
        let cap = self.capacity();
        let payload_len = data.len();
        let record_len = RECORD_HEADER_SIZE + payload_len;

        if record_len > cap {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Payload size {} + {} header exceeds ring capacity {}",
                    payload_len, RECORD_HEADER_SIZE, cap
                ),
            ));
        }

        #[cfg(windows)]
        let _win_guard = WinMutexGuard::acquire(self.mapping.mutex_handle);

        let mut state = self
            .state
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Lock poisoned"))?;

        let head = state.write_head;
        let (write_offset, next_head, did_wrap) = if head + record_len <= cap {
            (head, (head + record_len) % cap, false)
        } else {
            // Write wrap sentinel at the tail gap
            let body_ptr = self.body_ptr();
            if cap - head >= 4 {
                unsafe {
                    (body_ptr.add(head) as *mut u32).write_unaligned(WRAP_SENTINEL.to_le());
                }
            }
            (0, record_len % cap, true)
        };

        // Prune overwritten records from state.records
        let write_end = write_offset + record_len;
        state.records.retain(|rec| {
            let rec_start = rec.offset;
            let rec_end = rec.offset + RECORD_HEADER_SIZE + rec.length;
            if did_wrap {
                let overlaps_tail = rec_start < cap && rec_end > head;
                let overlaps_head = rec_start < write_end && rec_end > 0;
                !overlaps_tail && !overlaps_head
            } else {
                let overlaps = rec_start < write_end && rec_end > write_offset;
                !overlaps
            }
        });

        let new_seq = state.sequence + 1;
        let body_ptr = self.body_ptr();

        unsafe {
            // Write record header
            (body_ptr.add(write_offset) as *mut u32).write_unaligned((payload_len as u32).to_le());
            (body_ptr.add(write_offset + 4) as *mut u64).write_unaligned((new_seq as u64).to_le());
            // Write payload
            if payload_len > 0 {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    body_ptr.add(write_offset + RECORD_HEADER_SIZE),
                    payload_len,
                );
            }

            // Zero out gap until next valid record or next 12 bytes
            let zero_limit =
                if let Some(next_rec) = state.records.front().filter(|r| r.offset > write_end) {
                    next_rec.offset
                } else {
                    (write_end + RECORD_HEADER_SIZE).min(cap)
                };
            if zero_limit > write_end {
                std::ptr::write_bytes(body_ptr.add(write_end), 0, zero_limit - write_end);
            }

            // Update shared memory header atomically
            let header_ptr = self.mapping.ptr;
            (&*(header_ptr.add(SEQ_OFFSET) as *const AtomicU64)).store(new_seq, Ordering::Release);
            (&*(header_ptr.add(HEAD_OFFSET) as *const AtomicU64))
                .store(next_head as u64, Ordering::Release);
        }

        state.records.push_back(RecordMeta {
            sequence: new_seq,
            offset: write_offset,
            length: payload_len,
        });
        state.write_head = next_head;
        state.sequence = new_seq;

        Ok(new_seq)
    }

    /// Reads the latest payload written into the buffer.
    pub fn read_latest(&self) -> std::io::Result<Vec<u8>> {
        let state = self
            .state
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Lock poisoned"))?;

        if state.records.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Ring buffer is empty",
            ));
        }

        let latest = state.records.back().unwrap();
        let body_ptr = self.body_ptr();
        let slice = unsafe {
            std::slice::from_raw_parts(
                body_ptr.add(latest.offset + RECORD_HEADER_SIZE),
                latest.length,
            )
        };
        Ok(slice.to_vec())
    }

    /// Reads all records whose sequence ID is strictly greater than `sequence`.
    /// Returns ordered pairs of `(sequence, payload)`.
    pub fn read_since(&self, sequence: u64) -> std::io::Result<Vec<(u64, Vec<u8>)>> {
        let state = self
            .state
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Lock poisoned"))?;

        let body_ptr = self.body_ptr();
        let mut results = Vec::new();
        for rec in state.records.iter() {
            if rec.sequence > sequence {
                let slice = unsafe {
                    std::slice::from_raw_parts(
                        body_ptr.add(rec.offset + RECORD_HEADER_SIZE),
                        rec.length,
                    )
                };
                results.push((rec.sequence, slice.to_vec()));
            }
        }
        Ok(results)
    }

    /// Executes a closure borrowing the latest payload directly without heap copying.
    pub fn with_latest<F, R>(&self, f: F) -> std::io::Result<R>
    where
        F: FnOnce(&[u8]) -> R,
    {
        let state = self
            .state
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Lock poisoned"))?;

        if state.records.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Ring buffer is empty",
            ));
        }

        let latest = state.records.back().unwrap();
        let body_ptr = self.body_ptr();
        let slice = unsafe {
            std::slice::from_raw_parts(
                body_ptr.add(latest.offset + RECORD_HEADER_SIZE),
                latest.length,
            )
        };
        Ok(f(slice))
    }

    /// Returns the capacity of the ring buffer body.
    pub fn capacity(&self) -> usize {
        self.mapping.capacity
    }

    /// Returns the name of the shared memory mapping.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current atomic write sequence from the header.
    pub fn sequence(&self) -> u64 {
        let header_ptr = self.mapping.ptr;
        unsafe { (&*(header_ptr.add(SEQ_OFFSET) as *const AtomicU64)).load(Ordering::Acquire) }
    }

    /// Returns the current atomic write head offset from the header.
    pub fn write_head(&self) -> u64 {
        let header_ptr = self.mapping.ptr;
        unsafe { (&*(header_ptr.add(HEAD_OFFSET) as *const AtomicU64)).load(Ordering::Acquire) }
    }

    /// Returns a slice over the entire mapped buffer (Header + Body).
    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.mapping.ptr, self.mapping.total_size) }
    }

    /// Returns a slice over the ring buffer body.
    pub fn body_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.body_ptr(), self.capacity()) }
    }

    fn body_ptr(&self) -> *mut u8 {
        unsafe { self.mapping.ptr.add(HEADER_SIZE) }
    }
}
