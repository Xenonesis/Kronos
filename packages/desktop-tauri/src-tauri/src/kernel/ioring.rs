use std::path::{PathBuf};

#[cfg(windows)]
pub mod windows_native {
    use std::sync::Arc;
    use windows_sys::Win32::Foundation::{FreeLibrary, HANDLE, HMODULE};
    use windows_sys::Win32::Storage::FileSystem::{
        IORING_BUFFER_REF, IORING_CAPABILITIES, IORING_CQE, IORING_CREATE_FLAGS, IORING_HANDLE_REF,
    };
    use windows_sys::Win32::System::LibraryLoader::{
        GetModuleHandleA, GetProcAddress, LoadLibraryA,
    };

    pub type FnQueryIoRingCapabilities = unsafe extern "system" fn(
        capabilities: *mut IORING_CAPABILITIES,
    ) -> i32;

    pub type FnCreateIoRing = unsafe extern "system" fn(
        ioringversion: i32,
        flags: IORING_CREATE_FLAGS,
        submissionqueuesize: u32,
        completionqueuesize: u32,
        h: *mut *mut core::ffi::c_void,
    ) -> i32;

    pub type FnCloseIoRing = unsafe extern "system" fn(
        ioring: *mut core::ffi::c_void,
    ) -> i32;

    pub type FnSubmitIoRing = unsafe extern "system" fn(
        ioring: *mut core::ffi::c_void,
        waitoperations: u32,
        milliseconds: u32,
        submittedentries: *mut u32,
    ) -> i32;

    pub type FnPopIoRingCompletion = unsafe extern "system" fn(
        ioring: *mut core::ffi::c_void,
        cqe: *mut IORING_CQE,
    ) -> i32;

    pub type FnBuildIoRingReadFile = unsafe extern "system" fn(
        ioring: *mut core::ffi::c_void,
        fileref: IORING_HANDLE_REF,
        dataref: IORING_BUFFER_REF,
        numberofbytestoread: u32,
        fileoffset: u64,
        userdata: usize,
        sqeflags: i32,
    ) -> i32;

    pub struct IoRingApi {
        module: HMODULE,
        pub query_capabilities: FnQueryIoRingCapabilities,
        pub create_ioring: FnCreateIoRing,
        pub close_ioring: FnCloseIoRing,
        pub submit_ioring: FnSubmitIoRing,
        pub pop_completion: FnPopIoRingCompletion,
        pub build_read_file: FnBuildIoRingReadFile,
    }

    unsafe impl Send for IoRingApi {}
    unsafe impl Sync for IoRingApi {}

    impl Drop for IoRingApi {
        fn drop(&mut self) {
            if !self.module.is_null() {
                unsafe {
                    FreeLibrary(self.module);
                }
            }
        }
    }

    #[repr(C)]
    struct OsVersionInfoExW {
        dw_os_version_info_size: u32,
        dw_major_version: u32,
        dw_minor_version: u32,
        dw_build_number: u32,
        dw_platform_id: u32,
        sz_csd_version: [u16; 128],
        w_service_pack_major: u16,
        w_service_pack_minor: u16,
        w_suite_mask: u16,
        w_product_type: u8,
        w_reserved: u8,
    }

    pub fn is_windows_11_or_greater() -> bool {
        unsafe {
            let ntdll = GetModuleHandleA(b"ntdll.dll\0".as_ptr());
            if ntdll.is_null() {
                return false;
            }
            let get_ver = GetProcAddress(ntdll, b"RtlGetVersion\0".as_ptr());
            if let Some(get_ver_fn) = get_ver {
                let rtl_get_version: unsafe extern "system" fn(*mut OsVersionInfoExW) -> i32 =
                    std::mem::transmute(get_ver_fn);
                let mut info = std::mem::zeroed::<OsVersionInfoExW>();
                info.dw_os_version_info_size = std::mem::size_of::<OsVersionInfoExW>() as u32;
                if rtl_get_version(&mut info) == 0 {
                    return info.dw_major_version > 10
                        || (info.dw_major_version == 10 && info.dw_build_number >= 22000);
                }
            }
            false
        }
    }

