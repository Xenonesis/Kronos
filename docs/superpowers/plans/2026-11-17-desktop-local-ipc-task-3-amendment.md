# Task 3 amendment — adapter parity and terminal slots

Status: approved by user's `okay continue`. Task 2 verified; Task 3 executing inline. Parent plan: [desktop local IPC](2026-11-17-desktop-local-ipc.md).

## Source findings

- `kernel/server.rs::serve_ipc` currently implements admitted newline-delimited text RPC only. It does not subscribe to watcher/CRDT/provider/PTY broadcasts or transport terminal binary frames. `handle_ws` does all of those after admission. A single unqualified full-session loop would silently change legacy pipe/socket behavior; separate business loops would defeat shared-session ownership.
- `dispatch_session_message` answers every `subscribe_terminal_request` with `payload.slot = 1`. Meanwhile `handle_ws` assigns `(subscribed_terminals.len() + 1) as u8`, reads only top-level terminalId, and uses that table for binary input/output. A second terminal can therefore be announced as slot 1 while being routed as slot 2. Removing/resubscribing entries can also reuse occupied slots. These are source findings, not an executed terminal failure or the original disconnect reproduction.
- Existing `test_terminal_wire_protocol_full_roundtrip` exercises dispatcher operations, not an authenticated multi-terminal WebSocket session, and does not cover this response/routing disagreement.

## Narrow proposed changes

1. Add an internal native adapter profile to `SessionIo` (full session versus legacy newline RPC), beyond its previously fixed incoming/outgoing fields. Shared core owns admission and request dispatch; full-session profile additionally owns existing subscriptions/binary terminal behavior. Existing pipe/socket adapter selects legacy profile; WebSocket and future Tauri IPC select full profile. No public hello/message/schema field or new feature protocol.
2. Make full-session terminal slot allocation authoritative for both existing subscribe response's slot field and binary routing. Preserve slot on repeated subscription, allocate distinct available nonzero u8 slots, remove only corresponding terminal on unsubscribe/kill, handle exhaustion explicitly. Accept both existing terminalId placements already supported by dispatcher. Do not create terminals or change provider logic during subscription.
3. Keep current auth/admission limits, newline framing and malformed-JSON error-and-continue behavior. Preserve distinctions: terminal/watcher/CRDT lag closes for resync; provider-push lag currently continues. No blanket conversion of all lag into close.
4. Use bounded shared frame queues under approved frame/byte limits. No dispatcher shortcut, second engine, credential change, new runtime/dependency, or default IPC switch.

## Evidence required

- Before the slot correction, an isolated real WebSocket fixture must demonstrate announced-slot/routing-table disagreement for two terminal subscriptions; no provider or production PTY process is needed merely to test assignment. Label source findings separately until runtime reproduction exists.
- Add focused regressions for duplicate subscription, removal/reuse/exhaustion and nested terminalId; include actual owned PTY/binary integration where safely available, otherwise leave that acceptance unverified.
- Run the plan's shared-session auth/correlation/order/cleanup/cancellation/lag fixtures through in-process channels and isolated real WebSocket. Add legacy-profile regression proving no unsolicited full-session pushes or binary framing is introduced into existing newline adapter.
- Keep all prior dirty-state, temporary-home, exact-process ownership, sequential-test and packaging/default-switch gates. Original intermittent disconnect remains unconfirmed.

This changes only the private extraction seam and corrects an existing response-value/routing disagreement. It does not authorize changes to `packages/protocol`, `packages/client`, remote authentication, provider behavior or installer/live-app state.
