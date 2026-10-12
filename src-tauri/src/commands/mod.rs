use crate::app::AppState;
use chrono::Utc;
use sift_core::error::{Result, SiftError};
use sift_core::models::{
    AppSettings, IndexStatus, PreviewResponse, RootFolder, SearchQuery, SearchResponse,
};
use sift_core::storage::Database;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::State;

fn spawn_scan_worker(
    db: Arc<Database>,
    is_scanning: Arc<AtomicBool>,
    current_op: Arc<Mutex<Option<String>>>,
    last_scan: Arc<Mutex<Option<chrono::DateTime<Utc>>>>,
) {
    if is_scanning.swap(true, Ordering::SeqCst) {
        // Already scanning
        return;
    }

    std::thread::spawn(move || {
        let roots = match db.list_roots() {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to list roots for scan: {}", e);
                is_scanning.store(false, Ordering::SeqCst);
                return;
            }
        };

        let exclusions = db
            .get_settings()
            .map(|s| s.default_exclusions)
            .unwrap_or_default();

        let mut total_indexed = 0;

        for root in roots {
            let root_path = Path::new(&root.path);
            if !root_path.exists() {
                continue;
            }

            {
                let mut op = current_op.lock().unwrap();
                *op = Some(format!("Scanning {}", root.path));
            }

            tracing::info!("Scanning root: {}", root.path);
            match sift_core::scan_directory(root_path, &exclusions) {
                Ok(files) => {
                    let count = files.len();
                    if let Err(e) = db.upsert_files_batch(&root.id, &files) {
                        tracing::error!("Failed to upsert files for root {}: {}", root.path, e);
                    } else {
                        total_indexed += count;
                    }
                }
                Err(e) => {
                    tracing::warn!("Error scanning directory {}: {}", root.path, e);
                }
            }
        }

        tracing::info!("Scan completed: indexed {} files", total_indexed);
        *last_scan.lock().unwrap() = Some(Utc::now());
        *current_op.lock().unwrap() = None;
        is_scanning.store(false, Ordering::SeqCst);
    });
}

#[tauri::command]
pub async fn list_roots(state: State<'_, AppState>) -> Result<Vec<RootFolder>> {
    state.db.list_roots()
}

#[tauri::command]
pub async fn add_root(path: String, state: State<'_, AppState>) -> Result<RootFolder> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(SiftError::PathNotFound(path));
    }
    if !p.is_dir() {
        return Err(SiftError::NotADirectory(path));
    }

    let root = state.db.add_root(&path)?;

    // Trigger asynchronous background scan for the new root
    spawn_scan_worker(
        state.db.clone(),
        state.is_scanning.clone(),
        state.current_operation.clone(),
        state.last_scan_completed_at.clone(),
    );

    Ok(root)
}

#[tauri::command]
pub async fn remove_root(path: String, state: State<'_, AppState>) -> Result<bool> {
    state.db.remove_root(&path)
}

#[tauri::command]
pub async fn get_index_status(state: State<'_, AppState>) -> Result<IndexStatus> {
    Ok(state.get_status())
}

#[tauri::command]
pub async fn start_scan(state: State<'_, AppState>) -> Result<IndexStatus> {
    spawn_scan_worker(
        state.db.clone(),
        state.is_scanning.clone(),
        state.current_operation.clone(),
        state.last_scan_completed_at.clone(),
    );
    Ok(state.get_status())
}

#[tauri::command]
pub async fn search_files(
    query: SearchQuery,
    state: State<'_, AppState>,
) -> Result<SearchResponse> {
    let start = Instant::now();
    let limit = query.limit.unwrap_or(50);

    let results = state.db.search_filenames(&query.query, limit)?;
    let total_matches = results.len();
    let execution_time_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(SearchResponse {
        results,
        total_matches,
        execution_time_ms,
        query: query.query,
    })
}

#[tauri::command]
pub async fn preview_result(
    file_id: String,
    state: State<'_, AppState>,
) -> Result<PreviewResponse> {
    let file = state
        .db
        .get_file_by_id(&file_id)?
        .ok_or_else(|| SiftError::PathNotFound(format!("File with id {} not found", file_id)))?;

    let path = Path::new(&file.path);
    let preview_content = sift_core::read_file_preview(path, 65536)
        .unwrap_or_else(|e| format!("[Preview error: {}]", e));

    let ext = file.extension.to_lowercase();
    let mime_type = match ext.as_str() {
        "md" => "text/markdown",
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "c" | "cpp" | "h" | "hpp" | "go" | "java"
        | "html" | "css" | "scss" | "json" | "toml" | "yaml" | "yml" | "sh" | "bash" | "sql"
        | "xml" => "text/code",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "txt" | "log" | "csv" | "tsv" => "text/plain",
        _ => "application/octet-stream",
    };

    Ok(PreviewResponse {
        id: file.id,
        path: file.path,
        filename: file.filename,
        mime_type: mime_type.to_string(),
        size_bytes: file.size_bytes,
        preview_content,
        snippet: None,
        page_number: None,
    })
}

#[tauri::command]
pub async fn open_result(path: String) -> Result<()> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(SiftError::PathNotFound(path));
    }

    // Trusted OS open via xdg-open on Ubuntu Linux
    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(SiftError::Io)?;

    Ok(())
}

#[tauri::command]
pub async fn reveal_result(path: String) -> Result<()> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(SiftError::PathNotFound(path));
    }

    // Trusted reveal: open parent folder via xdg-open on Ubuntu Linux
    let parent = p.parent().unwrap_or(p);
    Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(SiftError::Io)?;

    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings> {
    state.db.get_settings()
}

#[tauri::command]
pub async fn set_settings(
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<AppSettings> {
    state.db.save_settings(&settings)?;
    Ok(settings)
}
