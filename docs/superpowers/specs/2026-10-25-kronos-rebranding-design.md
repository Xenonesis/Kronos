# Kronos Unified Rebranding Design Spec

- **Date**: 2026-10-25
- **Status**: Approved
- **Scope**: Option A (UI & Branding Focus + CLI & Deep-Link Aliases)
- **Target Repository**: `https://github.com/Xenonesis/Kronos.git`

---

## 1. Executive Summary

Kronos is the high-performance AI developer workstation and autonomous agent swarm engine, migrated from Electron to a native Rust Tauri v2 desktop architecture (<100MB RAM, sub-second boot, NSIS installer distribution).

While the desktop executable, repository name, and marketing website are named Kronos, various user-facing UI strings, desktop notification headers, quitting/onboarding dialogs, CLI command invocations, and documentation still refer to "Paseo".

This specification establishes a clean, unified Kronos identity across all user-visible surfaces and developer touchpoints, while deliberately maintaining upstream compatibility with `getpaseo/paseo:main` so future protocol updates, AI models, and bug fixes can be merged without conflicts.

---

## 2. Goals & Non-Goals

### Goals

1. **100% Consistent Kronos User Experience**:
   - Every user-facing UI element (Welcome/Onboarding, Settings, Notifications, Quitting dialogs, Error messages, Tooltips, System Tray) consistently displays **Kronos**.
2. **First-Class CLI Command**:
   - Provide `kronos` command in terminal alongside existing `paseo` binary.
3. **Deep Link Scheme Unification**:
   - Register and handle `kronos://` deep-link scheme alongside `paseo://`.
4. **Documentation & Website Alignment**:
   - Update repository `README.md`, `README.zh-CN.md`, `docs/architecture.md`, and `packages/website` metadata/copy to reflect Kronos architecture, features, and direct download links.
5. **Zero Upstream Conflicts**:
   - Avoid renaming internal NPM package scopes (`@getpaseo/*`) or filesystem storage directories (`~/.paseo`) to ensure frictionless upstream merges.

### Non-Goals

- Renaming internal package namespaces (`@getpaseo/protocol`, `@getpaseo/server`, `@getpaseo/relay`, `@getpaseo/client`).
- Renaming low-level internal wire protocol message schemas or JSON RPC identifiers.
- Breaking backwards compatibility for users with existing `~/.paseo` state directories.

---

## 3. Detailed Component Changes

### 3.1 Frontend UI & String Resources (`packages/app`)

**File**: `packages/app/src/i18n/resources/en.ts`

Update all user-facing string occurrences from "Paseo" to "Kronos":

- **Identity & Navigation**:
  - `appName`: `"Kronos"`
  - `onboarding.title`: `"Welcome to Kronos"`
  - `quitting.title`: `"Quitting Kronos..."`
  - `rootError.title`: `"Kronos ran into a problem."`
- **Settings & Daemon Management**:
  - `daemon.manage.hint`: `"Let Kronos start and stop the built-in daemon"`
  - `daemon.manage.registrationFailed`: `"Built-in daemon started, but Kronos could not save the localhost connection..."`
  - `daemon.manage.pausedStopFailed`: `"Built-in daemon management was paused, but Kronos could not stop the daemon."`
  - `daemon.keepRunning.hint`: `"Daemon keeps running when you quit Kronos"`
  - `updates.installMessage`: `"This updates Kronos on this computer"`
  - `metadata.description`: `"Choose the model Kronos uses for workspace titles, branch names, commit messages, and pull request drafts"`
  - `browser.description`: `"Browser tabs share sign-ins and site data across Kronos."`
  - `browser.openLink.inApp`: `"In Kronos"`
  - `browser.openLink.inPaseo`: `"In Kronos"`
- **Notifications & Tooling**:
  - `testNotification.title`: `"Kronos notification test"`
  - `sentDescription`: `"Kronos handed the notification to the operating system."`
  - `skills.updateTitle`: `"Update Kronos skills?"`
  - `skills.uninstallTitle`: `"Uninstall Kronos skills?"`
  - `tools.enableTools.title`: `"Enable Kronos tools"`
  - `tools.enableTools.accessibilityLabel`: `"Inject Kronos tools"`
