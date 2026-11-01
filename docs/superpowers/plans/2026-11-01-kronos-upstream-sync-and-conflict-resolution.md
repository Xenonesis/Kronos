# Kronos Upstream Sync & Conflict Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sync upstream `getpaseo/paseo` updates (`upstream/main`, 48 commits ahead including v0.15.0-beta.1 fixes) into `Xenonesis/Kronos` `main` branch, resolving all 7 merge conflicts without regressing any desktop `.exe` features (Windows 11 Mica, Quake hotkeys, native PTY, auto-updater, instant boot).

**Architecture:** 3-way merge strategy (`git merge upstream/main`). For UI and tool call components, layer upstream terminal character-width & timeline improvements while preserving Kronos tactical styling, native terminal bridge, and output windowing. For server and release workflows, adopt upstream process failure protections while retaining Kronos package manifests (Winget/Scoop), Tauri release automations, and daemon health gating.

**Tech Stack:** Git, TypeScript/React Native/Expo, Rust (Tauri v2), Node.js, Vitest, Cargo.

## Global Constraints

- **Zero Regression on Current `.exe` Capabilities:**
  1. Windows 11 DWM Mica backdrop (`apply_mica_effect`) and `#0A0A0A` dark titlebar chrome in `packages/desktop-tauri/src-tauri/src/main.rs`.
  2. Dual global shortcuts (`Ctrl+Shift+K` and `Ctrl+Shift+P`) with `kronos://quick-toggle` event dispatch.
  3. Native Tauri IPC updater commands (`check_app_update`, `install_app_update`) and `latest.json` release manifest.
  4. Instant daemon connection (<500ms): parallel plugin initialization in `builtin/index.ts`, WebSocket-gated health in `bootstrap.ts`, and zero loopback backoff in `host-runtime.ts`.
  5. Native PTY terminal streaming (`NativeTerminalBridge`) directly connected to xterm.
  6. Rebranded Kronos endpoints pointing to `https://github.com/Xenonesis/Kronos`.
- **Typing & Linting:** Strict zero `any` (`ts-no-any`). All modified TypeScript files must pass `oxlint` and typecheck.
- **Verification:** `cargo check`, full workspace TypeScript typecheck, Vitest unit tests, and package manifest tests must pass cleanly before merge completion.

---

### Task 1: Initiate Upstream Merge and Inspect Working Tree State

**Files:**

- Merge branch: `upstream/main` into `main`

**Interfaces:**

- Consumes: `upstream/main` commit `6ae6308cf`, common ancestor `deb1bdc2666c7fdf964933875635767fe8b66c20`.
- Produces: Working tree in conflicted state with exactly 7 conflicted files marked.

- [ ] **Step 1: Execute git merge upstream/main**

```bash
git merge upstream/main
```

Expected: Git reports conflicts in:

1. `.github/workflows/desktop-release.yml`
2. `packages/app/src/components/add-project-flow.tsx`
3. `packages/app/src/components/terminal-emulator-contract.ts`
4. `packages/app/src/components/terminal-emulator.tsx`
5. `packages/app/src/components/tool-call-details.tsx`
6. `packages/cli/package.json`
7. `packages/server/src/server/agent/providers/acp-agent.ts`

- [ ] **Step 2: Verify git status confirms the exact 7 unmerged paths**

```bash
git status --porcelain | grep "^UU "
```

---

### Task 2: Resolve Server and CLI Conflicts (`acp-agent.ts`, `cli/package.json`)

**Files:**

- Modify: `packages/server/src/server/agent/providers/acp-agent.ts`
- Modify: `packages/cli/package.json`

**Interfaces:**

- Consumes: Upstream `rejectOnProcessFailure` and `waitForCommandsToSettle` quiet window logic.
- Produces: Clean `acp-agent.ts` with no duplicate exit/close handlers; `packages/cli/package.json` preserving `"bin": { "kronos": "bin/paseo", "paseo": "bin/paseo" }` and Kronos description.

- [ ] **Step 1: Resolve `packages/server/src/server/agent/providers/acp-agent.ts`**
      Upstream's `rejectOnProcessFailure` already listens to both `error` and `close` events, which cleanly subsumes our earlier `child.once("exit")` patch.
      Accept upstream's implementation for `rejectOnProcessFailure` and `waitForCommandsToSettle`.

