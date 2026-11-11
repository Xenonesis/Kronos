use crate::kernel::ast::{AstEngine, AstSymbol, AstSymbolKind};
use std::path::Path;

const MAX_REPO_MAP_CHARS: usize = 6_000; // ~1,500 tokens
const MAX_FILES_TO_SCAN: usize = 80;
const MAX_FILE_SIZE_BYTES: u64 = 64 * 1024; // 64 KB

/// Ignored directory names when scanning the workspace.
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    ".next",
    ".turbo",
    "vendor",
    "out",
    ".cache",
    ".vscode",
    ".idea",
];

/// Generates a high-altitude compact symbol outline of the workspace for AI models.
/// Gives the model instant structural context of files, classes, methods, and types.
pub fn generate_repo_map(cwd: &Path) -> Option<String> {
    if !cwd.is_dir() {
        return None;
    }

    let mut source_files = Vec::new();
    collect_source_files(cwd, cwd, &mut source_files, 0, 8);

    if source_files.is_empty() {
        return None;
    }

    // Sort files by relative path for deterministic outline
    source_files.sort();

    let ast_engine = AstEngine::new();
    let mut sections = Vec::new();
    let mut total_chars = 0usize;

    for rel_path in source_files.into_iter().take(MAX_FILES_TO_SCAN) {
        let full_path = cwd.join(&rel_path);
        let ext = full_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let symbols = extract_file_symbols(&full_path, &ext, &ast_engine);
        if !symbols.is_empty() {
            let mut file_block = format!("{rel_path}:\n");
            for sym in symbols {
                file_block.push_str(&format!("  {sym}\n"));
            }
            total_chars += file_block.len();
            sections.push(file_block);

            if total_chars >= MAX_REPO_MAP_CHARS {
                break;
            }
        }
    }

    if sections.is_empty() {
        return None;
    }

    let mut output = String::with_capacity(total_chars + 128);
    output.push_str("### Codebase High-Altitude Symbol Map\n");
    output.push_str("Concise structural map of key types, interfaces, and functions in this workspace:\n\n```\n");
    for sec in sections {
        output.push_str(&sec);
    }
    output.push_str("```\n");

    Some(output)
}

fn collect_source_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth || files.len() >= MAX_FILES_TO_SCAN * 2 {
        return;
    }

    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        if path.is_dir() {
            if name_str.starts_with('.') || IGNORED_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            collect_source_files(root, &path, files, depth + 1, max_depth);
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_clean = ext.to_lowercase();
                if matches!(
                    ext_clean.as_str(),
                    "ts" | "tsx" | "js" | "jsx" | "rs" | "py" | "go"
                ) {
                    if let Ok(meta) = entry.metadata() {
                        if meta.len() > 0 && meta.len() <= MAX_FILE_SIZE_BYTES {
                            if let Ok(rel) = path.strip_prefix(root) {
                                files.push(rel.to_string_lossy().replace('\\', "/"));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_file_symbols(path: &Path, ext: &str, ast_engine: &AstEngine) -> Vec<String> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    match ext {
        "ts" | "tsx" | "js" | "jsx" => {
            if let Ok(analysis) = ast_engine.analyze(&source, ext) {
                let mut out = Vec::new();
                for sym in analysis.symbols {
                    format_ast_symbol(&sym, &mut out);
                    if out.len() >= 12 {
                        break;
                    }
                }
                out
            } else {
                extract_symbols_heuristic(&source, ext)
            }
        }
        "rs" | "py" | "go" => extract_symbols_heuristic(&source, ext),
        _ => Vec::new(),
    }
}

fn format_ast_symbol(sym: &AstSymbol, out: &mut Vec<String>) {
    let kind_str = match sym.kind {
        AstSymbolKind::Function | AstSymbolKind::AsyncFunction => "fn",
        AstSymbolKind::Class => "class",
        AstSymbolKind::Interface => "interface",
        AstSymbolKind::TypeAlias => "type",
        AstSymbolKind::Enum => "enum",
        AstSymbolKind::Method => "method",
        _ => return,
    };

    let sig = sym
        .signature
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(&sym.name);

    out.push(format!("{kind_str} {sig}"));

    for child in &sym.children {
        if matches!(child.kind, AstSymbolKind::Method | AstSymbolKind::Property) {
            out.push(format!("  .{}", child.name));
            if out.len() >= 12 {
                break;
            }
        }
    }
}

fn extract_symbols_heuristic(source: &str, ext: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines().take(400) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        let is_match = match ext {
            "rs" => {
                trimmed.starts_with("pub fn ")
                    || trimmed.starts_with("pub async fn ")
                    || trimmed.starts_with("pub struct ")
                    || trimmed.starts_with("pub enum ")
                    || trimmed.starts_with("pub trait ")
            }
            "py" => {
                trimmed.starts_with("def ")
                    || trimmed.starts_with("async def ")
                    || trimmed.starts_with("class ")
            }
            "go" => {
                trimmed.starts_with("func ")
                    || (trimmed.starts_with("type ") && (trimmed.contains("struct") || trimmed.contains("interface")))
            }
            _ => false,
        };

        if is_match {
            let display = trimmed.split('{').next().unwrap_or(trimmed).trim();
            out.push(display.to_string());
            if out.len() >= 10 {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_generate_repo_map_empty() {
        let dir = tempdir().unwrap();
        assert!(generate_repo_map(dir.path()).is_none());
    }

    #[test]
    fn test_generate_repo_map_with_ts_and_rs() {
        let dir = tempdir().unwrap();
        // TS file
        let ts_path = dir.path().join("service.ts");
        {
            let mut f1 = File::create(&ts_path).unwrap();
            writeln!(f1, "export interface UserService {{\n  getUser(id: string): User;\n}}\nexport class UserServiceImpl implements UserService {{\n  getUser(id: string) {{ return {{}}; }}\n}}").unwrap();
            f1.sync_all().unwrap();
        }

        // Rust file
        let rs_path = dir.path().join("main.rs");
        {
            let mut f2 = File::create(&rs_path).unwrap();
            writeln!(f2, "pub struct Config {{\n  pub port: u16,\n}}\npub async fn start_server() -> Result<(), ()> {{\n  Ok(())\n}}").unwrap();
            f2.sync_all().unwrap();
        }

        let map = generate_repo_map(dir.path()).expect("Should generate repo map");
        assert!(map.contains("### Codebase High-Altitude Symbol Map"));
        assert!(map.contains("service.ts:"));
        assert!(map.contains("main.rs:"));
        assert!(map.contains("pub struct Config"));
    }
}
