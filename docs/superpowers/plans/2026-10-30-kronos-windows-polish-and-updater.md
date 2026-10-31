# Kronos Windows 11 Native Integration & Automated Auto-Updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Elevate Kronos desktop to a premier native Windows experience by implementing Windows 11 DWM Mica/dark-theme window chrome, dual global Quake-style hotkey overlay (`Ctrl+Shift+K` / `Ctrl+Shift+P`), exposing native Tauri v2 auto-updater commands (`check_app_update`, `install_app_update`) to the frontend UI, and automating `latest.json` manifest generation and publishing.

**Architecture:** Rust Tauri v2 core manages OS-level DWM window attributes (Mica blur + immersive dark mode + tactical titlebar colors) via `windows-sys::Win32::Graphics::Dwm` and captures global shortcuts via `tauri-plugin-global-shortcut`. The updater lifecycle is driven by `tauri-plugin-updater` exposed through clean IPC commands to the existing frontend `DesktopAppUpdater` state machine. Deployment scripts are enhanced to generate and sign `latest.json` update manifests for seamless 1-click updates.

**Tech Stack:** Tauri v2, Rust 1.80+, `windows-sys` (Win32 DWM / JobObjects), `tauri-plugin-global-shortcut`, `tauri-plugin-updater`, React/Expo frontend, Node.js packaging scripts.

**Spec:** `docs/superpowers/plans/2026-10-30-kronos-windows-polish-and-updater.md`

## Global Constraints

- Retain strict Tactical Telemetry / Industrial Brutalism styling (`#0A0A0A` background, `#EAEAEA` text, `#FF2A2A` accent).
- Zero fabrication: all version numbers, endpoints, and asset names must match verified repo values (`v0.14.0`, `Xenonesis/Kronos`).
- Do not use `: any` or `as any` in TypeScript files (`ts-no-any` rule).
- Use `edit` tool with exact line anchors for file modifications.
- Windows-specific APIs must be gated behind `#[cfg(target_os = "windows")]` to preserve cross-platform compilation (Linux/macOS).

---

### Task 1: Windows 11 DWM Mica / Immersive Dark Mode Window Chrome

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml:32-40`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:975-995`

**Interfaces:**

- Consumes: `windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute}`
- Produces: `fn apply_windows_window_decorations(win: &tauri::WebviewWindow)`

- [ ] **Step 1: Add `Win32_Graphics_Dwm` feature to `windows-sys` in `Cargo.toml`**

Update `packages/desktop-tauri/src-tauri/Cargo.toml`:

```toml
windows-sys = { version = "0.59", features = [
    "Win32_System_JobObjects",
    "Win32_System_Threading",
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_Memory",
    "Win32_System_LibraryLoader",
    "Win32_Storage_FileSystem",
    "Win32_Graphics_Dwm",
] }
```

- [ ] **Step 2: Implement `apply_windows_window_decorations` in `main.rs`**

```rust
#[cfg(target_os = "windows")]
fn apply_windows_window_decorations(win: &tauri::WebviewWindow) {
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };

    if let Ok(hwnd) = win.hwnd() {
        let hwnd_raw = hwnd.0 as windows_sys::Win32::Foundation::HWND;
        unsafe {
            // 1. Force Immersive Dark Mode for window frame & titlebar
            let dark_mode: i32 = 1;
            DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<i32>() as u32,
            );

            // 2. Apply Mica Backdrop on Windows 11 (DWMSBT_MAINWINDOW = 2)
            // Value 2 = Mica, 3 = Acrylic, 4 = Mica Alt
            let backdrop_type: u32 = 2;
            let _ = DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                &backdrop_type as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 3. Caption background: #0A0A0A (BGR format: 0x000A0A0A)
            let caption_color: u32 = 0x000A0A0A;
            let _ = DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_CAPTION_COLOR as u32,
                &caption_color as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 4. Caption text: #EAEAEA (BGR format: 0x00EAEAEA)
            let text_color: u32 = 0x00EAEAEA;
            let _ = DwmSetWindowAttribute(
                hwnd_raw,
                DWMWA_TEXT_COLOR as u32,
                &text_color as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_windows_window_decorations(_win: &tauri::WebviewWindow) {}
```

- [ ] **Step 3: Call `apply_windows_window_decorations` after window creation in `setup`**

```rust
let win = win_builder.build().ok();
if let Some(ref w) = win {
    apply_windows_window_decorations(w);
    let _ = w.show();
    let _ = w.set_focus();
}
```

- [ ] **Step 4: Verify Cargo builds cleanly**

Run: `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: zero errors, compiles cleanly.

- [ ] **Step 5: Commit changes**

```bash
git add packages/desktop-tauri/src-tauri/Cargo.toml packages/desktop-tauri/src-tauri/src/main.rs
git commit -m "feat(desktop-tauri): add Windows 11 DWM Mica backdrop and immersive dark titlebar"
```

---

### Task 2: Dual Global Shortcuts (`Ctrl+Shift+K` & `Ctrl+Shift+P`) with Frontend Focus Dispatch

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:1010-1025`
- Modify: `packages/app/src/desktop/native-desktop-bridge.ts`

