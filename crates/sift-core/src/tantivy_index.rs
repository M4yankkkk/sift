use crate::error::{Result, SiftError};
use crate::extract::DocumentChunk;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, Term};

/// In any-term fallback mode, hits scoring below this fraction of the best hit are dropped.
const ANY_TERM_RELATIVE_FLOOR: f32 = 0.4;

#[derive(Clone)]
pub struct TantivySchemaFields {
    pub file_id: Field,
    pub path: Field,
    pub filename: Field,
    pub chunk_index: Field,
    pub page_number: Field,
    pub line_number: Field,
    pub content: Field,
    pub modality: Field,
}

#[derive(Debug, Clone)]
pub struct FullTextHit {
    pub file_id: String,
    pub path: String,
    pub filename: String,
    pub chunk_index: u32,
    pub page_number: Option<i32>,
    pub line_number: Option<i32>,
    pub snippet: String,
    pub score: f32,
    pub modality: String,
}

pub struct TantivySearchIndex {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    fields: TantivySchemaFields,
}

impl TantivySearchIndex {
    fn build_schema() -> (Schema, TantivySchemaFields) {
        let mut schema_builder = Schema::builder();

        // file_id: exact untokenized string for deletion and mapping
        let file_id = schema_builder.add_text_field("file_id", STRING | STORED);
        // path: exact untokenized path
        let path = schema_builder.add_text_field("path", STRING | STORED);
        // filename: tokenized text for full-text search
        let filename = schema_builder.add_text_field("filename", TEXT | STORED);
        // chunk_index, page_number, line_number: numeric fields
        let chunk_index = schema_builder.add_u64_field("chunk_index", INDEXED | STORED);
        let page_number = schema_builder.add_i64_field("page_number", INDEXED | STORED);
        let line_number = schema_builder.add_i64_field("line_number", INDEXED | STORED);
        // content: full text body, indexed and stored for snippet generation
        let content = schema_builder.add_text_field("content", TEXT | STORED);
        // modality: e.g. "code", "pdf", "markdown", "text"
        let modality = schema_builder.add_text_field("modality", STRING | STORED);

        let schema = schema_builder.build();
        let fields = TantivySchemaFields {
            file_id,
            path,
            filename,
            chunk_index,
            page_number,
            line_number,
            content,
            modality,
        };

        (schema, fields)
    }

