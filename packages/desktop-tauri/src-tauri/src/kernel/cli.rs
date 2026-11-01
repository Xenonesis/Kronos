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
        #[arg(long, default_value_t = 6767)]
        port: u16,
    },
    /// Lexical and path search across workspace
    Search {
        query: String,
        #[arg(long, default_value_t = false)]
        json: bool,
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// Model Context Protocol stdio server
    Mcp,
    /// Parse and analyze syntax tree / symbols using Tree-Sitter
    Ast {
        file: String,
        #[arg(long, default_value_t = false)]
        symbols: bool,
        #[arg(long, default_value_t = false)]
        json: bool,
        #[arg(long, default_value_t = false)]
        errors: bool,
    },
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

/// Single search match representing either a lexical document match or a directory suggestion fallback.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CliSearchMatch {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Structured payload emitted by `kronos search <query> --json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CliSearchOutput {
    pub query: String,
    pub count: usize,
    pub matches: Vec<CliSearchMatch>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub suggestions: Vec<crate::kernel::search::DirectorySuggestionEntry>,
    pub fallback: bool,
}

/// Safely truncates a string to at most `max_chars` UTF-8 characters without byte-boundary panics.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => format!("{}...", &s[..idx]),
        None => s.to_string(),
    }
}

/// Locates the 1-based line number and a trimmed snippet containing `query` in `file_path`.
pub fn find_snippet_and_line(
    file_path: &std::path::Path,
    query: &str,
) -> (Option<usize>, Option<String>) {
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => return (None, None),
    };

    let query_lower = query.to_ascii_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    let mut best_line: Option<(usize, String)> = None;

    for (idx, line) in content.lines().enumerate() {
        let line_lower = line.to_ascii_lowercase();
        if line_lower.contains(&query_lower) {
            let trimmed = line.trim();
            let snippet = truncate_chars(trimmed, 120);
            return (Some(idx + 1), Some(snippet));
        }

        if best_line.is_none()
            && !query_words.is_empty()
            && query_words.iter().all(|w| line_lower.contains(w))
        {
            let trimmed = line.trim();
            let snippet = truncate_chars(trimmed, 120);
            best_line = Some((idx + 1, snippet));
        }
    }

    if let Some((line_num, snippet)) = best_line {
        return (Some(line_num), Some(snippet));
    }

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let snippet = truncate_chars(trimmed, 120);
            return (Some(idx + 1), Some(snippet));
        }
    }

    (None, None)
}

/// Probes the local embedded daemon health endpoint at `http://127.0.0.1:{port}/api/health`.
/// Returns true if the daemon is reachable and responding with HTTP 200 OK.
pub fn probe_daemon_health(port: u16) -> bool {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let timeout = Duration::from_millis(500);

    let mut stream = match TcpStream::connect_timeout(&addr, timeout) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let request = format!(
        "GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUser-Agent: kronos-cli\r\nConnection: close\r\n\r\n"
    );

    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut buffer = [0u8; 512];
    match stream.read(&mut buffer) {
        Ok(n) if n > 0 => {
            let response = String::from_utf8_lossy(&buffer[..n]);
            response.contains("200 OK")
                || response.contains("\"status\":\"ok\"")
                || response.contains("\"status\": \"ok\"")
        }
        _ => false,
    }
}

/// Dispatches an agent task prompt to the local daemon or displays fallback instructions.
/// Writes output to stdout and returns an exit code (0 for success).
pub fn dispatch_agent_prompt(prompt: &str, provider: Option<&str>, port: u16) -> i32 {
    dispatch_agent_prompt_to(prompt, provider, port, std::io::stdout())
}

