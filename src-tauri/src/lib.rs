pub mod app;
pub mod commands;
pub mod ipc;
pub mod resources;
pub mod tray;

use app::AppState;
use commands::*;
use sift_core::storage::Database;

pub use ipc::send_toggle_signal;

pub fn run() {
    sift_core::init_logging();
    tracing::info!("Starting Sift desktop application...");

    // Determine database path in user data directory or local fallback
    let db_path = std::env::var("SIFT_DB_PATH").unwrap_or_else(|_| {
        if std::path::Path::new("sift.db").exists() {
            "sift.db".to_string()
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let sift_data_dir = std::path::PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("sift");
            let _ = std::fs::create_dir_all(&sift_data_dir);
            sift_data_dir.join("sift.db").to_string_lossy().to_string()
        }
    });
    let embedding_engine = sift_core::create_default_embedding_engine();
    let db = Database::open(&db_path)
        .expect("Failed to initialize SQLite database")
        .with_embedding_engine(Some(embedding_engine));

    let app_state = AppState::new(db);
    let initial_shortcut = app_state
        .db
        .get_settings()
        .map(|s| s.global_shortcut)
        .unwrap_or_else(|_| "Ctrl+Alt+Space".to_string());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state() == ShortcutState::Pressed {
                        tracing::info!("Global shortcut triggered: {:?}", shortcut);
                        ipc::toggle_main_window(app);
                    }
                })
                .build(),
        )
        .manage(app_state)
        .setup(move |app| {
            // Start Unix domain socket listener for instant desktop toggling (like Ulauncher)
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                ipc::start_ipc_server(handle);
            });

            // Synchronize GNOME keybinding for Wayland support
            ipc::sync_gnome_shortcut(&initial_shortcut);

            // Setup system tray icon with menu (Open Sift, Preferences, Quit)
            if let Err(e) = tray::setup_tray(app) {
                tracing::warn!("Failed to setup system tray: {}", e);
            }

            // Register fallback in-process global shortcut
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let registered = app.global_shortcut().register(initial_shortcut.as_str());
            if let Err(e) = registered {
                tracing::warn!(
                    "Failed to register initial global shortcut '{}': {}",
                    initial_shortcut,
                    e
                );
            } else {
                tracing::info!(
                    "Successfully registered global shortcut '{}'",
                    initial_shortcut
                );
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
            open_in_terminal,
            get_settings,
            set_settings,
            hide_window,
            get_resource_usage,
            get_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sift tauri application");
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn test_shortcut_parsing() {
        assert!(Shortcut::from_str("Alt+Shift+S").is_ok());
        assert!(Shortcut::from_str("Ctrl+Alt+Space").is_ok());
        assert!(Shortcut::from_str("Ctrl+Alt+S").is_ok());
        assert!(Shortcut::from_str("Alt+Shift+Space").is_ok());
        assert!(Shortcut::from_str("Ctrl+Shift+Space").is_ok());
    }
}