- [ ] **Step 2: Resolve `packages/cli/package.json`**
      Keep Kronos branding in description and binary aliases:

```json
{
  "name": "@getpaseo/cli",
  "version": "0.15.0-beta.1",
  "description": "Kronos CLI - control your AI coding agents from the command line",
  "bin": {
    "kronos": "bin/paseo",
    "paseo": "bin/paseo"
  }, ...
}
```

- [ ] **Step 3: Stage resolved server/cli files**

```bash
git add packages/server/src/server/agent/providers/acp-agent.ts packages/cli/package.json
```

---

### Task 3: Resolve Terminal Emulator & Contract Conflicts (`terminal-emulator-contract.ts`, `terminal-emulator.tsx`)

**Files:**

- Modify: `packages/app/src/components/terminal-emulator-contract.ts`
- Modify: `packages/app/src/components/terminal-emulator.tsx`

**Interfaces:**

- Consumes: Upstream `characterWidths: TerminalCharacterWidths` and `replay: (event: TerminalReplayEvent) => void`.
- Produces: `TerminalEmulatorProps` and `TerminalEmulatorHandle` containing BOTH upstream character widths/replay AND Kronos native PTY bridge props (`sessionId`, `isLocal`, `cwd`, `command`, `args`, `nativeTerminal`, `isNative`).

- [ ] **Step 1: Merge `terminal-emulator-contract.ts`**
      Combine `NativeTerminalBridge` declarations with `TerminalCharacterWidths` and `replay`:

```typescript
import type { NativeTerminalBridge } from "@/desktop/host";
import type { TerminalCharacterWidths } from "../terminal/character-widths";

export interface TerminalEmulatorHandle {
  find?: TerminalFindHandle;
  writeOutput: (data: TerminalOutputData) => void;
  replay: (event: import("@getpaseo/client").TerminalReplayEvent) => void;
  paste: (text: string) => void;
  copySelection: (clipboard: TerminalClipboardWriter) => Promise<string>;
  clear: () => void;
  claimSize: () => void;
  showKeyboard: () => void;
  blur: () => void;
  isNative?: () => boolean;
}

export interface TerminalEmulatorProps {
  ref: Ref<TerminalEmulatorHandle>;
  streamKey: string;
  supportsTerminalInputModeReplay: boolean;
  characterWidths: TerminalCharacterWidths;
  sessionId?: string;
  isLocal?: boolean;
  cwd?: string;
  command?: string;
  args?: string[];
  nativeTerminal?: NativeTerminalBridge;
  testId?: string;
  xtermTheme?: ITheme;
  scrollbackLines: number;
  fontFamily?: string;
  fontSize?: number;
  swipeGesturesEnabled?: boolean;
  onSwipeLeft?: () => void;
  onSwipeRight?: () => void;
  onFindRequest?: () => void;
  onFindResult?: (result: TerminalFindResult) => void;
  onInput?: (data: string) => Promise<void> | void;
  ...
}
```

- [ ] **Step 2: Merge `packages/app/src/components/terminal-emulator.tsx`**
      Incorporate `characterWidths` prop, `characterWidthsRef`, `runtime.setCharacterWidths`, and `replay: (event) => ...`, while preserving `nativeConfigRef`, `isMacUserAgent()`, `nativeTerminal` instantiation in `TerminalEmulatorRuntime`, and `isNative: () => runtimeRef.current?.isNative ?? false`.

- [ ] **Step 3: Verify and stage terminal files**

```bash
npx oxlint packages/app/src/components/terminal-emulator-contract.ts packages/app/src/components/terminal-emulator.tsx
git add packages/app/src/components/terminal-emulator-contract.ts packages/app/src/components/terminal-emulator.tsx
```

---

### Task 4: Resolve UI Flow & Shell Tool Call Details Conflicts (`add-project-flow.tsx`, `tool-call-details.tsx`)

**Files:**

- Modify: `packages/app/src/components/add-project-flow.tsx`
- Modify: `packages/app/src/components/tool-call-details.tsx`

