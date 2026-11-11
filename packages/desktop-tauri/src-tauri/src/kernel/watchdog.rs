use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

static LAST_FRONTEND_HEARTBEAT: AtomicU64 = AtomicU64::new(0);
static WATCHDOG_RUNNING: AtomicBool = AtomicBool::new(false);
static STALL_REPORTED: AtomicBool = AtomicBool::new(false);

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn record_heartbeat() {
    LAST_FRONTEND_HEARTBEAT.store(current_time_ms(), Ordering::SeqCst);
    if STALL_REPORTED.swap(false, Ordering::SeqCst) {
        println!("[watchdog] Frontend heartbeat restored; UI is responsive.");
    }
}

pub fn get_last_heartbeat_ms() -> u64 {
    LAST_FRONTEND_HEARTBEAT.load(Ordering::SeqCst)
}

pub fn start_watchdog(app_handle: AppHandle) {
    if WATCHDOG_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    // Initialize with current time + grace period of 20 seconds for cold app load
    LAST_FRONTEND_HEARTBEAT.store(current_time_ms() + 20_000, Ordering::SeqCst);

    std::thread::Builder::new()
        .name("kronos-watchdog".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(5));

                let now = current_time_ms();
                let last = LAST_FRONTEND_HEARTBEAT.load(Ordering::SeqCst);

                // If grace period is active, continue
                if now < last {
                    continue;
                }

                let elapsed_ms = now.saturating_sub(last);

                // Check if main window is visible
                let is_window_visible = app_handle
                    .get_webview_window("main")
                    .and_then(|w| w.is_visible().ok())
                    .unwrap_or(false);

                // Only flag stalls if window is visible (backgrounded/minimized tabs throttle timers)
                if is_window_visible && elapsed_ms > 25_000 {
                    if !STALL_REPORTED.swap(true, Ordering::SeqCst) {
                        eprintln!(
                            "[watchdog] Warning: Frontend UI heartbeat missing for {} ms",
                            elapsed_ms
                        );
                    }

                    // Critical stall (> 60s while visible): trigger safe reload
                    if elapsed_ms > 60_000 {
                        eprintln!(
                            "[watchdog] Critical frontend freeze detected (> 60s). Triggering safe reload..."
                        );
                        // Reset heartbeat with fresh 15s grace period
                        LAST_FRONTEND_HEARTBEAT.store(current_time_ms() + 15_000, Ordering::SeqCst);
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let _ = win.eval("window.location.reload();");
                        }
                    }
                }
            }
        })
        .expect("Failed to spawn kronos watchdog thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_heartbeat() {
        let before = current_time_ms();
        record_heartbeat();
        let hb = get_last_heartbeat_ms();
        assert!(hb >= before);
        assert!(!STALL_REPORTED.load(Ordering::SeqCst));
    }
}
