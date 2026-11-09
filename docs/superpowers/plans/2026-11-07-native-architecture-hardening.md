# Native Desktop Architecture Hardening Implementation Plan

**Goal:** Retain Tauri v2 and the embedded Rust kernel while making local access, agent execution, history, persistence, installer builds and frontend loading safe and verifiable.

**Architecture:** Keep a single embedded daemon, the existing WebSocket wire protocol and file-backed agent records. Add explicit access and execution boundaries rather than a replacement runtime or microservices. Use existing client recovery and provider resume contracts.

**Tech stack:** Rust/Tokio/Axum/Tauri v2; Expo/React/Metro; npm; GitHub Actions/NSIS.

## Global constraints

- Do not restart or terminate the production daemon on port 6767; use temporary homes and ephemeral or dedicated test ports.
- Distribute NSIS installers only, never the raw desktop executable.
- Preserve cross-platform client behavior and backward-compatible protocol schemas. New wire fields remain optional.
- Do not invent credentials, signing keys or provider continuity. Release signing requires the repository owner's real key pair.
- No publication, tag push or shared-branch push is part of this implementation request.
- No full test suite and no concurrent builds/checks. Workers edit their owned files and report checks for the integration owner to run once afterward.
- No blanket component rewrite, speculative database migration or new service architecture.

## Design and acceptance criteria

### 1. Native access boundary (integration owner)

**Files:** `kernel/auth.rs`, `kernel/server.rs`, `daemon.rs`, `main.rs`, Tauri security config; existing security documentation.

- Reuse the existing local-credential and canonical hello auth shape. Require authentication before every socket command, binary frame and outbound subscription; fail closed and bound handshake time.
- Validate HTTP Host and browser Origin before upgrading. Do not treat a missing Origin as authentication. Only explicitly supported local desktop/development origins are accepted, with deliberate configured origins if needed.
- Protect HTTP browser-proxy operations as well; an attacker-controlled website must not reach a privileged bypass.
- Keep health/version probes non-sensitive; never terminate an unrelated process merely for occupying the daemon port.
- Replace null CSP with an explicit policy compatible with exported UI, Tauri IPC, permitted daemon connections and sandboxed diagrams.
- Verification: isolated real daemon rejects unauthenticated/malicious-origin traffic and accepts credentialed desktop/client traffic. Observe actual production frontend under the policy.

### 2. Agent execution ownership and provider continuity (runner worker)

**Files:** `agent/runner.rs`, `agent/runner_tests.rs`, `agent/manager.rs`, focused native process helper if required; `docs/agent-lifecycle.md`, `docs/providers.md`.

- Reserve the turn before acknowledging/persisting a new prompt. Reject overlapping turns for one agent without adding an orphan prompt; different agents remain independent.
- Cancel signals the owned turn, terminates the provider process tree, drains/reaps within bounds and preserves partial output. Keep ownership until cleanup ends; stale completion cannot overwrite cancellation or a newer turn.
- Apply cleanup to timeouts, errors and agent deletion too. Use Windows Job Objects rather than killing arbitrary PIDs.
- Persist observed provider session/thread identity before completion. Subsequent turns and explicitly imported handles use provider-specific resume arguments; never global last-session selection.
- Use isolated explicit session files for OMP/Pi when supported. If a provider cannot safely resume, return an actionable error rather than pretend continuation succeeded.
- Preserve configured model/thinking settings and existing streaming behavior.
- Interface: existing `send_message`, `cancel_agent`, `run_agent_turn` may be retained but their reservation/cleanup semantics must be explicit. Server callsite changes are supplied to the integration owner; worker does not edit `kernel/server.rs`.
- Verification: focused behavioral tests and a real local child-process smoke for overlap/cancel/descendant cleanup; session persistence and second-turn invocation on controlled provider harness plus installed CLI help validation.

### 3. Stream recovery and bounded history (integration owner)

**Files:** `kernel/server.rs`, focused timeline helper/tests if appropriate; `docs/timeline-sync.md`.

- A lagged broadcast receiver must cause explicit resynchronization/reconnect, not silently continue after dropping events. Reuse client recovery rather than invent a parallel protocol.
- Honor tail/before/after, requested bounded limit, epoch and stale cursors; return accurate cursors, window bounds, hasOlder/hasNewer and reset flags.
- Preserve turn boundaries when negotiated and avoid cloning all history just to return one page.
- Verification: deterministic pagination cases including empty history, multiple pages, cursor edges, stale epoch, and recovery after overflow; live socket smoke with canonical client parsing.

