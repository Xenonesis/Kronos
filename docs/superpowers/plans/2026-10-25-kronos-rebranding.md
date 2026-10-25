# Kronos Unified Rebranding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebrand user-facing surfaces, strings, notifications, CLI command, and deep-link schemes to Kronos while maintaining 100% upstream protocol compatibility with `getpaseo/paseo:main`.

**Architecture:** Update UI string catalog in `packages/app/src/i18n/resources/en.ts`, alias CLI command `kronos` in `packages/cli/package.json`, dual-register `kronos://` deep-links in Tauri configuration and Rust handler, and update repository documentation.

**Tech Stack:** React Native / Expo, Rust (Tauri v2), Node.js / Commander (CLI), Vite / TanStack Router (Website), Vitest.

**Spec:** `docs/superpowers/specs/2026-10-25-kronos-rebranding-design.md`

## Global Constraints

- Preserve exact parameter tokens in translations (`{{count}}`, `{{brand}}`, `{{version}}`, `{{url}}`, `{{noun}}`).
- Retain existing `~/.paseo` state directory resolution and internal `@getpaseo/*` package scopes to guarantee frictionless upstream merges.
- Dual-support `kronos` and `paseo` for CLI binaries and deep-link URI schemes.
- All code edits must pass TypeScript typecheck, oxfmt format check, and unit test suites.

---

### Task 1: UI & Visible String Resources (`packages/app`)

**Files:**

- Modify: `packages/app/src/i18n/resources/en.ts`
- Test: `packages/app/src/i18n/i18next.startup.test.ts`

**Interfaces:**

- Consumes: `en.ts` resource dictionary.
- Produces: Kronos-branded UI strings for headings, tooltips, toasts, dialogs, and error boundaries.

- [ ] **Step 1: Inspect target string entries in en.ts**

Run grep to verify existing string locations:

```bash
grep -n -E "(appName|onboarding|quitting|rootError|Manage built-in daemon|Paseo notification)" packages/app/src/i18n/resources/en.ts
```

- [ ] **Step 2: Update user-facing strings in `packages/app/src/i18n/resources/en.ts`**

Update the following keys:

1. `appName`: `"Kronos"`
2. `onboarding.title`: `"Welcome to Kronos"`
3. `quitting.title`: `"Quitting Kronos..."`
4. `rootError.title`: `"Kronos ran into a problem."`
5. `daemon.manage.hint`: `"Let Kronos start and stop the built-in daemon"`
6. `daemon.manage.registrationFailed`: `"Built-in daemon started, but Kronos could not save the localhost connection. Toggle daemon management off and on again, or add localhost manually."`
7. `daemon.manage.pausedStopFailed`: `"Built-in daemon management was paused, but Kronos could not stop the daemon."`
8. `daemon.keepRunning.hint`: `"Daemon keeps running when you quit Kronos"`
9. `testNotification.title`: `"Kronos notification test"`
10. `sentDescription`: `"Kronos handed the notification to the operating system."`
11. `updates.installMessage`: `"This updates Kronos on this computer"`
12. `metadata.description`: `"Choose the model Kronos uses for workspace titles, branch names, commit messages, and pull request drafts"`
13. `browser.description`: `"Browser tabs share sign-ins and site data across Kronos."`
14. `browser.openLink.inApp`: `"In Kronos"`
15. `browser.openLink.inPaseo`: `"In Kronos"`
16. `tools.enableTools.title`: `"Enable Kronos tools"`
17. `tools.enableTools.accessibilityLabel`: `"Inject Kronos tools"`
18. `skills.updateTitle`: `"Update Kronos skills?"`
19. `skills.uninstallTitle`: `"Uninstall Kronos skills?"`
20. `skills.uninstallMessage`: `"Removes all Kronos orchestration skills from ~/.agents, ~/.claude, ~/.codex on this host."`
21. `direct.helper`: `"Enter the address of a Kronos server."`
22. `remoteSsh.helper`: `"Connect to a Kronos daemon running on the remote host."`
23. `relay.hint`: `"Scan this QR code with Kronos on your phone, or copy the link below."`
24. `relay.updateRequired`: `"Update the host to enable relay from Kronos Desktop."`
25. `daemon.restart.offlineMessage`: `"This host is offline. Kronos reconnects automatically-wait until it's back online before restarting."`
26. `daemon.restart.requestFailedMessage`: `"Failed to send the restart request. Kronos reconnects automatically-try again once the host shows as online."`
27. `daemon.update.desktopManagedHint`: `"This daemon is managed by Kronos Desktop. Update Kronos Desktop on the host."`
28. `archiveNotWorktree`: `"Archive isn't available here because this workspace was not created as a Kronos worktree"`
29. `runningIntel`: `"You're running the Intel build of Kronos under Rosetta on Apple Silicon."`
30. `cli.installFailed`: `"Unable to install the Kronos CLI."`
31. `builtin.owner`: `"Kronos"`
32. `paseoCalls.one`: `"called Kronos {{count}} time"`
33. `paseoCalls.other`: `"called Kronos {{count}} times"`
34. `serviceHint`: `"Kronos supervises the process and assigns a port via $PASEO_PORT"`
35. `metadata.info`: `"Project-specific instructions injected into the AI prompts Kronos uses to generate metadata - use them to enforce your team's conventions like branch naming, commit style, or PR format"`

