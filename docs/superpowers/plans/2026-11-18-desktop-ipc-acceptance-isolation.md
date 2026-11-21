# Desktop IPC Acceptance Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unblock Windows native-channel acceptance with an owned, isolated run of the actual desktop app, without interacting with the user's production desktop.

**Architecture:** A non-default `ipc-acceptance` Cargo feature adds an early boot context, owned launcher and bounded scenario controls. The same exported frontend, normal Rust manager initialization, native caller checks and session/client implementations run inside per-run resource namespaces. Normal builds retain their defaults and reject acceptance invocations before startup side effects.

**Tech Stack:** Existing Rust/Tokio/Tauri 2.12.0, Windows Job Objects, Node tooling, Cargo tests and existing Node bridge-test suite. No new dependency, automation framework or public protocol.

**Spec:** [Approved isolation amendment](../specs/2026-11-17-desktop-local-ipc-design.md#pending-amendment-isolated-native-acceptance).

**Status:** Design approved by user's `yes` after the isolation proposal. **Implementation plan approved by the user's next `yes`; inline execution started.** Preserve the previously selected inline execution method. This plan supplies the isolation seam for original [Task 6](2026-11-17-desktop-local-ipc.md#task-6-integrated-verification-rollout-and-final-report), not replacement acceptance criteria or permission to switch IPC default.

## Global Constraints

- Preserve normal build defaults and all existing trust checks.
- Run the actual exported Expo frontend, Tauri channel/interceptor, managed Rust engine and existing provider/PTY/session code. No cfg(test) engine in integration runs.
- Acceptance binaries remain internal test artifacts; do not replace/distribute the validated NSIS installers or offer portable executables.
- No new keys, relaxed guest permissions, business dispatch API or silent WebSocket fallback. Fresh fixture homes use the existing local-admission mechanism, never copied production credentials or new provider API keys.
- This is a narrow acceptance seam, not an OS security sandbox or a second application architecture.
- Installer installation/upgrade, actual machine sleep and other platforms remain separate acceptance gates.
- No production restart/kill, canonical IPC access, default-home inspection/repair, registry registration, global hotkeys, updater/network-provider discovery, publishing, installation, staging or commits.
- Preserve dirty checkout, HEAD `86dc2d58295253bdf97b72301ab396d41396be9d`, branch `feat/desktop-local-ipc-20261117`. Before-copies/task-relative deltas remain authoritative; unchanged HEAD ranges are not a feature diff.
- One source writer and sequential test/build token. Exact test files/filters only; no full local suites or source-wide formatting.
- Each new behavioral test must execute RED before its fix. Missing symbols/compiler failures and already-green characterizations are not behavioral RED.
- New globals run in exact-test subprocesses with owned homes established before access. No environment mutation in a shared test process. Delete runtime homes only after exact leader/tree exit confirmation; otherwise retain and block.
- After implementation batches, run affected typechecks, npm lint scripts and targeted formatting. No unchanged expensive build repeat; capture before-hashes/backups for generated outputs before a justified rebuild.

## Review Focus

1. Old/ordinary executable or partial descriptor passed to launcher: reject before logger, portable detection, CLI dispatch, engine or GUI. Task 1 rejection tests and Task 3 build-receipt test.
2. Junction/alias path or pre-existing mapping/mutex/pipe: never escape owned root or reopen an unowned object. Tasks 1–2 path and collision tests.
3. Real provider/MCP configuration discovered despite temporary homes: no host credential/config read or real service probe. Task 2 fixture-discovery tests, Task 3 environment checks and Task 5 fixture launch receipts.
4. Guest or reloaded window submits a control/report for another incarnation: no privileged control, status/credential access or session takeover. Task 4 native window tests and actual GUI trust case.
5. Supervisor dies, child stalls or provider/WebView outlives app: deadline terminates only owned tree; retain directory if exit unconfirmed. Task 3 suspended-launch/descendant/stall cases and Task 4 real-WebView cleanup.

---

## File boundaries and fixed interfaces

Let `R = packages/desktop-tauri/src-tauri/src`. Paths below are relative to repository root; each `R/` reference expands to that exact directory.

| File                                                                      | Responsibility                                                                                                                                                                 |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `R/desktop_acceptance.rs` (new)                                           | Argument sentry; feature-gated descriptor/context and resource ownership. Inline tests in existing Cargo target.                                                               |
| `R/desktop_acceptance/supervisor.rs` (new)                                | Feature-only owned launch, deadline, bounded capture and reap; uses existing ProcessTree.                                                                                      |
| `R/desktop_acceptance/commands.rs` (new)                                  | Feature-only closed control/report handlers, renderer injection and scoped observations.                                                                                       |
| `packages/desktop-tauri/src-tauri/Cargo.toml`                             | Add `ipc-acceptance = ["windows-sys/Win32_System_Pipes"]` outside default features; no versions/defaults change.                                                               |
| `R/main.rs`                                                               | Call sentry first; use context for log/portable/CLI/window/plugin policy; share real trusted-window construction; compose feature-only handlers with unchanged normal handler. |
| `R/daemon.rs`, `R/kernel/server.rs`                                       | Pass owned pipe lease to existing listener path; acceptance-only network stop/restart hooks preserve current core.                                                             |
| `R/kernel/shm.rs`, `R/kernel/telemetry.rs`                                | Exclusive acceptance mappings/mutexes and owned logs; unchanged production constructors/default paths.                                                                         |
| `R/agent/registry.rs`, `R/kernel/mcp.rs`                                  | Acceptance context overrides fixture resolution/config paths before host fallbacks. No provider execution rewrite or external MCP changes.                                     |
| `R/agent/manager.rs`, `R/kernel/config_store.rs`, `R/kernel/schedules.rs` | Approved Task 4 amendment: report manager home and scheduler registration; skip external workspace-path probes only while acceptance is active. Normal behavior unchanged.     |
| `R/kernel/rpc_helpers.rs`, `R/kernel/access.rs`                           | Approved Task 2 amendment: owned skill/plugin/directory discovery and per-run managed credential target policy; normal defaults unchanged.                                     |
| `R/browser_automation.rs`                                                 | Apply owned browser profile to guest/browser windows; do not register these as trusted application surfaces.                                                                   |
| `R/agent/runner.rs`, `R/agent/turn_process.rs`                            | Crate-local access to existing suspended process-tree helper; strict-CWD entry for acceptance supervisor. Existing runner behavior unchanged.                                  |
| `R/local_transport.rs`, `R/kernel/session.rs`                             | Feature-only, owner-scoped fault gates and actual queue observations; no quota/ACK/auth changes.                                                                               |
| `scripts/desktop-ipc-acceptance.mjs` (new)                                | Import-safe Node driver, fresh fixture setup, build-receipt checks and report collection.                                                                                      |
| `scripts/desktop-ipc-acceptance-renderer.js` (new)                        | Per-run immutable injected driver over real DOM/bridge; loaded only by acceptance build from fixed owned scenario.js, not a network/command-supplied script.                   |
| `scripts/fixtures/native-codex-smoke.cjs` (new)                           | Extract existing controlled Codex process fixture, extending deterministic gates/descendant observations without changing existing smoke semantics.                            |
| `scripts/smoke-native-desktop.mjs`                                        | Early `--gui-acceptance` dispatch before legacy smoke allocates state; retain default daemon-only mode and reuse extracted fixture.                                            |
| `scripts/verify-desktop-bridge-script.test.mjs`                           | Extend existing Node suite with driver/parser/renderer-observer behavior; VM tests stay explicitly non-native.                                                                 |
| `docs/qa.md`, parent spec/plan/evidence, existing SDD ledger              | Commands, variant limitations, approval and dated evidence. No claim of original-disconnect repair.                                                                            |

No app route, protocol schema, provider implementation, shared client package, new CI job or release/signing configuration changes. New ownership outside this map requires a plan amendment. These owners form one isolation boundary; do not split them into independently enabled modes.

### Approved Task 4 amendment — 2026-11-19

User approved adding three preflight support owners: `R/agent/manager.rs` exposes the existing manager home read-only; `R/kernel/schedules.rs` exposes whether the existing scheduler has registered; `R/kernel/config_store.rs` suppresses external workspace-path existence probes only during active acceptance. These supply actual ownership evidence and avoid probing unrelated host paths. Feature-off and ordinary behavior remain unchanged. `R/kernel/mcp.rs` remains under its existing Task 2 ownership; Task 4 only reads its already-scoped path for preflight evidence. No other source ownership is added.

### Contract decisions

- Descriptor `run.json`: `{ schemaVersion: 1, runId, transport, caseId }`; reject unknown fields. `runId` is 32 lowercase hex characters; transport is exactly `ipc` or `websocket`. Define the closed case enum in Task 1: preflight, native-smoke, trust-reload, turn-history, approval, terminal, files-watch-plugins, network-independent, backpressure, remote-recovery, remote-peer, measurements. Derive every mutable path/name from the validated descriptor directory, not JSON-supplied paths. Maximum descriptor: 64 KiB.
- Run directory: newly created `kronos-ipc-acceptance-<runId>` under SDD-owned `.superpowers/builds/` (launcher-owned storage with an explicit task amendment), with exclusive owner/launch/child claim files. Reject symlinks, Windows reparse points, relative/traversal/device/UNC paths, mismatched homes and reuse. Validate before touching globals. Same-process engine restart is allowed; launching another process with a consumed descriptor is not.
- `desktop_acceptance::bootstrap(args: &[OsString]) -> Result<BootAction, String>` is always compiled; `BootAction = Continue | Exit(i32)`. Without feature, any `--ipc-acceptance-*` invocation returns an error before side effects. Feature recognizes `--ipc-acceptance-info`, `--ipc-acceptance-run <descriptor>` and private `--ipc-acceptance-child <descriptor>`. An acceptance-enabled binary rejects ordinary/no-argument/daemon invocations, including accidental double-click, rather than starting the live desktop. Only feature-off normal invocations return Continue without filesystem/environment mutation; only a validated supervised feature-on child can continue into acceptance startup. Child acceptance arguments never reach generic CLI parsing.
- `desktop_acceptance::enter(args: &[OsString], run: impl FnOnce()) -> Result<i32, String>` calls bootstrap and invokes run only on Continue; Exit returns its code. Main uses this wrapper around the existing body renamed `run_desktop`, so rejection-order tests cover the same entry seam without launching the GUI.
- Feature-only `AcceptanceContext::from_descriptor(path: &Path) -> Result<Self, String>`; `desktop_acceptance::active() -> Option<&'static AcceptanceContext>`. Install once in owned child, after validation/claims and before tracing/engine. Resource names use `kronos-ipc-acceptance-<runId>-{daemon,telemetry,pipe}`; Windows mappings retain `Local\\` scope and derived mutexes. Fresh app identifier: `sh.paseo.desktop.ipc-acceptance.<runId>`.
- Paths: `home/`, `profile/main/`, `profile/secondary/`, `profile/guest/`, `logs/`, `tmp/`, `workspace/`, `bin/`, `reports/` below run root. `AcceptanceContext::claim_resources(&self) -> Result<(), String>` claims mappings/mutexes/first pipe in owned child before engine initialization. `ShmRingBuffer::new_exclusive(name: &str, capacity: usize) -> std::io::Result<Self>` never opens an existing object; ordinary `new` retains its behavior. Global getters return these same owned rings, not another allocation. `AcceptanceContext::claim_pipe(&self) -> Result<std::os::windows::io::OwnedHandle, String>` transfers initial pipe lease, or exclusively reacquires after confirmed owned listener stop. Convert handle in the existing accept loop's Tokio runtime, not a runtime that is then dropped.
- `create_desktop_window(app: &tauri::AppHandle, label: &str, bridge_script: &str, ipc: Arc<IpcState>, startup_trace: bool, started: Instant) -> Result<WebviewWindow, String>` in main.rs is the shared main/secondary creation path. Keep existing navigation/page-load registration and title/icon/window configuration; acceptance applies its profile and avoids foreground-focus calls. Guest construction never calls trusted registration.
- Driver copies the repository renderer script to fixed run-root scenario.js before claims; reject linked/oversized script (256 KiB maximum), read once into immutable memory before startup, and record its hash separately in run receipt. No script supplied by RPC or descriptor field. This permits JS-only case development without native rebuilds; binary receipt covers compiled native/product assets, run receipt covers current driver/scenario/fixture source. Paired measurements require all hashes identical.
- Native feature-only control/report handlers are exact commands `ipc_acceptance_control` and `ipc_acceptance_report`; normal command registry contains neither. Control input `{ runId, surfaceGeneration, action }` has a closed enum: create/reload/destroy known role, stop/start owned listener, open/ACK/writer gate, observe, exit. Roles are `main`, `secondary`, `guest`; no arbitrary URL, label, command, executable or JS payload.
- Controls derive caller from actual WebviewWindow and use `IpcState::authorize_window`; run ID alone grants no authority. Feature-only `IpcState::acceptance_surface_generation(window: &WebviewWindow) -> Result<u64, String>` reads the actual registered caller's incarnation. Require surfaceGeneration to match it and recheck captured native generation before deferred effects/results; it is a stale-page check, not frontend-issued authority. Reports may come from known fixture guest probes but are non-privileged, stamped with actual native role/incarnation and cannot change state or supply another window's results. Maximum observation: 16 KiB; total report file: 8 MiB; overflow fails case. Never report credentials or raw auth/prompt payloads.
- Runtime startup deadline 120 s; each scenario 300 s; shutdown leader/tree confirmation 3 s each; stream-drain 3 s. Deadline/self-check overrides only shorten these in test-only checks. Normal build/test capture uses a separate monotonic wall clock, maximum 1200 s per exact Cargo check/test and 5400 s per acceptance build, followed by bounded owned cleanup. These are proposed safety budgets, not measured timing claims.

## Verification commands

Prepare `.superpowers/sdd/2026-11-18-desktop-ipc-acceptance-isolation/isolation-cargo.ps1` in Task 1 from the existing exact-filter driver, adding `-Acceptance` and isolated log names without editing old receipts. Preserve two build jobs and correctness-only test profile overrides, exact-filter allowlist and 1200 s deadline. Replace end-only/unbounded stream waits with incremental capture and bounded drain; self-check finite timeout and exact reaping first. Never reuse Task 6's unproven 40-minute watchdog as a strict deadline implementation.

Use this command for each named native test, once per RED/GREEN cycle:

```powershell
powershell.exe -NoProfile -File .superpowers/sdd/2026-11-18-desktop-ipc-acceptance-isolation/isolation-cargo.ps1 -TestName <full-exact-filter> -LogPrefix <unique-stage>
```

Add `-Acceptance` for feature-only tests. It executes `cargo test --offline --no-default-features [--features ipc-acceptance] <filter> -- --exact --test-threads=1 --nocapture`. Require exactly one executed test, expected assertion/exit and confirmed owned cleanup, not just exit 0. Correctness profile is not the GUI build profile.

Node suite: `node --test scripts/verify-desktop-bridge-script.test.mjs`. Changed JS syntax: `node --check <exact-file>`. Use repository `npm run lint -- <exact changed JS paths>`, `npm run format:files -- <exact supported changed files>`, then `npm run format:check:files -- <same files>`. Limit Rust formatting to owned spans/modules using the established task formatter pattern; never reformat entire main.rs/server.rs.

### Task 1: Fail-closed boot and run ownership

**Status:** Verified at correctness scope; six exact native filters passed with typecheck, targeted lint/format and preservation. See new-plan `task-1-report.md` and official task-done receipt. Acceptance startup remains fail-closed pending complete isolation/supervisor; no GUI/channel acceptance claimed.

**Files:** Create `R/desktop_acceptance.rs`; modify Cargo.toml and main.rs entry; extend existing Node bridge suite and SDD exact-test driver.

**Interfaces:** Produces descriptor validation, BootAction/bootstrap and feature-only active context contract above. No GUI or engine startup in this task.

- [ ] Capture task-relative before-copies, source/generated-output hashes and branch/status. Verify no concurrent source writer/build; retain all earlier receipts.
- [ ] Add `desktop_acceptance::tests::test_ordinary_build_rejects_acceptance_before_side_effects` and `test_normal_boot_policy_is_unchanged`: assert unsupported acceptance returns error; normal args return Continue with zero filesystem/engine/logger callbacks. Run without feature. Compile-only scaffolding may be necessary; record it separately, then execute the missing-policy assertion RED before implementing policy.
- [ ] Implement argument sentry/enter wrapper and descriptor parser/context schema. Rename the current main body to run_desktop without moving/reformatting its contents; enter must validate before invoking it. Exclude consumed child args from generic CLI. No feature implies no context installation, private handlers or environment test switch. Pin entry assertions:

```rust
let called = std::cell::Cell::new(false);
assert!(desktop_acceptance::enter(&acceptance_args, || called.set(true)).is_err());
assert!(!called.get());
assert_eq!(desktop_acceptance::enter(&normal_args, || called.set(true)).unwrap(), 0);
assert!(called.get());
```

- [ ] Add/run feature tests `test_acceptance_build_requires_supervised_invocation`, `test_acceptance_descriptor_is_strict` and `test_acceptance_root_cannot_escape_or_be_reused`, one cycle each. Feature-on empty/ordinary CLI args and child without supervisor handshake reject before startup. Assert version 2, unknown field, malformed ID/mode, oversized JSON, relative/UNC/device path, home mismatch, junction and duplicate claim all reject; valid fresh owned root derives every path beneath itself. Test only temporary links/roots.
- [ ] Re-run each directly changed case GREEN; check manifest default remains exactly `["local-whisper"]`, no dependency/lockfile additions. Gate: invalid invocation cannot reach any startup callback, not merely eventual CLI failure.

### Task 2: Wire and prove the entire isolation boundary

**Status:** Complete. Two-file ownership/security amendment approved and implemented. Owned resource plan, exclusive SHM/mutex/pipe creation, fixture-only provider resolution, owned logs/MCP/discovery and per-run access targets are verified by exact native tests; host OS integrations, portable detection, crashpad scan, CLI dispatch, window-state/updater/tray/notifications/deep-link/single-instance and browser guest profiles are gated in source. `active()` remains `None` until supervised startup (Tasks 3–4), so those gates are latent wiring, not runtime-proof. Source findings and limits: new-plan `task-2-isolation-inventory.md`.

**Files:** Modify main.rs, daemon.rs, kernel/server.rs, kernel/shm.rs, kernel/telemetry.rs, agent/registry.rs, kernel/mcp.rs, browser_automation.rs, kernel/rpc_helpers.rs, kernel/access.rs; extend desktop_acceptance inline tests and existing directly affected native test modules.

**Interfaces:** Consumes active context. Produces owned ring references/pipe lease, fixture-only resolution and shared create_desktop_window. Feature-only `daemon::acceptance_set_network(enabled: bool) -> impl Future<Output = Result<(), String>>` sends a generation-bound request to the existing run task that owns its DaemonHandle; it awaits actual stop/start completion, errors on stopped/superseded core and never starts an engine. Default callers keep existing signatures/behavior unless the private pipe-handoff parameter is required.

- [x] Audit each boot/global/plugin/automatic discovery caller against the approved isolation table; record exact path/owner. Include tracing, crashpad scan, both browser builders (including Mica fallback), window-state plugin, hidden browser windows, provider registry, scheduler/config store and MCP. New unlisted escape blocks launch until isolated or explicitly excluded.
- [x] Write/run `desktop_acceptance::tests::test_acceptance_resource_plan_owns_all_paths`: owned log/MCP/profile paths, per-run app identity, distinct ring/pipe/mutex names, no shell integrations, no portable-mode auto-detection, no host fallback. Pure checks do not inspect actual default-home contents.
- [x] Implement context wiring. Acceptance skips deep-link/notification/updater/window-state/global-shortcut setup, tray, notification listeners, automatic update checks and real-provider discovery. Acceptance-only invocation restriction rejects omitted OS actions, CLI installation and arbitrary external navigation; do not relax normal permissions. Native heartbeat/watchdog and actual engine/client startup remain.
- [x] Add exclusive mapping/mutex/first-pipe creation; retain handles through handoff. Collision assertions use actual owned mappings:

```rust
let owner = ShmRingBuffer::new_exclusive(&owned_name, 64 * 1024).unwrap();
owner.write(b"sentinel").unwrap();
assert!(ShmRingBuffer::new_exclusive(&owned_name, 64 * 1024).is_err());
assert_eq!(owner.read_latest().unwrap(), b"sentinel");
```

`kernel::shm::tests::test_acceptance_collision_never_opens_existing_object` precreates only a random owned test mapping/mutex with sentinel bytes and asserts second exclusive claim fails with sentinel unchanged. `kernel::server_test::tests::test_acceptance_pipe_collision_fails_closed` checks the equivalent owned pipe. Reacquire only after confirmed owned listener shutdown; no canonical fallback.

- [x] Add/run `desktop_acceptance::tests::test_fixture_discovery_never_uses_host_fallbacks`: a temp decoy PATH/config cannot resolve; only exact fixture Codex/Node paths are available; Ollama/LM Studio probes return unavailable without connecting. `kernel::mcp::tests::test_acceptance_mcp_path_is_owned` asserts run-home mcp_servers.json and no call to platform-home fallback. Do not read/change user's MCP file.
- [x] Satisfy tests with acceptance checks at shared resolver/config seams, not provider rewrites. Gate: coverage inventory has no unresolved mandatory path; default constructor behavior and production resource names remain unchanged.

### Task 3: Owned supervisor, fixtures and build receipt

**Files:** Create `R/desktop_acceptance/supervisor.rs`, Node driver/fixture; add early GUI branch to existing smoke script; modify agent/runner.rs and turn_process.rs; extend desktop_acceptance.rs and existing Node suite. No GUI execution yet.

**Interfaces:** `ProcessTree::spawn_strict(command: &mut tokio::process::Command) -> Result<(Self, Child), String>` rejects absent/invalid CWD; reuse suspended prepare/attach/resume and existing 3 s cleanup. Existing spawn retains its fallback. Make helper/module crate-visible only as needed. `desktop_acceptance::supervisor::supervise(command: &mut tokio::process::Command, run: &AcceptanceContext, deadline: Duration) -> impl Future<Output = Result<OwnedExit, String>>`, where `OwnedExit { exit_code: i32, leader_reaped: bool, tree_empty: bool, timed_out: bool }`, owns all waits/cleanup; any unconfirmed exit is Err with retained directory. `runDesktopIpcAcceptance({ executable, buildReceipt, caseId, transport, receiptDir }) -> Promise<void>` and `seedControlledCodexFixture(home: string) -> Promise<void>` are import-safe Node exports.

- [ ] Extract existing controlled `codex.exe`/`exec` fixture into the new fixture file/helper; preserve existing initial/resume strings and legacy default behavior. Add file/event gates only for owned acceptance scenarios; record launch argv/PIDs and bounded output, never call a model/network. Keep original smoke mode daemon-only.
- [ ] Add Node assertions in the existing suite: import/invalid argv causes no spawn; stale/wrong-feature executable hash rejects before spawn; descriptor resources/environment are owned. Assert parent environment unchanged and child allowlist excludes API tokens, NODE_OPTIONS, inherited provider paths and WebView profile overrides. Set fixture PATH/homes/TEMP/CODEX_HOME explicitly; allow only necessary OS runtime variables from parent.
- [ ] Implement native launcher with existing ProcessTree: suspended creation, assign owned Job before resume, supervisor handshake before child globals, monotonic deadlines, incremental bounded diagnostics and exact cleanup. Strict-CWD failure cannot enter the existing provider home fallback. Launcher stays outside its child Job; drop/timeout affects only that Job. Do not introduce another process-tree implementation.
- [ ] Add/run exact feature tests `test_supervisor_owns_descendants_before_resume`, `test_supervisor_deadline_is_wall_clock`, `test_supervisor_reap_failure_retains_home` in desktop_acceptance::supervisor::tests. Use owned finite self-check children, never a GUI. A deliberately 20 s child with a 1 s deadline must be reaped within deadline plus 6 s cleanup; assert descendant termination, parent env unchanged and directory retention on injected unconfirmed exit.
- [ ] Require actual supervisor self-check assertions before build:

```rust
assert!(exit.timed_out);
assert!(exit.leader_reaped);
assert!(exit.tree_empty);
assert!(elapsed <= Duration::from_secs(7)); // 1 s workload budget + 6 s cleanup
```

- [ ] Implement import-safe build-receipt writer/validator in Node driver: executable SHA-256, compiled native source manifest, exported asset manifest, feature/build argv and successful command receipt are required. Unit tests use owned fixture files, never launch an unverified executable. Defer expensive actual app build until Task 4's native controls/observers are complete; this task's independent deliverable is tested ownership/fixture/receipt tooling, not a GUI binary.

### Task 4: Actual WebView delivery and native trust smoke

**Files:** Create `R/desktop_acceptance/commands.rs` and injected renderer script; modify desktop_acceptance.rs, main.rs, local_transport.rs and session.rs observation hooks; extend existing Node suite/local_transport_test.rs. Approved amendment also owns the three read-only/isolation support seams in `R/agent/manager.rs`, `R/kernel/config_store.rs`, and `R/kernel/schedules.rs` described above. Build once after native controls/observers are complete; rebuild later only for compiled native/product changes.

**Interfaces:** Closed control/report contract above. `NativeAcceptanceObservation` serializes camelCase fields `runId`, `caseId`, `role`, `incarnation`, `engineEpoch`, `sessionId`, `generation`, `lastSentSequence`, `lastAckSequence`, `sessionQueuePeakFrames`, `sessionQueuePeakDecodedBytes`, `channelPeakEvents`, `channelPeakSerializedBytes`, `closeCode`, `providerPids`, `providerStarts`. Renderer report adds `lastReceivedSequence`, `visibleState` and `outcome` (`pass|fail|blocked`), stamped with native caller identity; never treat renderer fields as native measurements. `IpcState::acceptance_observe(window: &WebviewWindow) -> Result<NativeAcceptanceObservation, String>` authorizes actual caller before reading state. `validateObservation(native: object, renderer: object, expected: {runId, caseId, role}) -> void` in Node driver joins matching identities/session/generation and rejects missing case-required observations. Feature-only counters track actual admitted capacity leases (queued/reserved frames/decoded bytes) and serialized unacknowledged events, with release on drop/ACK. Configured caps are separate fields, not measurements.

- [ ] Add/run `local_transport_test::test_acceptance_controls_use_actual_caller` and `test_acceptance_reports_cannot_impersonate_window`: guest/stale/cross-run privileged control rejects; bounded guest probe report cannot claim trusted role or execute a control. Normal handler has no private command. Do not give guests status/credential access for observation convenience.
- [ ] Implement feature-only handler composition and share real trusted window builder. Known secondary uses same production lifecycle hooks; guest has owned profile but no registration. Stamp driver bootstrap with native page generation; guest report generation comes from its feature-only probe registration, never the trusted registry. Deferred controls revalidate native caller incarnation inside their effect boundary, so old document cannot stop/reload its successor. Acceptance avoids focus stealing. Apply profile to browser_automation window creation too.
- [ ] Add injected fixed scenarios using existing DOM test IDs/accessibility labels and normal app/bridge actions. Instrument only metadata and preserve real return promises/events. No fake Channel/transport factory, duplicated DaemonClient or rendered success overlay. Reports alone cannot synthesize a successful native callback.
- [ ] Add VM observer tests to existing Node suite: native send without renderer callback is not delivery; callback sequence/ACK and real app-visible state are required; hold at most one ACK gate and release in finally. Assert `validateObservation(nativeSendOnly, missingRenderer, expected)` throws; a pair with different session/generation/role also throws. Keep VM results labelled characterizations, not GUI acceptance.
- [ ] Add owner-scoped open gate after caller authorization/before opened event, writer-error gate at actual channel evaluator and enqueue/release high-water counters; compile out of ordinary build. Preserve 16 MiB frame, 128 frames/32 MiB decoded session queues and 64 events/32 MiB serialized channel window. No broad broadcast or quota change.
- [ ] After supervisor/handler/observer tests pass, back up/manifests for generated dist/server-dist and overwritten native binary; preserve NSIS bytes. Build exported assets and real non-cfg(test) application through installed Tauri CLI:

```bash
cd packages/desktop-tauri
node ../../node_modules/@tauri-apps/cli/tauri.js build --no-bundle --features ipc-acceptance --ci
```

- [ ] Use bounded owned build capture with child-only `CI=true`, `CARGO_BUILD_JOBS=2`, `CARGO_NET_OFFLINE=true`, `KRONOS_WITH_NODE=0`; remove correctness-profile overrides. Default local-whisper stays enabled. After exit 0 record command/native source/frontend/binary SHA-256 in build receipt. No installer regeneration/distribution. Failed build stops GUI work.
- [ ] Run isolated `preflight`, then `native-smoke`, then `trust-reload` sequentially:

```bash
node scripts/smoke-native-desktop.mjs --gui-acceptance --exe <receipt-bound-private-binary> --build-receipt <build-receipt.json> --case <caseId> --transport ipc --receipt-dir .superpowers/sdd/2026-11-17-desktop-local-ipc/isolation-runtime
```

- [ ] `preflight`: verify actual owned paths/claims, normal manager/scheduler initialization, no host provider/MCP resolution and cleanup; no GUI required. `native-smoke`: actual exported frontend ready, IPC selected, authenticated server_info/request observed in renderer and ACK observed natively; close app and prove WebView/provider tree empty. `trust-reload`: trusted second window delivers; guest/cross-owner/stale operations reject; hold real open, reload/destroy its window, release old generation and assert it cannot leak/reopen successor. Active fixture turn stays same process/engine.
- [ ] A smoke may pass immediately because product IPC already works; record characterization, not fabricated RED or original-disconnect fix. Any containment/isolation failure stops broader cases and retains evidence/home.

### Task 5: Original Task 6 matrix and comparable observations

**Files:** Node/renderer drivers and controlled fixture; observation hooks only if required in the owned files above. No production feature fixes without reproduced root cause and amended scope.

**Interfaces:** Same run/control/report contract and predeclared case enum. Add a Node contract test rejecting unknown IDs and pass-with-missing-observation reports. JS-only scenario/fixture edits update run receipts, not binary identity; native edits require a new verified build receipt.

- [ ] Add one scenario/assertion at a time; execute it before adding/fixing required harness behavior. Already-passing native behavior is characterization. Source-verify feature/capability support before claiming an approval/plugin test exists; unsupported current native behavior is a named blocker, not an invented fixture feature.

| Case ID               | Required observations/assertions                                                                                                                                                                                                                                                                                                                                                                                                                                |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `turn-history`        | Submit through real frontend; exactly one controlled provider start/prompt; reload during active turn leaves engine/child unchanged; cancel confirms descendant reap; saved user/assistant history reloads in order; explicit owned engine restart changes engine epoch, not duplicate history.                                                                                                                                                                 |
| `approval`            | Existing provider permission request reaches actual UI; approve/deny travels existing API and continues/blocks accordingly. If native request/response path is absent, mark BLOCKED; never insert fake pendingPermissions or add provider approval support in this harness.                                                                                                                                                                                     |
| `terminal`            | Spawn real ConPTY; text input reaches process, resize changes reported dimensions, output slot matches response, reconnect replays/snapshots without silent gaps, terminal kill/whole-run exit reaps child. Broadcaster-only fixture is insufficient.                                                                                                                                                                                                           |
| `files-watch-plugins` | Fixture file read/write produces real watcher event and visible update; subscribed real fixture plugin traffic uses existing plugin path. Missing native plugin capability stays BLOCKED; do not fabricate plugin events.                                                                                                                                                                                                                                       |
| `network-independent` | Stop only owned listeners; same core/turn continues via IPC while HTTP fails distinctly and native credential retrieval rejects the stopped endpoint. Bind a decoy only at the recorded, just-released owned ephemeral port; while held, restart same authority on port 0 and assert different endpoint, decoy gets no credential. Lost decoy bind is BLOCKED, not permission to displace another process. No production-port reservation or process/name kill. |
| `backpressure`        | Hold real consumption ACK, release finite fixture burst, observe actual high-water counts/bytes at each layer; explicit recoverable close/resync and snapshot/replay; provider start/prompt count still one. Test ordinary/writer-fault closes and oversized frame rejection separately, without logging payloads.                                                                                                                                              |
| `remote-recovery`     | Second owned native peer/home/namespace via same supervisor, no production remote. Real WebSocket rejects missing/wrong credential and recovers authenticated connection; frontend keeps explicit remote identity distinct. Read only peer's owned home/local-credential in memory, never publish it in status/report.                                                                                                                                          |
| `measurements`        | Alternating IPC/WebSocket runs with identical candidate binary/assets/workload; five fresh pairs, 100 sequential small requests per run, same finite stream. Record raw startup/attach/request/first-stream/teardown durations, p50/p95 where sample count permits, actual queue maxima and Windows working-set/private/peak memory definitions.                                                                                                                |

- [ ] Implement scenarios using existing feature calls/storage/providers; all files/processes/remote peers belong to run. Private controls only set faults/lifecycle, not fake RPC successes. Use condition/gate waits with deadlines, not timing sleeps to prove ordering.
- [ ] Run each listed case individually, sequentially. Functional cases use IPC; run equivalent WebSocket variant for parity cases where applicable, with identical fixture and receipt-bound binary. Missing observation produces failed/blocked outcome, never implicit pass. Collect DOM state as renderer evidence; it is not a compositor screenshot. Existing browser_automation screenshot draws text onto a canvas and cannot serve as visual proof.
- [ ] Keep actual suspend/resume pending unless separate machine-impact approval/environment exists. Candidate WebSocket comparison is not historical pre-change baseline: old latency/RAM baseline remains unmeasured unless reconstructed and matched independently. Do not present either limitation as resolved by this matrix.

### Task 6: Default-path regression, preservation and handoff

**Files:** Directly changed native/JS tests; docs/qa.md, parent spec/plan/evidence and existing SDD ledger. No default switch, commit, installer replacement or publication.

**Interfaces:** Consumes observed outcomes, current source/build manifests and earliest-before baseline-relative patch. Produces scoped report with PASS/FAIL/BLOCKED per acceptance requirement.

- [ ] Check ordinary-build rejection/caller tests under feature-off correctness profile and feature-on directly changed tests with exact filters. No zero-test receipts. Run `cargo check --offline --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml` without feature/profile overrides under owned deadline; this is normal compilation, not normal installer/runtime acceptance.
- [ ] Run exact Node bridge suite after its final edits, syntax/lint/format for changed JS, and app typecheck once after integration: `npm run typecheck --workspace=@getpaseo/app`. No unrelated UI/client test suite reruns. For future app source changes, stop for new ownership/tests instead of weakening routing or import gates.
- [ ] Verify non-owned tracked patch, latest verified previous-task hashes and captured before-copies. Compare all generated-output manifests, preserve old/new names and record justified changes. Original NSIS installers must remain byte-identical unless a separately recorded normal bundle is explicitly run; no raw test binary release link.
- [ ] Expand existing baseline-relative review package to cover isolation delta plus connected IPC code. Request fresh-context read-only review if available; otherwise explicitly report author-only review and leave independent review pending. Do not reuse Task 1's verdict as approval for this work.
- [ ] Integrate run commands/variant limits into docs/qa.md and link dated receipts from parent evidence/ledger. Parent Task 6 still requires installer installation/upgrade, real sleep/platform acceptance and missing functional cases. Earlier default bundle predates these normal-boot edits: rerun supported `npm run bundle:desktop` before any later current-source packaging claim, not as evidence that the old artifact contains the amendment.
- [ ] Stop with concrete report: implemented isolation seam; exact native/frontend results; observed vs configured bounds; owned cleanup; excluded integrations; original-disconnect reproduction still absent; missing independent review/platform/install/performance baseline. IPC remains opt-in. No Task 6 completion/default-rollout receipt while a parent gate remains open.

## Self-review and execution gate

Coverage: boot/path ownership → Task 1; complete namespace/shell/provider/MCP boundary → Task 2; supervisor/fixtures/build identity → Task 3; actual channel/caller/window/ACK boundary → Task 4; original functional/pressure/measurement matrix → Task 5; normal-build/preservation/docs/review → Task 6. Every Review Focus line has named owning tests. One immutable context supplies policy; driver/native reports share run/case/role identity; later tasks use the interfaces above.

Known boundaries remain explicit: isolated acceptance variant omits OS integrations; actual machine sleep and installer hooks are not exercised; current native approval/plugin support must not be invented; no historical latency/RAM baseline or original-disconnect reproduction exists. These are blockers for parent acceptance, not permission for scope expansion.

**Execution approved.** Use executing-plans inline on this dirty baseline, one verified task/batch at a time. New plan's ledger/artifacts live in `.superpowers/sdd/2026-11-18-desktop-ipc-acceptance-isolation/`; original IPC workspace remains a read-only baseline reference. No subagent launch, commit or production action is implied. If source makes these interfaces or isolation assumptions infeasible, stop for an amendment before GUI launch.
