# SDD ledger — plan: docs/superpowers/plans/2026-10-29-kronos-native-engine.md

## Pre-flight Conflict Scan

| Tasks           | Shared Interface / File                         | Check                                                                 | Finding / Ruling                                           |
| --------------- | ----------------------------------------------- | --------------------------------------------------------------------- | ---------------------------------------------------------- |
| Task 1 ↔ Task 3 | Win32 Job Object handle                         | Task 3 PTY child processes must assign to Task 1 JobSupervisor        | Clean: Task 1 produces `JobSupervisor`, Task 3 consumes it |
| Task 2 ↔ Task 4 | Diff structs (`FileDiffResult`)                 | Task 4 registers `compute_file_diff` command returning Task 2 structs | Clean: exact types match                                   |
| Task 3 ↔ Task 4 | PTY commands & events                           | Task 4 registers `spawn_native_terminal`, `write_native_terminal`     | Clean: exact types and Channel semantics match             |
| Task 4 ↔ Task 5 | `window.paseoDesktop.nativeTerminal/nativeDiff` | Task 5 React consumes Preload Bridge exposed in Task 4                | Clean: exact signatures match with `ts-no-any`             |
| Task 5 ↔ Task 6 | Build & runtime verification                    | Task 6 verifies build without regressions                             | Clean                                                      |

Scan status: Clean. Zero conflicts found.

## Task Log

- Task 1: complete (commits 6d78f6d..faaea33, review clean)
- Task 2: fix round 1/5 (1 addressed, 0 open — compute diff hunk start offsets from DiffOp range bounds; commits cde0884..4e6c0f8)
- Task 2: complete (commits faaea33..4e6c0f8, review clean)
- Task 3: minor (deferred): map raw_os_error 5 (EIO) to Exit(None) on POSIX in native_pty.rs reader thread
- Task 3: minor (deferred): kill spawned session if attach_reader_channel fails in spawn_native_terminal
- Task 3: complete (commits 4e6c0f8..2ab3e3d, review clean)
- Task 4: complete (commits 2ab3e3d..a20e8e5, review clean)
- Task 5: minor (deferred): route toolbar paste button to nativeTerminal.write if touch toolbar is triggered on desktop
- Task 5: complete (commits a20e8e5..754eede, review clean)
- Task 6: complete (installer built & verified, zero type errors, 22/22 cargo tests pass)
