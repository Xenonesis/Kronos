# Track 9 & 10: Multi-Call Universal CLI Dispatch & Native MCP Hub Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `kronos.exe` into a high-performance multi-call universal binary and compliant native Model Context Protocol (MCP) server that external AI clients (Claude Desktop, Cursor, Zed, Windsurf) and terminal users can execute without launching the graphical WebView2 interface.

**Architecture:** Embed a pure stdio JSON-RPC 2.0 loop in `kernel::cli` wired directly to `McpHub` in `kernel::mcp`. Standard output (`stdout`) is strictly reserved for JSON-RPC 2.0 frames with all diagnostic logs isolated to `stderr`. Wire the Tantivy lexical index and multi-tiered directory suggestion engine (`kernel::search`) into `kronos search`, and connect `kronos agent` to the local Tokio daemon or provider CLI.

**Tech Stack:** Rust 2021, Tokio, `clap` v4 (derive), `serde_json`, `tantivy` v0.22, `tree-sitter` v0.24, `gix` v0.66, Model Context Protocol (JSON-RPC 2.0).

**Spec:** `docs/superpowers/specs/2026-10-29-kronos-hyper-native-kernel-design.md` (§4.6 Multi-Call Universal Binary Architecture, §4.9 Native Embedded MCP Hub).

## Global Constraints

- **Single Binary Integrity:** `kronos.exe` must cleanly distinguish GUI execution from CLI subcommands (`daemon`, `agent`, `search`, `mcp`) before initializing windowing or WebView2.
- **MCP Stdio Purity:** In `mcp` mode, `stdout` MUST NEVER contain plain text logs, debug messages, or banners. Only valid single-line JSON-RPC 2.0 messages are permitted on `stdout`. Diagnostic logging is strictly restricted to `stderr`.
- **JSON-RPC 2.0 Protocol Compliance:** Notifications (requests without an `id`, e.g. `notifications/initialized`, `notifications/cancelled`) must be acknowledged silently without emitting error responses.
- **Sub-Millisecond Search Execution:** Lexical search must leverage in-memory Tantivy indexing and directory suggestion ranking with zero disk thrashing.
- **Zero Fabrication:** All performance claims, benchmark figures, and test assertions must reference real code and pass on Windows x64.
- **Type Safety (`ts-no-any`):** No `: any` or `as any` in any TypeScript bridge contracts or tests.

---

### File Structure Map

| File                                                         | Purpose                                                                               | Action     |
| :----------------------------------------------------------- | :------------------------------------------------------------------------------------ | :--------- |
| `packages/desktop-tauri/src-tauri/src/kernel/mcp.rs`         | Native Model Context Protocol (MCP) tool registry and JSON-RPC router                 | **Modify** |
| `packages/desktop-tauri/src-tauri/src/kernel/mcp_test.rs`    | Comprehensive unit tests for MCP methods, notifications, and tool dispatch            | **Modify** |
| `packages/desktop-tauri/src-tauri/src/kernel/cli.rs`         | Multi-call CLI argument parser, stdio MCP loop, search runner, and agent dispatcher   | **Modify** |
| `packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs`    | Integration tests for all CLI subcommands (`mcp`, `search`, `agent`, `daemon`, flags) | **Modify** |
| `packages/desktop-tauri/src-tauri/src/kernel/server.rs`      | In-process daemon wire protocol dispatch (adding `mcp/dispatch` envelope routing)     | **Modify** |
| `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs` | Verification of MCP wire protocol dispatch over WebSocket/IPC envelopes               | **Modify** |

---

### Task 1: Spec-Compliant MCP Protocol Engine & Notification Handling in `kernel::mcp`

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/mcp.rs:195-522`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/mcp_test.rs:1-80`

**Interfaces:**

- Consumes: `kernel::search::suggest_directory_entries`, `kernel::git::GitEngine`, `kernel::ast::AstEngine`.
- Produces: `McpHub::handle_jsonrpc(&self, request_json: &str) -> Option<String>`, `McpHub::register_tool`, `McpHub::list_tools`.

- [ ] **Step 1: Write failing unit tests for notification silencing and new built-in tools**

Add tests to `packages/desktop-tauri/src-tauri/src/kernel/mcp_test.rs`:

```rust
#[test]
fn test_native_mcp_notification_silencing() {
    let hub = McpHub::new();

    // Notifications (no 'id' field) MUST NOT yield a response
    let init_notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }).to_string();
    assert_eq!(hub.handle_jsonrpc(&init_notification), None);

    let cancel_notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": { "requestId": "req-1" }
    }).to_string();
    assert_eq!(hub.handle_jsonrpc(&cancel_notification), None);
}

#[test]
fn test_native_mcp_suggest_directory_tool() {
    let hub = McpHub::new();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "req-dir-1",
        "method": "tools/call",
        "params": {
            "name": "suggest_directory",
            "arguments": {
                "cwd": ".",
                "query": "src",
                "limit": 10
            }
        }
    }).to_string();

    let res_str = hub.handle_jsonrpc(&req).expect("Response expected for request with id");
    let res: serde_json::Value = serde_json::from_str(&res_str).unwrap();
    assert_eq!(res["id"], "req-dir-1");
    assert_eq!(res["result"]["isError"], false);
}

#[test]
fn test_native_mcp_terminal_exec_tool() {
    let hub = McpHub::new();
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "req-exec-1",
        "method": "tools/call",
        "params": {
            "name": "terminal_exec",
            "arguments": {
                "command": "cmd.exe",
                "args": ["/C", "echo KronosKernelReady"]
            }
        }
    }).to_string();

    let res_str = hub.handle_jsonrpc(&req).expect("Response expected for request with id");
    let res: serde_json::Value = serde_json::from_str(&res_str).unwrap();
    assert_eq!(res["result"]["isError"], false);
    let output = res["result"]["text"].as_str().unwrap();
    assert!(output.contains("KronosKernelReady"));
}
```

- [ ] **Step 2: Run cargo test to verify tests fail**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_native_mcp_notification_silencing`
Expected: FAIL (`Option<String>` vs `String` mismatch or missing tools).

- [ ] **Step 3: Update `McpHub::handle_jsonrpc` signature and implement notification silencing & new tools**

In `packages/desktop-tauri/src-tauri/src/kernel/mcp.rs`:

1. Change `handle_jsonrpc(&self, request_json: &str) -> Option<String>`.
2. Inspect if `id` is null or missing:
   ```rust
   let has_id = obj.contains_key("id") && !obj["id"].is_null();
   let id = obj.get("id").cloned().unwrap_or(Value::Null);
   ```
3. If `!has_id`, check if `method.starts_with("notifications/")`: return `None`.
4. Wrap all successful `json_rpc_result` and `json_rpc_error` returns in `Some(...)`.
5. Update `initialize` to report `env!("CARGO_PKG_VERSION")`.
6. Register `suggest_directory` tool wrapping `crate::kernel::search::suggest_directory_entries`.
7. Register `terminal_exec` tool executing child processes with a 15-second safety timeout.

- [ ] **Step 4: Run cargo test to verify MCP tests pass**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml mcp_test`
Expected: PASS (all tests passing).

- [ ] **Step 5: Commit MCP engine enhancement**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/mcp.rs packages/desktop-tauri/src-tauri/src/kernel/mcp_test.rs
git commit -m "feat(kernel): implement spec-compliant mcp protocol engine with notification silencing and directory tools"
```

---

### Task 2: Pure-Stdio JSON-RPC MCP Server Loop in `kernel::cli`

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/cli.rs:236-262`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs:130-147`

**Interfaces:**

- Consumes: `kernel::mcp::McpHub`.
- Produces: `run_mcp_stdio_loop<R: std::io::BufRead, W: std::io::Write>(reader: R, mut writer: W, hub: &McpHub) -> i32`.

- [ ] **Step 1: Write failing unit test for stdio MCP loop**

In `packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs`:

```rust
#[test]
fn test_cli_mcp_stdio_stream_roundtrip() {
    use std::io::Cursor;
    use crate::kernel::cli::run_mcp_stdio_loop;
    use crate::kernel::mcp::McpHub;

    let hub = McpHub::new();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"method\":\"initialize\"}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"2\",\"method\":\"tools/list\"}\n",
        "exit\n"
    );

    let reader = Cursor::new(input.as_bytes());
    let mut writer = Vec::new();

    let exit_code = run_mcp_stdio_loop(reader, &mut writer, &hub);
    assert_eq!(exit_code, 0);

    let output = String::from_utf8(writer).expect("Valid UTF-8 output");
    let lines: Vec<&str> = output.lines().collect();

    // Exactly 2 responses expected (initialize, tools/list).
    // The notification MUST NOT generate a response line.
    assert_eq!(lines.len(), 2, "Expected exactly 2 JSON-RPC lines, got: {:?}", lines);

    let init_res: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(init_res["id"], "1");
    assert_eq!(init_res["result"]["serverInfo"]["name"], "kronos-mcp-hub");

    let tools_res: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(tools_res["id"], "2");
    assert!(tools_res["result"]["tools"].as_array().unwrap().len() >= 5);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_cli_mcp_stdio_stream_roundtrip`
Expected: FAIL (`run_mcp_stdio_loop` does not exist).

- [ ] **Step 3: Implement `run_mcp_stdio_loop` and wire `CliCommand::Mcp`**

In `packages/desktop-tauri/src-tauri/src/kernel/cli.rs`:

```rust
pub fn run_mcp_stdio_loop<R: std::io::BufRead, W: std::io::Write>(
    reader: R,
    mut writer: W,
    hub: &crate::kernel::mcp::McpHub,
) -> i32 {
    for line in reader.lines() {
        let l = match line {
            Ok(val) => val,
            Err(_) => break,
        };
        let trimmed = l.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "exit" || trimmed == "quit" {
            break;
        }

        if let Some(response_json) = hub.handle_jsonrpc(trimmed) {
            if let Err(e) = writeln!(writer, "{}", response_json) {
                eprintln!("[MCP] Failed to write response to stdout: {e}");
                return 1;
            }
            let _ = writer.flush();
        }
    }
    0
}
```

In `dispatch(args: CliArgs)`:

```rust
CliCommand::Mcp => {
    eprintln!("[Kernel CLI] Starting native Model Context Protocol (MCP) stdio server...");
    let hub = crate::kernel::mcp::McpHub::new();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let code = run_mcp_stdio_loop(stdin.lock(), stdout.lock(), &hub);
    Some(code)
}
```

- [ ] **Step 4: Run cargo test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml cli_test`
Expected: PASS.