- **Connections & Host Management**:
  - `direct.helper`: `"Enter the address of a Kronos server."`
  - `remoteSsh.helper`: `"Connect to a Kronos daemon running on the remote host."`
  - `relay.hint`: `"Scan this QR code with Kronos on your phone, or copy the link below."`
  - `relay.updateRequired`: `"Update the host to enable relay from Kronos Desktop."`
  - `daemon.restart.offlineMessage`: `"This host is offline. Kronos reconnects automatically-wait until it's back online before restarting."`

### 3.2 System Integration & CLI (`packages/cli` & `packages/desktop-tauri`)

#### 1. CLI Commands (`packages/cli/package.json`)

Register `kronos` in the `bin` map:

```json
"bin": {
  "kronos": "bin/paseo",
  "paseo": "bin/paseo"
}
```

Update package description: `"Kronos CLI - control your AI coding agents from the command line"`.

#### 2. Deep-Link Schemes (`packages/desktop-tauri/src-tauri/tauri.conf.json`)

Configure Tauri to register both URI schemes with Windows/macOS:

```json
"deep-link": {
  "schemes": ["kronos", "paseo"]
}
```

#### 3. Deep-Link Listener (`packages/desktop-tauri/src-tauri/src/main.rs`)

Update desktop runtime deep-link parser to dispatch on both `kronos://` and `paseo://` payloads:

```rust
let deep_link = args.find(|a| a.starts_with("kronos://") || a.starts_with("paseo://"));
```

#### 4. Desktop Metadata (`packages/desktop-tauri/src-tauri/Cargo.toml`)

Update application package description:

```toml
description = "Kronos Desktop App"
```

### 3.3 Documentation & Website (`README.md`, `docs/`, `packages/website`)

#### 1. Primary Readmes (`README.md`, `README.zh-CN.md`)

- Ensure headers display:
  `Kronos — The Apex AI Developer Workstation & Autonomous Agent Swarm Engine`
- Ensure repository links point to `https://github.com/Xenonesis/Kronos.git`.
- Ensure release download badges point to `https://github.com/Xenonesis/Kronos/releases/latest/download/Kronos_x64-setup.exe`.
- Document Tauri v2 architecture (<100MB RAM, instant boot) and CLI usage (`kronos`).

#### 2. Architecture Documentation (`docs/architecture.md`)

- Clarify `packages/desktop-tauri` is the production desktop client targeting NSIS installers (`Kronos_x64-setup.exe`).
- Note dual CLI support (`kronos` / `paseo`).

#### 3. Website Pages & Metadata (`packages/website`)

- Verify page titles in `packages/website/src/meta.ts` and `routes/index.tsx` state "Kronos".
- Verify downloads page serves `Kronos_x64-setup.exe` under `/Kronos/` subpath.

---

## 4. Invariants & Compatibility Guardrails

1. **Storage Integrity**:
   - `~/.paseo/server-id`, `~/.paseo/local-credential`, and `~/.paseo/agents/` remain unchanged to guarantee that users do not lose data or credentials.
2. **Protocol Compatibility**:
   - Internal WebSocket messages and RPC method names remain unchanged to preserve daemon-client interoperability.
3. **No Dynamic Key Regression**:
   - All i18n translation keys in `en.ts` retain their exact parameter tokens (`{{count}}`, `{{brand}}`, `{{version}}`).

---

## 5. Verification Plan

1. **TypeScript Typecheck**:
   - `npm run typecheck --workspace=@getpaseo/app`
   - `npm run typecheck --workspace=@getpaseo/cli`
   - `npm run typecheck --workspace=@getpaseo/website`
2. **Rust Compile & Schema Validation**:
   - `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
3. **Unit Tests**:
   - `npm run test --workspace=@getpaseo/app -- host-runtime.test.ts`
   - `npm run test --workspace=@getpaseo/website`
4. **Website Production Build**:
   - `npm run build --workspace=@getpaseo/website`
5. **Git Hygiene**:
   - Working tree clean, zero uncommitted generated artifacts, and atomic commits.