    pub fn load_ioring_api() -> Option<Arc<IoRingApi>> {
        if !is_windows_11_or_greater() {
            return None;
        }

        // Try ioring.dll, kernelbase.dll, kernel32.dll
        let dll_names = [
            b"ioring.dll\0".as_ptr(),
            b"kernelbase.dll\0".as_ptr(),
            b"kernel32.dll\0".as_ptr(),
        ];

        for &dll_name in &dll_names {
            unsafe {
                let module = LoadLibraryA(dll_name);
                if module.is_null() {
                    continue;
                }

                let query_caps = GetProcAddress(module, b"QueryIoRingCapabilities\0".as_ptr());
                let create_ring = GetProcAddress(module, b"CreateIoRing\0".as_ptr());
                let close_ring = GetProcAddress(module, b"CloseIoRing\0".as_ptr());
                let submit_ring = GetProcAddress(module, b"SubmitIoRing\0".as_ptr());
                let pop_comp = GetProcAddress(module, b"PopIoRingCompletion\0".as_ptr());
                let build_read = GetProcAddress(module, b"BuildIoRingReadFile\0".as_ptr());

                if let (
                    Some(query_caps_fn),
                    Some(create_ring_fn),
                    Some(close_ring_fn),
                    Some(submit_ring_fn),
                    Some(pop_comp_fn),
                    Some(build_read_fn),
                ) = (
                    query_caps,
                    create_ring,
                    close_ring,
                    submit_ring,
                    pop_comp,
                    build_read,
                ) {
                    return Some(Arc::new(IoRingApi {
                        module,
                        query_capabilities: std::mem::transmute(query_caps_fn),
                        create_ioring: std::mem::transmute(create_ring_fn),
                        close_ioring: std::mem::transmute(close_ring_fn),
                        submit_ioring: std::mem::transmute(submit_ring_fn),
                        pop_completion: std::mem::transmute(pop_comp_fn),
                        build_read_file: std::mem::transmute(build_read_fn),
                    }));
                }

                FreeLibrary(module);
            }
        }

        None
    }

    pub struct NativeIoRing {
        pub handle: *mut core::ffi::c_void,
        pub api: Arc<IoRingApi>,
        pub queue_size: u32,
    }

    unsafe impl Send for NativeIoRing {}

    impl Drop for NativeIoRing {
        fn drop(&mut self) {
            if !self.handle.is_null() {
                unsafe {
                    (self.api.close_ioring)(self.handle);
                }
                self.handle = std::ptr::null_mut();
            }
        }
    }

    impl NativeIoRing {
        pub fn create(api: Arc<IoRingApi>, requested_queue_size: u32) -> Option<Self> {
            unsafe {
                let mut caps = std::mem::zeroed::<IORING_CAPABILITIES>();
                let hr = (api.query_capabilities)(&mut caps);
                if hr != 0 {
                    return None;
                }

                let max_sub = if caps.MaxSubmissionQueueSize == 0 {
                    1024
                } else {
                    caps.MaxSubmissionQueueSize
                };
                let queue_size = requested_queue_size.clamp(16, max_sub);
                let comp_queue_size = queue_size.saturating_mul(2).clamp(
                    queue_size,
                    if caps.MaxCompletionQueueSize == 0 {
                        queue_size * 2
                    } else {
                        caps.MaxCompletionQueueSize
                    },
                );

                let flags = IORING_CREATE_FLAGS {
                    Required: 0,
                    Advisory: 0,
                };
                let mut handle = std::ptr::null_mut();
                let hr = (api.create_ioring)(
                    1, // IORING_VERSION_1
                    flags,
                    queue_size,
                    comp_queue_size,
                    &mut handle,
                );

                if hr != 0 || handle.is_null() {
                    return None;
                }

                Some(NativeIoRing {
                    handle,
                    api,
                    queue_size,
                })
            }
        }
    }

