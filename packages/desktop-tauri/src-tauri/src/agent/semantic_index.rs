//! Kronos Codebase Semantic Vector & BM25 Hybrid Indexer (Cursor Parity)
//!
//! Chunks repository source files by AST symbol boundaries (functions, classes, structs,
//! interfaces, traits) using `AstEngine`, computes corpus-wide Okapi BM25 with true
//! Inverse Document Frequency (IDF) and camelCase/snake_case sub-tokenization, loads
//! workspace files via batched `IoRingBatchReader`, and fuses local Ollama dense embeddings.

use std::collections::{HashMap, HashSet};
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

/// Splits identifiers and text into lowercase tokens, expanding camelCase, PascalCase,
/// and snake_case identifiers into sub-tokens alongside full tokens.
pub fn tokenize_identifier_terms(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if raw.len() < 2 {
            continue;
        }
        let lower = raw.to_lowercase();
        out.push(lower.clone());

        if raw.contains('_') || raw.chars().any(|c| c.is_uppercase()) {
            let mut cur = String::new();
            let mut prev_lower = false;
            for ch in raw.chars() {
                if ch == '_' {
                    if cur.len() >= 2 {
                        let sub = cur.to_lowercase();
                        if sub != lower {
                            out.push(sub);
                        }
                    }
                    cur.clear();
                    prev_lower = false;
                } else if ch.is_uppercase() && prev_lower {
                    if cur.len() >= 2 {
                        let sub = cur.to_lowercase();
                        if sub != lower {
                            out.push(sub);
                        }
                    }
                    cur.clear();
                    cur.push(ch);
                    prev_lower = false;
                } else {
                    prev_lower = ch.is_lowercase();
                    cur.push(ch);
                }
            }
            if cur.len() >= 2 {
                let sub = cur.to_lowercase();
                if sub != lower {
                    out.push(sub);
                }
            }
        }
    }
    out
}

/// Chunks file content into overlapping windows of `chunk_size` lines.
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

/// Chunks source code along real AST symbol boundaries (functions, classes, structs,
/// interfaces, impls, methods) extracted via `AstEngine`, falling back to sliding windows
/// for uncovered regions or non-code files.
pub fn chunk_file_content_ast(
    content: &str,
    ext: Option<&str>,
    chunk_size: usize,
    overlap: usize,
) -> Vec<(usize, usize, String)> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let window = chunk_size.max(8);
    let step = window.saturating_sub(overlap).max(1);

    if let Some(ext_str) = ext {
        let engine = crate::kernel::ast::AstEngine::new();
        if let Ok(symbols) = engine.extract_symbols(content, ext_str) {
            if !symbols.is_empty() {
                let mut chunks = Vec::new();
                let mut covered = vec![false; lines.len() + 1];
                let mut flat_symbols = Vec::new();

                for sym in symbols {
                    let span = sym.range.end_line.saturating_sub(sym.range.start_line) + 1;
                    if span > window * 2 && !sym.children.is_empty() {
                        let header_end = (sym.range.start_line + 8).min(sym.range.end_line).min(lines.len());
                        if sym.range.start_line >= 1 && header_end >= sym.range.start_line {
                            let text = lines[sym.range.start_line - 1..header_end].join("\n");
                            chunks.push((sym.range.start_line, header_end, text));
                            for l in sym.range.start_line..=header_end {
                                if l < covered.len() {
                                    covered[l] = true;
                                }
                            }
                        }
                        for child in sym.children {
                            flat_symbols.push(child);
                        }
                    } else {
                        flat_symbols.push(sym);
                    }
                }

                for sym in flat_symbols {
                    let start = sym.range.start_line.clamp(1, lines.len());
                    let end = sym.range.end_line.clamp(start, lines.len());
                    let span = end - start + 1;

                    if span <= window * 2 {
                        let text = lines[start - 1..end].join("\n");
                        if !text.trim().is_empty() {
                            chunks.push((start, end, text));
                            for l in start..=end {
                                if l < covered.len() {
                                    covered[l] = true;
                                }
                            }
                        }
                    } else {
                        let mut i = start - 1;
                        while i < end {
                            let sub_end = (i + window).min(end);
                            let text = lines[i..sub_end].join("\n");
                            if !text.trim().is_empty() {
                                chunks.push((i + 1, sub_end, text));
                            }
                            if sub_end == end {
                                break;
                            }
                            i += step;
                        }
                        for l in start..=end {
                            if l < covered.len() {
                                covered[l] = true;
                            }
                        }
                    }
                }

                // Capture uncovered contiguous blocks (module headers, constants, top-level statements)
                let mut l = 1;
                while l <= lines.len() {
                    if !covered[l] {
                        let b_start = l;
                        while l <= lines.len() && !covered[l] {
                            l += 1;
                        }
                        let b_end = l - 1;
                        if b_end >= b_start {
                            let text = lines[b_start - 1..b_end].join("\n");
                            if text.trim().len() >= 20 {
                                chunks.push((b_start, b_end, text));
                            }
                        }
                    } else {
                        l += 1;
                    }
                }

                chunks.sort_by_key(|c| (c.0, c.1));
                if !chunks.is_empty() {
                    return chunks;
                }
            }
        }
    }

    chunk_file_content(content, chunk_size, overlap)
}

