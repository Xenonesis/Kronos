# Track 4: Lean Standalone Production Build (`--without-node`) & Verification Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package, compile, and verify the ultra-lean standalone Windows desktop production installer and portable binary without `node.exe` or bloated Node modules, achieving an installer under 25MB with 0ms in-process Rust daemon initialization.

**Architecture:**

1. Standalone Distribution Preparation (`prepare-dist.mjs`): Evaluate `--without-node` upfront. In standalone native mode, clean `server-dist`, omit copying 47.6MB of unneeded TypeScript server build, 50MB `node.exe`, and 238MB `node_modules`. Create minimal metadata marker (`dist/standalone.json`) and empty `.bin` shim directory so Tauri's `resources: ["../server-dist/**/*"]` resource packager succeeds instantly without error.
2. Build Orchestration (`update-and-build-exe.mjs`): Support `--without-node` CLI flag and `KRONOS_WITHOUT_NODE` environment variable. Propagate the flag to `prepare-dist.mjs`. Force a rebuild when `--without-node` is supplied so cached bloated installers are replaced.
3. Unit Test Suite: Write `packages/desktop-tauri/prepare-dist.test.mjs` verifying clean extraction, marker generation, and zero `node.exe` presence in native mode.
4. Production Compilation & Verification: Run `node scripts/update-and-build-exe.mjs --without-node`, verify output artifacts in `dist/`, measure binary size reduction, and smoke test the in-process Rust daemon.

**Tech Stack:** Node.js (build scripting), Tauri v2, Rust (Axum, Tokio), NSIS.

## Global Constraints

- Industrial Brutalism styling: `#0A0A0A`, `#EAEAEA`, `#FF2A2A`, strict 90° corners, monospaced typography.
- Desktop architecture: Tauri v2 + Windows WebView2 + native in-process Axum/Tokio daemon.
- Zero fabrication: all benchmark figures and claims must reference real code or verified measurement.
- All tests must pass: Node test runner (`node --test`) and Cargo (`cargo test`).

---

### Task 1: Refactor `prepare-dist.mjs` for Zero-Bloat Standalone Native Mode & Add Tests

**Files:**

- Modify: `packages/desktop-tauri/prepare-dist.mjs`
- Test: `packages/desktop-tauri/prepare-dist.test.mjs`

**Interfaces:**

- Consumes: `process.argv` (`--without-node`) and `process.env.KRONOS_WITHOUT_NODE`.
- Produces: `server-dist/dist/standalone.json` marker, `server-dist/package.json`, and `server-dist/node_modules/.bin` dummy directory. Omits `server-dist/node.exe`, `packages/server/dist` (47.6MB), and external npm packages.

- [ ] **Step 1: Write test for `prepare-dist.mjs`**

Write `packages/desktop-tauri/prepare-dist.test.mjs` verifying that running `node prepare-dist.mjs --without-node` produces `server-dist` without `node.exe`, with `dist/standalone.json`, and with a total directory size under 100KB.

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test packages/desktop-tauri/prepare-dist.test.mjs`
Expected: FAIL (because current prepare-dist still copies 47.6MB `server/dist`).

- [ ] **Step 3: Update `packages/desktop-tauri/prepare-dist.mjs`**

Move `withoutNode` check to the top. If `withoutNode` is true:

- Empty `server-dist`
- Write `server-dist/dist/standalone.json` with `{ "engine": "rust", "native": true }`
- Write `server-dist/package.json` with `{ "name": "@getpaseo/server-dist", "version": "0.15.0-beta.1", "private": true }`
- Create `server-dist/node_modules/.bin`
- Exit early before copying `server/dist`, `node.exe`, or running `npm install`.

- [ ] **Step 4: Run test to verify it passes**

Run: `node --test packages/desktop-tauri/prepare-dist.test.mjs`
Expected: PASS.

- [ ] **Step 5: Commit changes**

```bash
git add packages/desktop-tauri/prepare-dist.mjs packages/desktop-tauri/prepare-dist.test.mjs
git commit -m "feat(desktop): optimize standalone prepare-dist for zero-bloat native mode"
```

---

### Task 2: Update Build Orchestrator `scripts/update-and-build-exe.mjs` to Support `--without-node`

**Files:**

- Modify: `scripts/update-and-build-exe.mjs`

**Interfaces:**

- Consumes: `--without-node` flag from CLI args or `process.env.KRONOS_WITHOUT_NODE`.
- Produces: Sets `force = true` when `--without-node` is provided, skips unneeded `@getpaseo/server` TS build, and passes `--without-node` to `prepare-dist.mjs`.

- [ ] **Step 1: Add `--without-node` parsing and propagation in `scripts/update-and-build-exe.mjs`**
- [ ] **Step 2: Run test dry-run / execution check**
- [ ] **Step 3: Commit changes**

```bash
git add scripts/update-and-build-exe.mjs
git commit -m "feat(scripts): support --without-node flag in update-and-build-exe orchestrator"
```

---

### Task 3: Execute Production Standalone Build & Measure Metrics

**Files:**

- Produces: `dist/Kronos_0.15.0-beta.1_x64-setup.exe`
- Produces: `dist/kronos-portable.exe`
- Produces: `dist/latest.json`

**Interfaces:**

- Consumes: `node scripts/update-and-build-exe.mjs --without-node`

- [ ] **Step 1: Run production build with `--without-node`**
- [ ] **Step 2: Measure installer size and compare with fat node bundle**
- [ ] **Step 3: Smoke test standalone binary / Axum endpoint startup**
- [ ] **Step 4: Run cargo tests to ensure 100% test suite health**
- [ ] **Step 5: Commit and push Track 4 deliverable**
