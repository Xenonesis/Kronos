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

/// Returns true if an argument looks like a deep-link protocol URL or standard OS flag.
pub fn is_gui_or_protocol_arg(arg: &str) -> bool {
    let lower = arg.to_lowercase();
    lower.starts_with("kronos://")
        || lower.starts_with("paseo://")
        || lower.starts_with("kronos:")
        || lower.starts_with("paseo:")
        || lower.contains("://")
        || lower == "--allow-downgrade"
        || lower.starts_with("--squirrel")
        || lower.starts_with("-psn")
        || lower.starts_with("/prefetch:")
        || lower == "--minimized"
        || lower == "--hidden"
        || lower == "--startup"
        || lower == "/startup"
        || lower.starts_with("--disable-gpu")
        || lower.starts_with("--type=")
        || lower == "--type"
        || lower.starts_with("--original-process-start-time")
        || lower.starts_with("--webview-")
        || lower.starts_with("--edge-")
        || lower.starts_with("--mojo-")
        || lower.starts_with("--crash-handler")
        || lower == "--no-sandbox"
}

/// Recursively collects search candidate files up to `max_depth` (e.g. 10 levels),
/// skipping non-source directories (.git, node_modules, target, dist, etc.) and
/// selecting valid source/text extensions (.ts, .rs, .js, .json, .md, etc.).
pub fn collect_search_files(
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    files: &mut Vec<std::path::PathBuf>,
    max_files: usize,
) {
    if depth > max_depth || files.len() >= max_files {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if files.len() >= max_files {
            break;
        }
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        if path.is_dir() {
            if name_str.starts_with('.')
                || name_str == "node_modules"
                || name_str == "target"
                || name_str == "dist"
                || name_str == "build"
                || name_str == "out"
            {
                continue;
            }
            collect_search_files(&path, depth + 1, max_depth, files, max_files);
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_ascii_lowercase();
                if matches!(
                    ext_lower.as_str(),
                    "ts" | "tsx" | "rs" | "js" | "jsx" | "mjs" | "cjs" | "json" | "md" | "toml" | "html" | "css" | "scss" | "yaml" | "yml" | "txt"
                ) {
                    files.push(path);
                }
            }
        }
    }
}

/// Executes workspace lexical search rooted at `root` directory up to 10 levels deep.
pub fn execute_search_in(
    query: &str,
    root: &std::path::Path,
) -> Result<Vec<crate::kernel::search::SearchResult>, String> {
    let index = crate::kernel::search::WorkspaceSearchIndex::new_in_memory()?;
    let mut files = Vec::new();
    collect_search_files(root, 0, 10, &mut files, 1000);

    let mut doc_payloads: Vec<(String, String)> = Vec::new();
    for path in files {
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > 1_048_576 {
                continue;
            }
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            let path_str = path.to_string_lossy().to_string();
            doc_payloads.push((path_str, content));
        }
    }

    let doc_refs: Vec<(&str, &str)> = doc_payloads
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();

    index.index_documents_batch(&doc_refs)?;
    index.search(query)
}

/// Dispatches the search command across the current working directory.
pub fn execute_search(query: &str) -> Option<i32> {
    match execute_search_in(query, std::path::Path::new(".")) {
        Ok(matches) => {
            println!(
                "[Kernel CLI] Search query \"{}\" matched {} documents:",
                query,
                matches.len()
            );
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

/// Parses CLI arguments from an arbitrary iterator and dispatches commands.
pub fn parse_and_dispatch_from<I, T>(itr: I) -> Option<i32>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let raw_args: Vec<std::ffi::OsString> = itr.into_iter().map(Into::into).collect();
    match CliArgs::try_parse_from(&raw_args) {
        Ok(args) => dispatch(args),
        Err(err) => {
            use clap::error::ErrorKind;
            match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    let _ = err.print();
                    Some(0)
                }
                _ => {
                    if raw_args.iter().skip(1).any(|arg| {
                        is_gui_or_protocol_arg(&arg.to_string_lossy())
                    }) {
                        return None;
                    }
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
        CliCommand::Search { query } => execute_search(&query),
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
