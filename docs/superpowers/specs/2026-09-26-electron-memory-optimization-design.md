# Electron Hardening & Memory Optimization Design

- **Status**: Draft / Proposed
- **Target**: `packages/desktop`, `packages/server`
- **Objective**: Reduce Paseo's idle memory footprint by 40-50%, optimize CDP/webview tab lifecycle, and harden Chromium process management.

---

## 1. Problem Statement

Paseo is an Electron-based environment integrating:

1. Electron Main Process.
2. React Renderer GUI.
3. Embedded Browser Webviews (`<webview>` instances with CDP sessions for AI web agents).
4. Node.js Daemon / Server (`@getpaseo/server`).

Without strict lifecycle pruning, idle RAM footprint scales up to 500MB-800MB+ due to unthrottled background Chromium renderer processes, lingering CDP sessions, and unbounded server cache buffers.

---

## 2. Architecture & Components

```
+-----------------------------------------------------------+
|                   Electron Main Process                   |
|  - Process Memory Monitor                                 |
|  - Chromium Flag Tuning (--max-old-space-size, discarding)|
|  - Memory-Pressure Lifecycle Hooks                        |
+-----------------------------+-----------------------------+
                              |
         +--------------------+--------------------+
         |                                         |
+--------v------------------+            +---------v---------+
|  Embedded Browser Manager |            |  Node.js Daemon   |
|  - Tab Hibernation / Wake |            |  - Buffer Pruning |
|  - CDP Detach on Inactive |            |  - Cache Limits   |
|  - Screenshot Cache Only  |            |  - Idle Low Power |
+---------------------------+            +-------------------+
```

### Component Details:

### Component A: Chromium Process & Flag Optimizer (`packages/desktop/src/main.ts`)

- **Flags Configured at Startup**:
  - `--enable-features=AutomaticTabDiscarding,WebContentsDiscarding`
  - `--js-flags="--max-old-space-size=256 --expose-gc"`
  - `--disable-background-timer-throttling=false` (strictly enforce background throttling)
- **OS Memory Pressure Events**:
  - Listen to `app.on('render-process-gone')` and OS memory-pressure notifications.
  - Broadcast explicit `cleanUpCaches()` to webviews and renderer windows.

### Component B: Webview & CDP Tab Hibernation (`packages/desktop/src/features/browser-webviews/`)

- **Lifecycle States**:
  - `ACTIVE`: Attached to UI or currently targeted by AI Agent CDP queue.
  - `IDLE`: Inactive for > 60 seconds without pending CDP automation tasks.
  - `HIBERNATED`: WebContents unmounted / parked, retaining only the last screenshot thumbnail and navigation URL.
- **Wake Strategy**: Seamless reload/re-attach upon user tab click or agent task dispatch.

### Component C: Server Memory Pruning (`packages/server/`)

- Terminal scrollback buffers capped with circular buffers.
- Inactive git workspace diff caches evicted after inactivity.
- Explicit V8 GC invocation when daemon enters idle low-power state.

---

## 3. Data Flow & State Transitions

1. **Agent Task Start**:
   - Target tab transitioned `HIBERNATED -> ACTIVE`.
   - WebContents restored to last known URL.
   - CDP session attached via `CdpSessionQueue`.
2. **Agent Task End**:
   - Tab marked `IDLE`.
   - 60s idle timer armed.
3. **Hibernation Trigger**:
   - Timer fires -> Screenshot captured for UI placeholder -> CDP detached -> WebContents unloaded.
   - Memory reclaimed by OS.

---

## 4. Verification & Testing

1. **Benchmark Suite**:
   - Measure RSS & Heap across 3 states: Cold Start, Active 5-tab Agent Session, 5-minute Post-Task Idle.
   - Target metric: Idle RSS <= 200MB total across all Electron & Node processes.
2. **E2E Automation Regression**:
   - Run `packages/desktop/e2e/browser-tabs.e2e.mjs` to ensure zero breakages in CDP snapshot and input actions.
