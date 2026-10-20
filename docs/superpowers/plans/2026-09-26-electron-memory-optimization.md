# Electron Hardening & Memory Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce Paseo desktop idle RAM by 40-50% through Chromium flag tuning, webview/CDP hibernation, and daemon cache pruning.

**Architecture:** Electron main process controls Chromium flags and memory-pressure triggers; browser-webviews module introduces idle tab hibernation with thumbnail caching; server daemon trims terminal/git buffers when idle.

**Tech Stack:** TypeScript, Electron, Chromium commandLine API, Node.js V8 API, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-09-26-electron-memory-optimization-design.md`

## Global Constraints

- Preserve all existing CDP automation and agent interaction functionality.
- Vitest unit tests must pass via `npm run test` in `packages/desktop`.
- No broken backward compatibility for `@getpaseo/protocol`.

---

### Task 1: Chromium Flags & Memory Pressure Hooks

**Files:**

- Modify: `packages/desktop/src/main.ts`
- Create: `packages/desktop/src/features/memory-optimizer.ts`
- Test: `packages/desktop/src/features/memory-optimizer.test.ts`

**Interfaces:**

- Produces: `setupMemoryOptimization(app: Electron.App): void`

- [ ] **Step 1: Write the failing unit test**

Create `packages/desktop/src/features/memory-optimizer.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
import {
  configureChromiumMemoryFlags,
  registerMemoryPressureHandlers,
} from "./memory-optimizer.js";

