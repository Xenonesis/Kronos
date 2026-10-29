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
}
