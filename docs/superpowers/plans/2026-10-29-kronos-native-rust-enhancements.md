# Kronos Native Rust Performance & Reliability Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 4 core native Rust enhancements in Tauri v2 to elevate Kronos performance and reliability: native secure attachment storage (eliminating paste/drop upload errors), live daemon lifecycle control (real native restart and stop), native Win32 system idle detection, and sub-millisecond fast path inspection, with 100% backward compatibility and zero regressions.

**Architecture:**

1. **Native Attachment Engine (`attachments.rs`)**: Implement `write_attachment_base64`, `write_attachment_bytes`, `copy_attachment_file`, `read_file_base64`, `delete_attachment_file`, and `garbage_collect_attachment_files` in Rust with strict directory sandboxing under `~/.paseo/desktop-attachments`, eliminating attachment drop/paste failures in desktop chat composer.
2. **Native Daemon Lifecycle Control (`daemon.rs` / `main.rs`)**: Connect the desktop bridge commands `stop_desktop_daemon` and `restart_desktop_daemon` to the native Rust process controller in `daemon.rs` instead of static mocks.
3. **Native Win32 System Idle Detection**: Implement zero-overhead Win32 `GetLastInputInfo` in Rust to measure true hardware idle time for auto-hibernation and presence synchronization.
4. **Native Fast Path Inspection (`fast_inspect_path`)**: Expose sub-millisecond filesystem inspection via Tauri IPC to avoid redundant HTTP roundtrips during project discovery.

**Tech Stack:** Rust (Tauri v2, Tokio, Serde, Win32 API), React 19, TypeScript, Vitest, Cargo.

**Spec:** High-performance native Rust core enhancements for Kronos desktop.

## Global Constraints

- Retain Industrial Brutalism aesthetics: CRT substrate `#0A0A0A`, text `#EAEAEA`, hazard accent `#FF2A2A`, strict 90° corners (`rounded-none`).
- Strict TypeScript: no `: any` or `as any` (`ts-no-any`).
- Zero regressions: all existing bridge contracts (`window.paseoDesktop.invoke`) must maintain exact payload structures and error semantics.
- Path sandboxing: all attachment storage operations must be strictly bounded within `~/.paseo/desktop-attachments` to eliminate path traversal vulnerabilities.
- All Cargo tests and Vitest suites must pass.

---

### Task 1: Native Rust Attachment Storage Engine

**Files:**

- Create: `packages/desktop-tauri/src-tauri/src/attachments.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:1-40,470-525,640-660`
- Test: Unit tests within `packages/desktop-tauri/src-tauri/src/attachments.rs`

**Interfaces:**

- Consumes: `desktop-attachments` path under `dirs::home_dir()/.paseo`.
- Produces:
  - `write_attachment_base64(attachment_id: String, base64: String, extension: Option<String>) -> Result<AttachmentFileResult, String>`
  - `write_attachment_bytes(attachment_id: String, bytes: Vec<u8>, extension: Option<String>) -> Result<AttachmentFileResult, String>`
  - `copy_attachment_file(attachment_id: String, source_path: String, extension: Option<String>) -> Result<AttachmentFileResult, String>`
  - `read_file_base64(path: String) -> Result<String, String>`
  - `delete_attachment_file(path: String) -> Result<bool, String>`
  - `garbage_collect_attachment_files(referenced_ids: Vec<String>) -> Result<usize, String>`

- [ ] **Step 1: Write `attachments.rs` with sandboxed storage, base64 decode, and unit tests**
- [ ] **Step 2: Expose attachment commands in `main.rs` invoke handler and bridge script**
- [ ] **Step 3: Run `cargo test` to verify attachment tests pass**

---

### Task 2: Live Daemon Lifecycle Control (Native Stop & Restart in Rust)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/daemon.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:455-465,640-660`

**Interfaces:**

- Consumes: `DaemonState` Mutex from `main.rs`.
- Produces:
  - `#[tauri::command] restart_desktop_daemon(state: tauri::State<DaemonState>) -> Result<serde_json::Value, String>`
  - `#[tauri::command] stop_desktop_daemon(state: tauri::State<DaemonState>) -> Result<serde_json::Value, String>`

- [ ] **Step 1: Implement `restart_desktop_daemon` and `stop_desktop_daemon` Tauri commands**
- [ ] **Step 2: Connect bridge script commands `restart_desktop_daemon` and `stop_desktop_daemon` to invoke native commands**
- [ ] **Step 3: Verify clean process termination and port recycling**

---

### Task 3: Native Win32 System Idle Detection

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:180-220,505-515,640-660`

**Interfaces:**

- Produces:
  - `#[tauri::command] desktop_get_system_idle_time() -> u64` (milliseconds)

- [ ] **Step 1: Implement `get_system_idle_time_ms()` using Win32 `GetLastInputInfo` and fallback on non-Windows**
- [ ] **Step 2: Wire `desktop_get_system_idle_time` in bridge script to call native Tauri command**
- [ ] **Step 3: Write unit test validating non-negative idle duration**

---

### Task 4: Native Fast Path Inspection (`fast_inspect_path`)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:200-245,640-660`

**Interfaces:**

- Produces:
  - `#[tauri::command] fast_inspect_path(path: String) -> Result<PathInspectResult, String>`
  - Returns `exists`, `is_dir`, `is_file`, `byte_size`, `modified_ms`

- [ ] **Step 1: Implement `fast_inspect_path` in `main.rs`**
- [ ] **Step 2: Expose in invoke handler and bridge script**
- [ ] **Step 3: Write unit tests covering existing directories, files, and nonexistent paths**

---

### Task 5: End-to-End Verification & Zero-Regression Smoke Test

**Files:**

- Verify: Full compilation of `packages/desktop-tauri/src-tauri` via `cargo test` and `cargo check`.
- Verify: Frontend tests in `packages/app`.

- [ ] **Step 1: Run all Cargo unit tests**
- [ ] **Step 2: Run frontend test suites**
- [ ] **Step 3: Verify build integrity**