    pub fn hresult_to_io_error(hr: i32) -> std::io::Error {
        let u_hr = hr as u32;
        let win32_code = if (u_hr & 0xFFFF0000) == 0x80070000 {
            (u_hr & 0xFFFF) as i32
        } else {
            hr
        };
        std::io::Error::from_raw_os_error(win32_code)
    }

    pub fn to_windows_handle(file: &std::fs::File) -> HANDLE {
        use std::os::windows::io::AsRawHandle;
        file.as_raw_handle() as HANDLE
    }
}

enum Backend {
    #[cfg(windows)]
    Kernel(std::sync::Mutex<windows_native::NativeIoRing>),
    Threadpool,
}

/// Windows IoRing Kernel Disk I/O Engine with high-performance threadpool fallback.
pub struct IoRingBatchReader {
    queue_size: u32,
    backend: Backend,
}

impl IoRingBatchReader {
    /// Creates a new `IoRingBatchReader`.
    /// On Windows 11 build >= 22000, attempts to initialize the native kernel IoRing engine.
    /// If unavailable or on non-Windows platforms, cleanly falls back to the parallel threadpool engine.
    pub fn new(queue_size: u32) -> std::io::Result<Self> {
        Self::new_internal(queue_size, false)
    }

    /// Internal constructor allowing explicit selection of threadpool fallback (e.g. for testing).
    pub fn new_with_fallback(queue_size: u32, force_threadpool: bool) -> std::io::Result<Self> {
        Self::new_internal(queue_size, force_threadpool)
    }

    fn new_internal(queue_size: u32, force_threadpool: bool) -> std::io::Result<Self> {
        let queue_size = queue_size.max(16);

        #[cfg(windows)]
        {
            if !force_threadpool {
                if let Some(api) = windows_native::load_ioring_api() {
                    if let Some(native_ring) = windows_native::NativeIoRing::create(api, queue_size) {
                        return Ok(IoRingBatchReader {
                            queue_size: native_ring.queue_size,
                            backend: Backend::Kernel(std::sync::Mutex::new(native_ring)),
                        });
                    }
                }
            }
        }

        Ok(IoRingBatchReader {
            queue_size,
            backend: Backend::Threadpool,
        })
    }

