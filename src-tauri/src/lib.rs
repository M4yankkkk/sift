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
use tauri::Manager;


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
    let initial_shortcut = app_state
        .db
        .get_settings()
        .map(|s| s.global_shortcut)
        .unwrap_or_else(|_| "Alt+Space".to_string());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state() == ShortcutState::Pressed {
                        tracing::info!("Global shortcut triggered: {:?}", shortcut);
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            let is_focused = window.is_focused().unwrap_or(false);
                            if is_visible && is_focused {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(),
        )
        .manage(app_state)
        .setup(move |app| {
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let registered = app.global_shortcut().register(initial_shortcut.as_str());
            if let Err(e) = registered {
                tracing::warn!("Failed to register initial global shortcut '{}': {}", initial_shortcut, e);
            } else {
                tracing::info!("Successfully registered global shortcut '{}'", initial_shortcut);
            }
            Ok(())
        })
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
            set_settings,
            hide_window
        ])

        .run(tauri::generate_context!())
        .expect("error while running Sift tauri application");
}