- [ ] **Step 5: Commit MCP stdio integration**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/cli.rs packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs
git commit -m "feat(cli): wire compliant mcp stdio loop with pure stdout streaming and silent notifications"
```

---

### Task 3: High-Performance CLI Search & Directory Suggestion Output in `kernel::cli`

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/cli.rs:25-31, 150-170`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs:100-130`

**Interfaces:**

- Consumes: `kernel::search::suggest_directory_entries`, `kernel::search::WorkspaceSearchIndex`.
- Produces: `CliCommand::Search { query, json, limit }`, `execute_search(query: &str, as_json: bool, limit: usize) -> Option<i32>`.

- [ ] **Step 1: Write failing test for `--json` output and directory suggestions fallback**

In `packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs`:

```rust
#[test]
fn test_cli_search_json_flag_and_limit() {
    let args = [
        "paseo",
        "search",
        "WorkspaceSearchIndex",
        "--json",
        "--limit",
        "5",
    ];

    let parsed = CliArgs::try_parse_from(args).expect("Failed to parse search args with --json");
    assert_eq!(
        parsed.command,
        Some(CliCommand::Search {
            query: "WorkspaceSearchIndex".to_string(),
            json: true,
            limit: 5,
        })
    );
}

#[test]
fn test_cli_search_directory_fallback_for_paths() {
    // Queries containing path separators should return directory and file suggestions
    let (dirs, entries) = crate::kernel::search::suggest_directory_entries(
        std::path::Path::new("."),
        "kernel",
        true,
        true,
        10,
    );
    assert!(!dirs.is_empty() || !entries.is_empty(), "Expected directory suggestions for 'kernel'");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_cli_search_json_flag_and_limit`
Expected: FAIL (missing `json` and `limit` fields on `CliCommand::Search`).

- [ ] **Step 3: Implement `--json` and `--limit` flags on `CliCommand::Search` and format output**

In `packages/desktop-tauri/src-tauri/src/kernel/cli.rs`:

```rust
    /// Lexical and path search across workspace
    Search {
        query: String,
        #[arg(long, default_value_t = false)]
        json: bool,
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
```

Implement structured JSON output if `json == true`, and clean ANSI tabular output if `json == false`.
If lexical search yields 0 matches and query contains path components, automatically fall back to `suggest_directory_entries` to display file path matches.

- [ ] **Step 4: Run cargo test to verify search tests pass**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_cli_dispatch_search_mode test_cli_search_json_flag_and_limit`
Expected: PASS.

- [ ] **Step 5: Commit CLI search enhancements**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/cli.rs packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs
git commit -m "feat(cli): add structured json output and directory suggestion fallback to workspace search"
```

---

### Task 4: Autonomous CLI Agent Dispatch & Daemon Session Bridge in `kernel::cli`

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/cli.rs:20-25, 225-235`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs:70-98`

**Interfaces:**

- Consumes: Tokio HTTP / WebSocket client, local daemon port.
- Produces: `dispatch_agent_prompt(prompt: &str, provider: Option<&str>, port: u16) -> i32`.

- [ ] **Step 1: Write failing test for agent dispatch options**

In `packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs`:

```rust
#[test]
fn test_cli_agent_options_parsing() {
    let args = [
        "paseo",
        "agent",
        "Refactor ast.rs",
        "--provider",
        "anthropic",
        "--port",
        "6767",
    ];

    let parsed = CliArgs::try_parse_from(args).expect("Failed to parse agent args");
    assert_eq!(
        parsed.command,
        Some(CliCommand::Agent {
            prompt: "Refactor ast.rs".to_string(),
            provider: Some("anthropic".to_string()),
            port: 6767,
        })
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_cli_agent_options_parsing`
Expected: FAIL (`port` field missing on `CliCommand::Agent`).

- [ ] **Step 3: Implement `port` option and local daemon detection in `dispatch_agent_prompt`**

In `packages/desktop-tauri/src-tauri/src/kernel/cli.rs`:

```rust
    /// Dispatch an agent task prompt
    Agent {
        prompt: String,
        #[arg(short, long)]
        provider: Option<String>,
        #[arg(long, default_value_t = 6767)]
        port: u16,
    },
```

In `dispatch_agent_prompt`:

1. Check if the local daemon is responding at `http://127.0.0.1:{port}/api/health`.
2. If live, announce connection and submit the agent session prompt.
3. If not live, display clear instructions on running `kronos daemon` or fallback to direct provider CLI execution.

- [ ] **Step 4: Run cargo test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_cli_dispatch_agent_mode test_cli_agent_options_parsing`
Expected: PASS.

- [ ] **Step 5: Commit agent CLI dispatch**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/cli.rs packages/desktop-tauri/src-tauri/src/kernel/cli_test.rs
git commit -m "feat(cli): enhance agent prompt dispatch with daemon health probing and port configuration"
```

---

### Task 5: Wire MCP Wire Protocol Envelope in In-Process Daemon (`kernel::server`)

**Files:**

- Modify: `packages/desktop-tauri/src-tauri/src/kernel/server.rs:50-85, 120-185`
- Test: `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs`

**Interfaces:**

- Consumes: `kernel::mcp::McpHub`.
- Produces: WebSocket & Named Pipe envelope routing for `mcp/request` and `mcp/jsonrpc`.

- [ ] **Step 1: Write failing test for MCP daemon wire envelope**

In `packages/desktop-tauri/src-tauri/src/kernel/server_test.rs`:

```rust
#[tokio::test]
async fn test_embedded_daemon_mcp_envelope_dispatch() {
    let envelope = RpcEnvelope {
        id: "mcp-env-1".to_string(),
        msg_type: "mcp/request".to_string(),
        payload: serde_json::json!({
            "request": serde_json::json!({
                "jsonrpc": "2.0",
                "id": "inner-1",
                "method": "tools/list"
            }).to_string()
        }),
    };

    let response = dispatch_envelope(envelope).await;
    assert!(response.success);
    assert_eq!(response.msg_type, "mcp/response");
    let data = response.data.expect("Response data expected");
    let inner_json: &str = data["response"].as_str().expect("inner response string");
    let inner_val: serde_json::Value = serde_json::from_str(inner_json).unwrap();
    assert_eq!(inner_val["id"], "inner-1");
    assert!(inner_val["result"]["tools"].as_array().unwrap().len() >= 5);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_embedded_daemon_mcp_envelope_dispatch`
Expected: FAIL (`Method 'mcp/request' not implemented`).

- [ ] **Step 3: Implement `mcp/request` in `dispatch_envelope` and `dispatch_session_message`**

In `packages/desktop-tauri/src-tauri/src/kernel/server.rs`:
Lazily initialize a global static `McpHub` (via `std::sync::OnceLock<McpHub>`) and route `"mcp/request"` and `"mcp/jsonrpc"` through `mcp_hub.handle_jsonrpc(req_str)`.

- [ ] **Step 4: Run cargo test to verify it passes**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml test_embedded_daemon_mcp_envelope_dispatch`
Expected: PASS.

- [ ] **Step 5: Commit MCP daemon envelope integration**

```bash
git add packages/desktop-tauri/src-tauri/src/kernel/server.rs packages/desktop-tauri/src-tauri/src/kernel/server_test.rs
git commit -m "feat(server): wire native mcp hub dispatch into in-process daemon wire protocol"
```

---

### Task 6: Full Suite Verification & Multi-Call CLI Verification

**Files:**

- Test: All Rust test suites across `packages/desktop-tauri/src-tauri`
- Test: Monorepo typecheck (`npm run typecheck`)

- [ ] **Step 1: Run complete Cargo test suite**

Run: `cargo test --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml`
Expected: 105+ tests passing, 0 failures.

- [ ] **Step 2: Run monorepo typecheck**

Run: `npm run typecheck`
Expected: 0 errors across all TypeScript packages.

- [ ] **Step 3: Smoke test CLI subcommands**

Run:

```bash
cargo run --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- --help
cargo run --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- search "Tantivy" --limit 5
cargo run --manifest-path packages/desktop-tauri/src-tauri/Cargo.toml -- search "Tantivy" --json --limit 3
```

Expected: Clean help text and search output without GUI launch.

- [ ] **Step 4: Final verification commit**

```bash
git commit --allow-empty -m "chore(release): complete Track 9 & 10 multi-call universal cli and native mcp hub verification"
```
