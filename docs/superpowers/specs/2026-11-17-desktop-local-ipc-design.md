# Desktop local IPC: proposed design and rollout

Status: design approved by the user's request to implement and test. Detailed execution-plan approval remains required before product-code changes.

## Intent

Make Kronos desktop's managed embedded Rust connection independent of local TCP ports and WebSocket handshakes. Preserve existing agent, terminal, file, history, permission, and remote-client behavior. Adopt Monocode's direct local-control pattern without copying its application architecture or rewriting Kronos features.

This is an architectural improvement, not a verified diagnosis of the user's reported disconnection. No speed, RAM, or reliability improvement has been measured yet.

## Current evidence

Inspection baseline: HEAD `86dc2d582` plus existing uncommitted working-tree changes. Line numbers will move as concurrent changes land.

- `kernel/server.rs`: `DaemonHandle::start()` now calls `start_internal(0, true, None)`. The Rust desktop uses an OS-assigned port, not a mandatory port 6767. The previously reported Rust fixed-port startup blocker no longer applies.
- `runtime/host-runtime.ts`: direct TCP uses the client WebSocket transport; pipe/socket profiles use the desktop bridge. These are two possible local paths, not a single mandatory chain.
- `main.rs`: `open_local_daemon_transport` accepts an untyped target but always connects to the active local WebSocket. It does not implement the advertised SSH target semantics.
- The same bridge uses an unbounded outgoing channel, ignores some enqueue failures, and does not notify the UI on a stream ending without an explicit close frame.
- `kernel/server.rs::handle_ws` owns authentication, event subscriptions, terminal slots, binary handling, and overflow recovery as well as frame transport. Calling `dispatch_session_message` directly would bypass required behavior.
- Some lagged subscriptions intentionally close the connection for resynchronization. Removing WebSocket does not remove the need for overflow detection and replay/recovery.
- `daemon.rs::start_daemon` currently stops the embedded engine before starting it. A local session opening or reloading must not acquire this restart behavior.
- Existing work in `docs/superpowers/plans/2026-11-07-native-architecture-hardening.md` deliberately closes lagged connections rather than silently losing data. Preserve that invariant.

## Alternatives

1. Harden current dynamic-port WebSocket path only. Smallest immediate change; retains local endpoint discovery and bridge complexity. Useful first milestone, not the selected long-term desktop path.
2. Add an in-process transport adapter over the existing Rust session semantics. Recommended. Reuses the client transport interface and shared business logic while eliminating network connection setup from managed local desktop sessions.
3. Replace client/protocol behavior with feature-specific Tauri commands, matching Monocode more literally. Rejected: duplicates feature plumbing and risks terminal, subscription, plugin, permission, and remote parity.

Recommendation 2 is the selected direction, approved by the user's request to implement and test. Execution method requested: subagents.

## Target shape

```text
Managed local desktop UI
  -> existing DaemonClient transport interface
  -> Tauri IPC commands + caller-scoped streaming channel
  -> shared Rust session core
  -> existing agents / terminals / files / storage

Remote/mobile/CLI clients
  -> existing WebSocket adapter
  -> same Rust session core
```

IPC is transport, not a second engine. Existing JSON envelopes, response correlation, binary framing, capability negotiation, and recovery remain compatible. This is not a zero-copy design claim.

## Scope and invariants

- Only the managed embedded Rust desktop connection changes transport.
- Node-engine mode, non-managed hosts, mobile, browser, CLI, and relay keep their existing supported paths. Do not silently send their requests to the embedded local engine.
- Rust session core owns admission, subscriptions, terminal slots, routing, cancellation, and teardown. WebSocket-specific framing stays in its adapter.
- Initially preserve hello/auth semantics. IPC must validate the calling window/webview and bind every session to that owner; only trusted desktop surfaces can open local sessions.
- Use a session-specific Tauri channel rather than broadcasting private agent events to every window. Derive owner identity from the command caller, not frontend-supplied identity.
- Opening, reloading, or closing a UI transport must not restart the engine or cancel an active agent. Explicit user cancellation and shutdown retain their defined semantics.
- Never acknowledge a command after a known enqueue failure. Transport acceptance does not mean the agent operation succeeded; retain request-response acknowledgements.
- Use bounded queues with explicit byte/frame limits and an in-flight acknowledgement window. Tauri callback delivery alone is not evidence the frontend has consumed data. Trace every buffering layer; no unbounded queue hidden behind the channel.
- On overflow, signal a recoverable session close and reuse established client resubscribe/replay behavior. Do not silently discard ordered terminal or timeline data. Engine and agent processes remain alive.
- Emit terminal transport state once per session generation. Late callbacks from an old generation cannot affect its replacement.
- No silent local WebSocket fallback on IPC failure. Surface the failure instead of concealing transport regressions. Keep an explicit build/release rollback path until acceptance passes.
- No database migration, credential rotation, provider rewrite, UI redesign, new runtime, or global tooling installation.

