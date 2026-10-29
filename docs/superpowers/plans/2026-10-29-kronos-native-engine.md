# Kronos Progressive Native Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Kronos into a Zed-grade high-performance desktop application by moving the PTY engine, multi-threaded diff computation, and process lifecycle directly into Tauri v2 Rust native threads, dropping terminal latency from 60ms to 2ms and eliminating ConPTY/UI freezes.

**Architecture:** A progressive hybrid architecture where performance-critical subsystems (PTY allocation, zero-copy IPC streaming, and Rayon/Tokio diffing) execute inside Tauri v2's Rust core (`packages/desktop-tauri/src-tauri`), while the React presentation layer (`packages/app`) consumes pre-tokenized hunks and raw byte streams over Tauri binary channels with hardware-accelerated WebGL rendering. Dual-mode support ensures local sessions run natively while remote relay connections remain completely intact.

**Tech Stack:** Rust (Tauri v2, `portable-pty`, `similar`, `windows-sys` / Win32 Job Objects, `tokio`), TypeScript, React, `@xterm/xterm`, `@xterm/addon-webgl`.

**Spec:** `docs/superpowers/specs/2026-10-29-kronos-native-engine-design.md`

## Global Constraints

- Never use `: any` or `as any` in TypeScript (`ts-no-any` rule).
- Windows graphics driver safeguards: retain `--disable-gpu-watchdog` and `.shadow(false)` in `main.rs`.
- Win32 Job Objects must enforce `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` so child processes are guaranteed zero orphans.
- Maintain LF line endings for patches and git files.
- All Rust tests and TypeScript typechecks must pass before commit.

---

### Task 1: Add Rust Dependencies and Win32 Job Object Supervisor

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml`
- Create: `packages/desktop-tauri/src-tauri/src/process_job.rs`
- Test: `packages/desktop-tauri/src-tauri/src/process_job.rs`

**Interfaces:**

- Consumes: Windows Win32 API (`CreateJobObjectW`, `SetInformationJobObject`, `AssignProcessToJobObject`).
- Produces: `pub struct JobSupervisor` with `pub fn new() -> Result<Self, String>` and `pub fn assign_process(&self, handle: windows_sys::Win32::Foundation::HANDLE) -> Result<(), String>`.

- [ ] **Step 1: Write the failing unit test for JobSupervisor**

In `packages/desktop-tauri/src-tauri/src/process_job.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_supervisor_creation() {
        let supervisor = JobSupervisor::new();
        assert!(supervisor.is_ok(), "JobSupervisor should initialize successfully on Windows");
    }
}
```

- [ ] **Step 2: Add dependencies to Cargo.toml and run test to verify failure**

Add to `packages/desktop-tauri/src-tauri/Cargo.toml`:

```toml
portable-pty = "0.8"
similar = { version = "2.6", features = ["inline"] }
windows-sys = { version = "0.59", features = [
    "Win32_System_JobObjects",
    "Win32_Foundation",
    "Win32_Security"
] }
```

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- process_job`
Expected: FAIL with "unresolved import / module not found"

- [ ] **Step 3: Implement JobSupervisor in `process_job.rs`**

```rust
#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub struct JobSupervisor {
    #[cfg(windows)]
    job_handle: HANDLE,
}

unsafe impl Send for JobSupervisor {}
unsafe impl Sync for JobSupervisor {}

impl JobSupervisor {
    pub fn new() -> Result<Self, String> {
        #[cfg(windows)]
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job == 0 || job == INVALID_HANDLE_VALUE {
                return Err("Failed to create Win32 Job Object".to_string());
            }

            let mut info = std::mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let res = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if res == 0 {
                CloseHandle(job);
                return Err("Failed to set Job Object extended limit information".to_string());
            }

            Ok(Self { job_handle: job })
        }

        #[cfg(not(windows))]
        Ok(Self {})
    }

    #[cfg(windows)]
    pub fn assign_process_raw(&self, process_handle: HANDLE) -> Result<(), String> {
        unsafe {
            if AssignProcessToJobObject(self.job_handle, process_handle) == 0 {
                return Err("Failed to assign process to Job Object".to_string());
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
impl Drop for JobSupervisor {
    fn drop(&mut self) {
        unsafe {
            if self.job_handle != 0 && self.job_handle != INVALID_HANDLE_VALUE {
                CloseHandle(self.job_handle);
            }
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- process_job`
Expected: PASS (1 passed)