### 4. Storage commit semantics (storage worker)

**Files:** `agent/storage.rs`, `kernel/config_store.rs` atomic write helper only; `docs/data-model.md`.

- Publish memory mutations only after successful durable file commit, or rollback under the owning lock; failed writes must leave visible records/history unchanged.
- Do not silently interpret malformed/unreadable stores as empty writable stores. Missing files are valid empty stores; corruption must surface without overwriting original data.
- Replace full transcript rewrites on each append with a durable append-oriented representation while loading existing JSON histories safely. Avoid global timeline write locks during unrelated-agent disk I/O.
- Keep existing consumers working; add `pub fn with_timeline<T>(&self, agent_id: &str, read: impl FnOnce(&[AgentTimelineItem]) -> T) -> std::io::Result<T>` so history projection can read a bounded slice without cloning the entire transcript.
- Make atomic writes sync content before replace, use unique temporary paths and clean failures. Do not claim cross-file transactions or unsupported platform crash guarantees.
- Worker must not edit manager/server files; coordinate required API changes through the integration owner.
- Verification: disk-write failure rollback, malformed-store preservation, reload of legacy and appended history, delete/recreate cache correctness, and parallel independent-agent appends.

### 5. Reproducible installer and signed updater pipeline (release worker)

**Files:** root/desktop package scripts, `scripts/update-and-build-exe.mjs`, `.github/workflows/desktop-release.yml`, relevant existing build helpers; `docs/release.md`.

- Every official bundle command exports the current desktop frontend before packaging; no reliance on stale `packages/app/dist`.
- Default native bundle does not rebuild/package the optional Node daemon unnecessarily. Preserve an explicit supported Node bundle path.
- Never produce updater metadata with an empty signature or placeholder public key. Release workflow requires actual signing material, produces signed artifacts and validates metadata/artifacts before upload/publication.
- Local unsigned installer remains buildable without secrets but cannot masquerade as a signed auto-update release.
- Integrate existing smoke/version checks; keep local aliases regular identical copies of the installer.
- Worker owns updater configuration fields only if needed; parent owns CSP in the same JSON and must be notified before touching it. Do not overwrite config wholesale.
- Verification: focused existing script tests or throwaway pipeline checks; integration owner runs fresh frontend-to-NSIS bundle and reports signing prerequisites without exposing secrets.

### 6. Desktop frontend loading (frontend worker)

**Files:** implicated app imports/Metro config/runtime assets and existing performance docs only.

- Measure current exported main JS bytes and dependency contributors; optimize a demonstrated heavy eager dependency, not guessed runtime performance.
- Defer or externalize heavy optional renderers/assets (diagram runtime is an identified candidate) using existing Expo/Metro/Tauri-compatible conventions. Maintain loading/error behavior and native platform behavior.
- Keep sandbox isolation and diagram CSP; do not introduce unsafe eval or external network runtime downloads.
- Add a reproducible bundle measurement mechanism only if existing tooling cannot report the required before/after values.
- Verification: exported main/chunk sizes and actual browser render of ordinary startup plus deferred feature. Report observed size reduction, not an invented RAM/boot improvement.

## Execution and integration

1. Dispatch runner, storage, release and frontend slices together; each owns the files listed above and skips builds/checks/formatters until integration.
2. Integration owner implements access boundary and history/recovery and migrates server callsites to runner interfaces.
3. Review every worker result against its acceptance criteria; run focused native tests, JS tests where contracts changed, affected typechecks, npm-script lint/format and a single native build.
4. Launch only the new executable against a temporary home/port; exercise authenticated handshake, agent execution/cancellation, persistence/reload and paginated history. Stop only the process owned by this smoke.
5. Export and load the actual packaged frontend, exercise deferred feature and CSP, then build the NSIS installer through the corrected pipeline.
6. Update owning docs/changelog after runtime proof; record actual verification and any unreachable signing/publication prerequisite. Do not mark unexercised behavior complete.

## Design decisions

The user approved implementation of all improvements from the architecture assessment. Work occurs on `feat/native-architecture-hardening` in the existing clean checkout to reuse installed dependencies and native build artifacts; production state is separate. No additional UI redesign or provider authentication changes are included. A full native rewrite and microservices were rejected because they do not resolve the observed correctness/security failures and increase migration risk.