describe("memory-optimizer", () => {
  it("appends required memory and tab-discarding flags", () => {
    const appended: Record<string, string | undefined> = {};
    const mockCommandLine = {
      appendSwitch: (key: string, val?: string) => {
        appended[key] = val;
      },
      hasSwitch: () => false,
    };

    configureChromiumMemoryFlags(mockCommandLine as any);
    expect(appended["enable-features"]).toContain("AutomaticTabDiscarding");
    expect(appended["js-flags"]).toContain("--max-old-space-size=256");
  });

  it("registers memory-pressure listener when supported", () => {
    const listeners: Record<string, Function> = {};
    const mockApp = {
      on: (event: string, cb: Function) => {
        listeners[event] = cb;
      },
    };
    registerMemoryPressureHandlers(mockApp as any);
    expect(typeof listeners["render-process-gone"]).toBe("function");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix packages/desktop run test -- memory-optimizer.test.ts`
Expected: FAIL (module not found)

- [ ] **Step 3: Implement memory-optimizer module**

Create `packages/desktop/src/features/memory-optimizer.ts`:

```typescript
export interface CommandLineInterface {
  appendSwitch(key: string, value?: string): void;
  hasSwitch(key: string): boolean;
}

export function configureChromiumMemoryFlags(commandLine: CommandLineInterface): void {
  if (!commandLine.hasSwitch("enable-features")) {
    commandLine.appendSwitch("enable-features", "AutomaticTabDiscarding,WebContentsDiscarding");
  }
  if (!commandLine.hasSwitch("js-flags")) {
    commandLine.appendSwitch("js-flags", "--max-old-space-size=256");
  }
}

export function registerMemoryPressureHandlers(app: {
  on: (event: string, cb: (...args: any[]) => void) => void;
}): void {
  app.on("render-process-gone", (_event, details) => {
    if (details.reason === "killed" || details.reason === "crashed") {
      // Reclaim / clean orphan references
    }
  });
}

export function setupMemoryOptimization(app: any): void {
  configureChromiumMemoryFlags(app.commandLine);
  registerMemoryPressureHandlers(app);
}
```

- [ ] **Step 4: Integrate into main.ts and run test**

Modify `packages/desktop/src/main.ts` to call `setupMemoryOptimization(app)` before ready.
Run: `npm --prefix packages/desktop run test -- memory-optimizer.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/desktop/src/features/memory-optimizer.ts packages/desktop/src/features/memory-optimizer.test.ts packages/desktop/src/main.ts
git commit -m "feat(desktop): configure chromium memory flags and pressure handlers"
```

---

### Task 2: Webview Inactive Tab Hibernation State Machine

**Files:**

- Create: `packages/desktop/src/features/browser-webviews/hibernation.ts`
- Test: `packages/desktop/src/features/browser-webviews/hibernation.test.ts`
- Modify: `packages/desktop/src/features/browser-webviews/registry.ts`

**Interfaces:**

- Produces: `TabHibernationManager` class with `markActive(id)`, `markIdle(id)`, `shouldHibernate(id)`.

- [ ] **Step 1: Write failing unit test**

Create `packages/desktop/src/features/browser-webviews/hibernation.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";
import { TabHibernationManager } from "./hibernation.js";

describe("TabHibernationManager", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  it("transitions active tab to idle after timeout and signals hibernation", () => {
    const manager = new TabHibernationManager({ idleTimeoutMs: 60000 });
    manager.registerTab("tab-1");
    manager.markActive("tab-1");

    expect(manager.isHibernated("tab-1")).toBe(false);

    manager.markIdle("tab-1");
    vi.advanceTimersByTime(60000);

    expect(manager.isHibernated("tab-1")).toBe(true);
  });

  it("prevents hibernation if active CDP session is open", () => {
    const manager = new TabHibernationManager({ idleTimeoutMs: 60000 });
    manager.registerTab("tab-2");
    manager.setCdpActive("tab-2", true);
    manager.markIdle("tab-2");

    vi.advanceTimersByTime(60000);
    expect(manager.isHibernated("tab-2")).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix packages/desktop run test -- hibernation.test.ts`
Expected: FAIL (module not found)

- [ ] **Step 3: Implement TabHibernationManager**

Create `packages/desktop/src/features/browser-webviews/hibernation.ts`:

```typescript
export interface HibernationOptions {
  idleTimeoutMs: number;
}

export class TabHibernationManager {
  private tabs = new Map<
    string,
    {
      state: "ACTIVE" | "IDLE" | "HIBERNATED";
      hasActiveCdp: boolean;
      idleTimer?: ReturnType<typeof setTimeout>;
    }
  >();

  constructor(private options: HibernationOptions = { idleTimeoutMs: 60000 }) {}

  registerTab(tabId: string): void {
    this.tabs.set(tabId, { state: "ACTIVE", hasActiveCdp: false });
  }

  unregisterTab(tabId: string): void {
    const entry = this.tabs.get(tabId);
    if (entry?.idleTimer) clearTimeout(entry.idleTimer);
    this.tabs.delete(tabId);
  }

  setCdpActive(tabId: string, active: boolean): void {
    const entry = this.tabs.get(tabId);
    if (entry) {
      entry.hasActiveCdp = active;
      if (active && entry.state === "HIBERNATED") {
        this.markActive(tabId);
      }
    }
  }

  markActive(tabId: string): void {
    const entry = this.tabs.get(tabId);
    if (!entry) return;
    if (entry.idleTimer) clearTimeout(entry.idleTimer);
    entry.state = "ACTIVE";
  }

  markIdle(tabId: string): void {
    const entry = this.tabs.get(tabId);
    if (!entry) return;
    entry.state = "IDLE";
    if (entry.idleTimer) clearTimeout(entry.idleTimer);
    entry.idleTimer = setTimeout(() => {
      if (!entry.hasActiveCdp && entry.state === "IDLE") {
        entry.state = "HIBERNATED";
      }
    }, this.options.idleTimeoutMs);
  }

  isHibernated(tabId: string): boolean {
    return this.tabs.get(tabId)?.state === "HIBERNATED";
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix packages/desktop run test -- hibernation.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/desktop/src/features/browser-webviews/hibernation.ts packages/desktop/src/features/browser-webviews/hibernation.test.ts
git commit -m "feat(desktop): add TabHibernationManager for browser webviews"
```

---

### Task 3: Server Idle Cache Pruning

**Files:**

- Modify: `packages/server/src/server.ts` or session management
- Create: `packages/server/src/utils/memory-pruner.ts`
- Test: `packages/server/src/utils/memory-pruner.test.ts`

**Interfaces:**

- Produces: `runServerIdlePruning(sessionCount: number): void`

- [ ] **Step 1: Write failing test**

Create `packages/server/src/utils/memory-pruner.test.ts`:

```typescript
import { describe, it, expect, vi } from "vitest";
import { runServerIdlePruning } from "./memory-pruner.js";

describe("runServerIdlePruning", () => {
  it("triggers GC if available and no active sessions exist", () => {
    const gcMock = vi.fn();
    (globalThis as any).gc = gcMock;

    runServerIdlePruning(0);
    expect(gcMock).toHaveBeenCalled();

    delete (globalThis as any).gc;
  });

  it("does not trigger GC when sessions are active", () => {
    const gcMock = vi.fn();
    (globalThis as any).gc = gcMock;

    runServerIdlePruning(2);
    expect(gcMock).not.toHaveBeenCalled();

    delete (globalThis as any).gc;
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix packages/server run test -- memory-pruner.test.ts`
Expected: FAIL

- [ ] **Step 3: Implement memory-pruner**

Create `packages/server/src/utils/memory-pruner.ts`:

```typescript
export function runServerIdlePruning(activeSessionCount: number): void {
  if (activeSessionCount === 0 && typeof (globalThis as any).gc === "function") {
    (globalThis as any).gc();
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm --prefix packages/server run test -- memory-pruner.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add packages/server/src/utils/memory-pruner.ts packages/server/src/utils/memory-pruner.test.ts
git commit -m "feat(server): add idle low-power memory pruning and GC trigger"
```