/// Computes a semantic BM25-style relevance score between a query and code text.
pub fn compute_relevance_score(query: &str, text: &str) -> f32 {
    let query_terms = tokenize_identifier_terms(query);
    if query_terms.is_empty() || text.trim().is_empty() {
        return 0.0;
    }
    let idf_map = HashMap::new();
    compute_bm25_corpus_score(&query_terms, query, text, "", 60.0, 1, &idf_map)
}

/// Computes true corpus-wide Okapi BM25 relevance score with real Inverse Document Frequency (IDF):
/// `IDF(t) = ln(1 + (N - df(t) + 0.5) / (df(t) + 0.5))`
/// plus symbol definition and exact-phrase boosts.
pub fn compute_bm25_corpus_score(
    query_terms: &[String],
    raw_query: &str,
    text: &str,
    relative_path: &str,
    avg_doc_len: f32,
    total_docs: usize,
    doc_freq: &HashMap<String, usize>,
) -> f32 {
    if query_terms.is_empty() || text.trim().is_empty() {
        return 0.0;
    }

    let doc_tokens = tokenize_identifier_terms(text);
    if doc_tokens.is_empty() {
        return 0.0;
    }

    let text_lower = text.to_lowercase();
    let path_lower = relative_path.to_lowercase();
    let query_lower = raw_query.trim().to_lowercase();

    let mut tf_map: HashMap<&str, usize> = HashMap::with_capacity(doc_tokens.len());
    for tok in &doc_tokens {
        *tf_map.entry(tok.as_str()).or_insert(0) += 1;
    }

    let doc_len = doc_tokens.len() as f32;
    let k1 = 1.5f32;
    let b = 0.75f32;
    let n_docs = (total_docs.max(1)) as f32;

    let mut score = 0.0f32;
    let mut seen = HashSet::new();

    for term in query_terms {
        if !seen.insert(term.as_str()) {
            continue;
        }
        let tf = (*tf_map.get(term.as_str()).unwrap_or(&0)) as f32;
        let in_path = !path_lower.is_empty() && path_lower.contains(term.as_str());

        if tf > 0.0 || in_path {
            let df = (*doc_freq.get(term.as_str()).unwrap_or(&0)) as f32;
            let idf = if total_docs > 1 && df > 0.0 {
                (1.0 + (n_docs - df + 0.5) / (df + 0.5)).ln().max(0.15)
            } else {
                1.0
            };

            let denom = tf + k1 * (1.0 - b + b * (doc_len / avg_doc_len.max(1.0)));
            if denom > 0.0 && tf > 0.0 {
                score += idf * ((tf * (k1 + 1.0)) / denom);
            }

            if in_path {
                score += 1.25 * idf;
            }

            // Boost if term appears in function, struct, class, trait, or interface signature
            if text_lower.contains(&format!("fn {}", term))
                || text_lower.contains(&format!("function {}", term))
                || text_lower.contains(&format!("struct {}", term))
                || text_lower.contains(&format!("class {}", term))
                || text_lower.contains(&format!("interface {}", term))
                || text_lower.contains(&format!("trait {}", term))
            {
                score += 2.0 * idf;
            }
        }
    }

    if !query_lower.is_empty() && text_lower.contains(&query_lower) {
        score += 3.0;
    }

    score
}

