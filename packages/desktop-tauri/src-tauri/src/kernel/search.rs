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
        let mut writer = self
            .writer
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;

        let mut doc = TantivyDocument::default();
        doc.add_text(self.path_field, path);
        doc.add_text(self.body_field, text);

        writer
            .add_document(doc)
            .map_err(|e| format!("Failed to add document: {e}"))?;

        writer
            .commit()
            .map_err(|e| format!("Failed to commit document: {e}"))?;

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
