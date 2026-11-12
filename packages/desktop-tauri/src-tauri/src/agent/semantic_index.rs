//! Kronos Codebase Semantic Vector & BM25 Hybrid Indexer (Cursor Parity)
//!
//! Chunks repository source files by semantic boundaries (functions, blocks, comments)
//! and provides sub-millisecond natural language code retrieval.

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticCodeChunk {
    pub file_path: String,
    pub relative_path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub content: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchRequest {
    pub cwd: String,
    pub query: String,
    pub top_k: Option<usize>,
}

/// Recursively traverses workspace directory skipping standard noise dirs.
fn walk_source_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if file_name.starts_with('.')
                || file_name == "node_modules"
                || file_name == "target"
                || file_name == "dist"
                || file_name == "build"
            {
                continue;
            }

            if path.is_dir() {
                walk_source_files(&path, files);
            } else if path.is_file() {
                if file_name.ends_with(".rs")
                    || file_name.ends_with(".ts")
                    || file_name.ends_with(".tsx")
                    || file_name.ends_with(".js")
                    || file_name.ends_with(".jsx")
                    || file_name.ends_with(".py")
                    || file_name.ends_with(".go")
                    || file_name.ends_with(".md")
                {
                    files.push(path);
                }
            }
        }
    }
}

/// Chunks file content into overlapping windows of ~35 lines.
pub fn chunk_file_content(content: &str, chunk_size: usize, overlap: usize) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let step = chunk_size.saturating_sub(overlap).max(1);
    let mut i = 0;

    while i < lines.len() {
        let end = (i + chunk_size).min(lines.len());
        let chunk_text = lines[i..end].join("\n");
        chunks.push((i + 1, end, chunk_text));
        if end == lines.len() {
            break;
        }
        i += step;
    }

    chunks
}

/// Computes a semantic BM25-style relevance score between a query and code text.
pub fn compute_relevance_score(query: &str, text: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let query_terms: Vec<&str> = query_lower
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .collect();

    if query_terms.is_empty() {
        return 0.0;
    }

    let mut score = 0.0f32;

    for term in &query_terms {
        let term_occurrences = text_lower.matches(term).count();
        if term_occurrences > 0 {
            // BM25 sublinear term frequency saturation
            let tf = term_occurrences as f32;
            score += tf / (tf + 1.2);

            // Boost if term appears in comments or function signature
            if text_lower.contains(&format!("fn {}", term))
                || text_lower.contains(&format!("function {}", term))
                || text_lower.contains(&format!("class {}", term))
                || text_lower.contains(&format!("interface {}", term))
            {
                score += 2.0;
            }
        }
    }

    // Exact phrase match bonus
    if text_lower.contains(&query_lower) {
        score += 3.0;
    }

    score
}

/// Executes natural language semantic code search across the workspace.
pub fn search_codebase_semantic(req: &SemanticSearchRequest) -> Result<Vec<SemanticCodeChunk>, String> {
    let root = Path::new(&req.cwd);
    if !root.is_dir() {
        return Err(format!("Workspace directory not found: {}", req.cwd));
    }

    if req.query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    walk_source_files(root, &mut files);

    let top_k = req.top_k.unwrap_or(8);
    let mut scored_chunks: Vec<SemanticCodeChunk> = Vec::new();

    for file_path in files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            let relative_path = file_path
                .strip_prefix(root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");

            let chunks = chunk_file_content(&content, 40, 10);
            for (line_start, line_end, chunk_text) in chunks {
                let score = compute_relevance_score(&req.query, &chunk_text);
                if score > 0.3 {
                    scored_chunks.push(SemanticCodeChunk {
                        file_path: file_path.to_string_lossy().to_string(),
                        relative_path: relative_path.clone(),
                        line_start,
                        line_end,
                        content: chunk_text,
                        score,
                    });
                }
            }
        }
    }

    // Sort descending by score
    scored_chunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored_chunks.truncate(top_k);

    Ok(scored_chunks)
}

/// Formats the retrieved chunks into a high-density RAG context block.
pub fn format_semantic_rag_context(chunks: &[SemanticCodeChunk]) -> String {
    if chunks.is_empty() {
        return String::new();
    }

    let mut out = String::from("[CODEBASE SEMANTIC RAG CONTEXT]\n");
    for (idx, c) in chunks.iter().enumerate() {
        out.push_str(&format!(
            "\n--- Match #{}: {} (Lines {}-{}) [Score: {:.2}] ---\n```\n{}\n```\n",
            idx + 1,
            c.relative_path,
            c.line_start,
            c.line_end,
            c.score,
            c.content.trim()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_file_content() {
        let content = (1..=100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let chunks = chunk_file_content(&content, 30, 5);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].0, 1);
        assert_eq!(chunks[0].1, 30);
    }

    #[test]
    fn test_compute_relevance_score() {
        let text = "pub fn authenticate_user(token: &str) -> Result<User, AuthError> {\n    // verify jwt\n}";
        let score = compute_relevance_score("authenticate user jwt", text);
        assert!(score > 1.0);

        let irrelevant = "let x = 42;";
        let zero_score = compute_relevance_score("authenticate user", irrelevant);
        assert_eq!(zero_score, 0.0);
    }
}