    /// Opens or creates a Tantivy index in the given directory.
    pub fn open_or_create(index_path: &Path) -> Result<Self> {
        let (schema, fields) = Self::build_schema();

        let index = if index_path.exists() && index_path.join("meta.json").exists() {
            Index::open_in_dir(index_path).map_err(|e| SiftError::Storage(e.to_string()))?
        } else {
            std::fs::create_dir_all(index_path).map_err(SiftError::Io)?;
            Index::create_in_dir(index_path, schema)
                .map_err(|e| SiftError::Storage(e.to_string()))?
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| SiftError::Storage(e.to_string()))?;

        // 30MB writer budget keeps memory well under 100MB idle/active target
        let writer = index
            .writer(30_000_000)
            .map_err(|e| SiftError::Storage(e.to_string()))?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            fields,
        })
    }

    /// Creates an in-memory Tantivy index for unit testing.
    pub fn create_in_ram() -> Result<Self> {
        let (schema, fields) = Self::build_schema();
        let index = Index::create_in_ram(schema);

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| SiftError::Storage(e.to_string()))?;

        let writer = index
            .writer(15_000_000)
            .map_err(|e| SiftError::Storage(e.to_string()))?;

        Ok(Self {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            fields,
        })
    }

    /// Atomically removes existing chunks for a file and adds the new chunks.
    pub fn upsert_file_chunks(
        &self,
        file_id: &str,
        path: &str,
        filename: &str,
        modality: &str,
        chunks: &[DocumentChunk],
    ) -> Result<()> {
        let writer = self.writer.lock().map_err(|_| {
            SiftError::Internal("Failed to acquire Tantivy index writer lock".into())
        })?;

        // Delete any existing documents for this file_id
        let file_id_term = Term::from_field_text(self.fields.file_id, file_id);
        writer.delete_term(file_id_term);

        for chunk in chunks {
            let mut doc = tantivy::TantivyDocument::default();
            doc.add_text(self.fields.file_id, file_id);
            doc.add_text(self.fields.path, path);
            doc.add_text(self.fields.filename, filename);
            doc.add_u64(self.fields.chunk_index, chunk.chunk_index as u64);
            if let Some(page) = chunk.page_number {
                doc.add_i64(self.fields.page_number, page as i64);
            }
            if let Some(line) = chunk.line_number {
                doc.add_i64(self.fields.line_number, line as i64);
            }
            doc.add_text(self.fields.content, &chunk.content);
            doc.add_text(self.fields.modality, modality);

            writer
                .add_document(doc)
                .map_err(|e| SiftError::Storage(e.to_string()))?;
        }

        Ok(())
    }

    /// Deletes all documents for a specific file_id.
    pub fn delete_file(&self, file_id: &str) -> Result<()> {
        let writer = self.writer.lock().map_err(|_| {
            SiftError::Internal("Failed to acquire Tantivy index writer lock".into())
        })?;

        let term = Term::from_field_text(self.fields.file_id, file_id);
        writer.delete_term(term);
        Ok(())
    }

    /// Deletes all documents for a specific path.
    pub fn delete_path(&self, path: &str) -> Result<()> {
        let writer = self.writer.lock().map_err(|_| {
            SiftError::Internal("Failed to acquire Tantivy index writer lock".into())
        })?;

        let term = Term::from_field_text(self.fields.path, path);
        writer.delete_term(term);
        Ok(())
    }

    /// Deletes all documents from the Tantivy index.
    pub fn clear(&self) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|_| {
            SiftError::Internal("Failed to acquire Tantivy index writer lock".into())
        })?;
        writer
            .delete_all_documents()
            .map_err(|e| SiftError::Storage(e.to_string()))?;
        writer
            .commit()
            .map_err(|e| SiftError::Storage(e.to_string()))?;
        self.reader
            .reload()
            .map_err(|e| SiftError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Commits staged index operations to make them visible to searchers.
    pub fn commit(&self) -> Result<()> {
        let mut writer = self.writer.lock().map_err(|_| {
            SiftError::Internal("Failed to acquire Tantivy index writer lock".into())
        })?;

        writer
            .commit()
            .map_err(|e| SiftError::Storage(e.to_string()))?;
        self.reader
            .reload()
            .map_err(|e| SiftError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Searches Tantivy full-text index with BM25 scoring and snippet generation.
    ///
    /// Every query term must match by default. If that finds nothing, falls back to matching any
    /// term, keeping only hits within reach of the best one so a single common word
    /// ("how", "to") cannot pull in the whole corpus.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<FullTextHit>> {
        let all_terms = self.search_with_conjunction(query_str, limit, true)?;
        if !all_terms.is_empty() {
            return Ok(all_terms);
        }
        let mut any_term = self.search_with_conjunction(query_str, limit, false)?;
        if let Some(top) = any_term.first().map(|h| h.score) {
            any_term.retain(|h| h.score >= top * ANY_TERM_RELATIVE_FLOOR);
        }
        Ok(any_term)
    }

    fn search_with_conjunction(
        &self,
        query_str: &str,
        limit: usize,
        conjunction: bool,
    ) -> Result<Vec<FullTextHit>> {
        let trimmed = query_str.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let searcher = self.reader.searcher();
        let mut query_parser =
            QueryParser::for_index(&self.index, vec![self.fields.filename, self.fields.content]);
        if conjunction {
            query_parser.set_conjunction_by_default();
        }

        // Parse query; fallback to escaped phrase if user typed special syntax
        let query = match query_parser.parse_query(trimmed) {
            Ok(q) => q,
            Err(_) => {
                // If query parser fails (e.g. unclosed quotes or special characters), escape or build phrase
                let escaped = trimmed.replace([':', '\\'], " ");
                query_parser
                    .parse_query(&escaped)
                    .map_err(|e| SiftError::Storage(e.to_string()))?
            }
        };

        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| SiftError::Storage(e.to_string()))?;

        let mut snippet_generator =
            SnippetGenerator::create(&searcher, &*query, self.fields.content)
                .map_err(|e| SiftError::Storage(e.to_string()))?;
        snippet_generator.set_max_num_chars(160);

        let mut hits = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| SiftError::Storage(e.to_string()))?;

            let file_id = doc
                .get_first(self.fields.file_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let path = doc
                .get_first(self.fields.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let filename = doc
                .get_first(self.fields.filename)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let chunk_index = doc
                .get_first(self.fields.chunk_index)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;

            let page_number = doc
                .get_first(self.fields.page_number)
                .and_then(|v| v.as_i64())
                .map(|p| p as i32);

            let line_number = doc
                .get_first(self.fields.line_number)
                .and_then(|v| v.as_i64())
                .map(|l| l as i32);

            let snippet_obj = snippet_generator.snippet_from_doc(&doc);
            let snippet_html = snippet_obj.to_html();
            let snippet = if !snippet_html.is_empty() {
                snippet_html
            } else {
                // Fallback to first line of content
                doc.get_first(self.fields.content)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(120)
                    .collect()
            };

            let modality = doc
                .get_first(self.fields.modality)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            hits.push(FullTextHit {
                file_id,
                path,
                filename,
                chunk_index,
                page_number,
                line_number,
                snippet,
                score,
                modality,
            });
        }

        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tantivy_in_ram_index_and_search() {
        let index = TantivySearchIndex::create_in_ram().expect("create in ram");

        let chunks = vec![
            DocumentChunk {
                chunk_index: 0,
                page_number: None,
                line_number: Some(1),
                content: "Sift is an offline desktop search application built for Ubuntu Linux."
                    .to_string(),
            },
            DocumentChunk {
                chunk_index: 1,
                page_number: None,
                line_number: Some(15),
                content: "It uses a Tantivy inverted index for fast keyword BM25 retrieval."
                    .to_string(),
            },
        ];

        index
            .upsert_file_chunks(
                "file-uuid-1",
                "/docs/architecture.md",
                "architecture.md",
                "markdown",
                &chunks,
            )
            .expect("upsert chunks");

        index.commit().expect("commit");

        // Test search matching second chunk
        let hits = index.search("Tantivy inverted index", 10).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file_id, "file-uuid-1");
        assert_eq!(hits[0].line_number, Some(15));
        assert!(hits[0].snippet.to_lowercase().contains("tantivy"));

        // Test deletion
        index.delete_file("file-uuid-1").expect("delete file");
        index.commit().expect("commit");

        let hits_after = index.search("Tantivy", 10).expect("search after delete");
        assert_eq!(hits_after.len(), 0);
    }

    #[test]
    fn test_tantivy_pdf_page_provenance() {
        let index = TantivySearchIndex::create_in_ram().expect("create in ram");

        let pdf_chunks = vec![
            DocumentChunk {
                chunk_index: 0,
                page_number: Some(1),
                line_number: None,
                content: "Introduction to local storage and SQLite WAL catalog.".to_string(),
            },
            DocumentChunk {
                chunk_index: 1,
                page_number: Some(2),
                line_number: None,
                content:
                    "Section 4: Hyper-quantum entanglement vectors and sublinear graph search."
                        .to_string(),
            },
        ];

        index
            .upsert_file_chunks(
                "pdf-uuid-42",
                "/docs/research.pdf",
                "research.pdf",
                "pdf",
                &pdf_chunks,
            )
            .expect("upsert pdf");
        index.commit().expect("commit");

        // Search for phrase on page 2
        let hits = index
            .search("Hyper-quantum entanglement", 5)
            .expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_number, Some(2));
        assert_eq!(hits[0].file_id, "pdf-uuid-42");
    }
}
