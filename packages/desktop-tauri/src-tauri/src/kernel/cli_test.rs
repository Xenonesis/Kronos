#[cfg(test)]
mod tests {
    use crate::kernel::cli::{parse_and_dispatch_from, CliArgs, CliCommand};
    use clap::Parser;

    #[test]
    fn test_cli_dispatch_gui_fallback() {
        // When no subcommand is specified, CLI dispatch should yield None,
        // allowing main.rs to proceed into the graphical windowing runtime.
        let args = ["paseo"];
        let result = parse_and_dispatch_from(args);
        assert_eq!(result, None);

        let parsed = CliArgs::try_parse_from(["paseo"]).expect("Failed to parse base args");
        assert_eq!(parsed.command, None);
    }

    #[test]
    fn test_cli_dispatch_deep_link_and_os_flags_fallback() {
        // Protocol deep links should fall back to GUI mode (None)
        assert_eq!(parse_and_dispatch_from(["paseo", "kronos://auth/callback?code=xyz"]), None);
        assert_eq!(parse_and_dispatch_from(["paseo", "paseo://workspace/open"]), None);

        // Squirrel / OS / Windows shortcut flags should fall back to GUI mode (None)
        assert_eq!(parse_and_dispatch_from(["paseo", "--allow-downgrade"]), None);
        assert_eq!(parse_and_dispatch_from(["paseo", "/prefetch:1"]), None);
        assert_eq!(parse_and_dispatch_from(["paseo", "--squirrel-firstrun"]), None);
        assert_eq!(parse_and_dispatch_from(["paseo", "-psn_0_12345"]), None);
        assert_eq!(parse_and_dispatch_from(["paseo", "--type=gpu-process"]), None);

        // Truly invalid CLI commands/options should return Some(1)
        assert_eq!(parse_and_dispatch_from(["paseo", "nonexistent-command-xyz"]), Some(1));
        assert_eq!(parse_and_dispatch_from(["paseo", "--unsupported-flag-xyz"]), Some(1));
    }

    #[test]
    fn test_cli_dispatch_daemon_mode() {
        // Use an ephemeral test port to prevent collisions with default 6767
        let test_port = 16788;
        let pipe_name = format!(r"\\.\pipe\kronos-ipc-test-{}", test_port);
        let args = [
            "paseo",
            "daemon",
            "--port",
            &test_port.to_string(),
            "--pipe",
            &pipe_name,
        ];

        let result = parse_and_dispatch_from(args);
        assert_eq!(result, Some(0));

        let parsed = CliArgs::try_parse_from([
            "paseo",
            "daemon",
            "-p",
            "7070",
        ]).expect("Failed to parse daemon args");

        assert_eq!(
            parsed.command,
            Some(CliCommand::Daemon {
                port: 7070,
                pipe: None,
            })
        );
    }

    #[test]
    fn test_cli_dispatch_agent_mode() {
        let args = [
            "paseo",
            "agent",
            "Refactor AST engine for SIMD acceleration",
            "--provider",
            "anthropic",
        ];

        let result = parse_and_dispatch_from(args);
        assert_eq!(result, Some(0));

        let parsed = CliArgs::try_parse_from([
            "paseo",
            "agent",
            "Optimize Tantivy lexical queries",
            "-p",
            "openai",
        ]).expect("Failed to parse agent args");

        assert_eq!(
            parsed.command,
            Some(CliCommand::Agent {
                prompt: "Optimize Tantivy lexical queries".to_string(),
                provider: Some("openai".to_string()),
            })
        );
    }

    #[test]
    fn test_cli_dispatch_search_mode() {
        let args = [
            "paseo",
            "search",
            "WorkspaceSearchIndex",
        ];

        let result = parse_and_dispatch_from(args);
        assert_eq!(result, Some(0));

        let matches = crate::kernel::cli::execute_search_in("WorkspaceSearchIndex", std::path::Path::new("."))
            .expect("Search should succeed");
        assert!(!matches.is_empty(), "Expected to find matches for WorkspaceSearchIndex");
        let matched_search_rs = matches.iter().any(|m| m.path.contains("search.rs"));
        assert!(matched_search_rs, "Expected search results to include nested file src/kernel/search.rs, found: {:?}", matches);

        let parsed = CliArgs::try_parse_from([
            "paseo",
            "search",
            "tantivy",
        ]).expect("Failed to parse search args");

        assert_eq!(
            parsed.command,
            Some(CliCommand::Search {
                query: "tantivy".to_string(),
            })
        );
    }

    #[test]
    fn test_cli_dispatch_mcp_mode() {
        let args = [
            "paseo",
            "mcp",
        ];

        let result = parse_and_dispatch_from(args);
        assert_eq!(result, Some(0));

        let parsed = CliArgs::try_parse_from([
            "paseo",
            "mcp",
        ]).expect("Failed to parse mcp args");

        assert_eq!(parsed.command, Some(CliCommand::Mcp));
    }

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

    #[test]
    fn test_cli_mcp_stdio_notifications_only_produce_no_output() {
        use std::io::Cursor;
        use crate::kernel::cli::run_mcp_stdio_loop;
        use crate::kernel::mcp::McpHub;

        let hub = McpHub::new();
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\",\"params\":{}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"$/cancelRequest\",\"params\":{\"requestId\":1}}\n",
            "quit\n"
        );

        let reader = Cursor::new(input.as_bytes());
        let mut writer = Vec::new();

        let exit_code = run_mcp_stdio_loop(reader, &mut writer, &hub);
        assert_eq!(exit_code, 0);

        let output = String::from_utf8(writer).expect("Valid UTF-8 output");
        assert!(output.is_empty(), "Notifications must not produce any output on stdout");
    }

    #[test]
    fn test_cli_mcp_stdio_blank_lines_and_quit() {
        use std::io::Cursor;
        use crate::kernel::cli::run_mcp_stdio_loop;
        use crate::kernel::mcp::McpHub;

        let hub = McpHub::new();
        let input = "   \n\n\t  \nquit\n";

        let reader = Cursor::new(input.as_bytes());
        let mut writer = Vec::new();

        let exit_code = run_mcp_stdio_loop(reader, &mut writer, &hub);
        assert_eq!(exit_code, 0);
        assert!(writer.is_empty());
    }

    #[test]
    fn test_cli_mcp_stdio_writer_error_returns_nonzero() {
        use std::io::{Cursor, Error, ErrorKind, Write};
        use crate::kernel::cli::run_mcp_stdio_loop;
        use crate::kernel::mcp::McpHub;

        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(Error::new(ErrorKind::BrokenPipe, "simulated broken pipe"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(Error::new(ErrorKind::BrokenPipe, "simulated broken pipe"))
            }
        }

        let hub = McpHub::new();
        let input = "{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"method\":\"initialize\"}\n";
        let reader = Cursor::new(input.as_bytes());
        let writer = FailingWriter;

        let exit_code = run_mcp_stdio_loop(reader, writer, &hub);
        assert_eq!(exit_code, 1);
    }
}
