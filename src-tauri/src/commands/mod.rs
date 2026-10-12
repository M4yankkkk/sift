use crate::app::AppState;
use sift_core::error::{Result, SiftError};
use sift_core::models::{
    AppSettings, IndexStatus, PreviewResponse, RootFolder, SearchQuery, SearchResponse,
};
use std::process::Command;
use tauri::State;

#[tauri::command]
pub async fn list_roots(state: State<'_, AppState>) -> Result<Vec<RootFolder>> {
    state.db.list_roots()
}

#[tauri::command]
pub async fn add_root(path: String, state: State<'_, AppState>) -> Result<RootFolder> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(SiftError::PathNotFound(path));
    }
    if !p.is_dir() {
        return Err(SiftError::NotADirectory(path));
    }
    state.db.add_root(&path)
}

#[tauri::command]
pub async fn remove_root(path: String, state: State<'_, AppState>) -> Result<bool> {
    state.db.remove_root(&path)
}

#[tauri::command]
pub async fn get_index_status(_state: State<'_, AppState>) -> Result<IndexStatus> {
    // Baseline status for Phase 0
    Ok(IndexStatus {
        total_files_discovered: 0,
        files_indexed: 0,
        queue_length: 0,
        is_scanning: false,
        is_indexing: false,
        last_scan_completed_at: None,
        current_operation: Some("Ready".to_string()),
    })
}

#[tauri::command]
pub async fn start_scan(state: State<'_, AppState>) -> Result<IndexStatus> {
    get_index_status(state).await
}

#[tauri::command]
pub async fn search_files(
    query: SearchQuery,
    _state: State<'_, AppState>,
) -> Result<SearchResponse> {
    // Foundation response: searches will be implemented in Phase 1
    Ok(SearchResponse {
        results: Vec::new(),
        total_matches: 0,
        execution_time_ms: 0.0,
        query: query.query,
    })
}

#[tauri::command]
pub async fn preview_result(
    file_id: String,
    _state: State<'_, AppState>,
) -> Result<PreviewResponse> {
    Ok(PreviewResponse {
        id: file_id,
        path: String::new(),
        filename: String::new(),
        mime_type: "text/plain".to_string(),
        size_bytes: 0,
        preview_content: String::new(),
        snippet: None,
        page_number: None,
    })
}

#[tauri::command]
pub async fn open_result(path: String) -> Result<()> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(SiftError::PathNotFound(path));
    }

    // Trusted OS open via xdg-open on Ubuntu Linux
    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| SiftError::Io(e))?;

    Ok(())
}

#[tauri::command]
pub async fn reveal_result(path: String) -> Result<()> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(SiftError::PathNotFound(path));
    }

    // Trusted reveal: open parent folder via xdg-open or dbus filemanager
    let parent = p.parent().unwrap_or(p);
    Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|e| SiftError::Io(e))?;

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