**Interfaces:**

- Consumes: `tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState}`
- Produces: Global shortcut registrations + `"kronos://quick-toggle"` event

- [ ] **Step 1: Register both `CommandOrControl+Shift+K` and `CommandOrControl+Shift+P` in `main.rs`**

```rust
// Register global shortcuts to toggle main window visibility
for shortcut in ["CommandOrControl+Shift+K", "CommandOrControl+Shift+P"] {
    if let Err(err) = app.global_shortcut().on_shortcut(shortcut, move |app, registered_shortcut, event| {
        if event.state == ShortcutState::Pressed {
            toggle_main_window(app);
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.emit("kronos://quick-toggle", serde_json::json!({
                    "shortcut": registered_shortcut,
                    "timestamp": chrono_or_now_ms(),
                }));
            }
        }
    }) {
        eprintln!("[shortcut] Failed to register global shortcut '{shortcut}': {err}");
    }
}
```

- [ ] **Step 2: Update `toggle_main_window` to ensure window is brought to foreground**

```rust
fn toggle_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let is_visible = win.is_visible().unwrap_or(false);
        let is_focused = win.is_focused().unwrap_or(false);
        if is_visible && is_focused {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }
}
```

- [ ] **Step 3: Listen for `"kronos://quick-toggle"` in frontend bridge**

In `packages/app/src/desktop/native-desktop-bridge.ts`, add listener hook for `"kronos://quick-toggle"` to focus search/terminal when summoned.

- [ ] **Step 4: Verify Cargo builds cleanly**

Run: `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: zero warnings or errors.

- [ ] **Step 5: Commit changes**

```bash
git add packages/desktop-tauri/src-tauri/src/main.rs packages/app/src/desktop/native-desktop-bridge.ts
git commit -m "feat(desktop): enable dual global hotkeys Ctrl+Shift+K / Ctrl+Shift+P with focus dispatch"
```

---

### Task 3: Expose Native `check_app_update` & `install_app_update` Tauri Commands

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:945-975`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:add command functions`

**Interfaces:**

- Consumes: `tauri_plugin_updater::{UpdaterExt, Update}`
- Produces: `check_app_update`, `install_app_update` commands exposed to IPC

- [ ] **Step 1: Define `UpdateState` and response DTO structs in `main.rs`**

```rust
#[derive(Clone, Default)]
pub struct UpdateState(pub std::sync::Arc<tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>>);

#[derive(serde::Serialize)]
#[allow(non_snake_case)]
pub struct DesktopAppUpdateCheckResult {
    pub hasUpdate: bool,
    pub readyToInstall: bool,
    pub currentVersion: Option<String>,
    pub latestVersion: Option<String>,
    pub body: Option<String>,
    pub date: Option<String>,
    pub errorMessage: Option<String>,
}

#[derive(serde::Serialize)]
pub struct DesktopAppUpdateInstallResult {
    pub installed: bool,
    pub version: Option<String>,
    pub message: String,
}
```

- [ ] **Step 2: Implement `check_app_update` Tauri command**

```rust
#[tauri::command]
async fn check_app_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdateState>,
    _release_channel: Option<String>,
    _intent: Option<String>,
) -> Result<DesktopAppUpdateCheckResult, String> {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            return Ok(DesktopAppUpdateCheckResult {
                hasUpdate: false,
                readyToInstall: false,
                currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
                latestVersion: None,
                body: None,
                date: None,
                errorMessage: Some(format!("Updater not initialized: {e}")),
            });
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let result = DesktopAppUpdateCheckResult {
                hasUpdate: true,
                readyToInstall: true,
                currentVersion: Some(update.current_version.clone()),
                latestVersion: Some(update.version.clone()),
                body: update.body.clone(),
                date: update.date.map(|d| d.to_string()),
                errorMessage: None,
            };
            *state.0.lock().await = Some(update);
            Ok(result)
        }
        Ok(None) => Ok(DesktopAppUpdateCheckResult {
            hasUpdate: false,
            readyToInstall: false,
            currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
            latestVersion: None,
            body: None,
            date: None,
            errorMessage: None,
        }),
        Err(e) => Ok(DesktopAppUpdateCheckResult {
            hasUpdate: false,
            readyToInstall: false,
            currentVersion: Some(env!("CARGO_PKG_VERSION").to_string()),
            latestVersion: None,
            body: None,
            date: None,
            errorMessage: Some(e.to_string()),
        }),
    }
}
```

- [ ] **Step 3: Implement `install_app_update` Tauri command**

```rust
#[tauri::command]
async fn install_app_update(
    state: tauri::State<'_, UpdateState>,
) -> Result<DesktopAppUpdateInstallResult, String> {
    let mut lock = state.0.lock().await;
    if let Some(update) = lock.take() {
        let version = update.version.clone();
        update
            .download_and_install(|_chunk, _total| {}, || {})
            .await
            .map_err(|e| format!("Download/install failed: {e}"))?;

        Ok(DesktopAppUpdateInstallResult {
            installed: true,
            version: Some(version),
            message: "Update applied successfully. Restart Kronos to launch the new version.".into(),
        })
    } else {
        Err("No pending update ready to install. Please check for updates first.".into())
    }
}
```

- [ ] **Step 4: Register state and commands in `main.rs` builder**

Register `UpdateState::default()` in `.manage(...)` and add `check_app_update` and `install_app_update` to `invoke_handler![...]`.

- [ ] **Step 5: Run Cargo test / check**

Run: `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: zero errors.