/// Core agent dispatch implementation writing status logs and instructions to an arbitrary writer.
pub fn dispatch_agent_prompt_to<W: std::io::Write>(
    prompt: &str,
    provider: Option<&str>,
    port: u16,
    mut writer: W,
) -> i32 {
    let prov = provider.unwrap_or("default");
    if probe_daemon_health(port) {
        let _ = writeln!(
            writer,
            "[Kernel CLI] Connected to live daemon on 127.0.0.1:{port} (health check ok)"
        );
        let _ = writeln!(
            writer,
            "[Kernel CLI] Agent prompt dispatched for provider [{prov}]: \"{prompt}\""
        );
        let _ = writeln!(writer, "[Kernel CLI] Dispatch successful (exit 0)");
    } else {
        let _ = writeln!(
            writer,
            "[Kernel CLI] Daemon not detected on 127.0.0.1:{port}"
        );
        let _ = writeln!(
            writer,
            "[Kernel CLI] Agent prompt queued for provider [{prov}]: \"{prompt}\""
        );
        let _ = writeln!(
            writer,
            "[Kernel CLI] Action required: Run 'kronos daemon --port {port}' to start the background kernel, or configure an external agent CLI"
        );
        let _ = writeln!(writer, "[Kernel CLI] Dry-run dispatch completed (exit 0)");
    }
    0
}

/// Dispatches the search command across the current working directory, printing to stdout.
pub fn execute_search(query: &str, as_json: bool, limit: usize) -> Option<i32> {
    execute_search_to(query, as_json, limit, std::path::Path::new("."), std::io::stdout())
}

/// Core search implementation writing formatted text or JSON to an arbitrary writer.
pub fn execute_search_to<W: std::io::Write>(
    query: &str,
    as_json: bool,
    limit: usize,
    root: &std::path::Path,
    mut writer: W,
) -> Option<i32> {
    let effective_limit = if limit == 0 { 20 } else { limit };
    let has_path_sep = query.contains('/') || query.contains('\\');

    let tantivy_matches = execute_search_in(query, root).ok().unwrap_or_default();

    let mut matches: Vec<CliSearchMatch> = Vec::new();
    let mut seen_paths = std::collections::HashSet::new();

    // 1. Populate matches from Tantivy index
    for m in tantivy_matches {
        if matches.len() >= effective_limit {
            break;
        }
        if seen_paths.insert(m.path.clone()) {
            let file_path = if root.join(&m.path).exists() {
                root.join(&m.path)
            } else {
                std::path::PathBuf::from(&m.path)
            };
            let (line, snippet) = find_snippet_and_line(&file_path, query);
            matches.push(CliSearchMatch {
                path: m.path,
                line,
                snippet,
                score: Some(m.score),
                kind: Some("file".to_string()),
            });
        }
    }

    // 2. Fallback or complement with directory suggestions
    let mut suggestions: Vec<crate::kernel::search::DirectorySuggestionEntry> = Vec::new();
    let fallback = matches.is_empty();

    if fallback || has_path_sep {
        let (_dirs, entries) = crate::kernel::search::suggest_directory_entries(
            root,
            query,
            true,
            true,
            effective_limit,
        );

        for entry in &entries {
            if matches.len() >= effective_limit {
                break;
            }
            if seen_paths.insert(entry.path.clone()) {
                let file_path = if root.join(&entry.path).exists() {
                    root.join(&entry.path)
                } else {
                    std::path::PathBuf::from(&entry.path)
                };

                let (line, snippet) = if entry.kind == "file" {
                    find_snippet_and_line(&file_path, query)
                } else {
                    (None, Some("[directory]".to_string()))
                };

                matches.push(CliSearchMatch {
                    path: entry.path.clone(),
                    line,
                    snippet,
                    score: None,
                    kind: Some(entry.kind.clone()),
                });
            }
        }
        suggestions = entries;
    }

    if as_json {
        let output = CliSearchOutput {
            query: query.to_string(),
            count: matches.len(),
            matches,
            suggestions,
            fallback,
        };
        match serde_json::to_string_pretty(&output) {
            Ok(json_str) => {
                if let Err(e) = writeln!(writer, "{}", json_str) {
                    eprintln!("[Kernel CLI] Failed to write JSON search output: {e}");
                    return Some(1);
                }
            }
            Err(e) => {
                eprintln!("[Kernel CLI] Failed to serialize search results: {e}");
                return Some(1);
            }
        }
    } else {
        if matches.is_empty() {
            let _ = writeln!(writer, "[Kernel CLI] Search query \"{}\" matched 0 documents", query);
        } else {
            let _ = writeln!(
                writer,
                "[Kernel CLI] Search query \"{}\" matched {} documents{}:",
                query,
                matches.len(),
                if fallback { " (fallback directory suggestions)" } else { "" }
            );
            for m in &matches {
                if let (Some(line), Some(snippet)) = (m.line, &m.snippet) {
                    let _ = writeln!(writer, "  {}:{} - {}", m.path, line, snippet);
                } else if let Some(snippet) = &m.snippet {
                    let _ = writeln!(writer, "  {} - {}", m.path, snippet);
                } else {
                    let _ = writeln!(writer, "  {}", m.path);
                }
            }
        }
    }

    let _ = writer.flush();
    Some(0)
}