## Engine lifecycle

Expose one authoritative state: `stopped`, `starting`, `ready`, `failed`, with a useful failure reason. Transport session state is separate from engine state.

Concurrent ensure-start requests share one startup operation. A healthy or starting engine is not stopped and recreated. Restart is a separate explicit action. Failed startup completes pending requests with an error rather than leaving them waiting on a stale port.

Local UI attachment uses readiness of the shared Rust core, not an HTTP health probe. HTTP readiness remains available to network consumers.

IPC session teardown releases its subscriptions and bounded buffers, not agent ownership or durable history. Desktop process termination remains a different event; this design does not promise that embedded agents survive application exit.

## Network surfaces

Removing WebSocket from managed desktop transport does not remove HTTP endpoints used by indexed history, OMP session inspection, browser proxy, or external clients. Keep those surfaces and their existing access checks in the first release. Audit their dynamic endpoint resolution separately before claiming the entire app is port-independent.

Remote networking remains supported but is not a dependency of local IPC readiness. Disabling network listeners by default is outside this migration until HTTP feature dependencies are resolved.

## Proposed milestones

### 0. Baseline and reproduction

Record current startup-to-ready, transport attach time, request latency, stream delivery delay, and queue growth under fixed workloads. Use temporary state and owned test processes. Capture failure reasons at UI -> transport -> session -> engine boundaries without logging credentials or prompt contents.

Reproduce cold start, UI reload, sleep/resume, explicit engine restart, and slow-consumer behavior. If the original disconnect cannot be reproduced, document that limitation and use deterministic fault injection for individual failure paths.

Deliverable: before-measurements and a failure-path matrix, not an assumed root cause.

### 1. Lifecycle and existing bridge correctness

Make ensure-start idempotent; publish engine state explicitly. Ensure EOF, writer failure, invalid frames, and close racing registration have observable, single terminal outcomes. Reject unsupported targets rather than silently connecting to local. Bound queueing and preserve cancellation cleanup.

Candidate files: `src-tauri/src/daemon.rs`, `src-tauri/src/main.rs`, existing app desktop transport files and their tests. Confirm all callers before editing. Do not replace a working SSH path with an unimplemented adapter; preserve behavior that is actually supported and report current gaps.

Deliverable: deterministic lifecycle and transport-failure tests before changing the default transport.

### 2. Shared session extraction

Extract transport-neutral session behavior from `handle_ws` and related pipe/socket session handling. Keep existing dispatch functions as implementation dependencies rather than routing IPC straight to them.

Candidate new owner: `src-tauri/src/kernel/session.rs`; existing `kernel/server.rs` remains network-adapter owner. Confirm final boundary during the detailed plan.

Deliverable: WebSocket retains equivalent admission, requests, subscriptions, binary terminal behavior, recovery, and teardown through the shared core. Compare IPC and WebSocket with the same contract fixtures.

### 3. Local IPC adapter

Add local open/send/close commands backed by bounded in-process session queues and a caller-scoped Tauri channel. Include generation ownership, acknowledgements/backpressure, cancellation, and cleanup on window destruction. Reuse existing serialization and binary contracts first; optimize encoding only after measurement.

Candidate new owner: `src-tauri/src/local_transport.rs`; Tauri command registration remains in `main.rs`. Frontend adapts existing desktop RPC/transport wrappers rather than rewriting `DaemonClient` or feature components.

Deliverable: an opt-in internal adapter that passes shared session contract tests and cannot leak events across windows.

### 4. Managed-local selection and compatibility

Select IPC only for the managed embedded Rust host. Keep stable host identity independent of ephemeral ports, preserve serverId and workspace/session selection, and remove duplicate managed-local connection entries only through a narrow, tested registry normalization.

Candidate files: `desktop/daemon/*`, `runtime/host-runtime.ts`, `runtime/daemon-start-service.ts`, `types/host-connection.ts`, and client transport definitions only if necessary.

Keep explicit network addresses for non-managed connections. Preserve protocol compatibility; any new shared wire fields must be optional. Use a single capability selection boundary rather than fallback logic scattered through features.

Deliverable: a managed desktop session makes no local WebSocket connection; remote/mobile/CLI paths remain functional.

### 5. Acceptance and default switch

Run checks sequentially, not in parallel. Start with exact impacted test files and filtered Rust tests; do not run the full suite locally. Rebuild generated workspace declarations if needed before diagnosing TypeScript errors. Run affected typechecks and repository npm lint/format scripts after implementation changes.