struct CorpusCandidateChunk {
    file_path: String,
    relative_path: String,
    line_start: usize,
    line_end: usize,
    content: String,
}

/// Executes natural language semantic code search across the workspace using AST symbol boundary
/// chunking, IoRing batched file reads, and corpus-wide Okapi BM25 with true IDF.
pub fn search_codebase_semantic(req: &SemanticSearchRequest) -> Result<Vec<SemanticCodeChunk>, String> {
    let root = Path::new(&req.cwd);
    if !root.is_dir() {
        return Err(format!("Workspace directory not found: {}", req.cwd));
    }

    if req.query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let query_terms = tokenize_identifier_terms(&req.query);
    if query_terms.is_empty() {
        return Ok(Vec::new());
    }
    let query_term_set: HashSet<&str> = query_terms.iter().map(|s| s.as_str()).collect();

    let mut files = Vec::new();
    walk_source_files(root, &mut files);

    // Load workspace files in parallel batches via the pure-Rust Kernel IoRing / Rayon batch reader
    let loaded_files = crate::kernel::ioring::IoRingBatchReader::read_files_lossy(&files, 512_000);

    let mut corpus_chunks: Vec<CorpusCandidateChunk> = Vec::new();
    let mut doc_freq: HashMap<String, usize> = HashMap::new();
    let mut total_token_count = 0usize;

    for (file_path, raw_bytes) in loaded_files {
        let content = match String::from_utf8(raw_bytes) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let file_path_str = file_path.to_string_lossy().to_string();
        let relative_path = file_path
            .strip_prefix(root)
            .unwrap_or(&file_path)
            .to_string_lossy()
            .replace('\\', "/");
        let ext = file_path.extension().and_then(|e| e.to_str());

        let chunks = chunk_file_content_ast(&content, ext, 40, 10);
        for (line_start, line_end, chunk_text) in chunks {
            let tokens = tokenize_identifier_terms(&chunk_text);
            if tokens.is_empty() {
                continue;
            }
            total_token_count += tokens.len();
            let unique: HashSet<String> = tokens.into_iter().collect();
            for qt in &query_term_set {
                if unique.contains(*qt) {
                    *doc_freq.entry((*qt).to_string()).or_insert(0) += 1;
                }
            }
            corpus_chunks.push(CorpusCandidateChunk {
                file_path: file_path_str.clone(),
                relative_path: relative_path.clone(),
                line_start,
                line_end,
                content: chunk_text,
            });
        }
    }

    let total_docs = corpus_chunks.len();
    if total_docs == 0 {
        return Ok(Vec::new());
    }
    let avg_doc_len = (total_token_count as f32 / total_docs as f32).max(12.0);

    let top_k = req.top_k.unwrap_or(8);
    let mut scored_chunks: Vec<SemanticCodeChunk> = Vec::new();

    for cand in corpus_chunks {
        let score = compute_bm25_corpus_score(
            &query_terms,
            &req.query,
            &cand.content,
            &cand.relative_path,
            avg_doc_len,
            total_docs,
            &doc_freq,
        );
        if score > 0.3 {
            scored_chunks.push(SemanticCodeChunk {
                file_path: cand.file_path,
                relative_path: cand.relative_path,
                line_start: cand.line_start,
                line_end: cand.line_end,
                content: cand.content,
                score,
            });
        }
    }

    // Sort descending by score
    scored_chunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored_chunks.truncate(top_k);

    Ok(scored_chunks)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom <= 1e-8 { 0.0 } else { (dot / denom).max(0.0) }
}

