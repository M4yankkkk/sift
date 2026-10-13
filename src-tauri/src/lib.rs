pub mod app;
pub mod commands;
pub mod embeddings;
pub mod extract;
pub mod indexing;
pub mod jobs;
pub mod scanner;
pub mod search;
pub mod storage;
pub mod watcher;

use app::AppState;
use commands::*;
use sift_core::storage::Database;

pub fn run() {
    sift_core::init_logging();
    tracing::info!("Starting Sift desktop application...");

    // Determine database path in user data directory or local fallback
    let db_path = std::env::var("SIFT_DB_PATH").unwrap_or_else(|_| "sift.db".to_string());
    let embedding_engine = sift_core::create_default_embedding_engine();
    let db = Database::open(&db_path)
        .expect("Failed to initialize SQLite database")
        .with_embedding_engine(Some(embedding_engine));

    let app_state = AppState::new(db);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            list_roots,
            add_root,
            remove_root,
            get_index_status,
            start_scan,
            reindex_all,
            search_files,
            preview_result,
            open_result,
            reveal_result,
            get_settings,
            set_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sift tauri application");
}
