# Track 2: Zero-Copy ConPTY & WebGL Terminal Pipeline Plan

**Goal:** Unlock 60fps locked hardware-accelerated terminal rendering and eliminate IPC congestion under massive log streaming by wiring adaptive ConPTY micro-batching and configuring the WebGL GPU rendering pipeline.

**Tech Stack:** Rust (Tauri v2, `portable_pty`, Windows ConPTY, `tauri::ipc::Channel`), TypeScript / React (`@xterm/xterm`, `@xterm/addon-webgl`, Industrial Brutalism custom glyphs).

---

## Architecture Overview

```
+-------------------------------------------------------------------------+
|                              Desktop Shell                              |
|                                                                         |
|  +--------------------+         Tauri IPC Channel       +------------+  |
|  | Windows ConPTY     |  ============================>  | WebView2   |  |
|  | (portable_pty)     |   Adaptive Micro-Batching       | Frontend   |  |
|  | Reader Thread      |   (up to 64KB / 2ms window)     +------------+  |
|  +--------------------+                                       |         |
|                                                               v         |
|                                                     +----------------+  |
|                                                     | @xterm/xterm   |  |
|                                                     | + WebglAddon   |  |
|                                                     | (GPU Direct3D) |  |
|                                                     +----------------+  |
+-------------------------------------------------------------------------+
```

1. **Native ConPTY Layer (`native_pty.rs`):**
   - Naive reader currently emits a Tauri `channel.send(TerminalEvent::Data(...))` on every small slice from `reader.read(&mut buf)`.
   - Under heavy commands (`cargo build`, `npm install`, `cat large.log`), thousands of tiny IPC messages saturate the Windows WebView2 message pump.
   - **Improvement:** Introduce an adaptive coalescing reader loop that gathers bytes up to 64KB or flushes within 2ms of idle, reducing IPC serialization overhead by >90%.

2. **UI Integration Layer (`terminal-pane.tsx`):**
   - `terminal-pane.tsx` already computes `isNativeTerminal` from `window.paseoDesktop?.nativeTerminal && isLocalHost`, but omits the prop when rendering `<TerminalEmulator>`.
   - **Improvement:** Pass `nativeTerminal={isNativeTerminal ? window.paseoDesktop?.nativeTerminal : undefined}` into `<TerminalEmulator>`.

3. **GPU Hardware Rendering Layer (`terminal-emulator-runtime.ts`):**
   - Configure `new WebglAddon({ customGlyphs: true, preserveDrawingBuffer: false })`.
   - Enables continuous box-drawing and tactical symbols without line gaps, rendering directly via ANGLE Direct3D 11 on Windows.
   - Guard `webglAddon.onContextLoss` to safely fall back to Canvas 2D without dropping terminal state.

---

## Tasks

### Phase 1: Native ConPTY High-Throughput Micro-Batching

- [ ] **Task 1: Implement adaptive micro-batching in `native_pty.rs`**
  - **File:** `packages/desktop-tauri/src-tauri/src/native_pty.rs`
  - In `attach_reader_channel`, wrap reader consumption in a coalescing loop that buffers incoming bytes and dispatches coalesced chunks.
  - Keep latency under 2ms for interactive typing while batching rapid multi-kilobyte bursts.
- [ ] **Task 2: Add burst streaming benchmark test in `native_pty.rs`**
  - **File:** `packages/desktop-tauri/src-tauri/src/native_pty.rs`
  - Verify that rapid sequential writes are coalesced and received without deadlock or buffer truncation.
  - Run `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml native_pty`.

### Phase 2: Frontend Native Terminal & WebGL Pipeline

- [ ] **Task 3: Wire `nativeTerminal` prop in `packages/app/src/components/terminal-pane.tsx`**
  - **File:** `packages/app/src/components/terminal-pane.tsx`
  - Pass `nativeTerminal={isNativeTerminal ? window.paseoDesktop?.nativeTerminal : undefined}` to `<TerminalEmulator>`.
  - Ensure local desktop sessions leverage direct native ConPTY streaming.
- [ ] **Task 4: Configure WebGL Addon with Tactical Custom Glyphs in `terminal-emulator-runtime.ts`**
  - **File:** `packages/app/src/terminal/runtime/terminal-emulator-runtime.ts`
  - Instantiate `new WebglAddon({ customGlyphs: true, preserveDrawingBuffer: false })`.
  - Validate context loss listener and error recovery.

### Phase 3: Verification & Commit

- [ ] **Task 5: Execute full test suites & typechecking**
  - Run `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml` (all Rust tests).
  - Run `npm run typecheck --workspace=@getpaseo/app` (ensure 0 TypeScript errors, zero `any`).
  - Run `npx oxfmt` and stage/commit all changes.