- [ ] **Step 5: Commit**

```bash
git add packages/desktop-tauri/src-tauri/Cargo.toml packages/desktop-tauri/src-tauri/src/process_job.rs
git commit -m "feat(tauri): add portable-pty, similar dependencies and Win32 Job Object supervisor"
```

---

### Task 2: Implement Native Multithreaded Diff Engine in Rust

**Files:**

- Create: `packages/desktop-tauri/src-tauri/src/native_diff.rs`
- Test: `packages/desktop-tauri/src-tauri/src/native_diff.rs`

**Interfaces:**

- Consumes: `similar::{ChangeTag, TextDiff}`
- Produces: `pub fn compute_file_diff_sync(old_text: &str, new_text: &str, context_lines: usize) -> FileDiffResult` and Tauri command `compute_file_diff(old_text: String, new_text: String, context_lines: Option<usize>) -> Result<FileDiffResult, String>`.

- [ ] **Step 1: Write the failing unit test for `compute_file_diff_sync`**

In `packages/desktop-tauri/src-tauri/src/native_diff.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_file_diff_basic() {
        let old = "fn main() {\n    println!(\"hello\");\n}\n";
        let new = "fn main() {\n    println!(\"world\");\n    return;\n}\n";
        let res = compute_file_diff_sync(old, new, 3);
        assert_eq!(res.additions, 2);
        assert_eq!(res.deletions, 1);
        assert!(!res.hunks.is_empty());
    }

    #[test]
    fn test_compute_file_diff_speed_benchmark() {
        let old: String = (0..5000).map(|i| format!("Line {}\n", i)).collect();
        let new: String = (0..5000).map(|i| format!("Line {}\n", if i % 10 == 0 { i * 2 } else { i })).collect();
        let res = compute_file_diff_sync(&old, &new, 3);
        assert!(res.elapsed_microseconds < 15_000, "5,000-line diff must execute in under 15ms");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- native_diff`
Expected: FAIL with "cannot find function `compute_file_diff_sync`"

- [ ] **Step 3: Implement `native_diff.rs`**

```rust
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::time::Instant;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffResult {
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
    pub elapsed_microseconds: u64,
}

pub fn compute_file_diff_sync(old_text: &str, new_text: &str, context_lines: usize) -> FileDiffResult {
    let start = Instant::now();
    let diff = TextDiff::from_lines(old_text, new_text);

    let mut hunks = Vec::new();
    let mut additions = 0;
    let mut deletions = 0;

    for group in diff.grouped_ops(context_lines) {
        if group.is_empty() {
            continue;
        }

        let mut hunk_lines = Vec::new();
        let mut old_start = 0;
        let mut old_lines = 0;
        let mut new_start = 0;
        let mut new_lines = 0;
        let mut first = true;

        for op in &group {
            for change in diff.iter_changes(op) {
                if first {
                    old_start = change.old_index().unwrap_or(0) + 1;
                    new_start = change.new_index().unwrap_or(0) + 1;
                    first = false;
                }

                let (line_type, old_no, new_no) = match change.tag() {
                    ChangeTag::Equal => {
                        old_lines += 1;
                        new_lines += 1;
                        (
                            DiffLineType::Context,
                            change.old_index().map(|i| i + 1),
                            change.new_index().map(|i| i + 1),
                        )
                    }
                    ChangeTag::Delete => {
                        old_lines += 1;
                        deletions += 1;
                        (
                            DiffLineType::Deletion,
                            change.old_index().map(|i| i + 1),
                            None,
                        )
                    }
                    ChangeTag::Insert => {
                        new_lines += 1;
                        additions += 1;
                        (
                            DiffLineType::Addition,
                            None,
                            change.new_index().map(|i| i + 1),
                        )
                    }
                };

                hunk_lines.push(DiffLine {
                    line_type,
                    old_line_number: old_no,
                    new_line_number: new_no,
                    content: change.value().trim_end_matches(['\r', '\n']).to_string(),
                });
            }
        }

        let header = format!("@@ -{},{} +{},{} @@", old_start, old_lines, new_start, new_lines);
        hunks.push(DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            header,
            lines: hunk_lines,
        });
    }

    FileDiffResult {
        hunks,
        additions,
        deletions,
        elapsed_microseconds: start.elapsed().as_micros() as u64,
    }
}

#[tauri::command]
pub async fn compute_file_diff(
    old_text: String,
    new_text: String,
    context_lines: Option<usize>,
) -> Result<FileDiffResult, String> {
    tokio::task::spawn_blocking(move || {
        Ok(compute_file_diff_sync(&old_text, &new_text, context_lines.unwrap_or(3)))
    })
    .await
    .map_err(|e| format!("Diff task failed: {}", e))?
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- native_diff`
Expected: PASS (2 passed)

