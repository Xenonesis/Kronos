//! Native Rust Kernel Telemetry
//!
//! Provides zero-cost, memory-safe system and kernel telemetry metrics for the Kronos application,
//! including process working set (RAM), peak memory, OS identity, uptime, and kernel subsystem flags.

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Instant;

static START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);
static TRACING_GUARD: std::sync::OnceLock<tracing_appender::non_blocking::WorkerGuard> =
    std::sync::OnceLock::new();

pub fn init_tracing(startup_trace: bool) {
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Kronos")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "kronos.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = TRACING_GUARD.set(guard);

    let default_filter = if startup_trace {
        "info,kronos=debug,startup=debug"
    } else {
        "info"
    };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .finish();

    let _ = tracing::subscriber::set_global_default(subscriber);
}

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

/// Parses Linux `/proc/self/status` content into `(working_set_bytes, peak_working_set_bytes, private_bytes)`.
pub fn parse_proc_status_memory(status_text: &str) -> (u64, u64, u64) {
    let mut vm_rss_kb = 0u64;
    let mut vm_hwm_kb = 0u64;
    let mut rss_anon_kb = 0u64;
    let mut vm_data_kb = 0u64;

    for line in status_text.lines() {
        let mut parts = line.split_whitespace();
        let Some(key) = parts.next() else {
            continue;
        };
        let Some(val_str) = parts.next() else {
            continue;
        };
        let Ok(val_kb) = val_str.parse::<u64>() else {
            continue;
        };
        match key {
            "VmRSS:" => vm_rss_kb = val_kb,
            "VmHWM:" => vm_hwm_kb = val_kb,
            "RssAnon:" => rss_anon_kb = val_kb,
            "VmData:" => vm_data_kb = val_kb,
            _ => {}
        }
    }

    let working_set = vm_rss_kb * 1024;
    let peak_working_set = vm_hwm_kb.max(vm_rss_kb) * 1024;
    let private_bytes = if rss_anon_kb > 0 {
        rss_anon_kb * 1024
    } else {
        vm_data_kb.max(vm_rss_kb) * 1024
    };
    (working_set, peak_working_set, private_bytes)
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

#[cfg(target_os = "linux")]
pub fn query_process_memory() -> (u64, u64, u64) {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        let parsed = parse_proc_status_memory(&status);
        if parsed.0 > 0 {
            return parsed;
        }
    }
    if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
        let mut parts = statm.split_whitespace();
        let _total_pages = parts.next();
        if let Some(rss_pages) = parts.next().and_then(|s| s.parse::<u64>().ok()) {
            let rss_bytes = rss_pages * 4096;
            return (rss_bytes, rss_bytes, rss_bytes);
        }
    }
    (0, 0, 0)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "linux")))]
pub fn query_process_memory() -> (u64, u64, u64) {
    let pid = std::process::id().to_string();
    if let Ok(out) = std::process::Command::new("ps")
        .args(["-o", "rss=,vsz=", "-p", &pid])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut parts = text.split_whitespace();
            let rss_kb = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let vsz_kb = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(rss_kb);
            let rss_bytes = rss_kb * 1024;
            return (rss_bytes, rss_bytes, vsz_kb.min(rss_kb * 2) * 1024);
        }
    }
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
