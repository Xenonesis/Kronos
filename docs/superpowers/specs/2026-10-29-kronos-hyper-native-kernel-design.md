# Kronos Hyper-Native Kernel (HNK) Design Specification

- **Date:** 2026-10-29
- **Status:** Approved for Implementation Planning
- **Author:** Kronos Engineering Core
- **Target Release:** Kronos v0.15.0+

---

## 1. Executive Summary & Goals

Kronos began as a Tauri v2 migration of Paseo, progressively eliminating Electron dependencies. While the native PTY engine (`portable-pty`), multithreaded Myers diff (`similar`), and Win32 Job Object process supervisor (`process_job.rs`) brought keystroke latency to ~2ms, the background architecture still relies on a Node.js daemon (`@getpaseo/server` running via `node.exe` on port 6767).

This specification establishes the **Kronos Hyper-Native Kernel (HNK)** — a single-process, low-latency native core written in Rust that completely assimilates the background daemon, eliminates Node.js from the distribution, and replaces traditional operating system and filesystem bottlenecks with high-throughput native primitives inspired by Zed, Cursor, and Gitoxide.

### Key Objectives

1. **Zero External Runtimes (Kill `node.exe`)**: Eliminate `node.exe` and `server-dist` from the installer bundle, cutting idle memory consumption from ~600MB–1.2GB down to ~70MB–90MB.
2. **Instant Sub-Millisecond Cold Boot**: Reduce daemon startup from ~800ms down to <5ms via an in-process Tokio asynchronous runtime.
3. **In-Memory Shadow VFS & Gitoxide (`gix`)**: Move workspace branch switching, diffing, and worktree operations from physical NTFS disk thrashing into an in-memory Copy-on-Write (CoW) virtual filesystem.
4. **Shared Memory Ring Buffer (SMRB) Zero-Copy IPC**: Eliminate JSON serialization overhead for high-throughput AI agent token streaming and terminal output.
5. **Native Tree-Sitter AST & Semantic Intelligence**: Provide AST-level semantic diffing and structural code inspection in background threads.
6. **Embedded Tantivy Workspace Retrieval**: Sub-millisecond workspace search (<1ms) via an in-memory lexical trigram index, giving AI agents immediate context retrieval.
7. **Zero Frontend Churn**: Maintain full protocol parity with `packages/app` using automated type generation (`@getpaseo/protocol` $\to$ Rust `serde`) and dual-transport support (Tauri IPC for local, WebSocket for remote relay).
8. **Multi-Call Universal Binary (Single 50MB Executable)**: The single `kronos.exe` operates as GUI Desktop app, headless daemon (`kronos daemon`), autonomous CLI agent runner (`kronos agent`), AST searcher (`kronos search`), or native Model Context Protocol server (`kronos mcp`).
9. **CRDT Multi-Agent Concurrent Memory (`yrs`)**: Integrate Conflict-free Replicated Data Types (CRDT) so multiple AI agents and human users can concurrently edit the same files with zero merge conflicts or clobbered states.

---

## 2. Problem Statement & Bottlenecks

| Bottleneck in Previous Architecture      | Technical Cause                                                                                                                                          | Consequence                                                                                          |
| :--------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------------- |
| **Node.js Runtime Overhead**             | V8 engine startup, JIT compilation, and separate process lifecycle (`node.exe`).                                                                         | 200MB+ baseline RAM overhead, `EADDRINUSE: 6767` port collisions, and process management complexity. |
| **NTFS Disk I/O & Process Creation Lag** | Spawning `git.exe` on Windows incurs `CreateProcessW` overhead, NTFS directory lookups, and Windows Defender / Antivirus real-time filter driver delays. | Creating worktrees or refreshing 50,000 files takes 50ms–300ms per agent step.                       |
| **V8 JSON Serialization Churn**          | Agent streaming emits thousands of small JSON fragments over WebSocket, serialized by Node and deserialized by V8 in WebView2.                           | Heavy CPU spikes and V8 Garbage Collection (GC) pauses during rapid agent output.                    |
| **Syntactic Blindness in Diffs**         | Line-based Myers diff lacks AST context; cannot distinguish between a variable rename and a semantic logic rewrite.                                      | AI diff reviewers lack semantic confidence and highlight trivial whitespace changes.                 |

---

## 3. System Architecture & Topology

