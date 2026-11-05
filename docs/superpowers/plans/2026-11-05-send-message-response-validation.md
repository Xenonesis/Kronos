# Send-message Response Validation Implementation Plan

> **For agentic workers:** Execute inline using the executing-plans skill. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the invalid send-message acknowledgement without changing message execution or weakening client validation.

**Architecture:** Keep the existing protocol schema and client validator. Repair the native producer to match the established Node producer: successful acknowledgements contain `error: null`; rejected acknowledgements retain their error string. Preserve persistence, background dispatch, streams, and request correlation.

**Tech Stack:** Rust/Tokio/Axum, TypeScript, Zod/generated outbound validators, Tauri v2/NSIS.

**Spec:** User report: screenshot shows `Response validation failed for send_agent_message_response` despite rendered responses; find related causes and fix without losing functionality.

## Global Constraints

- Do not disable validation or make the required nullable `error` field optional.
- Do not restart or terminate the main daemon on port 6767.
- Do not change providers, catalogs, routing, persistence, or streaming in this repair.
- Only installable NSIS packages are distributed; do not publish portable executables.
- Run targeted tests only, not the full suite.

## Evidence

`packages/protocol/src/messages.ts:5093-5101` requires `payload.error: string | null`. `packages/desktop-tauri/src-tauri/src/kernel/server.rs:2501-2509` omits it on success; rejection includes it. `packages/server/src/server/session.ts:8372-8380` already emits `error: null`. `packages/client/src/daemon-client.ts:6926-6944` rejects the correlated request on malformed responses. Persistence and runner launch happen before the invalid acknowledgement, explaining the visible reply plus failed composer submission. `server_test.rs:1719-1721` checked acceptance only; `serde_json::Value` indexing also returns null for absent keys, so a null-index assertion alone cannot catch this regression.

## Task 1: Protect and repair the native acknowledgement

**Files:** Modify `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs` and `packages/desktop-tauri/src-tauri/src/kernel/server.rs`.

**Interfaces:** Existing `dispatch_session_message(&serde_json::Value)` and `send_agent_message_response` contract remain unchanged.

- [x] Extend the existing roundtrip test with `assert_eq!(msg_res["payload"].get("error"), Some(&serde_json::Value::Null));`, plus request/agent correlation assertions. This distinguishes absent from explicit null.
- [x] Add a rejection test using an unknown agent ID: assert `accepted: false`, matching request and agent IDs, and a real nonempty error string; assert no timeline entry was created.
- [x] Run `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_agent_wire_protocol_full_roundtrip` and observe failure for the absent error field.
- [x] Add `"error": null` after `"accepted": true` in the native success response only. Leave dispatch and rejection logic intact.
- [x] Run the roundtrip and rejection tests and Rust compilation/lints. Run npm typecheck and lint. Both targeted tests and workspace typecheck passed. Global lint reported 23 unrelated errors; Clippy reported an existing raw-pointer safety error at `src/process_job.rs:52` and 60 warnings.

## Task 2: Verify the built runtime and bundle

**Files:** Update the regression guidance in `docs/protocol-validation.md`; create no permanent smoke scaffolding.

- [x] Build the existing Tauri NSIS bundle via `npm --prefix packages/desktop-tauri run build`. This runs the repository prepare-dist lifecycle.
- [x] Launch the built executable in daemon CLI mode on a separate loopback port. Native `dirs::home_dir()` uses the Windows profile rather than the supplied temporary HOME, so use uniquely created smoke agents and delete only those through the API; do not claim storage isolation. Port 6767 was not restarted.
- [x] Use the actual generated outbound validator and Zod response schema against raw success and rejection frames. `DaemonClient.sendAgentMessage()` resolved accepted submissions and rejected unknown-agent submissions with a service error, not `DaemonProtocolError`.
- [x] Exercise real OpenCode initial turns and follow-ups. Both the CLI-selected default model and the screenshot's live-catalog `opencode/crouton-alpha-free` returned `KRONOS_MESSAGE_ONE` and `KRONOS_MESSAGE_TWO` as live streams; fetched timelines contained the exact two user messages and two assistant replies.
- [x] Stop only the owned smoke daemons and remove temporary smoke files/state. Successful and timed-out smoke agents were deleted through the API.
- [x] Document the cross-language required-nullable acknowledgement pitfall in the existing validation documentation. Installer copied to `dist/Kronos_0.15.0-beta.1_x64-setup.exe` and `dist/Kronos_x64-setup.exe`; SHA-256 `05426399AF48533FCBD2563C4E80EF41BA094B78595C6E220CDED2294738D9D2`, 15,793,542 bytes.

## Verification limits and deployment safety

- Runtime/API evidence uses the built release executable, the actual SDK, Zod, and the generated outbound validator. No new GUI screenshot was captured.
- The first provider smoke launch inherited an open stdin pipe, causing OpenCode to wait for EOF. Launching the owned daemon with stdin set to NUL (`stdio: ["ignore", "inherit", "inherit"]`) resolved the harness stall; both model runs then passed without changing provider production code.
- No automatic install was performed. The existing NSIS uninstall hook recursively deletes `$PROFILE\\.paseo`, and the preinstall hook broadly terminates matching processes. These pre-existing packaging behaviors are outside this acknowledgement repair; do not use an unattended upgrade without protecting existing state. No registered Kronos/Paseo installation was found during this run.