Validate the actual bundled frontend and installable NSIS build through the supported bundle pipeline. Do not publish or distribute raw executables. No production daemon restart, release publication, or credentials changes are authorized by this document.

Switch IPC to default only after acceptance. Retire obsolete local WebSocket bridge code only when remaining callers are accounted for; retain the network server for its supported consumers.

## Acceptance matrix

- Concurrent local attachments start at most one engine; attaching never restarts a running engine.
- Cold start, UI reload, sleep/resume, and explicit owned-engine restart recover with useful status and no duplicate sessions.
- Text and binary traffic retain ordering, request correlation, cancellation behavior, and existing frame limits.
- Slow consumers produce bounded memory growth and explicit recoverable resync; no silent data loss.
- Chat send/cancel, tool approval, provider settings, timeline reload, terminal input/output/resize/replay, file browsing/watch updates, plugins, and two independent windows preserve behavior.
- Untrusted webviews cannot open or send to a local session; one window cannot control another window's session or receive its private channel events.
- Existing remote WebSocket handshake, authentication rejection, subscription recovery, and client parsing still pass.
- Managed desktop IPC remains usable when the test network listener is unavailable; HTTP-dependent features report their separate availability accurately.
- Before/after measurements use the same machine, workload, build mode, and provider stub. Report observed results; no guaranteed model-speed, startup, or RAM claim.

## Risks and decision gates

- Session extraction is the highest-risk change because current connection code owns substantial behavior beyond serialization. Complete WebSocket parity before default switching.
- Tauri streaming can still back up; native bounded queues without frontend consumption control are insufficient.
- Dynamic network endpoints remain relevant to HTTP-dependent features and external clients even after local IPC works.
- Existing uncommitted changes overlap this work. Preserve them; establish an agreed source baseline before implementation and stop if concurrent changes invalidate the plan.
- Fault-injection tests cannot prove the original intermittent disconnect is fixed. Keep original-issue reproduction evidence separate from adapter correctness evidence.

## Design review

Self-reviewed against current callsites and the existing native-hardening plan; no independent reviewer/subagent was available. Remaining design choices for the detailed plan: exact session extraction seam, queue byte limits/acknowledgement batching, test-file ownership, and supported rollout switch. These must be resolved from source and measurements before implementation, not invented as already-verified behavior.

## Pending amendment: isolated native acceptance

Status: **DESIGN APPROVED** by user's `yes` after the isolation proposal. [Isolation implementation plan](../plans/2026-11-18-desktop-ipc-acceptance-isolation.md) subsequently approved by user's next `yes`; inline execution started. Existing implementation and Task 6's blocked/not-default result remain unchanged.

### Purpose and boundaries

Run the actual exported Expo frontend, Tauri channel/interceptor, managed Rust engine and existing provider/PTY/session code beside the user's live desktop without sharing its mutable state or OS registrations. Start with Windows. Preserve normal build defaults and all existing trust checks.

This is a narrow acceptance seam, not an OS security sandbox or a second application architecture. A fake session server, cfg(test) engine initialization, separate example frontend or dispatcher-only test cannot satisfy this amendment. Acceptance binaries remain internal test artifacts; do not replace/distribute the validated NSIS installers or offer portable executables. Installer installation/upgrade, actual machine sleep and other platforms remain separate acceptance gates.

### Approaches considered

- **Acceptance-only build of the same application — recommended proposal.** A non-default Cargo feature includes the driver/control seam; default features, production manager initialization and actual exported frontend stay enabled. Isolation policy changes resource names and shell side effects, not session/auth/provider behavior. Costs an additional build and does not prove unchanged installer startup behavior.
- **Runtime test flag in ordinary installers — rejected.** Avoids another variant but ships test controls and broader boot policy in the normal application.
- **Separate minimal Tauri harness — rejected.** Easier to isolate but omits the real application's startup, host hydration and caller registration; duplicates the boundary being tested.

### Boot contract

An acceptance invocation must validate its run descriptor before tracing, portable-mode detection, CLI dispatch, engine/global initialization or Tauri plugin startup. The descriptor belongs to an exclusively created run directory and identifies its run, owned paths, selected adapter and controlled fixture. Reject invalid, partial, reused or non-owned configuration before side effects; never fall through to normal desktop startup. A normal build must reject an acceptance invocation rather than start the user's app accidentally.

One immutable acceptance context supplies the run-specific policy. Production code retains its existing defaults when the acceptance feature is absent; no global environment mutation in a shared test process. The launcher sets child-only homes and fixture configuration before spawn. Do not copy the user's registry, agent history, provider credentials or browser profile into the fixture.

Keep native engine initialization on the normal non-cfg(test) path. Use existing session/auth/client abstractions and the existing controlled-provider strategy; no new keys, relaxed guest permissions, business dispatch API or silent WebSocket fallback.