```
+---------------------------------------------------------------------------------------------------------+
|                                        KRONOS.EXE (Single OS Process)                                   |
|                                                                                                         |
|  +---------------------------------------------------------------------------------------------------+  |
|  |                       FRONTEND SHELL (WebView2 / React 19 UI / xterm WebGL)                       |  |
|  +---------------------------------------------------------------------------------------------------+  |
|          ▲                                        ▲                                      ▲              |
|          │ Zero-Copy SMRB / TypedArrays           │ Tauri IPC Channels                   │ Named Pipe   |
|          ▼                                        ▼                                      ▼ (Legacy WS)  |
|  +---------------------------------------------------------------------------------------------------+  |
|  |                                  KRONOS HYPER-NATIVE KERNEL (HNK)                                 |  |
|  |                                (Tokio Multi-Thread Async Runtime)                                 |  |
|  |                                                                                                   |  |
|  |  +--------------------------+  +--------------------------+  +---------------------------------+  |  |
|  |  |   Universal RPC Dispatch |  |  Shadow VFS & Gitoxide   |  |   Agent Host & ACP Runtime      |  |  |
|  |  |  - Automated Typegen     |  |  - Pure Rust `gix` Core  |  |  - Standard ACP Client (Zed)   |  |  |
|  |  |  - Named Pipes / IPC     |  |  - CoW Memory Workspace  |  |  - Win32 Job Object Supervisor  |  |  |
|  |  |  - WebSocket (Relay)     |  |  - NTFS Background Flush |  |  - Claude / OMP / Codex CLI    |  |  |
|  |  +--------------------------+  +--------------------------+  +---------------------------------+  |  |
|  |                                                                                                   |  |
|  |  +--------------------------------------------------------+  +---------------------------------+  |  |
|  |  |             Semantic Intelligence & Search             |  |        Persistence Engine       |  |  |
|  |  |  - Tree-sitter Background AST Parser                    |  |  - Atomic JSON Storage Engine   |  |  |
|  |  |  - Tantivy In-Memory Trigram / Lexical Search Index     |  |  - SQLite (WAL Mode, 0ms Boot) |  |  |
  |  |  +--------------------------------------------------------+  +---------------------------------+  |  |
  |  |  |             Semantic Intelligence & Search             |  |        Persistence Engine       |  |  |
  |  |  |  - Tree-sitter Background AST Parser                    |  |  - Atomic JSON Storage Engine   |  |  |
  |  |  |  - Tantivy In-Memory Trigram / Lexical Search Index     |  |  - SQLite (WAL Mode, 0ms Boot) |  |  |
  |  |  +--------------------------------------------------------+  +---------------------------------+  |  |
  |  |                                                                                                   |  |
  |  |  +--------------------------------------------------------+  +---------------------------------+  |  |
  |  |  |               Multi-Agent CRDT Memory (`yrs`)          |  |      Multi-Call CLI Engine      |  |  |
  |  |  |  - Real-time Concurrent Edit Convergence               |  |  - `kronos` (GUI Desktop)       |  |  |
  |  |  |  - Conflict-Free Collaborative Buffer State            |  |  - `kronos daemon / agent / mcp`|  |  |
  |  |  +--------------------------------------------------------+  +---------------------------------+  |  |
  |  +---------------------------------------------------------------------------------------------------+  |
|  +---------------------------------------------------------------------------------------------------+  |
+---------------------------------------------------------------------------------------------------------+
```

---

## 4. Subsystem Specifications

### 4.1 In-Process Daemon & Universal Transport

- **Module Location**: `packages/desktop-tauri/src-tauri/src/kernel/`
- **Embedded Lifecycle**: Spawns inside `tauri::Builder::setup` as a background Tokio task. Terminates cooperatively upon `tauri::RunEvent::Exit`.
- **Dual Transport Layer**:
  1. **Native IPC Channel (Local Default)**: Uses Tauri v2 IPC channels and Windows Named Pipes (`\\.\pipe\kronos-ipc`). Zero open network ports, immune to local port scans and Windows Firewall popups.
  2. **WebSocket Server (Remote Relay Fallback)**: Listens on loopback or designated relay socket to support remote SSH/headless connections without changing wire behavior.
- **Automated Protocol Typegen**:
  - Script: `scripts/generate-rust-protocol.mjs`
  - Compiles `@getpaseo/protocol` TypeScript schemas to Rust `serde` structs with fallback dynamic envelope routing (`serde_json::Value`) for unknown UI extensions.

### 4.2 In-Memory Shadow VFS & Pure Rust Gitoxide (`gix`)

- **Core Technology**: `gix` (Gitoxide pure Rust engine) + memory-mapped file buffers.
- **Copy-on-Write (CoW) Virtual Worktrees**:
  - Instead of executing `git worktree add` to create multi-megabyte disk directories on NTFS, Kronos maintains an in-memory shadow index of file paths, tree objects, and blobs.
  - Read queries from the UI or AI agent are resolved against the shadow VFS in **<0.5ms**.
  - Modifications are buffered in memory and flushed asynchronously to physical disk in background thread pools without blocking UI interaction.
- **Elimination of Git CLI Dependency**: Common operations (`gix::status`, `gix::commit`, `gix::branch`, `gix::diff`) run in-process without invoking `git.exe`. External `git.exe` is retained only as an asynchronous fallback for rare custom credentials / GPG signing.

### 4.3 Shared Memory Ring Buffer (SMRB) Zero-Copy Telemetry

