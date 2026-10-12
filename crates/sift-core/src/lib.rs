pub mod error;
pub mod models;
pub mod scanner;
pub mod storage;

pub use error::{Result, SiftError};
pub use models::*;
pub use scanner::{
    read_file_metadata, read_file_preview, scan_directory, FileMeta, PathRuleChecker,
};
pub use storage::Database;

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
