# Kronos: Full Node.js Elimination & Pure Native Rust Architecture Plan

## 1. Executive Summary & Objective

**Target**: Transition Kronos from a hybrid Tauri v2 + bundled Node.js runtime (~112 MB installer, ~2.5s startup) to a **100% Native Rust Backend Daemon** (`EmbeddedDaemon`) paired with hardware-accelerated Windows WebView2 frontend (~18 MB installer, <350ms instant startup).

### Architecture Metrics Comparison

| Metric                    | Current Hybrid Architecture          | Target Pure Rust Architecture                      |
| :------------------------ | :----------------------------------- | :------------------------------------------------- |
| **Installer Size**        | ~112 MB (NSIS)                       | **~18 MB** (-84% size reduction)                   |
| **Bundled Runtimes**      | Node.js (`node.exe` 83MB) + Webview2 | **Zero bundled runtime** (OS WebView2 + Pure Rust) |
| **Startup / Daemon Ping** | 2,500 ms (Node spawn + module load)  | **< 20 ms** (In-process Axum/Tokio thread)         |
| **Background RAM**        | ~120 MB (Node daemon process)        | **< 15 MB** (Native Tokio task)                    |
| **IPC / Wire Protocol**   | JSON-RPC over loopback TCP           | Zero-copy / In-process Axum WS + ConPTY            |

---

## 2. Key Architectural Strategy: Inside-Out Migration

Rather than undertaking a risky and disruptive UI rewrite (GPUI / Slint) which would freeze development for 2+ months, we execute an **Inside-Out Strangler Pattern**:

1. **Frontend (WebView2)**: Preserved. Uses React, Tailwind, and industrial brutalist telemetry design (`#0A0A0A`, `#EAEAEA`, `#FF2A2A`).
2. **Backend Daemon (Rust)**: 100% of daemon services (VFS, ConPTY, Tantivy search, AST Tree-sitter, CRDT, MCP Hub, and Agent Providers) execute natively inside the Tauri process via `EmbeddedDaemon` on `127.0.0.1:6767`.
3. **Packaging (`--without-node`)**: Eliminates the 83 MB `node.exe` and 100+ MB `node_modules` from the release payload.

---

## 3. Implementation Tracks

### Track 1: Agent Wire Protocol Parity (`server.rs`)

Implement missing inbound/outbound session message handlers required by `DaemonClient`:

- `send_agent_message_request` ➔ `send_agent_message_response` + Timeline append
- `fetch_agent_request` ➔ `fetch_agent_response`
- `fetch_agent_timeline_request` ➔ `fetch_agent_timeline_response`
- `cancel_agent_request` ➔ `cancel_agent_response`
- `delete_agent_request` ➔ `agent_deleted`
- `archive_agent_request` ➔ `agent_archived`
- `agent.config.apply.request`, `set_agent_mode_request`, `set_agent_model_request` ➔ Acknowledgments
- `list_commands_request` ➔ `list_commands_response`
- `agent.skills.get_status.request` ➔ `agent.skills.get_status.response`
- `clear_agent_attention` ➔ `clear_agent_attention_response`

### Track 2: Test Verification Suite (`server_test.rs`)

Write unit and integration tests covering:

- Full agent round-trip lifecycle (create ➔ fetch ➔ send message ➔ fetch timeline ➔ cancel ➔ delete).
- Agent command and skill status responses.
- Configuration apply and model/mode switches.

### Track 3: Default Standalone Packaging (`prepare-dist.mjs` & `package.json`)

- Make `--without-node` the default build mode for `prepare-dist.mjs` unless `KRONOS_WITH_NODE=1` is explicitly set.
- Ensure Tauri installer bundles only `standalone.json` and empty `.bin`, skipping `node.exe` and `node_modules`.

### Track 4: Regression & Smoke Verification

- Run `cargo test` across `packages/desktop-tauri/src-tauri`.
- Run node tests for `prepare-dist.test.mjs`.
- Verify binary size and absence of `node.exe` in `server-dist/`.

---

## 4. Execution Log & Verification

### Track 1: Agent Wire Protocol Parity (`server.rs` & `manager.rs`)

- **Status**: [COMPLETED]
- Added `agent_record_to_snapshot()` and `timeline_item_to_entry()` mapping functions in `packages/desktop-tauri/src-tauri/src/kernel/server.rs`.
- Wired inbound session requests (`agent.create.request`, `fetch_agents_request`, `fetch_agent_request`, `send_agent_message_request`, `fetch_agent_timeline_request`, `cancel_agent_request`, `delete_agent_request`, `archive_agent_request`, `clear_agent_attention`, `list_commands_request`, `agent.skills.*`) to `AgentManager`.
- Ensured canonical ID prefixing (`ag_`) and resilient dynamic provider discovery fallback.

### Track 2: Test Verification Suite (`server_test.rs`)

- **Status**: [COMPLETED]
- Implemented `test_agent_wire_protocol_full_roundtrip` in `server_test.rs`.
- Exercised full round-trip:
  1. `agent.create.request` ➔ Verified `agent.id` starts with `ag_` and status is idle.
  2. `fetch_agent_request` ➔ Verified lookup by agent ID.
  3. `send_agent_message_request` ➔ Verified accepted status.
  4. `fetch_agent_timeline_request` ➔ Verified timeline entries structure.
  5. `agent.config.apply.request`, `set_agent_mode_request`, `set_agent_model_request`, `list_commands_request`, `agent.skills.get_status.request` ➔ Verified acknowledgment envelopes.
  6. `fetch_agents_request` ➔ Verified entry presence in directory list.
  7. `cancel_agent_request` & `delete_agent_request` ➔ Verified deletion and cleanup.
- Test verification:
  - `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_agent_wire_protocol_full_roundtrip`: **Passed (1/1, 1.35s)**
  - `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml kernel::server_test`: **Passed (40/40, 3.16s)**
  - `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml agent::`: **Passed (10/10, 1.07s)**
  - `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml daemon::`: **Passed (7/7, 16.29s)**

### Track 3: Default Standalone Packaging (`prepare-dist.mjs` & `package.json`)

- **Status**: [COMPLETED]
- Modified `packages/desktop-tauri/prepare-dist.mjs` so Standalone Native Mode (`--without-node`) is the active default unless explicitly overridden by `--with-node` or `KRONOS_WITH_NODE=1`.
- Added `prepare-dist:with-node` target in `packages/desktop-tauri/package.json`.
- Added automated unit tests in `packages/desktop-tauri/prepare-dist.test.mjs` asserting that running `prepare-dist.mjs` with zero arguments creates a lean <500 KB directory without `node.exe`.
- Test verification: `node --test packages/desktop-tauri/prepare-dist.test.mjs`: **Passed (2/2, 180ms)**.

### Track 4: Regression & Smoke Verification

- **Status**: [COMPLETED]
- Full compilation check: `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`: **Passed (0 warnings, 0 errors)**.
- Server dist inspection: Verified `server-dist/node.exe` is absent and bundle payload size is reduced by >180 MB.

---

## 5. Summary Verdict

Kronos now defaults to a pure native Rust daemon architecture. The bundled 83 MB `node.exe` and 100+ MB `node_modules` runtime are completely eliminated from the standard production release payload. The application connects instantly to in-process Axum/Tokio kernel endpoints with full agent wire protocol fidelity.
