# Track 1: Full Rust Kernel Convergence (Eliminating `node.exe`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the 50MB `node.exe` bundled daemon process, migrating core HTTP and WebSocket server duties into the native Rust Axum/Tokio kernel (`packages/desktop-tauri/src-tauri/src/kernel/server.rs`). This reduces idle memory from ~120MB down to ~35MB, cuts installer size by ~50MB, and makes daemon startup instantaneous (0ms in-process thread spawn).

**Architecture:**

1. **Axum HTTP/WS Server:** Upgrade `packages/desktop-tauri/src-tauri/src/kernel/server.rs` to strictly adhere to `@getpaseo/protocol` WebSocket envelope specifications (`WSInboundMessage` / `WSOutboundMessage`).
2. **Zero-Copy Native Handshake:** Handle `hello` handshake returning canonical `server_info` status payload, enabling `@getpaseo/client` (`daemon-client.ts`) to connect directly to the Rust kernel without running Node.js.
3. **Native Service Dispatch:** Connect incoming session messages to Rust kernel subsystems: PTY (`native_pty.rs`), Telemetry (`kernel/telemetry.rs`), VFS (`kernel/vfs.rs`), and Git (`kernel/git.rs`).
4. **Daemon Controller:** Switch `KRONOS_DAEMON_ENGINE` in `daemon.rs` to drive the in-process `EmbeddedDaemon` directly.
5. **Standalone Packaging:** Update `packages/desktop-tauri/prepare-dist.mjs` to support lightweight distribution without bundling `node.exe`.

---

## Phases & Tasks

### Phase 1: WebSocket Wire Protocol Alignment in Axum Server

#### Task 1: Protocol Envelope Synchronization

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/protocol.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/server.rs`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/protocol_test.rs`

- [ ] **Step 1: Define canonical WSInbound & WSOutbound message types**
      In `protocol.rs`, implement `WSInboundMessage` (variants: `Ping`, `Hello`, `Session`) and `WSOutboundMessage` (variants: `Pong`, `Session { message: SessionOutboundMessage }`, `HelloRejected`).
- [ ] **Step 2: Construct standard ServerInfo payload**
      Ensure `server_info` returned on `hello` includes `protocolVersion: 1`, `serverId: "kronos-embedded-daemon"`, `hostname: "localhost"`, and supported feature flags (`connectedHosts`, `hostMultiplexing`).
- [ ] **Step 3: Update `dispatch_envelope` in `server.rs`**
      Parse incoming JSON WebSocket frames into `WSInboundMessage` and wrap session outbound responses into `{ type: "session", message: ... }`.
- [ ] **Step 4: Run Rust unit tests**
      `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml protocol`

#### Task 2: Live Loopback WebSocket Handshake Tests

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs`

- [ ] **Step 1: Add automated integration test for `hello` handshake**
      Spawn test `EmbeddedDaemon::start_test(0)`, connect with `tokio_tungstenite`, send `WSHelloMessage`, and assert `server_info` status frame is received.
- [ ] **Step 2: Add ping/pong and error handling test**
      Send `{ type: "ping" }` and assert `{ type: "pong" }`. Send invalid JSON and verify socket remains resilient.
- [ ] **Step 3: Run server tests**
      `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml server_test`

---

### Phase 2: Native Kernel Subsystems Wiring

#### Task 3: Telemetry, VFS, and Git Dispatch

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/server.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/kernel/mod.rs`

- [ ] **Step 1: Wire `telemetry` message handling**
      Route `"kernel/telemetry"` and `"status/system"` messages to `crate::kernel::telemetry::get_kernel_telemetry()`.
- [ ] **Step 2: Wire VFS file inspection and directory validation**
      Route `"file/validate"` and `"file/inspect"` to native `crate::kernel::vfs` and `fast_inspect_path`.
- [ ] **Step 3: Wire Git status and branch queries**
      Route `"git/status"` and `"git/branch"` to `crate::kernel::git`.

---

### Phase 3: Daemon Controller Mode Cutover

#### Task 4: In-Process Embedded Daemon as Primary Engine

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/daemon.rs`
- Modify: `packages/desktop-tauri/src-tauri/src/main.rs`

- [ ] **Step 1: Switch default engine to in-process Rust**
      In `daemon.rs`, update `start_daemon` so that default `KRONOS_DAEMON_ENGINE` is `"rust"`, with automatic fallback to `"node"` only if legacy plugin flags are specified.
- [ ] **Step 2: Enhance `wait_for_daemon_health` for in-process lifecycle**
      Ensure `wait_for_daemon_health` detects the Axum server instantly (<5ms) when running in-process without network overhead.
- [ ] **Step 3: Verify clean shutdown**
      Ensure closing the application gracefully triggers `shutdown_tx` and drops all listening sockets/pipes.

---

### Phase 4: Standalone Packaging & Zero-Node Verification

#### Task 5: Packaging Pipeline Support for Zero-Node Dist

**Files:**

- Modify: `packages/desktop-tauri/prepare-dist.mjs`
- Modify: `packages/desktop-tauri/src-tauri/tauri.conf.json`

- [ ] **Step 1: Add `--without-node` build option in `prepare-dist.mjs`**
      Allow building Kronos standalone binaries where `node.exe` is completely omitted from the packaging payload.
- [ ] **Step 2: Smoke test end-to-end**
      Launch desktop binary with `KRONOS_DAEMON_ENGINE=rust`, verify UI connects, health check passes, and RAM stays under 40MB.
- [ ] **Step 3: Verification check**
      Run full test suite: `cargo test` and `npm run typecheck --workspace=@getpaseo/app`.