- **Mechanism**: Windows `CreateFileMappingW` / `MapViewOfFile` shared memory block exposed to the Tauri WebView2 via custom URI scheme (`kronos-shm://`) or direct `SharedArrayBuffer` binding.
- **Telemetry Throughput**:
  - High-frequency events (terminal output frames, live token streams from Claude/OMP, high-volume diff chunks) are written into a circular ring buffer with a 64-bit atomic sequence header.
  - The frontend reads chunks directly as `Uint8Array`, bypassing `JSON.stringify` and V8 garbage collection entirely.

### 4.4 Semantic Intelligence (Tree-Sitter AST & Tantivy Search)

- **Tree-sitter Parsing**:
  - Supported grammars: Rust, TypeScript, JavaScript, Python, Go, JSON, Markdown, HTML, CSS.
  - Executed on a dedicated background rayon thread pool (`tokio::task::spawn_blocking`).
  - Emits structural diff representations (e.g., function boundaries, declaration mutations) alongside line-based diffs.
- **Tantivy In-Memory Search Engine**:
  - Maintains an in-memory trigram index for instant code navigation.
  - Sub-millisecond response time (<1ms) for symbol lookups and exact string occurrences across 100,000+ lines of code.

### 4.5 Agent Host & ACP Execution

- **Zed-Standard Agent Client Protocol (ACP)**:
  - Implements full ACP client specifications in native Rust over standard I/O pipes.
- **Win32 Job Object Supervision**:
  - Every external CLI agent (`claude`, `omp`, `codex`, language servers) is spawned through `process_job.rs` with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
  - Guarantees zero orphaned child processes upon Kronos termination or crash.

### 4.6 Multi-Call Universal Binary Architecture

- **CLI Dispatcher (`clap` in Rust)**:
  `kronos.exe` inspects invocation arguments before initializing the GUI:
  - `kronos`: Boots desktop GUI with Tauri v2 window and WebView2.
  - `kronos daemon [--port <p>] [--pipe <name>]`: Runs headless background Tokio daemon for remote servers/VPS.
  - `kronos agent <prompt> [--provider <p>]`: Executes autonomous CLI agent session in terminal stdio without launching any GUI window.
  - `kronos search <query>`: Instant in-memory AST and trigram search across the workspace.
  - `kronos mcp`: Native Model Context Protocol (MCP) server over stdio for external LLM integrations.

### 4.7 CRDT Multi-Agent Concurrent Memory (`yrs`)

- **Conflict-Free Replicated Data Types**:
  - Integrated via `yrs` (native Rust port of Yjs).
  - Every active buffer in the Shadow VFS is backed by a `yrs::Doc` containing a `yrs::Text` state.
  - Multiple agents (e.g. Claude modifying backend logic while OMP modifies frontend components) submit transactional diff updates (`yrs::Transact`).
  - Update states merge deterministically using state vectors without file locking, overwrite errors, or git merge conflicts.

---

## 5. Migration & Safety Transition

### 5.1 Dual-Engine Switch (`KRONOS_DAEMON_ENGINE`)

To prevent regressions during development:

```bash
# Default: Runs the new embedded Rust Hyper-Native Kernel
KRONOS_DAEMON_ENGINE="rust"

# Fallback: Launches legacy node.exe daemon-worker.js
KRONOS_DAEMON_ENGINE="node"
```

### 5.2 Phased Rollout Plan

1. **Phase 1: Embedded Tokio Daemon & Protocol Dispatcher**
   - Implement in-process RPC server, session store, and dual-transport bridge.
2. **Phase 2: Pure Rust Gitoxide (`gix`) & Shadow VFS**
   - Replace disk-bound git status and worktree creation with in-memory CoW trees.
3. **Phase 3: SMRB Zero-Copy Streaming**
   - Bind ring buffers to agent telemetry and terminal output.
4. **Phase 4: Semantic Tree-Sitter & Tantivy Search Integration**
   - Enable structural diffs and sub-millisecond workspace search.
5. **Phase 5: Node Elimination**
   - Remove `node.exe` and `server-dist` from NSIS packaging scripts permanently.

---

## 6. Verification & Acceptance Criteria

- [ ] **Zero Node Processes**: When running Kronos, `Get-Process node -ErrorAction SilentlyContinue` returns 0 processes.
- [ ] **Memory Footprint**: Total memory usage of `kronos.exe` + WebView2 stays under **120MB** at idle.
- [ ] **Startup Benchmark**: Daemon initializes and accepts RPC connections in **<5ms**.
- [ ] **Worktree Creation Latency**: In-memory shadow worktree creation benchmark executes in **<2ms** (compared to 150ms on physical NTFS).
- [ ] **Search Latency**: Tantivy workspace search returns top 50 matches across 50,000 lines in **<1ms**.
- [ ] **Process Cleanup**: Killing `kronos.exe` via Task Manager cleans up 100% of child processes with zero leaks.
- [ ] **All Rust Tests Passing**: Complete test suite passes with 0 compiler warnings.
