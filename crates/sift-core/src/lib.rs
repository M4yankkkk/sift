pub mod embeddings;
pub mod error;
pub mod extract;
pub mod models;
pub mod ocr;
pub mod ranking;
pub mod scanner;
pub mod storage;
pub mod tantivy_index;
pub mod watcher;

pub use embeddings::{
    cosine_similarity, create_default_embedding_engine, deserialize_vector,
    is_image_supported_for_vision, normalize_vector, serialize_vector, DeterministicMockEngine,
    EmbeddingEngine, FastEmbedEngine,
};
pub use error::{Result, SiftError};
pub use extract::{extract_document_chunks, DocumentChunk};
pub use models::*;
pub use ocr::{extract_image_ocr, is_ocr_available, is_ocr_supported_extension};
pub use scanner::{
    extract_pdf_page_preview, extract_pdf_preview, read_file_metadata, read_file_preview,
    render_pdf_first_page, render_pdf_page, scan_directory, FileMeta, PathRuleChecker,
};
pub use storage::Database;
pub use tantivy_index::{FullTextHit, TantivySearchIndex};
pub use watcher::FileWatcher;

/// Initializes structured logging using `tracing-subscriber`
pub fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("sift_core=info,sift_tauri=info,warn"));

    let _ = fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_logging_and_init() {
        init_logging();
        tracing::info!("Core initialized successfully");
    }

    #[test]
    fn test_create_engine() {
        let engine = create_default_embedding_engine();
        assert!(engine.dimension() > 0);
        assert!(!engine.model_name().is_empty());
        let vec = engine.embed_text("sunset").unwrap();
        assert_eq!(vec.len(), engine.dimension());
    }
}

