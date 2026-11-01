# Kronos Master Roadmap & v0.15.0-beta.1 Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute the complete 4-option Kronos optimization and release roadmap, starting immediately with Option 1 (v0.15.0-beta.1 release packaging, verification, and GitHub publish), followed by Option 2 (Winget/Scoop automated distribution pipeline), Option 3 (Decoupled usage source plugin loading for sub-100ms daemon cold boot), and Option 4 (Rust kernel telemetry IPC integration).

**Architecture:** Tauri v2 desktop architecture for Windows x64. Option 1 synchronizes desktop version metadata and executes a verified production build. Option 2 implements automated release manifest distribution. Option 3 decouples non-critical usage source plugin startup from the critical daemon ready path. Option 4 wires native Rust kernel capabilities directly to Tauri IPC.

**Tech Stack:** Rust (Tauri v2, Windows DWM, Tokio, Axum), TypeScript/Node.js, GitHub Actions, NSIS, Winget manifest v1.6.0.

## Global Constraints

- Target platform: Windows 10/11 x64 (primary) with Tauri v2.
- Do not use `: any` or `as any` in TypeScript (`ts-no-any` rule).
- Never publish contracts through `ReturnType<typeof fn>`. Name types in owning modules.
- Zero fabrication: all benchmark figures, test assertions, and download links must reference real code or verified measurement.
- Preserve Windows 11 Mica backdrop (`apply_mica_effect`), Global Quake Hotkey (`Ctrl+Shift+K`), and in-app updater handlers.

---

### Phase 1: Option 1 — Production Build & Publish v0.15.0-beta.1

#### Task 1: Version Alignment Across Desktop Metadata

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json:4`
- Modify: `packages/desktop-tauri/package.json:3`
- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml:3`

**Interfaces:**

- Consumes: Root `package.json` version `0.15.0-beta.1`
- Produces: Synchronized desktop version `0.15.0-beta.1` for installer naming, Tauri updater metadata, and binary branding.

- [ ] **Step 1: Check current version discrepancy**
      Run: `node -e "console.log({ root: require('./package.json').version, tauri: require('./packages/desktop-tauri/src-tauri/tauri.conf.json').version })"`
      Expected: `{ root: '0.15.0-beta.1', tauri: '0.14.0' }`

- [ ] **Step 2: Update `tauri.conf.json`**
      Set `"version": "0.15.0-beta.1"` in `packages/desktop-tauri/src-tauri/tauri.conf.json`.

- [ ] **Step 3: Update `packages/desktop-tauri/package.json`**
      Set `"version": "0.15.0-beta.1"` in `packages/desktop-tauri/package.json`.

- [ ] **Step 4: Update `Cargo.toml`**
      Set `version = "0.15.0-beta.1"` in `packages/desktop-tauri/src-tauri/Cargo.toml`.