    /// Returns `true` if the native Windows kernel IoRing engine is active,
    /// or `false` if the threadpool fallback engine is being used.
    pub fn is_kernel_ioring_active(&self) -> bool {
        #[cfg(windows)]
        {
            matches!(self.backend, Backend::Kernel(_))
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Returns the configured queue size.
    pub fn queue_size(&self) -> u32 {
        self.queue_size
    }

    /// Reads a batch of files asynchronously / concurrently and returns their byte contents
    /// preserving the exact input order.
    pub fn read_files_batched(&self, paths: &[PathBuf]) -> std::io::Result<Vec<Vec<u8>>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        #[cfg(windows)]
        {
            if let Backend::Kernel(mutex) = &self.backend {
                return self.read_files_kernel_ioring(mutex, paths);
            }
        }

        self.read_files_threadpool(paths)
    }

    /// High-throughput parallel read using Rayon threadpool.
    pub fn read_files_threadpool(&self, paths: &[PathBuf]) -> std::io::Result<Vec<Vec<u8>>> {
        use rayon::prelude::*;
        paths
            .par_iter()
            .map(|path| std::fs::read(path))
            .collect::<std::io::Result<Vec<Vec<u8>>>>()
    }

    #[cfg(windows)]
    fn read_files_kernel_ioring(
        &self,
        mutex: &std::sync::Mutex<windows_native::NativeIoRing>,
        paths: &[PathBuf],
    ) -> std::io::Result<Vec<Vec<u8>>> {
        use windows_sys::Win32::Storage::FileSystem::{
            IORING_BUFFER_REF, IORING_BUFFER_REF_0, IORING_CQE, IORING_HANDLE_REF,
            IORING_HANDLE_REF_0,
        };

        let ring = mutex.lock().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("IoRing mutex poisoned: {}", e),
            )
        })?;

        let chunk_limit = (ring.queue_size as usize).max(1);
        let mut results = Vec::with_capacity(paths.len());

        for chunk in paths.chunks(chunk_limit) {
            // 1. Open all files in this chunk and retrieve metadata
            let mut opened_files = Vec::with_capacity(chunk.len());
            for path in chunk {
                let file = std::fs::File::open(path)?;
                let meta = file.metadata()?;
                if meta.is_dir() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Path is a directory: {}", path.display()),
                    ));
                }
                let len = meta.len();
                if len > u32::MAX as u64 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("File {} exceeds max 4GB IoRing buffer limit", path.display()),
                    ));
                }
                opened_files.push((file, len));
            }

            // 2. Allocate receive buffers
            // Note: allocate at least 1 byte so buffer address is non-null for 0-byte reads
            let mut chunk_buffers: Vec<Vec<u8>> = opened_files
                .iter()
                .map(|(_, len)| vec![0u8; (*len as usize).max(1)])
                .collect();

            // 3. Build SQEs for each file
            for (i, (file, len)) in opened_files.iter().enumerate() {
                let h_file = windows_native::to_windows_handle(file);
                let file_ref = IORING_HANDLE_REF {
                    Kind: 0, // IORING_REF_RAW
                    Handle: IORING_HANDLE_REF_0 { Handle: h_file },
                };
                let data_ref = IORING_BUFFER_REF {
                    Kind: 0, // IORING_REF_RAW
                    Buffer: IORING_BUFFER_REF_0 {
                        Address: chunk_buffers[i].as_mut_ptr() as *mut core::ffi::c_void,
                    },
                };

                let hr = unsafe {
                    (ring.api.build_read_file)(
                        ring.handle,
                        file_ref,
                        data_ref,
                        *len as u32,
                        0, // fileOffset
                        i, // userData = index in chunk
                        0, // sqeFlags
                    )
                };

                if hr != 0 {
                    return Err(windows_native::hresult_to_io_error(hr));
                }
            }

            // 4. Submit ring
            let count = opened_files.len() as u32;
            let mut submitted: u32 = 0;
            let hr = unsafe {
                (ring.api.submit_ioring)(ring.handle, count, 30_000, &mut submitted)
            };
            if hr != 0 {
                return Err(windows_native::hresult_to_io_error(hr));
            }

            // 5. Pop completions (draining all submitted entries before error return)
            let mut first_error: Option<std::io::Error> = None;
            for _ in 0..count {
                let mut cqe = unsafe { std::mem::zeroed::<IORING_CQE>() };
                let hr = unsafe {
                    (ring.api.pop_completion)(ring.handle, &mut cqe)
                };
                if hr != 0 {
                    if first_error.is_none() {
                        first_error = Some(windows_native::hresult_to_io_error(hr));
                    }
                    continue;
                }
                if cqe.ResultCode < 0 {
                    if first_error.is_none() {
                        first_error = Some(windows_native::hresult_to_io_error(cqe.ResultCode));
                    }
                    continue;
                }

                let idx = cqe.UserData;
                if idx < chunk_buffers.len() {
                    let actual_bytes = cqe.Information;
                    let expected_len = opened_files[idx].1 as usize;
                    let final_len = actual_bytes.min(expected_len);
                    chunk_buffers[idx].truncate(final_len);
                }
            }

            if let Some(err) = first_error {
                return Err(err);
            }

            results.extend(chunk_buffers);
        }

        Ok(results)
    }
}
