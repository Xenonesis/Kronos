# Tauri v2 Desktop Migration Design

- **Status**: Approved / In Progress
- **Target**: `packages/desktop-tauri`
- **Objective**: Replace the high-memory Electron shell (~1GB RAM) with a lightweight Tauri v2 desktop shell (<100MB RAM), while retaining full React UI, voice pipeline, and agent coding features.

---

## 1. Architectural Strategy

We use a parallel package approach:

- `packages/desktop`: Retained as-is (Electron).
- `packages/desktop-tauri`: New Tauri v2 package.

```
+-------------------------------------------------------------+
|                 Paseo Tauri v2 Shell (Rust)                 |
|  - Window Management & Custom Titlebar Chrome               |
|  - Native OS Dialogs, Menus, & System Tray                  |
|  - Editor Launchers (VS Code, Cursor, Zed, JetBrains)       |
|  - Deep-link Router (paseo://)                              |
+------------------------------+------------------------------+
                               |
         +---------------------+---------------------+
         |                                           |
+--------v-------------------+             +---------v-------------------+
|  System WebView (OS)       |             |  Node.js Daemon Sidecar     |
|  - React Web UI (Expo Web) |  WebSocket  |  - @getpaseo/server         |
|  - Voice Audio Capture     |<----------->|  - Coding Agent Pipelines   |
|  - Terminal & Diff Views   |  127.0.0.1  |  - Git Worktree Management  |
+----------------------------+             +-----------------------------+
```

---

## 2. Component Specifications

### 2.1 Tauri Configuration (`tauri.conf.json`)

- Bundle identifier: `sh.paseo.desktop.tauri`
- Frontend dist: `../../packages/app/dist`
- Dev server URL: `http://localhost:8081` (for live Expo development)
- Windows: Single main window, titleBarStyle: `Overlay` for custom Paseo titlebar.

### 2.2 Server Daemon Sidecar Management (`src-tauri/src/daemon.rs`)

- Tauri app setup hook spawns Node daemon with environment:
  - `PASEO_LISTEN=127.0.0.1:6768`
  - Inherits system login shell PATH.
- App exit listener sends SIGTERM / graceful shutdown IPC to ensure no orphan Node processes.

### 2.3 Rust Native Commands (`src-tauri/src/commands/`)

- `open_in_editor(editor: String, path: String, line: Option<u32>, col: Option<u32>)`
- `get_daemon_status()`
- `show_native_dialog(options: DialogOptions)`

### 2.4 Browser Automation Adapter

- Agent web browsing is routed via external Playwright / Chrome execution instead of in-app WebKit CDP.

---

## 3. Verification & Acceptance Criteria

1. `cargo tauri build` compiles cleanly into `src-tauri/target/release/Paseo.exe`.
2. App launches with `< 100MB` idle RAM (validated via Task Manager / `tasklist`).
3. React UI loads and connects to local daemon over WebSocket.
4. Voice input and terminal agent actions operate without regressions.
