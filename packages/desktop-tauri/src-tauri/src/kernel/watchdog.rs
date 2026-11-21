use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

static LAST_FRONTEND_HEARTBEAT: AtomicU64 = AtomicU64::new(0);
static WATCHDOG_RUNNING: AtomicBool = AtomicBool::new(false);
static STALL_REPORTED: AtomicBool = AtomicBool::new(false);

/// Rolling reload counter — resets to 0 when RELOAD_WINDOW_START_MS is more
/// than 5 minutes old.
static RELOAD_COUNT: AtomicU32 = AtomicU32::new(0);
static RELOAD_WINDOW_START_MS: AtomicU64 = AtomicU64::new(0);

/// Maximum reloads allowed within a 5-minute window before the watchdog stops
/// attempting further reloads (prevents infinite reload loops).
const MAX_RELOADS_PER_WINDOW: u32 = 2;
const RELOAD_WINDOW_MS: u64 = 5 * 60 * 1000; // 5 minutes

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
        // Heartbeat restored — reset reload counter
        RELOAD_COUNT.store(0, Ordering::SeqCst);
    }
}

pub fn get_last_heartbeat_ms() -> u64 {
    LAST_FRONTEND_HEARTBEAT.load(Ordering::SeqCst)
}

/// Returns true if another reload is allowed within the rolling window.
fn reload_allowed() -> bool {
    let now = current_time_ms();
    let window_start = RELOAD_WINDOW_START_MS.load(Ordering::SeqCst);

    // If the window has expired, start a new one.
    if now.saturating_sub(window_start) > RELOAD_WINDOW_MS {
        RELOAD_WINDOW_START_MS.store(now, Ordering::SeqCst);
        RELOAD_COUNT.store(0, Ordering::SeqCst);
    }

    let count = RELOAD_COUNT.load(Ordering::SeqCst);
    if count >= MAX_RELOADS_PER_WINDOW {
        return false;
    }
    RELOAD_COUNT.fetch_add(1, Ordering::SeqCst);
    true
}

/// Try to fix a window that's visible but has degenerate (tiny) dimensions.
fn recover_window_size<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>) {
    let size = match win.inner_size() {
        Ok(s) => s,
        Err(_) => return,
    };
    if size.width > 0 && size.width < 100 || size.height > 0 && size.height < 100 {
        eprintln!(
            "[watchdog] Window has degenerate size {}x{}, forcing restore",
            size.width, size.height
        );
        let _ = win.set_size(tauri::LogicalSize::new(1280.0, 800.0));
        let _ = win.center();
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
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
                let win = match app_handle.get_webview_window("main") {
                    Some(w) => w,
                    None => continue,
                };
                let is_window_visible = win.is_visible().unwrap_or(false);

                // Only flag stalls if window is visible (backgrounded/minimized tabs throttle timers)
                if is_window_visible && elapsed_ms > 15_000 {
                    if !STALL_REPORTED.swap(true, Ordering::SeqCst) {
                        eprintln!(
                            "[watchdog] Warning: Frontend UI heartbeat missing for {} ms",
                            elapsed_ms
                        );
                    }

                    // Check for degenerate window size (crash symptom)
                    recover_window_size(&win);

                    // Critical stall (> 45s while visible): attempt recovery
                    if elapsed_ms > 45_000 {
                        // Before reloading, try showing + focusing first — the window
                        // might just be hidden or in a bad state.
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();

                        if reload_allowed() {
                            eprintln!(
                                "[watchdog] Critical frontend freeze detected ({}ms). Triggering safe reload...",
                                elapsed_ms
                            );
                            // Reset heartbeat with fresh 20s grace period
                            LAST_FRONTEND_HEARTBEAT.store(current_time_ms() + 20_000, Ordering::SeqCst);
                            let _ = win.eval("window.location.reload();");
                        } else {
                            eprintln!(
                                "[watchdog] Critical freeze persists but reload limit ({}) reached. \
                                 Stopping further reloads to prevent loop.",
                                MAX_RELOADS_PER_WINDOW
                            );
                            // Reset heartbeat so we don't spam this message every 5s
                            LAST_FRONTEND_HEARTBEAT.store(current_time_ms() + 60_000, Ordering::SeqCst);
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

    #[test]
    fn test_reload_allowed_first_time() {
        // Reset state for test
        RELOAD_COUNT.store(0, Ordering::SeqCst);
        RELOAD_WINDOW_START_MS.store(0, Ordering::SeqCst);
        assert!(reload_allowed());
        assert!(reload_allowed());
        // Third call should be denied (MAX_RELOADS_PER_WINDOW == 2)
        assert!(!reload_allowed());
    }
}