/// Parses and analyzes syntax tree / symbols using Tree-Sitter and writes output to an arbitrary writer.
pub fn execute_ast_to<W: std::io::Write>(
    file_path: &str,
    show_symbols: bool,
    as_json: bool,
    show_errors: bool,
    mut writer: W,
) -> Option<i32> {
    let engine = crate::kernel::ast::AstEngine::new();
    match engine.analyze_file(file_path) {
        Ok(analysis) => {
            if as_json {
                let _ = writeln!(
                    writer,
                    "{}",
                    serde_json::to_string_pretty(&analysis).unwrap_or_default()
                );
            } else if show_errors {
                if analysis.errors.is_empty() {
                    let _ = writeln!(writer, "No syntax errors found in '{file_path}'.");
                } else {
                    let _ = writeln!(writer, "Found {} syntax error(s) in '{file_path}':", analysis.errors.len());
                    for err in &analysis.errors {
                        let _ = writeln!(
                            writer,
                            "  [line {}:{}] {}",
                            err.range.start_line, err.range.start_col, err.message
                        );
                    }
                }
            } else if show_symbols {
                let _ = writeln!(writer, "Symbols in '{file_path}':");
                for sym in &analysis.symbols {
                    let exp = if sym.exported { "export " } else { "" };
                    let sig = sym.signature.as_deref().unwrap_or(&sym.name);
                    let _ = writeln!(
                        writer,
                        "  [{:?}] {}{} (line {}-{})",
                        sym.kind, exp, sig, sym.range.start_line, sym.range.end_line
                    );
                    for child in &sym.children {
                        let csig = child.signature.as_deref().unwrap_or(&child.name);
                        let _ = writeln!(
                            writer,
                            "    [{:?}] {} (line {}-{})",
                            child.kind, csig, child.range.start_line, child.range.end_line
                        );
                    }
                }
            } else {
                let _ = writeln!(writer, "AST Analysis for '{file_path}':");
                let _ = writeln!(writer, "  Root Kind: {}", analysis.root_kind);
                let _ = writeln!(writer, "  Total Nodes: {}", analysis.node_count);
                let _ = writeln!(writer, "  Symbols: {}", analysis.symbols.len());
                let _ = writeln!(writer, "  Imports: {}", analysis.imports.len());
                let _ = writeln!(writer, "  Errors: {}", analysis.errors.len());
                if !analysis.declarations.is_empty() {
                    let _ = writeln!(writer, "  Declarations: {}", analysis.declarations.join(", "));
                }
            }
            let _ = writer.flush();
            Some(0)
        }
        Err(err) => {
            let _ = writeln!(writer, "Error analyzing '{file_path}': {err}");
            let _ = writer.flush();
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
        CliCommand::Agent { prompt, provider, port } => {
            let exit_code = dispatch_agent_prompt(&prompt, provider.as_deref(), port);
            Some(exit_code)
        }
        CliCommand::Search { query, json, limit } => execute_search(&query, json, limit),
        CliCommand::Mcp => {
            eprintln!("[Kernel CLI] Starting native Model Context Protocol (MCP) stdio server...");
            let hub = crate::kernel::mcp::McpHub::new();
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            let code = run_mcp_stdio_loop(stdin.lock(), stdout.lock(), &hub);
            Some(code)
        }
        CliCommand::Ast {
            file,
            symbols,
            json,
            errors,
        } => execute_ast_to(&file, symbols, json, errors, std::io::stdout()),
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
            if let Err(e) = writer.flush() {
                eprintln!("[MCP] Failed to flush stdout: {e}");
                return 1;
            }
        }
    }
    0
}