- [ ] **Step 6: Commit changes**

```bash
git add packages/desktop-tauri/src-tauri/src/main.rs
git commit -m "feat(desktop-tauri): implement native check_app_update and install_app_update IPC commands"
```

---

### Task 4: Fix Update Base URLs and Channels in Frontend

**Files:**

- Modify: `packages/app/src/desktop/updates/desktop-updates.ts:40-45`
- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json:42-46`

**Interfaces:**

- Consumes: GitHub Releases endpoint for `Xenonesis/Kronos`
- Produces: Correct release URLs across the desktop app

- [ ] **Step 1: Update `RELEASE_DOWNLOAD_BASE_URL` in `desktop-updates.ts`**

Change `https://github.com/getpaseo/paseo/releases/download` to `https://github.com/Xenonesis/Kronos/releases/download`.

- [ ] **Step 2: Update updater endpoints in `tauri.conf.json`**

Ensure `plugins.updater.endpoints` is:

```json
"endpoints": [
  "https://github.com/Xenonesis/Kronos/releases/latest/download/latest.json"
]
```

- [ ] **Step 3: Run TypeScript typecheck on `packages/app`**

Run: `npx tsc -p packages/app/tsconfig.json --noEmit`
Expected: zero errors.

- [ ] **Step 4: Commit changes**

```bash
git add packages/app/src/desktop/updates/desktop-updates.ts packages/desktop-tauri/src-tauri/tauri.conf.json
git commit -m "fix(desktop): rebrand updater download URLs to Xenonesis/Kronos"
```

---

### Task 5: Automated `latest.json` Generation and Publishing

**Files:**

- Modify: `scripts/update-and-build-exe.mjs`
- Test: `scripts/update-and-build-exe.mjs --help` or dry run

**Interfaces:**

- Consumes: Built installer `.exe` in `dist/` and git release tag
- Produces: Valid `latest.json` uploaded to GitHub release

- [ ] **Step 1: Add `generateAndUploadUpdateManifest` function in `scripts/update-and-build-exe.mjs`**

Implement automated generation of `latest.json`:

- Read `packages/desktop-tauri/src-tauri/Cargo.toml` or `tauri.conf.json` for current version (`v0.14.0`).
- Format `latest.json` matching Tauri v2 schema:

```json
{
  "version": "v0.14.0",
  "notes": "Kronos v0.14.0 Release: Instant boot, Windows 11 Mica backdrop, native Myers diff, and dual global hotkeys.",
  "pub_date": "<ISO Timestamp>",
  "platforms": {
    "windows-x86_64": {
      "url": "https://github.com/Xenonesis/Kronos/releases/download/v0.14.0/Kronos_x64-setup.exe"
    }
  }
}
```

- Write `dist/latest.json`.
- Upload via `gh release upload v0.14.0 dist/latest.json --clobber --repo Xenonesis/Kronos`.

- [ ] **Step 2: Execute manifest generation and upload for current release v0.14.0**

Run: `node -e "..."` or via `update-and-build-exe.mjs`
Expected: `latest.json` appears on GitHub release `v0.14.0`.

- [ ] **Step 3: Verify GitHub release has `latest.json`**

Run: `gh release view v0.14.0 --repo Xenonesis/Kronos --json assets --jq ".assets[].name"`
Expected: includes `latest.json`.

- [ ] **Step 4: Commit changes**

```bash
git add scripts/update-and-build-exe.mjs
git commit -m "feat(scripts): automate latest.json updater manifest generation and upload"
```

---

### Task 6: End-to-End Verification & Sanity Check

**Files:**

- Verify: Rust build, TypeScript typecheck, live updater test

- [ ] **Step 1: Run full Cargo test suite**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: 82+ tests pass.

- [ ] **Step 2: Run full app typecheck**

Run: `npx tsc -p packages/app/tsconfig.json --noEmit`
Expected: zero errors.

- [ ] **Step 3: Test updater check command against live GitHub release**

Verify `curl -s https://github.com/Xenonesis/Kronos/releases/latest/download/latest.json` returns valid JSON with `200 OK`.

- [ ] **Step 4: Push to main**

```bash
git push origin main
```
