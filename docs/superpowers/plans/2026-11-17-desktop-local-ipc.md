# Desktop Local IPC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move managed embedded-Rust desktop sessions to direct Tauri IPC without losing existing session, security, terminal, or remote-client behavior.

**Architecture:** Extract a transport-neutral Rust session core, keep WebSocket and named-pipe/socket adapters over it, and add an owner-scoped Tauri channel adapter. Reuse `DaemonClient` and its transport interface rather than adding feature-specific IPC APIs. Engine readiness is independent of network-listener readiness; existing HTTP consumers retain network endpoints.

**Tech Stack:** Rust/Tokio/Axum/Tauri v2, TypeScript/Expo, Vitest, existing Cargo tests and installer pipeline. No new orchestration framework or runtime dependency.

**Spec:** `docs/superpowers/specs/2026-11-17-desktop-local-ipc-design.md`

**Status:** Execution plan approved by the user's `yes`. Tasks 1–5 verified at task scope on `feat/desktop-local-ipc-20261117`; preserve the recorded dirty baseline. User explicitly selected inline execution (option `2`) after Task 2's Orca caller-binding blocker. Task 6 is blocked at native runtime isolation: default-feature check, optimized NSIS build, installer structure/freshness, VM bridge tests and affected checks passed. Actual GUI/channel, installation, platform/performance acceptance and whole-branch review remain pending. Managed-host IPC selection stays opt-in, with no default switch or production-readiness acceptance. Progress and reports live in `.superpowers/sdd/2026-11-17-desktop-local-ipc/`.

## Global Constraints

- Only the managed embedded Rust desktop connection changes transport.
- Node-engine mode, non-managed hosts, mobile, browser, CLI, and relay keep their existing supported paths. Do not silently send their requests to the embedded local engine.
- Use a session-specific Tauri channel rather than broadcasting private agent events to every window. Derive owner identity from the command caller, not frontend-supplied identity.
- Opening, reloading, or closing a UI transport must not restart the engine or cancel an active agent.
- No silent local WebSocket fallback on IPC failure.
- No database migration, credential rotation, provider rewrite, UI redesign, new runtime, or global tooling installation.
- Preserve existing uncommitted changes. Do not reset, stash, mass-format, or commit unrelated files.
- Never restart/kill the production daemon or match processes by executable name. Runtime tests use temporary homes and only processes created by that test.
- Tests/builds run sequentially. Only exact affected JS test files and filtered Rust tests; never the full local suite.
- After implementation edits, run affected typechecks, repository npm lint scripts, and targeted format checks. Rebuild generated workspace declarations before treating stale declarations as source errors.
- Installer validation uses `npm run bundle:desktop`. No release publication, global installation, signing-key changes, or portable-executable distribution.
- No completion claim until fresh verification covers it. Missing platform/hardware/manual evidence stays explicitly unchecked.

## Review Focus

1. Window reload while an open command is unresolved: old generation cannot leak or replace the new session; Task 4 tests this.
2. Another local process listening on the former managed port: no credential forwarding based on loopback/6767 alone; Task 5 tests this.
3. A slow or destroyed frontend with unacknowledged frames: bounded native and channel buffering; explicit recovery and cleanup, not agent termination; Task 4 tests this.
4. Network listener unavailable while the engine is healthy: local IPC still attaches; HTTP/remote status reports unavailable separately; Task 2 tests this.
5. Overflow during terminal output or a timeline update: a visible resync signal followed by replay/snapshot recovery, not silent dropped bytes or duplicated turn actions; Tasks 3 and 6 test this.

## Ownership and execution method

User initially requested subagents. Task 1 used actual Orca `pi` implementer and independent reviewer workers. Task 2's fresh launch was rejected with `no_active_sender_terminal`; user then explicitly chose inline execution without subagents. Continue under `executing-plans`: task-level TDD, sequential checks, and persistent ledger. No more worker launches are required. Final review is fresh-context if available; otherwise disclose weaker author self-review. Do not install another framework or claim delegation that did not run.

