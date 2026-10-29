# Kronos Upstream Sync (0.14.0-beta.2) & Production .EXE Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Synchronize the 8 latest upstream commits from `getpaseo/paseo:main` (`0.14.0-beta.2`), preserve all Kronos native Rust enhancements, bump desktop package versioning, verify with full test suites, and compile & publish the updated production Windows setup `.exe` installer.

**Architecture:** Seamlessly merge `upstream/main` into `origin/main` without conflicts. Preserve Kronos custom Native Rust subsystems (`attachments.rs`, daemon lifecycle management, Win32 hardware idle detection, fast path inspection). Rebuild core workspace packages, compile the production release binary with Tauri v2, package the NSIS installer, and atomically publish to GitHub Releases (`v0.14.0`) and GitHub Pages.

**Tech Stack:** Rust (Tauri v2, Win32 FFI, `tokio`), React 19, TypeScript, Expo Router, Node.js 22, NSIS (`makensis`), GitHub CLI (`gh`).

**Spec:** `docs/superpowers/plans/2026-10-29-upstream-sync-beta2-and-exe-update.md`

## Global Constraints

- Preserve all Kronos branding, Industrial Brutalism styling (`#0A0A0A`, `#FF2A2A`, strict 90° corners), and custom Tauri bridge methods.
- Zero TypeScript `any` additions (`ts-no-any`).
- Preserve Native Rust Attachment Storage Engine (`packages/desktop-tauri/src-tauri/src/attachments.rs`).
- Zero regression in native daemon lifecycle control (`restart_desktop_daemon`, `stop_desktop_daemon`).
- Maintain valid Windows PE semantic versioning in `tauri.conf.json` (`0.14.0`).
- Ensure persistent latest setup executable (`Kronos_x64-setup.exe`) is uploaded to GitHub Releases with `--clobber`.

---

### Task 1: Clean Up Working Directory & Merge Upstream

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/gen/schemas/` (restore or format generated schemas)
- Git Merge: `upstream/main` into `main`

**Interfaces:**

- Consumes: `upstream/main` (8 new commits: Codex agent memory leak fix, turn steer delivery race fix, usage reset timers, Claude settings flags preservation)
- Produces: Clean merged `main` branch with zero conflict markers

- [ ] **Step 1: Discard transient Tauri schema diffs in working tree**

```bash
git checkout -- packages/desktop-tauri/src-tauri/gen/schemas/
```

- [ ] **Step 2: Merge upstream/main into local main**

```bash
git merge upstream/main -m "merge: sync upstream main (0.14.0-beta.2) with Codex memory and steer race fixes"
```

- [ ] **Step 3: Verify clean merge status**

```bash
git status
```

Expected: `nothing to commit, working tree clean`

---

### Task 2: Verify & Align Package Versions & Tauri Configuration

**Files:**

- Modify: `packages/desktop-tauri/package.json`
- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json`

**Interfaces:**

- Consumes: Root package version `0.14.0-beta.2`
- Produces: Desktop package version `0.14.0` (clean 3-component semver for Windows PE installer)

- [ ] **Step 1: Check desktop package versioning**

Inspect `packages/desktop-tauri/package.json` and `Cargo.toml`. Confirm both are set to `0.14.0`.

- [ ] **Step 2: Format files with oxfmt**

```bash
npx oxfmt package.json packages/app/package.json packages/server/package.json
```

---

### Task 3: Full Workspace Tests & Zero-Regression Check

**Files:**

- Test: `packages/desktop-tauri/src-tauri/src/main.rs`
- Test: `packages/desktop-tauri/src-tauri/src/attachments.rs`
- Test: `packages/app/src/desktop/`

**Interfaces:**

- Consumes: Rust native test suite, TypeScript app test suite
- Produces: 100% passing tests across all desktop and core subsystems

- [ ] **Step 1: Run Rust test suite**

```bash
cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml
```