- [ ] **Step 3: Run typecheck on app package**

```bash
npm run typecheck --workspace=@getpaseo/app
```

Expected: PASS (0 errors).

- [ ] **Step 4: Run i18n startup tests**

```bash
npx vitest run packages/app/src/i18n/i18next.startup.test.ts
```

Expected: PASS.

- [ ] **Step 5: Format and commit changes**

```bash
npx oxfmt packages/app/src/i18n/resources/en.ts
git add packages/app/src/i18n/resources/en.ts
git commit -m "feat(app): rebrand user-facing UI strings, notifications, and dialogs to Kronos"
```

---

### Task 2: System CLI Command & Deep Links (`packages/cli` & `packages/desktop-tauri`)

**Files:**

- Modify: `packages/cli/package.json`
- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json:37-40`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:313-318`
- Modify: `packages/desktop-tauri/src-tauri/Cargo.toml:4`
- Test: `packages/desktop-tauri/src-tauri/Cargo.toml` (`cargo check`)

**Interfaces:**

- Consumes: CLI bin map, Tauri deep-link plugin configuration, and Rust window event emitter.
- Produces: Dual CLI commands (`kronos` / `paseo`) and dual URI scheme triggers (`kronos://` / `paseo://`).

- [ ] **Step 1: Add `kronos` command to CLI package.json**

In `packages/cli/package.json`:

```json
"description": "Kronos CLI - control your AI coding agents from the command line",
"bin": {
  "kronos": "bin/paseo",
  "paseo": "bin/paseo"
},
```

- [ ] **Step 2: Add `kronos` scheme to `tauri.conf.json`**

In `packages/desktop-tauri/src-tauri/tauri.conf.json`:

```json
"deep-link": {
  "schemes": ["kronos", "paseo"]
},
```

- [ ] **Step 3: Update deep-link argument parser in `packages/desktop-tauri/src-tauri/src/main.rs`**

In `packages/desktop-tauri/src-tauri/src/main.rs` line ~314:

```rust
let deep_link = args.find(|a| a.starts_with("kronos://") || a.starts_with("paseo://"));
```

- [ ] **Step 4: Update package description in `Cargo.toml`**

In `packages/desktop-tauri/src-tauri/Cargo.toml`:

```toml
description = "Kronos Desktop App"
```

- [ ] **Step 5: Run Cargo check and CLI verification**

```bash
cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml
node -e "const pkg = require('./packages/cli/package.json'); if (pkg.bin.kronos && pkg.bin.paseo) console.log('CLI bin OK'); else process.exit(1);"
```

Expected: PASS.

- [ ] **Step 6: Format and commit changes**

```bash
npx oxfmt packages/cli/package.json packages/desktop-tauri/src-tauri/tauri.conf.json
git add packages/cli/package.json packages/desktop-tauri/src-tauri/tauri.conf.json packages/desktop-tauri/src-tauri/src/main.rs packages/desktop-tauri/src-tauri/Cargo.toml
git commit -m "feat(desktop,cli): alias kronos CLI command and register kronos:// deep-link scheme"
```

---

### Task 3: Documentation & Marketing Alignment (`README.md`, `docs/`)

**Files:**

- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `docs/architecture.md`

**Interfaces:**

- Consumes: Project documentation.
- Produces: Up-to-date Kronos developer setup guides and architecture documentation.

- [ ] **Step 1: Update README.md command examples & references**

In `README.md`:

- Change `paseo plugin add` to `kronos plugin add` (noting `paseo plugin add` also works).
- In Section `### Server`: note `kronos` / `paseo` command execution.
- In Section `### Docker`: update container name references where appropriate.

- [ ] **Step 2: Update README.zh-CN.md**

Synchronize the same command examples (`kronos plugin add`, `kronos` command) in `README.zh-CN.md`.

- [ ] **Step 3: Update `docs/architecture.md`**

In `docs/architecture.md`:

- Highlight `packages/desktop-tauri` as the production Tauri v2 desktop application.
- Clarify dual `kronos` / `paseo` command availability.

- [ ] **Step 4: Check markdown formatting and commit**

```bash
npx oxfmt README.md README.zh-CN.md docs/architecture.md
git add README.md README.zh-CN.md docs/architecture.md
git commit -m "docs: align README and architecture guides with Kronos branding and CLI aliases"
```

---

### Task 4: Full Static Verification & Regression Testing

**Files:**

- Test: Full monorepo typecheck and test suites.

- [ ] **Step 1: Monorepo workspace typecheck**

```bash
npm run typecheck --workspace=@getpaseo/app
npm run typecheck --workspace=@getpaseo/cli
npm run typecheck --workspace=@getpaseo/website
```

Expected: All 3 workspaces pass typecheck cleanly.

- [ ] **Step 2: Host runtime unit test suite**

```bash
npm run test --workspace=@getpaseo/app -- host-runtime.test.ts
```

Expected: 104 passed (104).

- [ ] **Step 3: Website production build**

```bash
npm run build --workspace=@getpaseo/website
```

Expected: Build succeeds with 0 errors.

- [ ] **Step 4: Verify working tree hygiene**

```bash
git status -sb
```

Expected: Clean working tree on branch `main`.
