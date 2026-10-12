pub mod error;
pub mod extract;
pub mod models;
pub mod scanner;
pub mod storage;
pub mod tantivy_index;
pub mod watcher;

pub use error::{Result, SiftError};
pub use extract::{extract_document_chunks, DocumentChunk};
pub use models::*;
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
}
