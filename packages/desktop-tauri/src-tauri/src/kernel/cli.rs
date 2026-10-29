use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(name = "paseo", about = "Kronos Hyper-Native Kernel CLI", version)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    /// Start the embedded WebSocket and IPC daemon
    Daemon {
        #[arg(short, long, default_value_t = 6767)]
        port: u16,
        #[arg(long)]
        pipe: Option<String>,
    },
    /// Dispatch an agent task prompt
    Agent {
        prompt: String,
        #[arg(short, long)]
        provider: Option<String>,
    },
    /// Lexical search across workspace index
    Search {
        query: String,
    },
    /// Model Context Protocol stdio server
    Mcp,
}

/// Parses CLI arguments from `std::env::args_os()` and dispatches commands if any.
/// Returns `None` if running GUI mode (no subcommand provided).
/// Returns `Some(exit_code)` if a CLI command was processed.
pub fn parse_and_dispatch() -> Option<i32> {
    parse_and_dispatch_from(std::env::args_os())
}

/// Parses CLI arguments from an arbitrary iterator and dispatches commands.
pub fn parse_and_dispatch_from<I, T>(itr: I) -> Option<i32>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    match CliArgs::try_parse_from(itr) {
        Ok(args) => dispatch(args),
        Err(err) => {
            use clap::error::ErrorKind;
            match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    let _ = err.print();
                    Some(0)
                }
                _ => {
                    let _ = err.print();
                    Some(1)
                }
            }
        }
    }
}

/// Dispatches a parsed `CliArgs` instance.
pub fn dispatch(args: CliArgs) -> Option<i32> {
    let command = match args.command {
        Some(cmd) => cmd,
        None => return None,
    };

    match command {
        CliCommand::Daemon { port, pipe } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to initialize tokio runtime for CLI daemon");

            rt.block_on(async {
                let daemon = crate::kernel::server::EmbeddedDaemon::start_daemon(port, pipe)
                    .await
                    .expect("Failed to start EmbeddedDaemon");
                println!("[Kernel CLI] EmbeddedDaemon started on port {}", daemon.port);

                wait_for_daemon_shutdown().await;

                daemon.stop().await;
                println!("[Kernel CLI] EmbeddedDaemon stopped cleanly");
            });

            Some(0)
        }
        CliCommand::Agent { prompt, provider } => {
            let prov = provider.as_deref().unwrap_or("default");
            println!("[Kernel CLI] Agent prompt received for provider [{prov}]: \"{prompt}\"");
            println!("[Kernel CLI] Dry-run dispatch successful (exit 0)");
            Some(0)
        }
        CliCommand::Search { query } => {
            match crate::kernel::search::WorkspaceSearchIndex::new_in_memory() {
                Ok(index) => {
                    if let Ok(entries) = std::fs::read_dir(".") {
                        for entry in entries.flatten().take(50) {
                            let path = entry.path();
                            if path.is_file() {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    let _ = index.index_document(&path.to_string_lossy(), &content);
                                }
                            }
                        }
                    }

                    match index.search(&query) {
                        Ok(matches) => {
                            println!("[Kernel CLI] Search query \"{}\" matched {} documents:", query, matches.len());
                            for m in &matches {
                                println!("  - {} (score: {:.2})", m.path, m.score);
                            }
                            Some(0)
                        }
                        Err(err) => {
                            eprintln!("[Kernel CLI] Search query error: {err}");
                            Some(1)
                        }
                    }
                }
                Err(err) => {
                    eprintln!("[Kernel CLI] Failed to initialize search index: {err}");
                    Some(1)
                }
            }
        }
        CliCommand::Mcp => {
            println!("[Kernel CLI] Entering Model Context Protocol (MCP) stdio loop...");
            #[cfg(not(test))]
            {
                use std::io::BufRead;
                let stdin = std::io::stdin();
                for line in stdin.lock().lines() {
                    match line {
                        Ok(l) => {
                            let trimmed = l.trim();
                            if trimmed == "exit" || trimmed == "quit" {
                                break;
                            }
                            eprintln!("[MCP] Received stdio line: {trimmed}");
                        }
                        Err(_) => break,
                    }
                }
            }
            #[cfg(test)]
            {
                println!("[Kernel CLI] MCP stdio loop ready");
            }
            Some(0)
        }
    }
}

async fn wait_for_daemon_shutdown() {
    if std::env::var("KRONOS_CLI_TEST").is_ok() {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        return;
    }

    #[cfg(test)]
    {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
    }

    #[cfg(not(test))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
