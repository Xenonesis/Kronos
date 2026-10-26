# Kronos Performance, Memory & Reliability Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 5 core performance, reliability, and UX enhancements across Kronos Desktop: instant zero-race project opening with full error propagation, OpenCode process idle auto-hibernation to cut idle RAM to 0 MB, tactical live telemetry HUD (TPS, latency, cost), prompt cache stabilization with proactive context compaction warnings, and native Tauri fast path directory validation.

**Architecture:**

1. **Frontend Project Addition**: Synchronize desktop daemon status directly inside `useOpenProject` and `openProjectDirectly` to eliminate race conditions right at boot, and propagate explicit server error strings in `add-project-flow.tsx`.
2. **Backend Daemon Server Management**: Add an idle hibernation timer (`OPENCODE_IDLE_TIMEOUT_MS`) to `OpenCodeServerManager` that gracefully shuts down `opencode serve` when no active turns exist for >15 minutes and restarts on demand in ~300ms.
3. **Tactical Telemetry HUD**: Implement a real-time streaming telemetry meter calculating tokens-per-second (TPS), time-to-first-token (TTFT), and session cost styled in Kronos Industrial Brutalism.
4. **Agent Context Optimization**: Preserve byte-stable prompt prefixes across turns for OpenCode and display proactive context compaction alerts at ≥75% context threshold.
5. **Native Desktop Fast Path**: Add a native Rust `fast_validate_directory` Tauri command for sub-millisecond local path inspection prior to daemon submission.

**Tech Stack:** React 19, React Native Web, Unistyles, Tauri v2, Rust (`std::fs`), Node.js, TypeScript, Vitest.

**Spec:** Local Kronos enhancements covering developer experience, process memory footprint, and telemetry.

## Global Constraints

- Retain Industrial Brutalism aesthetics: CRT substrate `#0A0A0A`, text `#EAEAEA`, hazard accent `#FF2A2A`, strict 90° corners (`rounded-none`), monospace formatting.
- Strict TypeScript: no `: any` or `as any` (`ts-no-any`).
- Backward compatibility: existing Paseo host runtime protocols and WebSocket envelopes must remain intact.
- Every task must include tests and verify passing runs.

---

### Task 1: Instant Zero-Race Project Addition & Unmasked Error Propagation

**Files:**

- Modify: `packages/app/src/hooks/open-project.ts:80-116`
- Modify: `packages/app/src/hooks/use-open-project.ts:18-56`
- Modify: `packages/app/src/components/add-project-flow.tsx:543-575`
- Test: `packages/app/src/hooks/open-project.test.ts`

**Interfaces:**

- Consumes: `OpenProjectDirectlyInput` from `packages/app/src/hooks/open-project.ts`.
- Produces: `openProjectDirectly` with automatic connection awaiting for local host, fallback `canAddProject` for built-in local server, and transparent error strings in `openAddedProject`.

- [ ] **Step 1: Write the failing unit tests for `openProjectDirectly` and unmasked error propagation**

Create/update `packages/app/src/hooks/open-project.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { openProjectDirectly } from "./open-project";

describe("openProjectDirectly", () => {
  it("returns explicit error message when host is disconnected", async () => {
    const result = await openProjectDirectly({
      serverId: "local",
      projectPath: "C:/projects/demo",
      isConnected: false,
      canAddProject: true,
      client: null,
      upsertProject: vi.fn(),
      setHasHydratedWorkspaces: vi.fn(),
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("Connecting to host... Please wait a moment and try again.");
    }
  });

  it("returns explicit error message when canAddProject is false", async () => {
    const result = await openProjectDirectly({
      serverId: "local",
      projectPath: "C:/projects/demo",
      isConnected: true,
      canAddProject: false,
      client: { addProject: vi.fn() },
      upsertProject: vi.fn(),
      setHasHydratedWorkspaces: vi.fn(),
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("Update the host to add projects without creating a workspace.");
    }
  });

  it("propagates daemon error string directly on add failure", async () => {
    const mockClient = {
      addProject: vi.fn().mockResolvedValue({
        error: "Permission denied reading directory C:/root",
        errorCode: "permission_denied",
      }),
    };

    const result = await openProjectDirectly({
      serverId: "local",
      projectPath: "C:/root",
      isConnected: true,
      canAddProject: true,
      client: mockClient,
      upsertProject: vi.fn(),
      setHasHydratedWorkspaces: vi.fn(),
    });

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("Permission denied reading directory C:/root");
      expect(result.errorCode).toBe("permission_denied");
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run packages/app/src/hooks/open-project.test.ts`
Expected: FAIL due to missing descriptive error on disconnected host.

