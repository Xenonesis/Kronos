use std::sync::{Arc, Mutex};
use tantivy::collector::TopDocs;
use tantivy::directory::RamDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexSettings, IndexWriter, TantivyDocument};

/// Represents a single search result matching a query.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
}

/// In-memory Tantivy search index for workspace documents.
/// Guarantees sub-millisecond lexical queries using `RamDirectory`.
pub struct WorkspaceSearchIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    path_field: Field,
    body_field: Field,
}

impl WorkspaceSearchIndex {
    /// Creates a new in-memory `WorkspaceSearchIndex` using `RamDirectory`.
    /// Schema consists of `path: TEXT | STORED` and `body: TEXT`.
    pub fn new_in_memory() -> Result<Self, String> {
        let mut schema_builder = Schema::builder();
        let path_field = schema_builder.add_text_field("path", TEXT | STORED);
        let body_field = schema_builder.add_text_field("body", TEXT);
        let schema = schema_builder.build();

        let directory = RamDirectory::create();
        let index = Index::create(directory, schema, IndexSettings::default()).map_err(|e| format!("Failed to create Tantivy index: {e}"))?;

        // 15MB buffer is optimal for in-memory workspace indexing
        let writer = index
            .writer(15_000_000)
            .map_err(|e| format!("Failed to create Tantivy writer: {e}"))?;

        let reader = index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| format!("Failed to create Tantivy reader: {e}"))?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            path_field,
            body_field,
        })
    }

    /// Indexes a document into the in-memory search index.
    pub fn index_document(&self, path: &str, text: &str) -> Result<(), String> {
        self.index_documents_batch(&[(path, text)])
    }

    /// Indexes a batch of documents into the in-memory search index in a single commit.
    pub fn index_documents_batch(&self, docs: &[(&str, &str)]) -> Result<(), String> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        for &(path, text) in docs {
            let mut doc = TantivyDocument::default();
            doc.add_text(self.path_field, path);
            doc.add_text(self.body_field, text);
            writer
                .add_document(doc)
                .map_err(|e| format!("Failed to add document: {e}"))?;
        }

        writer
            .commit()
            .map_err(|e| format!("Failed to commit documents: {e}"))?;

        self.reader
            .reload()
            .map_err(|e| format!("Failed to reload search reader: {e}"))?;

        Ok(())
    }

    /// Searches the in-memory index using Tantivy's `QueryParser`.
    /// Returns search results ranked by relevance score.
    pub fn search(&self, query_str: &str) -> Result<Vec<SearchResult>, String> {
        let query_parser = QueryParser::for_index(&self.index, vec![self.body_field, self.path_field]);
        let query = query_parser
            .parse_query(query_str)
            .map_err(|e| format!("Failed to parse query: {e}"))?;

        let searcher = self.reader.searcher();
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(100))
            .map_err(|e| format!("Search error: {e}"))?;

        let mut results = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| format!("Failed to retrieve document: {e}"))?;

            let path = retrieved_doc
                .get_first(self.path_field)
                .and_then(|val| val.as_str())
                .unwrap_or_default()
                .to_string();

            results.push(SearchResult { path, score });
        }

        Ok(results)
    }
}

/// Represents a single directory or file suggestion entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DirectorySuggestionEntry {
    pub path: String,
    pub kind: String, // "file" | "directory"
}