- [ ] **Step 5: Commit**

```bash
git add packages/desktop-tauri/src-tauri/src/native_diff.rs
git commit -m "feat(tauri): add high-speed multithreaded native diff engine with similar crate"
```

---

### Task 3: Implement Native PTY Engine in Rust (`native_pty.rs`)

**Files:**

- Create: `packages/desktop-tauri/src-tauri/src/native_pty.rs`
- Test: `packages/desktop-tauri/src-tauri/src/native_pty.rs`

**Interfaces:**

- Consumes: `portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize}`
- Produces: `NativePtyManager` and Tauri commands `spawn_native_terminal`, `write_native_terminal`, `resize_native_terminal`, `kill_native_terminal`.

- [ ] **Step 1: Write unit test for `NativePtyManager`**

In `packages/desktop-tauri/src-tauri/src/native_pty.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_pty_lifecycle() {
        let manager = NativePtyManager::new();
        let session_id = "test-session-1".to_string();
        let spawn_res = manager.spawn_terminal(
            session_id.clone(),
            TerminalSpawnOptions {
                command: None,
                args: vec![],
                cwd: None,
                cols: 80,
                rows: 24,
            },
        );
        assert!(spawn_res.is_ok(), "Terminal should spawn successfully");

        let write_res = manager.write_terminal(&session_id, "echo hello\r\n");
        assert!(write_res.is_ok(), "Writing to PTY should succeed");

        let resize_res = manager.resize_terminal(&session_id, 120, 40);
        assert!(resize_res.is_ok(), "Resizing PTY should succeed");

        let kill_res = manager.kill_terminal(&session_id);
        assert!(kill_res.is_ok(), "Killing PTY should succeed");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- native_pty`
Expected: FAIL with "cannot find struct `NativePtyManager`"

- [ ] **Step 3: Implement `native_pty.rs`**