Workers share the current working-tree source baseline, not a clean checkout that omits existing changes. Use read-only reviewers in that baseline. Only one product-code implementer edits at a time because session extraction, lifecycle, and adapter changes overlap. The coordinator owns integration and serializes all checks. No worker starts a production app or runs builds without coordination.

For each remaining inline task: run vertical RED -> GREEN cycles and inspect connected callers before advancing. Preserve Task 1's independent verdicts without redoing them. Task 5 waits for Task 4's interface. A final independent review, if available, is not replaced by author self-review; missing independent review remains explicit.

## File ownership

Let `R = packages/desktop-tauri/src-tauri/src`, `A = packages/app/src` for this document only.

| File                                                                 | Responsibility                                                              |
| -------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `R/daemon.rs`                                                        | Idempotent engine lifecycle, readiness and explicit restart                 |
| `R/kernel/server.rs`                                                 | Network startup and WebSocket/pipe/socket adapters                          |
| `R/kernel/access.rs`                                                 | Existing admission policy, network-port view, managed-target security       |
| `R/kernel/session.rs` (new)                                          | Shared frame/session loop, subscriptions, terminal mapping and recovery     |
| `R/kernel/session_test.rs` (new)                                     | Deterministic shared-session contract tests                                 |
| `R/kernel/mod.rs`                                                    | Register session and test modules                                           |
| `R/local_transport.rs` (new)                                         | Tauri owner-bound registry, bounded queues, channel/ack lifecycle           |
| `R/main.rs`                                                          | Register IPC commands/channel bridge and window teardown                    |
| `R/local_transport_test.rs` (new)                                    | Pure adapter ownership/backpressure tests, registered under local_transport |
| `A/desktop/daemon/desktop-daemon.ts`                                 | Host bridge calls and typed status/adapter capability                       |
| `A/desktop/daemon/local-daemon-transport-rpc.ts`                     | Local adapter command/event/ack contract                                    |
| `A/desktop/daemon/desktop-daemon-transport.ts`                       | Existing `DaemonTransportFactory` integration                               |
| `A/desktop/daemon/test-local-daemon-transport-rpc.ts`                | In-memory adapter for frontend lifecycle contract tests                     |
| `A/desktop/daemon/desktop-daemon-transport.test.ts`                  | Frontend adapter ordering, close/open races, stale generations              |
| `A/runtime/daemon-start-service.ts`, `A/runtime/host-runtime.ts`     | Managed-local selection and readiness integration                           |
| `A/types/host-connection.ts`                                         | Stable managed-local profile normalization; no broad schema rewrite         |
| Existing corresponding `.test.ts` files                              | Regression tests alongside the affected module                              |
| `docs/architecture.md`, `docs/terminal-performance.md`, `docs/qa.md` | Update owning docs only after measured/verified behavior                    |

Do not modify `packages/protocol` or `packages/client` by default: current transport `send` already permits `Promise<void>` and receives text/binary. If a shared wire change is necessary, stop for a plan amendment and compatibility review.

## Fixed adapter contract

These are new local adapter APIs, not public network protocol additions.