- [ ] **Step 5: Verify Rust compilation**
      Run: `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
      Expected: `Finished dev profile [unoptimized + debuginfo] target(s) in 0.xx s` with zero errors.

- [ ] **Step 6: Commit version synchronization**

```bash
git add packages/desktop-tauri/src-tauri/tauri.conf.json packages/desktop-tauri/package.json packages/desktop-tauri/src-tauri/Cargo.toml
git commit -m "chore(desktop): bump desktop tauri and cargo version to 0.15.0-beta.1"
```

---

#### Task 2: Build Production Installer, Portable Exe & Update Manifest

**Files:**

- Execute: `scripts/update-and-build-exe.mjs`
- Generate: `dist/Kronos_0.15.0-beta.1_x64-setup.exe`
- Generate: `dist/Kronos_x64-setup.exe`
- Generate: `dist/kronos-portable.exe`
- Generate: `dist/latest.json`
- Generate: `dist/Xenonesis.Kronos.yaml`, `dist/Xenonesis.Kronos.installer.yaml`, `dist/Xenonesis.Kronos.locale.en-US.yaml`, `dist/Kronos.json`

**Interfaces:**

- Consumes: Production Expo web bundle (`packages/app/dist`), production server bundle (`packages/desktop-tauri/server-dist`), Tauri v2 compiler.
- Produces: Signed and hashed release artifacts in `dist/`.

- [ ] **Step 1: Clean previous build artifacts**
      Run: `node -e "const fs = require('fs'); ['dist', 'packages/desktop-tauri/src-tauri/target/release/bundle'].forEach(p => fs.existsSync(p) && fs.rmSync(p, { recursive: true, force: true }));"`

- [ ] **Step 2: Execute full release build**
      Run: `node scripts/update-and-build-exe.mjs --force`
      Expected:
- `[1/5] Checking git repository status & pulling updates... [OK]`
- `[2/5] Core packages & server build... [OK]`
- `[3/5] Frontend web export... [OK]`
- `[4/5] Preparing lean server-dist assets... [OK]`
- `[5/5] Compiling Tauri NSIS installer .exe... [OK]`
- Output logs showing `Kronos_0.15.0-beta.1_x64-setup.exe` created and `latest.json` generated.

- [ ] **Step 3: Verify artifact existence and byte sizes**
      Run: `node -e "const fs = require('fs'); const files = fs.readdirSync('dist'); console.log(files.map(f => ({ file: f, size: fs.statSync('dist/' + f).size })));"`
      Expected:
- `Kronos_0.15.0-beta.1_x64-setup.exe` > 100MB
- `Kronos_x64-setup.exe` > 100MB
- `kronos-portable.exe` > 20MB
- `latest.json` > 100 bytes

- [ ] **Step 4: Verify `latest.json` schema and URLs**
      Run: `node -e "const j = JSON.parse(require('fs').readFileSync('dist/latest.json', 'utf8')); console.log(j);"`
      Expected:
- `version`: `"v0.15.0-beta.1"`
- `platforms['windows-x86_64'].url`: contains `Kronos_0.15.0-beta.1_x64-setup.exe`

---

#### Task 3: Publish GitHub Release v0.15.0-beta.1 & Smoke Verification

**Files:**

- Execute: `gh release create` / `gh release upload`

**Interfaces:**

- Consumes: Built artifacts in `dist/`
- Produces: Live GitHub Release `v0.15.0-beta.1` under `Xenonesis/Kronos`

- [ ] **Step 1: Check GitHub release status**
      Run: `gh release view v0.15.0-beta.1 --repo Xenonesis/Kronos`

- [ ] **Step 2: Create release if not present**
      Run: `gh release create v0.15.0-beta.1 --repo Xenonesis/Kronos --title "Kronos v0.15.0-beta.1 - Upstream Sync, Windows 11 Mica & Global Hotkey" --notes "### Kronos v0.15.0-beta.1 Release Notes%0A%0A- **Upstream Sync**: Merged 48 commits including v0.15.0-beta.1 core improvements and terminal updates.%0A- **Windows 11 Mica & Dark Frame**: Native DWM Mica backdrop (DWMSBT_MAINWINDOW) and dark titlebar styling.%0A- **Global Quake Hotkey**: Ctrl+Shift+K and Ctrl+Shift+P hotkeys with kronos://quick-toggle dispatch.%0A- **In-App Auto-Updater**: Native Tauri v2 updater IPCs with automated latest.json distribution.%0A- **Ultra-Fast Boot**: Parallelized built-in plugin loader (<500ms) and WebSocket-gated health readiness."`

- [ ] **Step 3: Upload all release assets**
      Run: `gh release upload v0.15.0-beta.1 dist/Kronos_0.15.0-beta.1_x64-setup.exe dist/Kronos_x64-setup.exe dist/kronos-portable.exe dist/latest.json dist/Xenonesis.Kronos.yaml dist/Xenonesis.Kronos.installer.yaml dist/Xenonesis.Kronos.locale.en-US.yaml dist/Kronos.json --repo Xenonesis/Kronos --clobber`

- [ ] **Step 4: Verify live release URL and downloadability**
      Run: `curl -sI https://github.com/Xenonesis/Kronos/releases/download/v0.15.0-beta.1/latest.json`
      Expected: HTTP 302 / 200 OK.

---

