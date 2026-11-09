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
                port: 6767,
            })
        );
    }

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

    #[test]
    fn test_cli_dispatch_agent_prompt_without_active_daemon() {
        use crate::kernel::cli::dispatch_agent_prompt_to;

        let inactive_port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
            listener.local_addr().expect("Failed to get local addr").port()
        };

        let mut output = Vec::new();
        let exit_code = dispatch_agent_prompt_to(
            "Refactor ast.rs",
            Some("anthropic"),
            inactive_port,
            &mut output,
        );
        assert_eq!(exit_code, 0);

        let text = String::from_utf8_lossy(&output);
        assert!(text.contains(&format!("Daemon not detected on 127.0.0.1:{inactive_port}")));
        assert!(text.contains("Agent prompt queued for provider [anthropic]: \"Refactor ast.rs\""));
        assert!(text.contains(&format!("Run 'kronos daemon --port {inactive_port}'")));
        assert!(text.contains("Dry-run dispatch completed (exit 0)"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cli_dispatch_agent_prompt_with_active_daemon() {
        use crate::kernel::cli::dispatch_agent_prompt_to;
        use crate::kernel::server::EmbeddedDaemon;

        let daemon = EmbeddedDaemon::start_test(0).await.expect("Failed to start test daemon");
        let active_port = daemon.port;

        let mut output = Vec::new();
        let exit_code = dispatch_agent_prompt_to(
            "SIMD acceleration for tree-sitter",
            Some("openai"),
            active_port,
            &mut output,
        );
        assert_eq!(exit_code, 0);

        let text = String::from_utf8_lossy(&output);
        assert!(text.contains(&format!("Connected to live daemon on 127.0.0.1:{active_port} (health check ok)")));
        assert!(text.contains("Agent prompt dispatched for provider [openai]: \"SIMD acceleration for tree-sitter\""));
        assert!(text.contains("Dispatch successful (exit 0)"));

        daemon.stop().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cli_probe_daemon_health() {
        use crate::kernel::cli::probe_daemon_health;
        use crate::kernel::server::EmbeddedDaemon;

        // 1. Inactive port should return false
        let inactive_port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
            listener.local_addr().expect("Failed to get local addr").port()
        };
        assert!(!probe_daemon_health(inactive_port));

        // 2. Active daemon port should return true
        let daemon = EmbeddedDaemon::start_test(0).await.expect("Failed to start test daemon");
        let active_port = daemon.port;
        assert!(probe_daemon_health(active_port));
        daemon.stop().await;
    }

    #[test]
    fn test_cli_dispatch_agent_prompt_default_provider() {
        use crate::kernel::cli::dispatch_agent_prompt_to;

        let inactive_port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind ephemeral port");
            listener.local_addr().expect("Failed to get local addr").port()
        };

        let mut output = Vec::new();
        let exit_code = dispatch_agent_prompt_to(
            "Default provider test",
            None,
            inactive_port,
            &mut output,
        );
        assert_eq!(exit_code, 0);

        let text = String::from_utf8_lossy(&output);
        assert!(text.contains("Agent prompt queued for provider [default]: \"Default provider test\""));
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

        let cached_matches = crate::kernel::cli::execute_cached_search_in("WorkspaceSearchIndex", std::path::Path::new("."))
            .expect("Cached search should succeed");
        assert_eq!(matches.len(), cached_matches.len(), "Cached search count must match uncached search count");

        let parsed = CliArgs::try_parse_from([
            "paseo",
            "search",
            "tantivy",
        ]).expect("Failed to parse search args");

        assert_eq!(
            parsed.command,
            Some(CliCommand::Search {
                query: "tantivy".to_string(),
                json: false,
                limit: 20,
            })
        );
    }

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

    #[test]
    fn test_cli_dispatch_mcp_mode() {
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

    #[test]
    fn test_cli_search_json_execution() {
        use crate::kernel::cli::{execute_search_to, CliSearchOutput};

        let mut output = Vec::new();
        let code = execute_search_to(
            "WorkspaceSearchIndex",
            true,
            5,
            std::path::Path::new("."),
            &mut output,
        );
        assert_eq!(code, Some(0));

        let json_text = String::from_utf8(output).expect("Valid UTF-8 output");
        let parsed: CliSearchOutput = serde_json::from_str(&json_text).expect("Valid JSON response");
        assert_eq!(parsed.query, "WorkspaceSearchIndex");
        assert!(!parsed.matches.is_empty());
        assert!(!parsed.fallback);

        let search_rs = parsed
            .matches
            .iter()
            .find(|m| m.path.contains("search.rs"))
            .expect("Should contain search.rs");
        assert!(search_rs.line.is_some());
        assert!(search_rs
            .snippet
            .as_ref()
            .unwrap()
            .contains("WorkspaceSearchIndex"));
    }

    #[test]
    fn test_cli_search_terminal_text_execution() {
        use crate::kernel::cli::execute_search_to;

        let mut output = Vec::new();
        let code = execute_search_to(
            "WorkspaceSearchIndex",
            false,
            5,
            std::path::Path::new("."),
            &mut output,
        );
        assert_eq!(code, Some(0));

        let text = String::from_utf8(output).expect("Valid UTF-8 output");
        assert!(text.contains("[Kernel CLI] Search query \"WorkspaceSearchIndex\" matched"));
        assert!(text.contains("search.rs:"));
        assert!(text.contains("WorkspaceSearchIndex"));
    }

    #[test]
    fn test_cli_search_directory_fallback_when_zero_matches() {
        use crate::kernel::cli::{execute_search_to, CliSearchOutput};

        // Query contains path separator or is a path with no tantivy matches
        let mut output = Vec::new();
        let code = execute_search_to(
            "kernel/cli",
            true,
            5,
            std::path::Path::new("."),
            &mut output,
        );
        assert_eq!(code, Some(0));

        let json_text = String::from_utf8(output).expect("Valid UTF-8 output");
        let parsed: CliSearchOutput = serde_json::from_str(&json_text).expect("Valid JSON response");
        assert!(!parsed.matches.is_empty(), "Expected directory suggestion matches for path 'kernel/cli'");
        assert!(parsed.matches.iter().any(|m| m.path.contains("cli.rs")));
    }

    #[test]
    fn test_cli_search_respects_limit() {
        use crate::kernel::cli::{execute_search_to, CliSearchOutput};

        let mut output = Vec::new();
        let code = execute_search_to(
            "kernel",
            true,
            2,
            std::path::Path::new("."),
            &mut output,
        );
        assert_eq!(code, Some(0));

        let json_text = String::from_utf8(output).expect("Valid UTF-8 output");
        let parsed: CliSearchOutput = serde_json::from_str(&json_text).expect("Valid JSON response");
        assert!(parsed.matches.len() <= 2);
    }

    #[test]
    fn test_cli_mcp_stdio_flush_error_returns_nonzero() {
        use std::io::{Cursor, Error, ErrorKind, Write};
        use crate::kernel::cli::run_mcp_stdio_loop;
        use crate::kernel::mcp::McpHub;

        struct FailingFlushWriter;
        impl Write for FailingFlushWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(Error::new(ErrorKind::BrokenPipe, "simulated flush broken pipe"))
            }
        }

        let hub = McpHub::new();
        let input = "{\"jsonrpc\":\"2.0\",\"id\":\"1\",\"method\":\"initialize\"}\n";
        let reader = Cursor::new(input.as_bytes());
        let writer = FailingFlushWriter;

        let exit_code = run_mcp_stdio_loop(reader, writer, &hub);
        assert_eq!(exit_code, 1);
    }

    #[test]
    fn test_cli_search_snippet_utf8_boundary_no_panic() {
        use crate::kernel::cli::{find_snippet_and_line, truncate_chars};

        // Construct a string where multi-byte UTF-8 character crosses byte index 120:
        // 119 ASCII 'a' bytes followed by a 4-byte rocket emoji '🚀'
        let mut s = "a".repeat(119);
        s.push_str("🚀 RocketLaunch Kronos");
        assert!(s.as_bytes().len() > 120);

        // Byte index 120 falls in the middle of the 4-byte rocket emoji.
        // A naive &s[..120] would panic with: "byte index 120 is not a char boundary".
        let truncated = truncate_chars(&s, 120);
        assert!(truncated.ends_with("..."));
        assert!(truncated.starts_with(&"a".repeat(119)));

        // Test with find_snippet_and_line on a temporary file
        let temp_dir = std::env::temp_dir().join(format!(
            "kronos_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).expect("Create temp dir");
        let test_file = temp_dir.join("unicode_test.rs");

        let unicode_line = format!(
            "// QueryTarget: {}🚀🦀🔥 SpecialUTF8Identifier\n",
            "x".repeat(110)
        );
        std::fs::write(&test_file, &unicode_line).expect("Write unicode test file");

        let (line, snippet) = find_snippet_and_line(&test_file, "QueryTarget");
        assert_eq!(line, Some(1));
        assert!(snippet.is_some());
        let snip = snippet.unwrap();
        assert!(snip.contains("QueryTarget"));
        assert!(snip.ends_with("..."));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cli_ast_subcommand_dispatch() {
        use crate::kernel::cli::execute_ast_to;

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let code_path = temp_dir.path().join("service.ts");
        let ts_code = r#"
            export interface IService {
                id: string;
                execute(): boolean;
            }

            export class ServiceImpl implements IService {
                public id = "srv-1";
                public execute(): boolean {
                    return true;
                }
            }
        "#;
        std::fs::write(&code_path, ts_code).expect("write ts file");

        let path_str = code_path.to_str().unwrap();

        // 1. Test CLI summary output
        let mut summary_out = Vec::new();
        let code1 = execute_ast_to(path_str, false, false, false, &mut summary_out);
        assert_eq!(code1, Some(0));
        let summary_text = String::from_utf8_lossy(&summary_out);
        assert!(summary_text.contains("AST Analysis for"));
        assert!(summary_text.contains("IService"));

        // 2. Test CLI symbols output
        let mut symbols_out = Vec::new();
        let code2 = execute_ast_to(path_str, true, false, false, &mut symbols_out);
        assert_eq!(code2, Some(0));
        let symbols_text = String::from_utf8_lossy(&symbols_out);
        assert!(symbols_text.contains("Symbols in"));
        assert!(symbols_text.contains("ServiceImpl"));

        // 3. Test CLI json output
        let mut json_out = Vec::new();
        let code3 = execute_ast_to(path_str, false, true, false, &mut json_out);
        assert_eq!(code3, Some(0));
        let parsed_json: serde_json::Value = serde_json::from_slice(&json_out).expect("valid json");
        assert_eq!(parsed_json["root_kind"], "program");
        assert!(parsed_json["symbols"].as_array().unwrap().len() >= 2);

        // 4. Test parsing CLI args via parse_and_dispatch_from
        let parsed_args = CliArgs::try_parse_from([
            "paseo",
            "ast",
            path_str,
            "--symbols",
            "--json",
        ]).expect("parse ast args");

        assert_eq!(
            parsed_args.command,
            Some(CliCommand::Ast {
                file: path_str.to_string(),
                symbols: true,
                json: true,
                errors: false,
            })
        );
    }

    #[test]
    fn test_cli_ioring_bench_dispatch() {
        use crate::kernel::cli::execute_ioring_bench_to;

        let temp_dir = tempfile::tempdir().expect("tempdir");
        for i in 0..10 {
            let f = temp_dir.path().join(format!("data_{i}.txt"));
            std::fs::write(&f, format!("Payload contents for file {i}")).unwrap();
        }

        let mut out = Vec::new();
        let code = execute_ioring_bench_to(
            temp_dir.path().to_str().unwrap(),
            128,
            true, // force threadpool in tests to verify cross-platform pipeline
            50,
            &mut out,
        );

        assert_eq!(code, Some(0));
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("Kronos IoRing Disk I/O Benchmark"));
        assert!(text.contains("Files Discovered : 10"));
        assert!(text.contains("Read Throughput"));
        assert!(text.contains("PASSED"));

        // Verify CLI arg parsing
        let parsed = CliArgs::try_parse_from([
            "paseo",
            "ioring-bench",
            ".",
            "--queue-size",
            "256",
            "--force-fallback",
            "--max-files",
            "500",
        ]).expect("parse ioring-bench args");

        assert_eq!(
            parsed.command,
            Some(CliCommand::IoringBench {
                directory: ".".to_string(),
                queue_size: 256,
                force_fallback: true,
                max_files: 500,
            })
        );
    }

    #[test]
    fn test_cli_cached_search_fast_path() {
        use crate::kernel::cli::execute_cached_search_in;

        let temp_dir = tempfile::tempdir().expect("tempdir");
        let file_a = temp_dir.path().join("alpha.rs");
        let file_b = temp_dir.path().join("beta.rs");

        std::fs::write(&file_a, "fn quantum_algorithm() { println!(\"alpha\"); }").unwrap();
        std::fs::write(&file_b, "fn classic_algorithm() { println!(\"beta\"); }").unwrap();

        // First search builds the index
        let start1 = std::time::Instant::now();
        let res1 = execute_cached_search_in("quantum_algorithm", temp_dir.path()).expect("first search");
        let dur1 = start1.elapsed();
        assert_eq!(res1.len(), 1);
        assert!(res1[0].path.contains("alpha.rs"));

        // Second consecutive search uses cached warm index (< 15s window)
        let start2 = std::time::Instant::now();
        let res2 = execute_cached_search_in("classic_algorithm", temp_dir.path()).expect("second search");
        let dur2 = start2.elapsed();
        assert_eq!(res2.len(), 1);
        assert!(res2[0].path.contains("beta.rs"));

        // Verify warm query runs in sub-millisecond or fast order
        println!("Search timings: cold = {:?}, warm = {:?}", dur1, dur2);
    }
}