- `SessionFrame = Text(String) | Binary(Vec<u8>) | Close { code: u16, reason: String }` in `kernel/session.rs`; transport Ping/Pong remain adapter-level concerns.
- `SessionIo { incoming: FrameReceiver, outgoing: FrameSender, profile: SessionProfile }` uses byte-and-frame-bounded queues. `SessionProfile::Full` preserves WebSocket/Tauri subscriptions and binary behavior; `LegacyLine` preserves existing pipe/socket text RPC only. This private seam amendment was approved by user's `okay continue`; see [Task 3 amendment](2026-11-17-desktop-local-ipc-task-3-amendment.md). `run_session(context: Arc<SessionContext>, io: SessionIo) -> Result<(), SessionError>` owns admission, dispatch and per-session subscriptions. Cancellation uses existing core shutdown signal; network adapter EOF drops only its own session.
- `EnginePhase = Stopped | Starting | Ready | Failed { reason: String }`; `ensure_embedded_ready() -> Result<Arc<SessionContext>, String>` shares an existing startup and never restarts an existing engine. `restart_daemon` remains the explicit restart entry point.
- `AccessControl::with_network_port(&self, port: u16) -> Self` derives an HTTP guard view from the same credential/password authority; do not issue another credential for IPC.
- Tauri commands: `open_embedded_transport(input: OpenEmbeddedInput, on_event: Channel<EmbeddedTransportEvent>, window: WebviewWindow)`, `send_embedded_transport(input: SendEmbeddedInput, window: WebviewWindow)`, `ack_embedded_transport(input: AckEmbeddedInput, window: WebviewWindow)`, `close_embedded_transport(input: OpenEmbeddedInput, window: WebviewWindow)`. All return `Result<(), String>`; owner comes from `window`, never from JSON.
- `OpenEmbeddedInput { session_id: String, generation: String }`; other commands carry the same identifiers. Send carries exactly one text or base64-binary field. Ack carries the last contiguous delivered sequence number. Reject malformed, cross-owner, duplicate-open, or stale-generation inputs.
- Events carry `sessionId`, `generation`, monotonically increasing `sequence`, `kind: open|message|close|error`, and existing text/binary/close fields. Only data-message events consume the data acknowledgement window; terminal close is out-of-band and emitted at most once.
- Initial safety limits, not performance-tuned claims: text/binary frame at most 16 MiB (existing network limit); each incoming/outgoing session queue at most 128 data frames AND 32 MiB; channel in-flight at most 64 data events AND 32 MiB, allowing one maximum-size base64-encoded binary frame plus its envelope. Byte accounting uses decoded payload plus encoded serialized-channel envelope bytes at the respective layer. Capacity exhaustion closes for recovery rather than silently discarding an accepted message.
- Frontend serializes message delivery, awaits registered transport handlers, then cumulatively acknowledges only successfully delivered contiguous messages. Ack once per drained batch; no per-token timer required. Reject acknowledgements beyond the last sent sequence; older cumulative duplicates are harmless.
- Trusted caller: packaged main Tauri asset surface or the explicitly configured desktop dev origin, with supported window labels checked in native code. Guest/external webviews are rejected. Additional chat windows must be explicitly registered by their existing trusted creation path, not trusted by a name prefix.
- Rollout switch: opt-in `KRONOS_DESKTOP_LOCAL_TRANSPORT=ipc` during parity checks, explicit `websocket` rollback using the current managed network path. Final default becomes IPC for embedded Rust only after Task 6; unknown switch values fail visibly. Node-engine mode never selects embedded IPC.

### Task 1: Capture baseline and deterministic failure cases

**Files:** existing transport test file and fake RPC; existing `kernel/server_test.rs`; new results artifact `docs/superpowers/plans/2026-11-17-desktop-local-ipc-evidence.md`.

**Consumes:** Existing production transport and test helpers.
**Produces:** Recorded baseline, exact reproduction/fault cases and test commands; no claimed fix for an unreproduced user issue.

- [ ] Read current status/diff before touching files; record HEAD and overlapping existing changes. Identify every caller of modified lifecycle/transport functions.
- [ ] Capture cold start, UI attach, request round trip, stream delay, queue/memory observations using one isolated home and owned process. If packaged observation cannot run without interacting with the user's live application, record the block instead of restarting it.
- [ ] Add one failing frontend test at a time for close during registration, late open after close, ordered batched text/binary delivery, and a rejected send surfacing an error. Use existing swappable fake RPC for timing control; do not label it end-to-end.
- [ ] Run `cd packages/app && npx vitest run src/desktop/daemon/desktop-daemon-transport.test.ts --bail=1`; record each new failure and its reason before implementation in dependent tasks. Do not leave a deliberately failing test committed.
- [ ] Record deterministic native fault cases for stream EOF and writer failure plus known overflow/recovery behavior. Distinguish code defects from the user's still-unconfirmed intermittent failure.
- [ ] Reviewer checks evidence and test determinism. No source-wide cleanup or unrelated fix.

### Task 2: Idempotent engine readiness independent of listening sockets

**Files:** `R/daemon.rs`, `R/kernel/server.rs`, `R/kernel/access.rs`, `R/main.rs`; tests in existing daemon test module and `kernel/server_test.rs`.