### Phase 2: Option 2 — Automated Winget & Scoop Distribution

#### Task 4: Automated Winget Submission Script & GitHub Action

**Files:**

- Create: `scripts/submit-winget.mjs`
- Create: `.github/workflows/winget-publish.yml`
- Test: `scripts/submit-winget.test.mjs`

**Interfaces:**

- Consumes: Manifests generated by `scripts/generate-windows-package-manifests.mjs` (`dist/Xenonesis.Kronos.*.yaml`)
- Produces: GitHub Action workflow to automatically submit PRs to `microsoft/winget-pkgs` when a non-prerelease release tag is created.

- [ ] **Step 1: Write test for submit-winget CLI parsing**
      Verify that `scripts/submit-winget.mjs --dry-run` validates required manifests and GitHub tokens without making network mutations.

- [ ] **Step 2: Implement `scripts/submit-winget.mjs`**
      Supports `--manifests-dir`, `--token`, and `--dry-run` flags to either format a PR or call GitHub API.

- [ ] **Step 3: Create `.github/workflows/winget-publish.yml`**
      Workflow triggered on `release: [published]` that runs `vedantmgoyal2009/winget-releaser@v2` with `Xenonesis.Kronos`.

- [ ] **Step 4: Commit Winget distribution pipeline**

```bash
git add scripts/submit-winget.mjs scripts/submit-winget.test.mjs .github/workflows/winget-publish.yml
git commit -m "feat(distribution): add automated winget submission script and workflow"
```

---

### Phase 3: Option 3 — Decoupled Usage Source Plugin Loading (Sub-100ms Boot)

#### Task 5: Background Asynchronous Usage Source Startup

**Files:**

- Modify: `packages/server/src/server/plugins/builtin/index.ts`
- Modify: `packages/server/src/server/plugins/index.ts`
- Test: `packages/server/src/server/plugins/builtin/builtin-lazy.test.ts`

**Interfaces:**

- Consumes: `builtinPlugins` list.
- Produces: `criticalBuiltinPlugins` (providers + worktree) awaited prior to `isDaemonReady = true`, while `usageSourceBuiltinPlugins` are started concurrently in the background without blocking `/api/health` 200 OK.

- [ ] **Step 1: Write test verifying categorized plugin lists and background initialization**
- [ ] **Step 2: Split `builtinPlugins` into `CRITICAL_BUILTIN_PLUGINS` and `DEFERRED_USAGE_PLUGINS`**
- [ ] **Step 3: Update `PluginService.start()` to await critical plugins immediately and defer usage sources**
- [ ] **Step 4: Run unit tests and typecheck**
      Run: `npm run test --workspace=@getpaseo/server && pnpm --filter @paseo/server check:type`
- [ ] **Step 5: Measure cold boot time reduction**
      Verify `/api/health` returns 200 OK in <150ms.
- [ ] **Step 6: Commit lazy plugin optimization**

```bash
git add packages/server/src/server/plugins/builtin/index.ts packages/server/src/server/plugins/index.ts packages/server/src/server/plugins/builtin/builtin-lazy.test.ts
git commit -m "perf(server): decouple usage sources to achieve sub-100ms daemon readiness"
```

---

### Phase 4: Option 4 — Rust Kernel IPC Integration & Telemetry

#### Task 6: Expose Native Rust Kernel Telemetry to Tauri IPC

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/mod.rs`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs`

**Interfaces:**

- Consumes: Rust `kernel::server` and `kernel::vfs` metrics.
- Produces: `get_kernel_telemetry` Tauri IPC command returning live memory, active connections, and CRDT status.

- [ ] **Step 1: Add `get_kernel_telemetry` command in `main.rs`**
- [ ] **Step 2: Register in `tauri::generate_handler![..., get_kernel_telemetry]`**
- [ ] **Step 3: Run `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`**
- [ ] **Step 4: Commit kernel IPC integration**

```bash
git add packages/desktop-tauri/src-tauri/src/main.rs packages/desktop-tauri/src-tauri/src/kernel/mod.rs
git commit -m "feat(desktop): expose native rust kernel telemetry via tauri ipc"
```