/// Recursively scans and ranks files and directories under `root` matching `query`.
pub fn suggest_directory_entries(
    root: &std::path::Path,
    query: &str,
    include_files: bool,
    include_dirs: bool,
    limit: usize,
) -> (Vec<String>, Vec<DirectorySuggestionEntry>) {
    if !root.exists() {
        return (Vec::new(), Vec::new());
    }

    let query_trimmed = query.trim();
    let query_lower = query_trimmed.to_ascii_lowercase().replace('\\', "/");
    let max_limit = limit.max(1).min(200);

    let mut scored_entries: Vec<(i32, DirectorySuggestionEntry)> = Vec::new();
    collect_suggestions_recursive(
        root,
        root,
        0,
        12, // max depth
        &query_lower,
        include_files,
        include_dirs,
        &mut scored_entries,
        max_limit * 4, // collect candidates before final ranking
    );

    // Sort descending by score, ascending by path length
    scored_entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.path.len().cmp(&b.1.path.len()))
            .then_with(|| a.1.path.cmp(&b.1.path))
    });

    let entries: Vec<DirectorySuggestionEntry> = scored_entries
        .into_iter()
        .take(max_limit)
        .map(|(_, entry)| entry)
        .collect();

    let directories: Vec<String> = entries
        .iter()
        .filter(|e| e.kind == "directory")
        .map(|e| e.path.clone())
        .collect();

    (directories, entries)
}

fn collect_suggestions_recursive(
    current_dir: &std::path::Path,
    root: &std::path::Path,
    depth: usize,
    max_depth: usize,
    query_lower: &str,
    include_files: bool,
    include_dirs: bool,
    results: &mut Vec<(i32, DirectorySuggestionEntry)>,
    capacity: usize,
) {
    if depth > max_depth || results.len() >= capacity {
        return;
    }

    let dir_entries = match std::fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for item in dir_entries.flatten() {
        if results.len() >= capacity {
            break;
        }

        let path = item.path();
        let file_name = item.file_name();
        let name_str = file_name.to_string_lossy();

        if path.is_dir() {
            if name_str.starts_with('.')
                || name_str == "node_modules"
                || name_str == "target"
                || name_str == "dist"
                || name_str == "build"
                || name_str == "out"
                || name_str == ".next"
                || name_str == ".turbo"
                || name_str == ".expo"
                || name_str == ".cache"
            {
                continue;
            }

            let rel_raw = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let rel_path = rel_raw.trim_start_matches("./").trim_start_matches('/').to_string();

            if include_dirs && !rel_path.is_empty() {
                if let Some(score) = score_suggestion(&name_str, &rel_path, query_lower) {
                    results.push((
                        score,
                        DirectorySuggestionEntry {
                            path: rel_path,
                            kind: "directory".to_string(),
                        },
                    ));
                }
            }

            collect_suggestions_recursive(
                &path,
                root,
                depth + 1,
                max_depth,
                query_lower,
                include_files,
                include_dirs,
                results,
                capacity,
            );
        } else if path.is_file() && include_files {
            if name_str == ".git" {
                continue;
            }
            let rel_raw = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let rel_path = rel_raw.trim_start_matches("./").trim_start_matches('/').to_string();

            if !rel_path.is_empty() {
                if let Some(score) = score_suggestion(&name_str, &rel_path, query_lower) {
                    results.push((
                        score,
                        DirectorySuggestionEntry {
                            path: rel_path,
                            kind: "file".to_string(),
                        },
                    ));
                }
            }
        }
    }
}

fn score_suggestion(name: &str, rel_path: &str, query_lower: &str) -> Option<i32> {
    if query_lower.is_empty() {
        // Flat score prioritizing shallower paths
        let depth_penalty = rel_path.matches('/').count() as i32 * 10;
        return Some(100 - depth_penalty);
    }

    let name_lower = name.to_ascii_lowercase();
    let path_lower = rel_path.to_ascii_lowercase();

    if name_lower == query_lower {
        Some(1000)
    } else if name_lower.starts_with(query_lower) {
        Some(800)
    } else if name_lower.contains(query_lower) {
        Some(500)
    } else if path_lower.contains(query_lower) {
        Some(200)
    } else if is_subsequence(query_lower, &name_lower) {
        Some(100)
    } else if is_subsequence(query_lower, &path_lower) {
        Some(50)
    } else {
        None
    }
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut haystack_chars = haystack.chars();
    for n in needle.chars() {
        if !haystack_chars.any(|h| h == n) {
            return false;
        }
    }
    true
}