### Isolation coverage

| Surface                                                          | Acceptance policy                                                                                                                                                                                              |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Engine state, timeline DB, files, provider workspace and reports | Owned run directory; both home variables agree; explicit fixture executables and fixture-only registrations. Audit provider discovery/config paths before launch; no fallback to real providers.               |
| Native tracing/crash diagnostics                                 | Owned logs, configured before the first logger. Current tracing uses a platform data-local path independently of KRONOS_HOME; fixing only the home is insufficient.                                            |
| Named pipe/socket, both shared-memory rings and derived mutexes  | Fresh per-run names; HTTP/WS binds port 0. Never open canonical production names. Name collisions fail rather than reuse existing resources.                                                                   |
| Tauri identity and WebView2/local storage                        | Run-specific app identity and explicit owned browser data directories for all acceptance windows, including guests. Disable window-state persistence that would write outside the owned directory.             |
| Single-instance behavior                                         | Separate run identity, configured before plugin startup; never signal/focus the production instance. Installed single-instance plugin derives its Windows identity from app config.                            |
| Deep links, global shortcuts, tray, OS notifications and updater | Suppress acceptance shell integrations and updater actions. Do not register schemes/hotkeys, check/install updates or show OS notifications. These omitted behaviors remain untested, not implicitly accepted. |
| Application lifecycle and test controls                          | Actual window registration/navigation and session-generation checks remain. Acceptance-only controls require the existing trusted-caller boundary and current run; a guest cannot invoke them.                 |
| Child/process teardown                                           | Own the app, WebView and provider tree from launch; no executable-name kills or production-process attachment. Confirm exit before deleting the run directory.                                                 |

Installed Tauri 2.12.0 provides mutable app configuration and explicit WebView data directories. These APIs establish a feasible seam, not proof that every plugin/subsystem respects it. Any additional global path or side effect found during implementation must be isolated or excluded explicitly before a GUI launch.

### Driver and observation

Reuse the existing Node/Windows/native tooling; add no framework or dependency. The driver launches an owned acceptance process and exercises real frontend actions and normal transport calls. Prefer a feature-gated injected scenario/report bridge over exposing a new network debugger. Reports are run-local and bounded; do not log local credentials, user prompts or external provider secrets.

Native-only controls may create an explicitly trusted second window and an unregistered guest, stop only the owned network listener, inject bounded pressure/writer faults and request owned exit. They cannot bypass admission, replace feature RPCs or fabricate frontend consumption. Capture observed renderer receipt/order, close/recovery, engine/provider identities and process ownership, not only native send success. Both adapter comparisons use the same acceptance build, exported assets, fixture and instrumentation.

Windows process ownership must be established before the app can spawn descendants, with deadline supervision and exact-handle cleanup. Reuse existing Job Object support where possible; verify nested WebView/provider jobs rather than assuming they work. If containment or confirmed exit fails, stop and retain the owned directory. No fallback to name-based termination or untracked orphan cleanup. The earlier build runner's elapsed/deadline discrepancy is not a verified supervisor design to copy.

### Acceptance sequence and limits

1. Prove isolation policy and malformed/unsupported invocation rejection without launching a GUI or touching production paths.
2. Launch the real frontend in one owned run; verify server identity, selected IPC adapter, authenticated request and actual native callback delivery, then cleanup. Failure blocks broader runs.
3. Exercise guest/cross-window rejection, reload/open races, active controlled turn survival, real PTY input/resize/cleanup and network-listener independence. Verify failures through renderer-visible state as well as native receipts.
4. Exercise the remaining original Task 6 functional and pressure/recovery matrix with deterministic fixtures; do not replace it with a smaller smoke test.
5. Compare IPC and WebSocket with the same acceptance variant and fixture. Report instrumentation/variant differences from a normal installer; no runtime performance claim from compile times or configured queue caps.

Missing isolation coverage, normal-build behavior drift, failed process containment or a required public protocol/auth change stops execution for amendment. Actual suspend/resume must not suspend the user's working machine without approval. This harness cannot certify installation hooks, updater/deep-link/shortcut behavior or other platforms while those integrations are excluded.

### Proposed ownership and approval

Likely owners: an acceptance-only native module and Cargo feature; early boot/window/plugin wiring in main.rs; daemon/network namespace selection; shared-memory naming and tracing path selection; the existing native smoke/bridge test tooling. Keep one context rather than scattering environment flags. Detailed task/file/test boundaries belong in the implementation plan after this design is approved.

Design builder-check was author-only because no subagent tool is available; user approval is not independent technical review. Read-only recon included graph hints and direct source/locked Tauri verification; the graph was truncated/version-mismatched, so source is authoritative. Execution follows the approved plan's staged checks; GUI launch remains gated by complete isolation and owned supervision.