- [ ] **Step 3: Implement minimal fix in `open-project.ts`, `use-open-project.ts`, and `add-project-flow.tsx`**

In `packages/app/src/hooks/open-project.ts`:

```ts
export async function openProjectDirectly(
  input: OpenProjectDirectlyInput,
): Promise<OpenProjectResult> {
  const normalizedServerId = input.serverId.trim();
  const trimmedPath = input.projectPath.trim();
  if (!normalizedServerId || !trimmedPath) {
    return { ok: false, errorCode: null, error: "Project path cannot be empty." };
  }

  if (!input.client || !input.isConnected) {
    return {
      ok: false,
      errorCode: null,
      error: "Connecting to host... Please wait a moment and try again.",
    };
  }

  if (!input.canAddProject) {
    return {
      ok: false,
      errorCode: null,
      error: "Update the host to add projects without creating a workspace.",
    };
  }

  const payload = await input.client.addProject(trimmedPath);
  if (payload.error || !payload.project) {
    return {
      ok: false,
      errorCode: payload.errorCode ?? null,
      error: payload.error ?? "Failed to add project to host.",
    };
  }

  const registered = registerProjectDescriptor({
    serverId: normalizedServerId,
    project: payload.project,
    upsertProject: input.upsertProject,
    setHasHydratedWorkspaces: input.setHasHydratedWorkspaces,
  });
  return registered
    ? { ok: true, project: payload.project }
    : { ok: false, errorCode: null, error: "Unable to register project" };
}
```

In `packages/app/src/hooks/use-open-project.ts`:

```ts
export function useOpenProject(
  serverId: string | null,
  isBuiltInLocal = false,
): (path: string) => Promise<OpenProjectResult> {
  const normalizedServerId = serverId?.trim() ?? "";
  const client = useHostRuntimeClient(normalizedServerId);
  const isConnected = useHostRuntimeIsConnected(normalizedServerId);
  const canAddProject = useSessionStore((state) =>
    normalizedServerId
      ? isBuiltInLocal ||
        (state.sessions[normalizedServerId]?.serverInfo?.features?.projectAdd === true &&
          state.sessions[normalizedServerId]?.serverInfo?.features?.stableProjectIdentity === true)
      : false,
  );
...
```

In `packages/app/src/components/add-project-flow.tsx:547-563`:

```tsx
const result = await openProject(path);
if (result.ok) {
  openNewWorkspaceForProject(hostId, result.project);
  return;
}
const reason = getOpenProjectFailureReason(result);
const message =
  reason === "directory_not_found"
    ? "Directory not found"
    : (result.error ?? "Unable to add project");
setState((current) => setPageStatus(current, sourceKind, { isSubmitting: false, error: message }));
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `npx vitest run packages/app/src/hooks/open-project.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/app/src/hooks/open-project.ts packages/app/src/hooks/use-open-project.ts packages/app/src/components/add-project-flow.tsx packages/app/src/hooks/open-project.test.ts
git commit -m "fix(desktop): eliminate project add race condition and preserve server errors"
```

---

### Task 2: OpenCode Process Idle Hibernation & Memory Management

**Files:**

- Modify: `packages/server/src/server/agent/providers/opencode/server-manager.ts:25-35, 210-245`
- Test: `packages/server/src/server/agent/providers/opencode-server-manager.test.ts`

**Interfaces:**

- Consumes: `OpenCodeServerManager` process lifecycle.
- Produces: `idleTimeoutMs` on `OpenCodeServerManagerOptions`, graceful termination when server has had 0 active turns for > `idleTimeoutMs`, transparent auto-respawn on subsequent `acquireCurrent()`.

- [ ] **Step 1: Write unit tests for OpenCode server idle hibernation**

In `packages/server/src/server/agent/providers/opencode-server-manager.test.ts`:

```ts
it("automatically hibernates server after configured idle duration with zero active refs", async () => {
  vi.useFakeTimers();
  const manager = new OpenCodeServerManager({
    logger,
    idleTimeoutMs: 5_000,
    spawnServerProcess: mockSpawner,
    portAllocator: async () => 9090,
  });

  const acquisition = await manager.acquireCurrent();
  expect(manager.isServerAlive(manager.getCurrentServer())).toBe(true);

  // Turn completes and releases acquisition
  await acquisition.release();

  // Fast-forward past idle timeout
  await vi.advanceTimersByTimeAsync(5_001);

  // Server should now be shut down to save RAM
  expect(manager.getCurrentServer()).toBe(null);
  vi.useRealTimers();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run packages/server/src/server/agent/providers/opencode-server-manager.test.ts`
Expected: FAIL because idle timer does not exist yet.

- [ ] **Step 3: Implement idle hibernation in `server-manager.ts`**

In `packages/server/src/server/agent/providers/opencode/server-manager.ts`:

- Define `export const DEFAULT_OPENCODE_IDLE_TIMEOUT_MS = 15 * 60 * 1000;` (15 minutes).
- Add `private idleTimer: NodeJS.Timeout | null = null;` and `private readonly idleTimeoutMs: number;`.
- In `releaseServer(server: OpenCodeServerGeneration)`:
  - When `server.refCount === 0` and `this.currentServer === server`:
    - Clear any existing `this.idleTimer`.
    - Set `this.idleTimer = setTimeout(() => { void this.hibernateIdleServer(server); }, this.idleTimeoutMs);`
- In `acquireServer(server)`:
  - If `this.idleTimer` is running, call `clearTimeout(this.idleTimer); this.idleTimer = null;`.
- Implement `private async hibernateIdleServer(server: OpenCodeServerGeneration): Promise<void>`:
  - Verify `server.refCount === 0 && this.currentServer === server`.
  - Log `OpenCode server hibernated after idle timeout to reclaim memory`.
  - Set `this.currentServer = null; server.retired = true;`.
  - Await `this.killServer(server);`.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run packages/server/src/server/agent/providers/opencode-server-manager.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/server/src/server/agent/providers/opencode/server-manager.ts packages/server/src/server/agent/providers/opencode-server-manager.test.ts
git commit -m "perf(opencode): add idle auto-hibernation to reclaim background RAM"
```

---

### Task 3: Tactical Live Telemetry HUD (Tokens/Sec, Latency, and Cost Counter)

**Files:**

- Create: `packages/app/src/components/telemetry-hud.tsx`
- Create: `packages/app/src/components/telemetry-hud.test.tsx`
- Modify: `packages/app/src/composer/index.tsx:55-80, 200-240`

**Interfaces:**

- Consumes: Stream token events (`text_delta`), prompt submission timestamps from session store.
- Produces: `<TelemetryHud tps={number} ttftMs={number | null} costUsd={number | null} />` rendering inside composer.

- [ ] **Step 1: Write test for `TelemetryHud` formatting and render**

In `packages/app/src/components/telemetry-hud.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react-native";
import { describe, expect, it } from "vitest";
import { TelemetryHud, calculateTps } from "./telemetry-hud";

describe("TelemetryHud", () => {
  it("calculates TPS correctly given token count and elapsed milliseconds", () => {
    expect(calculateTps(120, 2000)).toBe(60);
    expect(calculateTps(0, 500)).toBe(0);
    expect(calculateTps(50, 0)).toBe(0);
  });

  it("renders tactical telemetry badges with TPS and TTFT", () => {
    render(<TelemetryHud tps={45.2} ttftMs={380} isStreaming={true} />);
    expect(screen.getByText(/45\.2 TPS/)).toBeTruthy();
    expect(screen.getByText(/380ms TTFT/)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run packages/app/src/components/telemetry-hud.test.tsx`
Expected: FAIL due to missing `telemetry-hud.tsx`.

- [ ] **Step 3: Implement `telemetry-hud.tsx` in Industrial Brutalism styling**

Create `packages/app/src/components/telemetry-hud.tsx`:

```tsx
import React from "react";
import { StyleSheet, Text, View } from "react-native";

export function calculateTps(tokens: number, elapsedMs: number): number {
  if (elapsedMs <= 0 || tokens <= 0) return 0;
  return Number(((tokens / elapsedMs) * 1000).toFixed(1));
}

export interface TelemetryHudProps {
  tps?: number;
  ttftMs?: number | null;
  costUsd?: number | null;
  isStreaming?: boolean;
}

export function TelemetryHud({ tps = 0, ttftMs, costUsd, isStreaming }: TelemetryHudProps) {
  if (!isStreaming && tps === 0 && !ttftMs) return null;

  return (
    <View style={styles.container} testID="telemetry-hud">
      {isStreaming && tps > 0 && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>SPD</Text>
          <Text style={styles.badgeValue}>{tps} TPS</Text>
        </View>
      )}
      {ttftMs !== undefined && ttftMs !== null && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>LAT</Text>
          <Text style={styles.badgeValue}>{ttftMs}ms TTFT</Text>
        </View>
      )}
      {costUsd !== undefined && costUsd !== null && costUsd > 0 && (
        <View style={styles.badge}>
          <Text style={styles.badgeLabel}>COST</Text>
          <Text style={styles.badgeValue}>${costUsd.toFixed(4)}</Text>
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: "row",
    alignItems: "center",
    gap: 6,
    paddingHorizontal: 6,
    paddingVertical: 2,
    backgroundColor: "#0A0A0A",
    borderWidth: 1,
    borderColor: "#222222",
  },
  badge: {
    flexDirection: "row",
    alignItems: "center",
    gap: 4,
  },
  badgeLabel: {
    fontSize: 10,
    fontFamily: "monospace",
    color: "#FF2A2A",
    fontWeight: "700",
  },
  badgeValue: {
    fontSize: 10,
    fontFamily: "monospace",
    color: "#EAEAEA",
  },
});
```

Mount `TelemetryHud` in `packages/app/src/composer/index.tsx` alongside the context meter.

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run packages/app/src/components/telemetry-hud.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/app/src/components/telemetry-hud.tsx packages/app/src/components/telemetry-hud.test.tsx packages/app/src/composer/index.tsx
git commit -m "feat(ui): add tactical telemetry HUD for real-time TPS, TTFT, and cost"
```

---

### Task 4: Prompt Cache Prefix Stability & Proactive Compaction Warning

**Files:**

- Modify: `packages/app/src/components/context-window-meter.tsx:60-90`
- Modify: `packages/server/src/server/agent/providers/opencode-agent.ts:1210-1250`
- Test: `packages/app/src/components/context-window-meter.test.ts`

**Interfaces:**

- Consumes: `percentage` from `ContextWindowMeter`.
- Produces: Proactive `/compact` suggestion pill when context usage reaches ≥ 75%.

- [ ] **Step 1: Write test for proactive context compaction warning threshold**

In `packages/app/src/components/context-window-meter.test.ts`:

```ts
it("triggers proactive compact warning at or above 75% context capacity", () => {
  expect(shouldShowCompactWarning(74)).toBe(false);
  expect(shouldShowCompactWarning(75)).toBe(true);
  expect(shouldShowCompactWarning(92)).toBe(true);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run packages/app/src/components/context-window-meter.test.ts`
Expected: FAIL due to missing `shouldShowCompactWarning`.

- [ ] **Step 3: Implement `shouldShowCompactWarning` and UI indicator**

In `packages/app/src/components/context-window-meter.utils.ts`:

```ts
export function shouldShowCompactWarning(percentage: number): boolean {
  return percentage >= 75;
}
```

In `packages/app/src/components/context-window-meter.tsx`:
Add a tactical compact warning chip beside the meter if `shouldShowCompactWarning(percentage)` is true, prompting the user with:
`[COMPACT RECOMMENDED: /compact]` in hazard red (`#FF2A2A`).

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run packages/app/src/components/context-window-meter.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/app/src/components/context-window-meter.tsx packages/app/src/components/context-window-meter.utils.ts packages/app/src/components/context-window-meter.test.ts
git commit -m "feat(agent): add proactive context compaction warning at 75% threshold"
```

---

### Task 5: Native Tauri Fast Directory & File Path Stat

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/main.rs:115-160`
- Modify: `packages/desktop-tauri/src-tauri/capabilities/default.json`
- Modify: `packages/desktop-tauri/src/lib/pick-directory.ts`
- Test: Build check via `cargo check` in `packages/desktop-tauri/src-tauri`

**Interfaces:**

- Consumes: Path string from File Explorer dialog.
- Produces: Native command `fast_validate_directory` returning `{ exists: bool, is_dir: bool, readable: bool, error: Option<String> }` in <1ms.

- [ ] **Step 1: Write Rust command in `packages/desktop-tauri/src-tauri/src/main.rs`**

```rust
#[derive(serde::Serialize)]
struct DirectoryValidationResult {
    exists: bool,
    is_dir: bool,
    readable: bool,
    error: Option<String>,
}

#[tauri::command]
fn fast_validate_directory(path: String) -> DirectoryValidationResult {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return DirectoryValidationResult {
            exists: false,
            is_dir: false,
            readable: false,
            error: Some("Directory does not exist.".to_string()),
        };
    }
    let metadata = match std::fs::metadata(p) {
        Ok(m) => m,
        Err(e) => return DirectoryValidationResult {
            exists: true,
            is_dir: false,
            readable: false,
            error: Some(format!("Cannot access path: {}", e)),
        },
    };
    if !metadata.is_dir() {
        return DirectoryValidationResult {
            exists: true,
            is_dir: false,
            readable: false,
            error: Some("Selected path is a file, not a directory.".to_string()),
        };
    }
    match std::fs::read_dir(p) {
        Ok(_) => DirectoryValidationResult {
            exists: true,
            is_dir: true,
            readable: true,
            error: None,
        },
        Err(e) => DirectoryValidationResult {
            exists: true,
            is_dir: true,
            readable: false,
            error: Some(format!("Permission denied reading directory: {}", e)),
        },
    }
}
```

Register `fast_validate_directory` in `generate_handler![...]` in `main.rs`.

- [ ] **Step 2: Add `fast_validate_directory` to Tauri capabilities if required**

Ensure `packages/desktop-tauri/src-tauri/capabilities/default.json` permits command invocation.

- [ ] **Step 3: Verify Rust compilation**

Run: `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: Finished with 0 errors.

- [ ] **Step 4: Wire validation check into `packages/app/src/components/add-project-flow.tsx`**

Before passing path to `openProject(path)`, invoke `window.paseoDesktop.validateDirectory(path)` if available to provide instantaneous pre-flight validation.

- [ ] **Step 5: Commit**

```bash
git add packages/desktop-tauri/src-tauri/src/main.rs packages/desktop-tauri/src-tauri/capabilities/default.json packages/app/src/components/add-project-flow.tsx
git commit -m "feat(tauri): add native fast directory pre-validation command"
```

---

### Task 6: End-to-End Verification & Installer Build

**Files:**

- Run suite: `npm run typecheck --workspaces --if-present`
- Run lint: `npm run lint`
- Run tests: `npm test`
- Build installer: `node scripts/update-and-build-exe.mjs --publish`

- [ ] **Step 1: Run typechecks and lint across the monorepo**
      Run: `npm run typecheck --workspaces --if-present && npm run lint`
      Expected: 0 errors.

- [ ] **Step 2: Run all provider and frontend tests**
      Run: `npx vitest run packages/server/src/server/agent/providers/opencode-agent.test.ts packages/app/src/components/add-project-flow/model.test.ts`
      Expected: All tests PASS.

- [ ] **Step 3: Rebuild and publish updated Kronos NSIS installer**
      Run: `node scripts/update-and-build-exe.mjs --force --publish`
      Expected: Fresh installer published to GitHub Releases.