**Consumes:** Current manager, auth authority, lifecycle and shutdown signals.
**Produces:** `EnginePhase`, `SessionContext`, `ensure_embedded_ready()`; listener failure no longer prevents local-core readiness.

- [ ] Write and run failing filtered tests `test_embedded_ensure_ready_is_idempotent` and `test_embedded_concurrent_ensure_shares_start`: two attaches get the same context and no stop/restart callback occurs.
- [ ] Extract core initialization from network listener setup; share existing auth/manager/storage instead of creating a second engine. Network guard gets its actual port through `with_network_port`; local admission does not depend on an HTTP health request.
- [ ] Make normal startup join `Starting` or reuse `Ready`. Publish failures with reasons and complete pending attaches; explicit restart is the only path stopping a ready engine. Clear stale network endpoint state on network shutdown.
- [ ] Add and satisfy `test_embedded_ready_when_network_bind_fails`, `test_embedded_start_failure_releases_waiters`, and `test_embedded_ui_detach_preserves_engine`. Use an injectable startup/listener boundary, temporary state, and explicit failure injection rather than occupying production ports.
- [ ] Expose engine phase and separate network availability in typed desktop status. Existing network-only consumers keep their compatible status fields.
- [ ] Native test command per new filter: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml <exact_test_name> -- --test-threads=1`. Record exit status and counts; compilation failure is not a passing test.
- [ ] Reviewer checks startup ownership, failure races, Node-engine behavior and no accidental production restart.

### Task 3: Extract and prove the shared session core

**Files:** `R/kernel/session.rs`, `R/kernel/session_test.rs`, `R/kernel/mod.rs`, `R/kernel/server.rs`, existing `R/kernel/server_test.rs`.

**Consumes:** Task 2 `Arc<SessionContext>` and existing dispatch/auth implementations.
**Produces:** `SessionFrame`, `SessionIo`, `run_session(...)` used by existing adapters and future IPC.

- [ ] Add `test_session_rejects_before_admission` and `test_session_preserves_request_id`, one RED -> GREEN cycle each. Reuse canonical local-credential hello; retain auth rejection/negotiation behavior before private events.
- [ ] Move session state and dispatch from current WebSocket/pipe handlers into the shared loop without bypassing subscription, terminal-slot or binary semantics. Leave network framing/Ping/Pong in network adapters.
- [ ] Add and satisfy `test_session_orders_text_and_binary`, `test_session_terminal_slot_lifecycle`, `test_session_close_releases_subscriptions`, and `test_session_cancel_does_not_restart_engine`.
- [ ] Add and satisfy `test_session_lag_signals_resync`, preserving existing close/recovery semantics rather than continuing after silently dropped terminal/history events.
- [ ] Run the same hello/request/response/recovery fixtures through an in-process channel pair and an isolated real WebSocket adapter. Reject missing credentials on both adapters; do not mock the real-network parity check.
- [ ] Run exact new test filters with the Cargo command from Task 2 and only directly affected existing network test filters. Confirm existing HTTP origin/host checks remain in the network boundary.
- [ ] Reviewer must approve WebSocket parity before Task 4 changes local attachment. If extraction exposes missing protocol behavior, stop for a plan amendment, not a new private feature protocol.

### Task 4: Owner-scoped bounded Tauri IPC adapter

**Files:** `R/local_transport.rs`, `R/local_transport_test.rs`, `R/main.rs`, `A/desktop/daemon/{desktop-daemon.ts,local-daemon-transport-rpc.ts,desktop-daemon-transport.ts,test-local-daemon-transport-rpc.ts,desktop-daemon-transport.test.ts}`; Tauri capabilities only if the existing command boundary requires a narrow explicit permission.

**Consumes:** Task 3 `run_session`; existing `DaemonTransport` interface.
**Produces:** Fixed open/send/ack/close contract above, opt-in IPC backend and generation-safe frontend delivery.

- [ ] Implement the registry/byte-budget state as a native module that can be tested without launching a GUI. Add failing tests `test_ipc_cross_owner_commands_rejected`, `test_ipc_stale_generation_rejected`, and `test_ipc_untrusted_surface_rejected`; satisfy them before exposing commands.
- [ ] Bind open/send/ack/close commands to the actual caller and channel, not app-wide events. Validate exactly one send payload and binary decoding; retain frame limits and auth hello. Reject unsupported local targets instead of silently selecting localhost.
- [ ] Add and satisfy `test_ipc_open_close_race`, `test_ipc_duplicate_open_rejected`, `test_ipc_close_emitted_once`, `test_ipc_writer_failure_surfaces`, and `test_ipc_window_destroy_releases_session`. Close does not cancel agent processes.
- [ ] Implement byte/frame quotas and cumulative acknowledgements. Add `test_ipc_queue_byte_and_frame_caps`, `test_ipc_channel_inflight_caps`, `test_ipc_ack_cannot_release_unsent_frames`, and `test_ipc_overflow_requires_resync` using exact contract limits, deterministic controlled readers and no timing sleeps.
- [ ] Extend the frontend RPC wrapper/fake RPC with per-session channel registration and ack. Use promise-returning transport sends; do not acknowledge a known failed enqueue. Add frontend tests for rejection, ordering, stale callbacks, delayed handlers, ack progression and binary payloads.
- [ ] Preserve legacy non-managed transport callers; reject advertised but unimplemented SSH behavior visibly rather than accidentally attaching the local engine. Do not claim new SSH support.
- [ ] Run exact Cargo filters and `cd packages/app && npx vitest run src/desktop/daemon/desktop-daemon-transport.test.ts --bail=1` sequentially. Reviewer checks both ownership and every buffering layer, including Tauri's channel backlog.

### Task 5: Select IPC only for the managed Rust host

**Files:** `A/runtime/{host-runtime.ts,host-runtime.test.ts,daemon-start-service.ts,daemon-start-service.test.ts}`, `A/types/{host-connection.ts,host-connection.test.ts}`, `A/desktop/daemon/desktop-daemon.ts`, `R/main.rs`, `R/kernel/access.rs`.

**Consumes:** Typed engine/status/capability and IPC adapter from Tasks 2/4.
**Produces:** One boundary selecting managed IPC; stable host identity and preserved explicit external connections.

- [x] Cover `managed Rust host selects IPC`, `Node host retains network transport`, `explicit loopback host is not silently managed`, `unknown transport setting is rejected`, and `IPC failure does not fallback to WebSocket`. Missing selection/validation behavior has executed RED/GREEN; already-supported network choices remain characterization, not fabricated REDs.
- [x] Select the adapter from desktop-managed server identity, engine mode and trusted native capability. Do not infer managed ownership from a label, path prefix, localhost address or default port alone. Native credential retrieval must use the same authority boundary.
- [x] Normalize only the exact managed host's duplicate local connections, preserving serverId, workspace/agent selection, lifecycle data and explicitly saved remote hosts. Do not delete arbitrary loopback/pipe profiles.
- [x] Add registry tests for old local profiles, changed ephemeral endpoint, duplicate entries, unrelated localhost:6767 host, and repeated normalization with identical output. Do not introduce an incompatible required protocol field.
- [x] Add startup/reload tests proving engine phase changes reach existing UI error/pending handling; stored endpoint snapshots cannot replace the IPC identity after a reload.
- [x] Run, individually and sequentially, `cd packages/app && npx vitest run src/runtime/host-runtime.test.ts --bail=1`, then the analogous exact commands for `src/runtime/daemon-start-service.test.ts` and `src/types/host-connection.test.ts`. If a changed case needs browser project selection, use that file's existing Vitest project rather than the full workspace.
- [x] Author inspected unmanaged/remote credential isolation and browser/mobile import gates; affected app typecheck and exact tests passed. No independent Task 5 review; final fresh-context review remains pending.

### Task 6: Integrated verification, rollout and final report

**Current outcome:** Blocked / not default; not complete. See [verification evidence](2026-11-17-desktop-local-ipc-evidence.md) and workspace `task-6-report.md`. Temporary data homes do not isolate the packaged app's canonical IPC/shared-memory names, single-instance handling or deep-link registration. No packaged process was launched. User approved both isolation design and [implementation plan](2026-11-18-desktop-ipc-acceptance-isolation.md); inline isolation execution started. Runtime acceptance still waits for its complete safety gates. Installer build/structure passed, installation smoke did not run. Independent review remains pending.

**Files:** Exact files from Tasks 1-5; evidence document; owning docs `docs/architecture.md`, `docs/terminal-performance.md`, `docs/qa.md`.

**Consumes:** Opt-in IPC passing all task-level checks.
**Produces:** Verified default selection or an explicit blocked/not-default outcome, plus concrete evidence and remaining risks.

- [ ] Use an isolated owned desktop process with temporary `KRONOS_HOME`, opt-in IPC, controlled provider fixture and the actual exported frontend. Verify native Tauri channel delivery; fake RPC tests alone do not pass this gate.
- [ ] Exercise cold start, UI reload during open, two registered trusted windows, untrusted guest rejection, sleep/resume, explicit test-engine restart, agent send/cancel/approval, persisted history reload, terminal input/resize/replay, files/watch, plugin traffic and separate remote WebSocket auth/recovery.
- [ ] Stop only the isolated network listener while keeping its core running: managed IPC continues; HTTP-dependent features show the correct distinct network failure. Inject slow consumption, overflow and writer errors; verify bounded queues, explicit recoverable status and resubscription without duplicated agent turns.
- [ ] Repeat baseline measurements using the same build mode, workloads and provider fixture. Record startup/attach/request/stream metrics and maximum queue/memory observations; distinguish observed wins, regressions, and unmeasured platform cases.
- [x] Run affected checks sequentially: `npm run typecheck --workspace=@getpaseo/app`; if client declarations changed, first `npm run build:client`; `cargo check --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`; `npm run lint -- <exact changed TypeScript file paths>`; `npm run format:check -- <exact supported changed file paths>`. Use repository script help to confirm targeted formatting args; never mass-format unrelated dirty files.
- [x] Run `npm run bundle:desktop` once and validate the resulting NSIS installable using the existing supported pipeline. Successful retry exit 0; fresh PE installer/independent alias validated. Installation and package runtime smoke remain blocked, not accepted. Do not install over the user's live app without approval; if isolated install/package smoke is unavailable, report packaging/manual acceptance blocked and do not call it production-ready.
- [ ] Only after acceptance, make IPC default for embedded Rust; retain explicit `KRONOS_DESKTOP_LOCAL_TRANSPORT=websocket` rollback during the first release. Run the directly changed selector tests once after switching default.
- [ ] Fresh independent reviewer examines diff plus connected code for security, correctness, parity, queue accounting and unnecessary complexity. Fix findings with targeted regression tests.
- [x] Update owning docs and evidence with exact commands, pass/fail counts, measurements and pending cases. Final report distinguishes implemented, tested, blocked and unverified. Do not claim the original intermittent disconnect fixed unless its reproduction now passes.

## Stop / amendment gates

- Source changed concurrently in an owned region: reconcile without overwriting, and ask for amendment if interfaces/behavior changed.
- Shared session cannot preserve current protocol/permissions/terminal semantics: stop before adding a bypass.
- Native Tauri delivery cannot bound its in-flight backlog or isolate the caller: IPC cannot become default.
- Independent workers cannot be launched: disclose missing review/delegation; user-authorized inline amendment permits implementation but never fictitious independent approval.
- Full sleep/resume, installer or platform acceptance cannot be exercised: mark those gates pending; no unsupported reliability/production-readiness claim.

## Commits and final integration

Keep each task independently reviewable. Commit only this task's changes if the user authorizes commits or the execution workflow explicitly selects that integration step; never stage all files from this dirty checkout. No push, PR, release or publication is included.

## Self-review and approval

Spec coverage mapped: baseline -> Task 1; lifecycle and network independence -> Task 2; shared semantics -> Task 3; channels/ownership/flow control -> Task 4; managed selection/compatibility -> Task 5; full acceptance/measurement/rollout -> Task 6. All five Review Focus cases have owning tests. Dependency order prevents concurrent edits to the shared Rust files.

Queue limits are implemented safety policy, not measured performance. Original execution approved; user subsequently selected inline execution after Orca caller binding failed. Isolation design and its separate implementation plan are approved; execution uses the new plan's own ledger and safety gates.
