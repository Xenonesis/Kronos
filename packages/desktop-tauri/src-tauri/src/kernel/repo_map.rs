use crate::kernel::ast::{AstEngine, AstSymbol, AstSymbolKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyGraphResult {
    pub node_count: usize,
    pub edge_count: usize,
    pub modules: Vec<String>,
    pub mermaid: String,
}

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

/// Generates a module-level Mermaid `flowchart LR` dependency graph across workspace source files.
pub fn generate_dependency_graph(cwd: &Path) -> Option<DependencyGraphResult> {
    if !cwd.is_dir() {
        return None;
    }

    let mut source_files = Vec::new();
    collect_source_files(cwd, cwd, &mut source_files, 0, 8);
    if source_files.is_empty() {
        return None;
    }
    source_files.sort();

    let mut module_edges: BTreeSet<(String, String)> = BTreeSet::new();
    let mut modules: BTreeSet<String> = BTreeSet::new();

    for rel_path in source_files.iter().take(MAX_FILES_TO_SCAN) {
        let src_module = module_bucket(rel_path);
        modules.insert(src_module.clone());
        let Ok(content) = std::fs::read_to_string(cwd.join(rel_path)) else {
            continue;
        };
        for target in extract_import_targets(&content) {
            let dst_module = normalize_import_bucket(&target, rel_path);
            if let Some(dst) = dst_module {
                if dst != src_module {
                    modules.insert(dst.clone());
                    module_edges.insert((src_module.clone(), dst));
                }
            }
        }
    }

    let module_list: Vec<String> = modules.into_iter().take(28).collect();
    let mut id_map: BTreeMap<String, String> = BTreeMap::new();
    for (idx, m) in module_list.iter().enumerate() {
        id_map.insert(m.clone(), format!("M{idx}"));
    }

    let mut mermaid = String::from("flowchart LR\n");
    for m in &module_list {
        if let Some(id) = id_map.get(m) {
            mermaid.push_str(&format!("  {id}[\"{m}\"]\n"));
        }
    }

    let mut rendered_edges = 0usize;
    for (src, dst) in &module_edges {
        if let (Some(src_id), Some(dst_id)) = (id_map.get(src), id_map.get(dst)) {
            mermaid.push_str(&format!("  {src_id} --> {dst_id}\n"));
            rendered_edges += 1;
            if rendered_edges >= 48 {
                break;
            }
        }
    }

    Some(DependencyGraphResult {
        node_count: module_list.len(),
        edge_count: rendered_edges,
        modules: module_list,
        mermaid,
    })
}

fn module_bucket(rel_path: &str) -> String {
    let parts: Vec<&str> = rel_path.split('/').collect();
    if parts.len() >= 3 && (parts[0] == "packages" || parts[0] == "crates") {
        format!("{}/{}", parts[0], parts[1])
    } else if parts.len() >= 2 && (parts[0] == "src" || parts[0] == "app") {
        format!("{}/{}", parts[0], parts[1])
    } else {
        parts.first().copied().unwrap_or(rel_path).to_string()
    }
}

fn extract_import_targets(source: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in source.lines().take(120) {
        let trimmed = line.trim();
        // JS/TS: import ... from "x"
        if let Some(idx) = trimmed.find(" from ") {
            let rest = trimmed[idx + 6..].trim();
            if let Some(quoted) = extract_quoted(rest) {
                targets.push(quoted);
                continue;
            }
        }
        // Rust: use crate::foo::...
        if let Some(rest) = trimmed.strip_prefix("use crate::") {
            if let Some(seg) = rest.split("::").next() {
                let clean = seg.trim_end_matches(';').trim();
                if !clean.is_empty() {
                    targets.push(format!("crate::{clean}"));
                }
            }
        }
    }
    targets
}

fn extract_quoted(s: &str) -> Option<String> {
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &s[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn normalize_import_bucket(target: &str, source_rel_path: &str) -> Option<String> {
    if let Some(mod_name) = target.strip_prefix("crate::") {
        return Some(format!("src/{mod_name}"));
    }
    if let Some(alias) = target.strip_prefix("@/") {
        let seg = alias.split('/').next()?;
        return Some(format!("src/{seg}"));
    }
    if target.starts_with("@getpaseo/") {
        let pkg = target.strip_prefix("@getpaseo/")?.split('/').next()?;
        return Some(format!("packages/{pkg}"));
    }
    if target.starts_with("./") || target.starts_with("../") {
        let parent = Path::new(source_rel_path).parent()?.to_str()?.replace('\\', "/");
        if !parent.is_empty() {
            return Some(module_bucket(&parent));
        }
    }
    None
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

    #[test]
    fn test_generate_dependency_graph() {
        let dir = tempdir().unwrap();
        let src_a = dir.path().join("src").join("alpha");
        let src_b = dir.path().join("src").join("beta");
        std::fs::create_dir_all(&src_a).unwrap();
        std::fs::create_dir_all(&src_b).unwrap();
        std::fs::write(src_a.join("index.ts"), "import { x } from \"@/beta/util\";\nexport const a = 1;").unwrap();
        std::fs::write(src_b.join("util.ts"), "export const x = 2;").unwrap();

        let graph = generate_dependency_graph(dir.path()).expect("Should build dependency graph");
        assert!(graph.node_count >= 2);
        assert!(graph.edge_count >= 1);
        assert!(graph.mermaid.contains("flowchart LR"));
        assert!(graph.mermaid.contains("-->"));
    }
}