**Interfaces:**

- Consumes: Upstream `Modal` from `@/components/ui/modal` and removal of legacy `sub_agent` section.
- Produces: `AddProjectFlow` with instant desktop discovery + native pre-validation; `ToolCallDetails` with Industrial Brutalist shell toolbar (Exit badge, Copy Cmd/Out, Expand/Collapse, output windowing).

- [ ] **Step 1: Merge `packages/app/src/components/add-project-flow.tsx`**
      Adopt upstream `import { Modal } from "@/components/ui/modal";` while retaining all Kronos desktop enhancements (`SpinnerFlowIcon`, `isHostConnecting`, `shouldUseDesktopDaemon()`, `window.paseoDesktop?.validateDirectory`, etc.).

- [ ] **Step 2: Merge `packages/app/src/components/tool-call-details.tsx`**
      Preserve Kronos tactical `ShellDetailSection` with `formatDisplayShellOutput`, `isExpanded`, `copyCmdIcon`, `copyOutIcon`, `shellExitBadge`, and `styles.shellToolbar`.
      Accept upstream's removal of `SubAgentDetailSection` since sub-agents are now first-class timeline launch markers.

- [ ] **Step 3: Verify and stage UI components**

```bash
npx oxlint packages/app/src/components/add-project-flow.tsx packages/app/src/components/tool-call-details.tsx
git add packages/app/src/components/add-project-flow.tsx packages/app/src/components/tool-call-details.tsx
```

---

### Task 5: Resolve Workflow Conflicts (`desktop-release.yml`) and Audit Auto-Merged Code

**Files:**

- Modify: `.github/workflows/desktop-release.yml`
- Audit: `packages/server/src/server/bootstrap.ts`
- Audit: `packages/app/src/runtime/host-runtime.ts`
- Audit: `packages/server/src/server/plugins/builtin/index.ts`
- Audit: `packages/desktop-tauri/src-tauri/src/main.rs`

**Interfaces:**

- Consumes: Upstream macos runner matrix (`macos-latest`, `macos-26-intel`).
- Produces: Intact `.github/workflows/desktop-release.yml` containing Winget and Scoop manifest generator steps and asset upload to `Xenonesis/Kronos`.

- [ ] **Step 1: Merge `.github/workflows/desktop-release.yml`**
      Update the macos runner definitions while retaining all Kronos package manifest generation (`node scripts/generate-windows-package-manifests.mjs`), `latest.json` upload, and `--repo Xenonesis/Kronos`.

- [ ] **Step 2: Audit auto-merged files for critical Kronos runtime guarantees**

1. Check `packages/server/src/server/bootstrap.ts` to ensure `let isDaemonReady = false;` and `/api/health` 503 gating remain active.
2. Check `packages/app/src/runtime/host-runtime.ts` to ensure 250ms loopback polling interval (zero 30s backoff) remains active.
3. Check `packages/server/src/server/plugins/builtin/index.ts` to ensure `Promise.all` parallel loading remains active.
4. Check `packages/desktop-tauri/src-tauri/src/main.rs` to ensure `apply_mica_effect`, `kronos://quick-toggle`, `check_app_update`, and `install_app_update` are unchanged.

- [ ] **Step 3: Stage `.github/workflows/desktop-release.yml`**

```bash
git add .github/workflows/desktop-release.yml
```

---

### Task 6: Comprehensive Verification & Merge Finalization

**Files:**

- All modified and auto-merged files across workspaces

- [ ] **Step 1: Run Rust Tauri compilation checks**

```bash
cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml
cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml
```

Expected: 0 errors, 82/82 tests pass.

- [ ] **Step 2: Run TypeScript and Package Manifest checks**

```bash
pnpm --filter @paseo/app check:type
npm run test:windows-package-manifests
npx vitest run packages/app/src/desktop/updates/desktop-updates.test.ts
```

Expected: All tests pass, zero type errors.

- [ ] **Step 3: Finalize Git Merge Commit and Push**

```bash
git commit -m "merge: sync upstream main (v0.15.0-beta.1) and resolve conflicts while preserving Kronos desktop features"
git push origin main
```

Expected: Working tree clean, main synced with remote origin.
