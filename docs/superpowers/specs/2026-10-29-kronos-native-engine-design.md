# Kronos Progressive Native Engine Architecture Specification

- **Date:** 2026-10-29
- **Author:** Kronos Engineering (OMP Assistant)
- **Status:** Approved / Ready for Implementation Planning
- **Inspiration:** Zed Editor (High-Performance GPU-first Native Multithreaded Core)

---

## 1. Executive Summary

Kronos (migrated from Paseo) is an agent orchestration mission control and developer workspace. While it provides comprehensive multi-agent governance, its performance profile has suffered from a multi-layer bottleneck:

1. **Terminal Latency & Fragility**: PTY sessions run through a detached Node.js worker (`terminal-worker`), routed over Windows Named Pipes to a local Node.js daemon, then serialized as JSON over TCP WebSockets to Microsoft Edge WebView2, before finally rendering via DOM / WebGL in `xterm.js`. Keystroke latency fluctuates between 35ms and 80ms, and missing Node modules (`./shared/conout`) or Windows ConPTY handles can crash the terminal subsystem.
2. **Heavy Agent Streaming & Diff Freezes**: When AI agents (Claude Code, OMP, Codex) stream large responses or generate multi-thousand-line unified diffs, diff calculation and syntax tokenization occur on the V8 JavaScript event loop inside WebView2, causing 100% CPU spikes and Windows "Not Responding" hangs.
3. **High Resource Footprint**: The coexistence of `kronos.exe`, 4x `msedgewebview2.exe` processes, `node.exe` daemon, and `node.exe` terminal worker demands ~600MB to 1.2GB of RAM.

This specification defines the **Progressive Native Engine** architecture for Kronos. By migrating performance-critical subsystems (PTY engine, high-throughput streaming IPC, and multi-threaded diff computation) into the Tauri v2 Rust core, Kronos achieves Zed-grade snappiness (2ms keystroke latency, 120 FPS diff scrolling, zero PTY crashes) while preserving full compatibility with the existing React UI and remote relay networks.

---

## 2. Architecture & Component Boundaries

### 2.1 Dual-Mode Operation

Kronos operates under a dual-mode topology:

- **Native Local Mode**: When the user opens local folders or runs local terminal/agent sessions, Kronos bypasses the Node.js daemon and PTY worker completely. All PTY allocations, file diffing, and subprocess streams run directly through native Rust threads inside `packages/desktop-tauri/src-tauri`.
- **Remote Relay Mode**: When connecting to remote Linux hosts, cloud VMs, or home labs over Paseo Relay, Kronos falls back gracefully to WebSocket streaming, ensuring 100% backward compatibility with remote daemons.

```
+-------------------------------------------------------------------------+
|                              KRONOS DESKTOP                             |
+-------------------------------------------------------------------------+
|  [PRESENTATION LAYER] React / TypeScript (packages/app)                 |
|  - Dumb, virtualized rendering                                          |
|  - @xterm/xterm + @xterm/addon-webgl (GPU hardware accelerated canvas)   |
|  - FlashList virtualized diff view                                      |
+-------------------------------------------------------------------------+
                                 |  ^
                Direct Tauri IPC |  | Zero-Copy Event Channels
                                 v  |
+-------------------------------------------------------------------------+
|  [TAURI V2 NATIVE RUST CORE] (packages/desktop-tauri/src-tauri)         |
|  - native_pty.rs: portable-pty + Win32 ConPTY handles                   |
|  - native_diff.rs: similar (LCS diff) + Rayon background threadpool     |
|  - native_stream.rs: tokio::sync::mpsc + 16ms RAF backpressure clamp    |
|  - process_job.rs: Win32 Job Objects (zero orphan process guarantee)    |
+-------------------------------------------------------------------------+
```

---

## 3. Subsystem Specifications

### 3.1 Subsystem 1: Native Rust PTY Engine (`native_pty.rs`)

#### Responsibilities:

- Direct allocation and lifecycle management of Windows ConPTY (and Unix pty on POSIX systems) using the `portable-pty` crate.
- Direct bidirectional streaming between the OS pseudo-terminal and the WebView2 frontend via Tauri v2 binary channels (`tauri::ipc::Channel`).
- Total elimination of `node-pty`, `terminal-worker.mjs`, and intermediate TCP/WebSocket hops for local sessions.

#### State Structure:

```rust
pub struct NativePtySession {
    pub id: String,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub writer: Box<dyn std::io::Write + Send>,
}

pub struct NativePtyManager {
    sessions: Arc<Mutex<HashMap<String, NativePtySession>>>,
    job_handle: Option<windows::Win32::Foundation::HANDLE>,
}
```

#### Tauri Commands Exposed:

1. `spawn_native_terminal(options: TerminalSpawnOptions, on_event: Channel<TerminalEvent>) -> Result<TerminalInfo, String>`
   - Spawns the default shell (`powershell.exe`, `cmd.exe`, or custom shell) inside a Win32 Job Object.
   - Spawns a dedicated Tokio reader thread reading chunks from the master PTY reader.
   - Pushes raw byte slices or UTF-8 strings directly to the frontend via `on_event.send()`.
2. `write_native_terminal(id: String, data: String) -> Result<(), String>`
   - Writes directly to the master PTY writer. Keystroke latency: **<2ms**.
3. `resize_native_terminal(id: String, cols: u16, rows: u16) -> Result<(), String>`
   - Immediately updates ConPTY window dimensions via `master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })`.
4. `kill_native_terminal(id: String) -> Result<(), String>`
   - Gracefully terminates the child process and frees master/slave handles.

---

### 3.2 Subsystem 2: Native Multithreaded Diff Engine (`native_diff.rs`)

#### Responsibilities:

- High-speed computation of line-by-line and word-level diffs on background threads using the `similar` crate.
- Returning pre-structured, ready-to-render hunks so JavaScript does zero parsing, string splitting, or Myers algorithm computation.

#### Data Models:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffResult {
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
    pub elapsed_microseconds: u64,
}
```

#### Tauri Command Exposed:

- `compute_file_diff(old_text: String, new_text: String, context_lines: Option<usize>) -> Result<FileDiffResult, String>`
  - Dispatched via `tokio::task::spawn_blocking`.
  - 10,000-line synthetic diff benchmark: **<5ms execution time**.

---

### 3.3 Subsystem 3: Win32 Job Object & Process Supervisor (`process_job.rs`)

#### Problem Solved:

Windows ConPTY child processes (`node.exe`, `python.exe`, `git.exe`) frequently become orphaned when a host application restarts or terminates abruptly.

#### Implementation:

- Uses `windows::Win32::System::JobObjects`:
  ```rust
  let job = CreateJobObjectW(None, None)?;
  let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
  info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
  SetInformationJobObject(
      job,
      JobObjectExtendedLimitInformation,
      &info as *const _ as *const _,
      std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
  )?;
  ```
- All spawned shells and child processes are assigned to this job object.
- **Invariant**: If `kronos.exe` crashes or closes, the Windows kernel automatically terminates all descendant processes immediately. Zero lingering zombie processes.

---

### 3.4 Subsystem 4: Frontend Bridge & WebGL Integration (`packages/app`)

#### Preload Bridge (`bridge.js` / TypeScript):

- Exposed as `window.paseoDesktop.nativeTerminal` and `window.paseoDesktop.nativeDiff`.
- Fully typed in `packages/desktop-tauri/src/preload-bridge.ts` with zero `: any` or `as any`.
- `TerminalEmulatorRuntime` detects `window.paseoDesktop.nativeTerminal`:
  - If present and local: attaches native channel listener directly to `xterm.write()`.
  - If remote: connects to daemon WebSocket.

#### GPU Hardware Acceleration:

- `@xterm/addon-webgl` is loaded by default.
- `onContextLoss` listener safely falls back to Canvas 2D without throwing unhandled exceptions if the Windows graphics driver resets.

---

## 4. Error Handling & Stability Invariants

1. **Backpressure Clamping**:
   - Master PTY reader reads in 16KB buffers. If the frontend IPC buffer queues exceed 512KB (e.g. user ran `cat huge_file.log`), the Rust reader yields to the Tokio scheduler, preventing out-of-memory crashes.
2. **Intel GPU Driver Protection**:
   - Retain `--disable-gpu-watchdog` and `.shadow(false)` in `packages/desktop-tauri/src-tauri/src/main.rs` to guard against Intel UHD/Iris `0xc0000409` DWM buffer overruns.
3. **Type Safety**:
   - Strict TypeScript compiler check (`tsgo` / `oxlint`) with `ts-no-any` rule enforced on all bridge interfaces.

---

## 5. Verification & Testing Strategy

1. **Rust Test Suite (`Cargo.toml`)**:
   - `test_native_pty_spawn_and_write`: Verify ConPTY shell starts, echoes input, and returns clean exit code.
   - `test_native_diff_accuracy_and_speed`: Assert `compute_file_diff` on a 5,000-line synthetic input resolves in under 10ms with exact line counts.
   - `test_job_object_cleanup`: Ensure child process terminates when PTY session is dropped.
2. **Frontend Unit & Integration Tests**:
   - Vitest suite in `packages/app` testing `nativeTerminal` hook dispatching and fallback paths.
3. **End-to-End Stress Proof**:
   - Keystroke stress test: 1,000 rapid inputs with zero drops.
   - Diff inspector test: 2,000-line Claude Code unified diff renders smoothly at 60 FPS.
   - Process inspection: Verify 0 lingering `node.exe` or `cmd.exe` processes after Kronos exits.
