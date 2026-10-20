# Tauri v2 Full Feature-Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the feature-parity migration from Electron (`packages/desktop`) to Tauri v2 (`packages/desktop-tauri`), enabling full deprecation and safe removal of Electron.

**Architecture:** Rust Tauri v2 desktop shell with native plugins (dialog, window-state, deep-link, notification, updater) + Node.js server daemon running Playwright for cross-platform browser automation.

**Spec Reference:** `docs/superpowers/specs/2026-09-27-tauri-feature-parity-design.md`

---

## Global Constraints

- Never break existing Electron builds until Phase 5 cutover is completed.
- Maintain idle RAM under 120MB for the Tauri shell.
- Keep all protocol contracts with `@getpaseo/protocol` intact.

---

### Task 1: Window State Persistence & Native Dialogs (Phase 1)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`
- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json`
- Create: `packages/desktop-tauri/src-tauri/src/dialogs.rs`

- [ ] **Step 1: Add Tauri plugins to Cargo.toml**
  - Add `tauri-plugin-window-state = "2"`
  - Add `tauri-plugin-dialog = "2"`
  - Add `tauri-plugin-opener = "2"`
- [ ] **Step 2: Configure capabilities and permissions**
  - Update `packages/desktop-tauri/src-tauri/gen/schemas/capabilities.json` to allow dialog and window-state APIs.
- [ ] **Step 3: Implement native dialog handlers in Rust**
  - Implement `pick_folder() -> Option<String>`
  - Implement `pick_file(filter: Vec<String>) -> Option<String>`
  - Expose as Tauri invoke commands.
- [ ] **Step 4: Update bridge script in main.rs**
  - Map `pick_folder` and `pick_file` commands to Tauri invoke so the React UI can open native pickers.
- [ ] **Step 5: Verify build & test window state restore**
  - Run `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`.
  - Launch app, resize/move window, close and reopen to confirm position/size restore.

---

### Task 2: Deep Link Registration & Single Instance (Phase 2)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`
- Create: `packages/desktop-tauri/src-tauri/src/deeplink.rs`

- [ ] **Step 1: Add dependencies to Cargo.toml**
  - Add `tauri-plugin-deep-link = "2"`
  - Add `tauri-plugin-single-instance = "2"`
- [ ] **Step 2: Configure protocol handler in tauri.conf.json**
  - Add `plugins.deep-link.schemes: ["paseo"]`.
- [ ] **Step 3: Implement single-instance lock and URL forwarder**
  - When secondary instance launches with `paseo://...`, focus main window and emit `paseo://` event to frontend.
- [ ] **Step 4: Connect deep-link listener in frontend bridge**
  - Listen for deep-link events and route to correct workspace/agent session.
- [ ] **Step 5: Verify deep-link via CLI**
  - Test opening `paseo://workspace?path=...` via Windows Run / CLI.

---

### Task 3: Daemon-Side Playwright Browser Automation (Phase 3)

**Files:**

- Modify: `packages/server/package.json`
- Create: `packages/server/src/server/agent/tools/playwright-browser.ts`
- Modify: `packages/server/src/server/agent/provider-registry.ts`
- Modify: `packages/app/src/desktop/host.ts`

- [ ] **Step 1: Verify Playwright availability in server**
  - Ensure `@playwright/test` or `playwright-core` is accessible from `packages/server`.
- [ ] **Step 2: Implement Headless Playwright Browser Manager**
  - Create singleton browser manager that launches headless Chromium on demand.
  - Implement RPC commands: `snapshot`, `click`, `type`, `scroll`, `hover`, `screenshot`, `list_tabs`.
- [ ] **Step 3: Wire into Agent Tool Catalog**
  - Expose browser automation tools directly through daemon tool registry so agents can operate without in-app CDP.
- [ ] **Step 4: Verify browser automation with agent test turn**
  - Run smoke test: prompt agent to visit a URL, take a snapshot, and click a button.

---

### Task 4: System Tray, Notifications & Auto-Update (Phase 4)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`
- Create: `packages/desktop-tauri/src-tauri/src/tray.rs`
- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json`

- [ ] **Step 1: Add notification & updater plugins to Cargo.toml**
  - Add `tauri-plugin-notification = "2"`
  - Add `tauri-plugin-updater = "2"`
- [ ] **Step 2: Implement system tray menu in Rust**
  - Create tray with items: Open Paseo, Daemon Status, Restart Daemon, Quit.
  - Add tray event handlers (show/hide on click).
- [ ] **Step 3: Implement native notifications**
  - Expose notification sender for agent turn completion.
- [ ] **Step 4: Configure Tauri updater**
  - Set updater endpoints in `tauri.conf.json`.
- [ ] **Step 5: Verify build & tray interaction**
  - Build release and test tray menu functions.

---

### Task 5: End-to-End Validation & Electron Removal (Phase 5)

**Files:**

- Remove: `packages/desktop/`
- Modify: `package.json` (remove workspace, electron dependencies, electron build scripts)
- Modify: Root CI/CD workflows (`.github/workflows/`)

- [ ] **Step 1: Run comprehensive smoke tests on Tauri build**
  - Memory consumption (<120MB idle).
  - Voice recording & transcription.
  - Terminal creation & execution.
  - Agent coding workflow with Git worktrees.
  - Browser automation via Playwright.
- [ ] **Step 2: Update root package.json**
  - Rename `dev:desktop` to point to `desktop-tauri`.
  - Remove `electron`, `electron-builder` from `devDependencies`.
- [ ] **Step 3: Deprecate and remove packages/desktop**
  - Archive or delete `packages/desktop/`.
- [ ] **Step 4: Final verification build**
  - Run full repository typecheck and build from root.
