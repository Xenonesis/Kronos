# Tauri v2 Full Feature-Parity Specification

- **Status**: Draft / Under Review
- **Target**: `packages/desktop-tauri`
- **Goal**: Bring the Tauri v2 desktop shell to complete feature-parity with the Electron implementation (`packages/desktop`), enabling safe deprecation and removal of Electron.
- **Reference Electron Source**: `packages/desktop/src/`

---

## 1. Architecture Overview

```
+-------------------------------------------------------------------+
|                  Paseo Tauri v2 Desktop Shell                     |
|                                                                   |
|  +------------------+  +--------------------+  +---------------+  |
|  | Window & Chrome  |  | Dialogs & Menus    |  | Deep Links    |  |
|  | - Window State   |  | - File/Folder Open |  | - paseo://    |  |
|  | - Custom Titlebar|  | - Native Context   |  | - Route Dispatch|
|  +------------------+  +--------------------+  +---------------+  |
|                                                                   |
|  +------------------+  +--------------------+  +---------------+  |
|  | System Tray      |  | Auto-Updater       |  | Editor Launch |  |
|  | - Tray Icon/Menu |  | - Tauri v2 Updater |  | - VS Code, etc|  |
|  | - Status Tooltip |  | - Channel Config   |  | - Line/Col Jmp|  |
|  +------------------+  +--------------------+  +---------------+  |
+---------------------------------+---------------------------------+
                                  |
            +---------------------+---------------------+
            |                                           |
+-----------v---------------+               +-----------v---------------+
|     Native WebView2       |               |     Node.js Daemon        |
|  - React Web UI (Expo)    |   WebSocket   |  - Agent Orchestration    |
|  - Voice Audio Pipeline   |<=============>|  - Git Worktree Management|
|  - Terminal / Chat / Tabs |   127.0.0.1   |  - External Playwright    |
+---------------------------+               |    Browser Automation     |
                                            +---------------------------+
```

---

## 2. Feature Workstreams & Technical Design

### Workstream 1: Window Management & State Persistence

- **Current State:** Single window created with fixed 1280x800 dimensions, no position/dimension save or restore.
- **Target State:**
  - Persist window position, dimensions, maximized state across restarts via `tauri-plugin-window-state`.
  - Support multi-window (opening multiple workspace windows) using a window registry in Rust.
  - Custom titlebar integration matching Paseo styling (minimize, maximize, close, window drag regions).

### Workstream 2: Native Dialogs, Menus & File Openers

- **Current State:** Stubs in bridge script returning `null`.
- **Target State:**
  - Integrate `tauri-plugin-dialog` for native directory/file pickers (`open_folder_dialog`, `open_file_dialog`).
  - Context menu support (cut, copy, paste, inspect, custom actions).
  - Open external URLs in system default browser via `tauri-plugin-opener` or `webbrowser::open`.

### Workstream 3: Deep Link & Protocol Registration (`paseo://`)

- **Current State:** Not registered; deep links fail on OS.
- **Target State:**
  - Register `paseo://` protocol handler in `tauri.conf.json` via `tauri-plugin-deep-link`.
  - Parse inbound URIs (e.g., `paseo://workspace?path=...`, `paseo://agent/...`) and route via WebSocket/IPC to the active window.
  - Handle single-instance locking: secondary launches with deep-link URI focus the primary window and dispatch the URL.

### Workstream 4: Browser Automation via Headless Chromium (Playwright)

- **Current State:** Electron uses direct `webContents.debugger` (CDP) for in-app browser tabs. Tauri has no CDP bridge.
- **Target State:**
  - Implement a Playwright-backed headless browser manager on the daemon side (`packages/server`).
  - Implement the `browser.automation.*` RPC protocol mapping:
    - `snapshot` -> Playwright accessibility tree / DOM snapshot
    - `click`, `hover`, `drag`, `scroll`, `type`, `press_key` -> Playwright input events
    - `screenshot` -> Playwright viewport / full-page capture
    - `list_tabs`, `open_tab`, `close_tab` -> Playwright browser contexts & pages
  - Cross-platform: works uniformly on Windows, macOS, and Linux without platform-specific webview hacks.

### Workstream 5: System Tray & Desktop Notifications

- **Current State:** No tray icon; app closes on window close.
- **Target State:**
  - Tauri system tray with options:
    - "Open Paseo"
    - "Daemon Status: Running (port 6767)"
    - "Restart Daemon"
    - "Quit Paseo"
  - Configurable "minimize to tray on close" setting.
  - Native OS notifications via `tauri-plugin-notification` for agent turn completions and permission requests.

### Workstream 6: Auto-Updater

- **Current State:** No updater logic; updates require manual reinstall.
- **Target State:**
  - Integrate `tauri-plugin-updater` with public release endpoints.
  - Check for updates on startup and notify user via in-app toast.
  - Download and install silently or on restart.

---

## 3. Implementation Phasing & Dependencies

```
Phase 1: Native Window State & Dialogs
  ├── tauri-plugin-window-state
  ├── tauri-plugin-dialog
  └── Custom Titlebar Drag & Controls

Phase 2: Deep Links & Single Instance
  ├── tauri-plugin-single-instance
  ├── tauri-plugin-deep-link
  └── Route dispatching to frontend

Phase 3: Browser Automation (Daemon Playwright Adapter)
  ├── Daemon-side Playwright worker
  ├── Browser automation RPC bridge
  └── Verification with browser automation test suite

Phase 4: Tray, Notifications & Auto-Update
  ├── Tauri system tray & icon
  ├── tauri-plugin-notification
  └── tauri-plugin-updater configuration

Phase 5: E2E Validation & Electron Cutover
  ├── Port Electron E2E tests to Tauri
  ├── Verify CI build across Windows, macOS, Linux
  └── Remove packages/desktop and obsolete dependencies
```

---

## 4. Verification & Acceptance Criteria

1. **Memory Invariant:** Total memory consumption under 120MB at idle (verified via Task Manager).
2. **Feature Parity:** All capabilities listed in the table below pass:
   - Window position/size restored accurately after restart.
   - Folder picker successfully adds new workspace.
   - Deep-linking (`paseo://...`) correctly opens requested workspace/agent.
   - Browser automation commands (`snapshot`, `click`, `type`, `screenshot`) execute successfully via agent.
   - System tray shows live daemon status and controls.
   - Native notifications fire on long-running task completion.
3. **No Regressions:** Existing chat, voice input, terminal execution, and Git worktree management operate seamlessly.