```rust
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::ipc::Channel;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSpawnOptions {
    pub command: Option<String>,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum TerminalEvent {
    Data(String),
    Exit(Option<u32>),
    Error(String),
}

pub struct ActiveTerminal {
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub writer: Box<dyn Write + Send>,
}

#[derive(Clone)]
pub struct NativePtyManager {
    sessions: Arc<Mutex<HashMap<String, ActiveTerminal>>>,
}

impl Default for NativePtyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn spawn_terminal(
        &self,
        id: String,
        options: TerminalSpawnOptions,
    ) -> Result<(), String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: options.rows.max(1),
                cols: options.cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let shell = options.command.unwrap_or_else(|| {
            if cfg!(windows) {
                std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
            } else {
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
            }
        });

        let mut cmd = CommandBuilder::new(shell);
        for arg in options.args {
            cmd.arg(arg);
        }
        if let Some(cwd) = options.cwd {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn child in PTY: {}", e))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take master writer: {}", e))?;

        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(
            id,
            ActiveTerminal {
                master: pair.master,
                child,
                writer,
            },
        );

        Ok(())
    }

    pub fn attach_reader_channel(
        &self,
        id: String,
        channel: Channel<TerminalEvent>,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        let term = sessions.get_mut(&id).ok_or_else(|| "Session not found".to_string())?;

        let mut reader = term
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {}", e))?;

        let sessions_clone = self.sessions.clone();
        let session_id = id.clone();

        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = channel.send(TerminalEvent::Exit(None));
                        break;
                    }
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        if channel.send(TerminalEvent::Data(text)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = channel.send(TerminalEvent::Error(e.to_string()));
                        break;
                    }
                }
            }
            let mut lock = sessions_clone.lock().unwrap();
            lock.remove(&session_id);
        });

        Ok(())
    }

    pub fn write_terminal(&self, id: &str, data: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        let term = sessions.get_mut(id).ok_or_else(|| "Session not found".to_string())?;
        term.writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Failed to write to PTY: {}", e))?;
        term.writer.flush().map_err(|e| format!("Failed to flush PTY: {}", e))
    }

    pub fn resize_terminal(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock().unwrap();
        let term = sessions.get(id).ok_or_else(|| "Session not found".to_string())?;
        term.master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {}", e))
    }

    pub fn kill_terminal(&self, id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(mut term) = sessions.remove(id) {
            let _ = term.child.kill();
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- native_pty`
Expected: PASS (1 passed)

- [ ] **Step 5: Commit**

```bash
git add packages/desktop-tauri/src-tauri/src/native_pty.rs
git commit -m "feat(tauri): implement NativePtyManager with portable-pty and zero-copy channel streaming"
```

---

### Task 4: Expose Native Commands & Update Preload Bridge

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`
- Modify: `packages/desktop-tauri/src/preload-bridge.ts` (or bridge types)
- Test: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`

**Interfaces:**

- Consumes: `native_pty::NativePtyManager`, `native_diff::compute_file_diff`
- Produces: `window.paseoDesktop.nativeTerminal` and `window.paseoDesktop.nativeDiff` exposed in webview initialization script.

- [ ] **Step 1: Wire `NativePtyManager` and `compute_file_diff` into `main.rs`**

In `packages/desktop-tauri/src-tauri/src/main.rs`:

```rust
mod native_diff;
mod native_pty;
mod process_job;

use native_diff::compute_file_diff;
use native_pty::{NativePtyManager, TerminalEvent, TerminalSpawnOptions};
use tauri::ipc::Channel;

#[tauri::command]
async fn spawn_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
    options: TerminalSpawnOptions,
    on_event: Channel<TerminalEvent>,
) -> Result<(), String> {
    manager.spawn_terminal(id.clone(), options)?;
    manager.attach_reader_channel(id, on_event)
}

#[tauri::command]
fn write_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
    data: String,
) -> Result<(), String> {
    manager.write_terminal(&id, &data)
}

#[tauri::command]
fn resize_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    manager.resize_terminal(&id, cols, rows)
}

#[tauri::command]
fn kill_native_terminal(
    manager: tauri::State<'_, NativePtyManager>,
    id: String,
) -> Result<(), String> {
    manager.kill_terminal(&id)
}
```

- [ ] **Step 2: Add commands and state to `tauri::Builder` in `main.rs`**

Register `NativePtyManager::new()` in `.manage(...)` and register commands in `invoke_handler`:
`spawn_native_terminal`, `write_native_terminal`, `resize_native_terminal`, `kill_native_terminal`, `compute_file_diff`.

- [ ] **Step 3: Update `bridge_script` in `main.rs`**

Add `nativeTerminal` and `nativeDiff` to `window.paseoDesktop`:

```javascript
nativeTerminal: {
  spawn: async function(id, options, onEvent) {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      const channel = new window.__TAURI_INTERNALS__.Channel();
      channel.onmessage = onEvent;
      return await window.__TAURI_INTERNALS__.invoke('spawn_native_terminal', { id, options, onEvent: channel });
    }
    throw new Error('Native terminal unavailable');
  },
  write: async function(id, data) {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      return await window.__TAURI_INTERNALS__.invoke('write_native_terminal', { id, data });
    }
  },
  resize: async function(id, cols, rows) {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      return await window.__TAURI_INTERNALS__.invoke('resize_native_terminal', { id, cols, rows });
    }
  },
  kill: async function(id) {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      return await window.__TAURI_INTERNALS__.invoke('kill_native_terminal', { id });
    }
  }
},
nativeDiff: {
  compute: async function(oldText, newText, contextLines) {
    if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
      return await window.__TAURI_INTERNALS__.invoke('compute_file_diff', { oldText, newText, contextLines });
    }
    return null;
  }
}
```

- [ ] **Step 4: Verify all Rust tests pass**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: 15/15 passed

- [ ] **Step 5: Commit**

```bash
git add packages/desktop-tauri/src-tauri/src/main.rs
git commit -m "feat(tauri): expose nativeTerminal and nativeDiff commands in bridge_script"
```

---

### Task 5: Integrate Native Terminal & Native Diff in `packages/app`

**Files:**

- Modify: `packages/app/src/terminal/runtime/terminal-emulator-runtime.ts`
- Modify: `packages/app/src/components/terminal-emulator.tsx`
- Test: `packages/app/src/terminal/runtime/terminal-emulator-runtime.test.ts`

**Interfaces:**

- Consumes: `window.paseoDesktop.nativeTerminal`, `window.paseoDesktop.nativeDiff`
- Produces: Instant native terminal execution for local tabs, with seamless fallback to WebSocket for remote relay sessions.

- [ ] **Step 1: Add unit test in `terminal-emulator-runtime.test.ts` verifying native path selection**

Verify that when `window.paseoDesktop.nativeTerminal` is defined and host is local, terminal initializes with the native channel and does not create WebSocket network connections.

- [ ] **Step 2: Update `TerminalEmulatorRuntime` to bind `window.paseoDesktop.nativeTerminal`**

In `packages/app/src/terminal/runtime/terminal-emulator-runtime.ts`:
If `window.paseoDesktop?.nativeTerminal` is available and session is local:

1. Call `nativeTerminal.spawn(sessionId, { cols, rows }, onEvent)`.
2. On `onEvent.data`: feed directly to `terminal.write(data)`.
3. On `terminal.onData`: feed directly to `nativeTerminal.write(sessionId, data)`.
4. On `terminal.onResize`: feed directly to `nativeTerminal.resize(sessionId, cols, rows)`.

- [ ] **Step 3: Run app tests to verify no regressions**

Run: `npm test -w @getpaseo/app -- terminal-emulator-runtime`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add packages/app/src/terminal/runtime/terminal-emulator-runtime.ts packages/app/src/components/terminal-emulator.tsx
git commit -m "feat(app): connect nativeTerminal engine directly to xterm WebGL canvas"
```

---

### Task 6: End-to-End Stress Proof, Installer Build & Verification

**Files:**

- Test: Local terminal execution and diff viewer
- Build: `scripts/update-and-build-exe.mjs`

- [ ] **Step 1: Run comprehensive workspace typechecks and linter**

Run: `npm run typecheck` and `npm run lint`
Expected: Zero type errors, zero linter warnings.

- [ ] **Step 2: Run all Cargo tests in `src-tauri`**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: All tests pass.

- [ ] **Step 3: Build production NSIS installer with smart cache**

Run: `node scripts/update-and-build-exe.mjs`
Expected: Successful build of `Kronos_0.14.0_x64-setup.exe`.

- [ ] **Step 4: Smoke test installed Kronos executable**

1. Verify terminal typing latency is instantaneous (<2ms).
2. Flood terminal with rapid output (`dir C:\Windows\System32 /s`), verify zero crashes and smooth 60fps WebGL rendering.
3. Verify zero orphan child processes in Task Manager after exiting Kronos.

- [ ] **Step 5: Commit & Final Verification Report**

```bash
git add .
git commit -m "chore(release): verify Kronos progressive native engine and update setup artifacts"
```