Expected: `12 passed; 0 failed`

- [ ] **Step 2: Run Rust clippy linter**

```bash
cargo clippy --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml
```

Expected: `0 warnings, 0 errors`

- [ ] **Step 3: Run TypeScript typecheck**

```bash
npm --prefix packages/app run typecheck
```

Expected: Clean exit `0 errors`

- [ ] **Step 4: Run Desktop Vitest unit tests**

```bash
npm --prefix packages/app test src/desktop/pick-directory.test.ts src/desktop/electron/idle.test.ts src/desktop/daemon/desktop-daemon-transport.test.ts src/desktop/daemon/daemon-management-error.test.ts src/desktop/daemon/daemon-management-toggle.test.ts src/desktop/attachments/desktop-preview-url.test.ts src/desktop/attachments/desktop-attachment-store.test.ts
```

Expected: `7 passed (36 tests)`

---

### Task 4: Core Packages & Frontend Web Bundle Build

**Files:**

- Output: `packages/protocol/dist/`
- Output: `packages/client/dist/`
- Output: `packages/relay/dist/`
- Output: `packages/highlight/dist/`
- Output: `packages/plugin/dist/`
- Output: `packages/server/dist/`
- Output: `packages/app/dist/`
- Output: `packages/desktop-tauri/server-dist/`

**Interfaces:**

- Consumes: Built server code + web frontend bundle
- Produces: Packaged lean `server-dist` with bundled Node runtime and dependencies ready for Tauri NSIS

- [ ] **Step 1: Build core packages in dependency order**

```bash
npm run build --workspace=@getpaseo/protocol
npm run build --workspace=@getpaseo/client
npm run build --workspace=@getpaseo/relay
npm run build --workspace=@getpaseo/highlight
npm run build --workspace=@getpaseo/plugin
npm run build --workspace=@getpaseo/server
```

- [ ] **Step 2: Export frontend web bundle**

```bash
npx cross-env NODE_ENV=production PASEO_WEB_PLATFORM=electron npx expo export --platform web --clear
```

- [ ] **Step 3: Prepare lean server-dist assets**

```bash
node packages/desktop-tauri/prepare-dist.mjs
```

---

### Task 5: Production Tauri NSIS Build, Publish & Verification

**Files:**

- Output: `packages/desktop-tauri/src-tauri/target/release/bundle/nsis/Kronos_0.14.0_x64-setup.exe`
- Output: `packages/desktop-tauri/src-tauri/target/release/bundle/nsis/Kronos_x64-setup.exe`

**Interfaces:**

- Consumes: `packages/desktop-tauri/server-dist` and `packages/app/dist`
- Produces: Live updated GitHub Release `v0.14.0` assets and direct `HTTP 200` download redirection

- [ ] **Step 1: Compile Tauri NSIS installer**

```bash
npx tauri build --bundles nsis
```

(inside `packages/desktop-tauri`)

- [ ] **Step 2: Copy persistent setup executable**

```bash
cp packages/desktop-tauri/src-tauri/target/release/bundle/nsis/Kronos_0.14.0_x64-setup.exe packages/desktop-tauri/src-tauri/target/release/bundle/nsis/Kronos_x64-setup.exe
```

- [ ] **Step 3: Commit and push changes to GitHub**

```bash
git push origin main
```

- [ ] **Step 4: Upload and clobber release assets on GitHub Releases**

```bash
gh release upload v0.14.0 packages/desktop-tauri/src-tauri/target/release/bundle/nsis/Kronos_0.14.0_x64-setup.exe packages/desktop-tauri/src-tauri/target/release/bundle/nsis/Kronos_x64-setup.exe --clobber --repo Xenonesis/Kronos
```

- [ ] **Step 5: Verify live download URL**

```bash
curl -I -L https://github.com/Xenonesis/Kronos/releases/latest/download/Kronos_x64-setup.exe
```

Expected: `HTTP/1.1 200 OK`
