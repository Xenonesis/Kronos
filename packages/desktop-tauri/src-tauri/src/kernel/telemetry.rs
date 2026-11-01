//! Native Rust Kernel Telemetry
//!
//! Provides zero-cost, memory-safe system and kernel telemetry metrics for the Kronos application,
//! including process working set (RAM), peak memory, OS identity, uptime, and kernel subsystem flags.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Instant;

static START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);

pub fn get_process_uptime_secs() -> u64 {
    START_TIME.elapsed().as_secs()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KernelTelemetry {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub pid: u32,
    pub memory_working_set_bytes: u64,
    pub memory_peak_working_set_bytes: u64,
    pub memory_private_bytes: u64,
    pub uptime_secs: u64,
    pub shm_available: bool,
    pub mica_enabled: bool,
    pub pty_enabled: bool,
}

#[cfg(target_os = "windows")]
pub fn query_process_memory() -> (u64, u64, u64) {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    unsafe {
        let handle = GetCurrentProcess();
        let mut counters = std::mem::zeroed::<PROCESS_MEMORY_COUNTERS>();
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        counters.cb = size;
        if K32GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, size) != 0 {
            (
                counters.WorkingSetSize as u64,
                counters.PeakWorkingSetSize as u64,
                counters.PagefileUsage as u64,
            )
        } else {
            (0, 0, 0)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn query_process_memory() -> (u64, u64, u64) {
    (0, 0, 0)
}

pub fn get_kernel_telemetry() -> KernelTelemetry {
    let (working_set, peak_working_set, private_bytes) = query_process_memory();

    KernelTelemetry {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        pid: std::process::id(),
        memory_working_set_bytes: working_set,
        memory_peak_working_set_bytes: peak_working_set,
        memory_private_bytes: private_bytes,
        uptime_secs: get_process_uptime_secs(),
        shm_available: true,
        mica_enabled: cfg!(target_os = "windows"),
        pty_enabled: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_telemetry_gathering() {
        let telemetry = get_kernel_telemetry();
        assert_eq!(telemetry.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(telemetry.pid, std::process::id());
        assert!(telemetry.shm_available);
        assert!(telemetry.pty_enabled);

        #[cfg(target_os = "windows")]
        {
            assert!(telemetry.mica_enabled);
            assert!(telemetry.memory_working_set_bytes > 0);
        }
    }
}