async fn discover_ollama_embedding_model(client: &reqwest::Client) -> Option<String> {
    let resp = client
        .get("http://127.0.0.1:11434/api/tags")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    let models = json["models"].as_array()?;
    let preferred = ["nomic-embed-text", "mxbai-embed-large", "all-minilm", "snowflake-arctic-embed", "bge-"];
    for pat in preferred {
        for m in models {
            if let Some(name) = m["name"].as_str() {
                if name.to_lowercase().contains(pat) {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

async fn fetch_ollama_embedding(
    client: &reqwest::Client,
    model: &str,
    prompt: &str,
) -> Option<Vec<f32>> {
    let resp = client
        .post("http://127.0.0.1:11434/api/embeddings")
        .json(&serde_json::json!({ "model": model, "prompt": prompt }))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    let arr = json["embedding"].as_array()?;
    let mut vec = Vec::with_capacity(arr.len());
    for v in arr {
        vec.push(v.as_f64()? as f32);
    }
    Some(vec)
}

/// Executes hybrid semantic code search: AST-boundary Okapi BM25 pre-filter
/// followed by real vector embedding cosine reranking via local Ollama when available.
pub async fn search_codebase_semantic_hybrid(
    req: SemanticSearchRequest,
) -> Result<Vec<SemanticCodeChunk>, String> {
    let final_k = req.top_k.unwrap_or(8);
    let candidate_req = SemanticSearchRequest {
        cwd: req.cwd.clone(),
        query: req.query.clone(),
        top_k: Some(final_k.saturating_mul(3).max(16)),
    };

    let mut candidates = tokio::task::spawn_blocking(move || search_codebase_semantic(&candidate_req))
        .await
        .map_err(|e| format!("Semantic search task panicked: {e}"))??;

    if candidates.is_empty() {
        return Ok(candidates);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();
    if let Ok(client) = client {
        if let Some(embed_model) = discover_ollama_embedding_model(&client).await {
            if let Some(q_vec) = fetch_ollama_embedding(&client, &embed_model, &req.query).await {
                let max_lex = candidates
                    .iter()
                    .map(|c| c.score)
                    .fold(1.0f32, f32::max);
                for chunk in &mut candidates {
                    let snippet: String = chunk.content.chars().take(1500).collect();
                    let doc_input = format!("// {}\n{}", chunk.relative_path, snippet);
                    if let Some(d_vec) = fetch_ollama_embedding(&client, &embed_model, &doc_input).await {
                        let sim = cosine_similarity(&q_vec, &d_vec);
                        let lex_norm = (chunk.score / max_lex).clamp(0.0, 1.0);
                        chunk.score = (0.45 * lex_norm + 0.55 * sim) * 10.0;
                    }
                }
                candidates.sort_by(|a, b| {
                    b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
    }

    candidates.truncate(final_k);
    Ok(candidates)
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

/// Queries the semantic index and formats top AST chunks into a RAG context block.
pub fn build_semantic_rag_context(cwd: &str, query: &str, top_k: usize) -> String {
    let req = SemanticSearchRequest {
        cwd: cwd.to_string(),
        query: query.to_string(),
        top_k: Some(top_k),
    };
    match search_codebase_semantic(&req) {
        Ok(chunks) => format_semantic_rag_context(&chunks),
        Err(_) => String::new(),
    }
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
    fn test_chunk_file_content_ast_respects_symbol_boundaries() {
        let ts_src = "export function validateToken(jwt: string): boolean {\n  const parts = jwt.split('.');\n  return parts.length === 3;\n}\n\nexport function revokeSession(sessionId: string): void {\n  console.log(sessionId);\n}\n";
        let chunks = chunk_file_content_ast(ts_src, Some("ts"), 40, 10);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().any(|(s, e, txt)| *s == 1 && *e == 4 && txt.contains("validateToken")));
        assert!(chunks.iter().any(|(s, e, txt)| *s == 6 && *e == 8 && txt.contains("revokeSession")));
    }

    #[test]
    fn test_compute_relevance_score_and_corpus_idf() {
        let text = "pub fn authenticate_user(token: &str) -> Result<User, AuthError> {\n    // verify jwt\n}";
        let score = compute_relevance_score("authenticate user jwt", text);
        assert!(score > 1.0);

        let irrelevant = "let x = 42;";
        let zero_score = compute_relevance_score("authenticate user", irrelevant);
        assert_eq!(zero_score, 0.0);

        // Verify rare term gets higher IDF score than ubiquitous term
        let query_terms = vec!["raretoken".to_string(), "common".to_string()];
        let mut df = HashMap::new();
        df.insert("raretoken".to_string(), 1);
        df.insert("common".to_string(), 95);
        let rare_doc = "fn helper() { let raretoken = 1; }";
        let common_doc = "fn helper() { let common = 1; }";
        let rare_score = compute_bm25_corpus_score(&query_terms, "raretoken common", rare_doc, "a.rs", 20.0, 100, &df);
        let common_score = compute_bm25_corpus_score(&query_terms, "raretoken common", common_doc, "b.rs", 20.0, 100, &df);
        assert!(rare_score > common_score);
    }
}
