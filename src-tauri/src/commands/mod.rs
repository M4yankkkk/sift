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

pub(crate) fn spawn_scan_worker(
    db: Arc<Database>,
    is_scanning: Arc<AtomicBool>,
    current_op: Arc<Mutex<Option<String>>>,
    last_scan: Arc<Mutex<Option<chrono::DateTime<Utc>>>>,
    needs_rescan: Arc<AtomicBool>,
) {
    if is_scanning.swap(true, Ordering::SeqCst) {
        // A scan is already running; signal it to perform another full pass once done
        needs_rescan.store(true, Ordering::SeqCst);
        return;
    }

    std::thread::spawn(move || {
        loop {
            needs_rescan.store(false, Ordering::SeqCst);

            let roots = match db.list_roots() {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to list roots for scan: {}", e);
                    break;
                }
            };

            let exclusions = db
                .get_settings()
                .map(|s| s.default_exclusions)
                .unwrap_or_default();

            let mut roots = roots;
            // Prioritize user Documents root for immediate responsiveness
            roots.sort_by(|a, b| {
                if a.path.contains("Documents") {
                    std::cmp::Ordering::Less
                } else if b.path.contains("Documents") {
                    std::cmp::Ordering::Greater
                } else {
                    a.path.cmp(&b.path)
                }
            });

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
                        {
                            let mut op = current_op.lock().unwrap();
                            *op = Some(format!(
                                "Indexing & embedding {} files in {}",
                                count, root.path
                            ));
                        }

                        if let Err(e) = db.upsert_files_batch(&root.id, &files) {
                            tracing::error!("Failed to upsert files for root {}: {}", root.path, e);
                        } else {
                            total_indexed += count;
                        }

                        // Periodic/startup reconciliation: prune deleted files
                        let active_paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
                        if let Ok(pruned) = db.reconcile_root(&root.id, &active_paths) {
                            if pruned > 0 {
                                tracing::info!(pruned, root = %root.path, "Reconciled deleted files for root");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Error scanning directory {}: {}", root.path, e);
                    }
                }
            }

            tracing::info!("Scan completed: indexed {} files", total_indexed);
            *last_scan.lock().unwrap() = Some(Utc::now());

            if !needs_rescan.load(Ordering::SeqCst) {
                break;
            }
            tracing::info!("Rescan requested while previous scan was running; performing fresh pass");
        }

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
    state.watch_root(&path);

    // Trigger asynchronous background scan for the new root
    spawn_scan_worker(
        state.db.clone(),
        state.is_scanning.clone(),
        state.current_operation.clone(),
        state.last_scan_completed_at.clone(),
        state.needs_rescan.clone(),
    );

    Ok(root)
}

#[tauri::command]
pub async fn remove_root(path: String, state: State<'_, AppState>) -> Result<bool> {
    state.unwatch_root(&path);
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
        state.needs_rescan.clone(),
    );
    Ok(state.get_status())
}

#[tauri::command]
pub async fn reindex_all(state: State<'_, AppState>) -> Result<IndexStatus> {
    state.needs_rescan.store(true, Ordering::SeqCst);
    state.db.clear_all_index_data()?;
    spawn_scan_worker(
        state.db.clone(),
        state.is_scanning.clone(),
        state.current_operation.clone(),
        state.last_scan_completed_at.clone(),
        state.needs_rescan.clone(),
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

    let results = state.db.search(&query.query, limit)?;
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
    page: Option<i32>,
    state: State<'_, AppState>,
) -> Result<PreviewResponse> {
    let file = state
        .db
        .get_file_by_id(&file_id)?
        .ok_or_else(|| SiftError::PathNotFound(format!("File with id {} not found", file_id)))?;

    let path = Path::new(&file.path);
    let ext = file.extension.to_lowercase();
    let target_page = page.unwrap_or(1).max(1) as u32;

    let (preview_content, snippet, page_number) = if ext == "pdf" {
        let visual = sift_core::render_pdf_page(path, target_page);
        let text = sift_core::extract_pdf_page_preview(path, target_page, 16384);
        match (visual, text) {
            (Some(v), Some(t)) => (v, Some(t), Some(target_page as i32)),
            (Some(v), None) => (v, None, Some(target_page as i32)),
            (None, Some(t)) => (t, None, Some(target_page as i32)),
            (None, None) => (
                "[PDF Document — Scanned or graphic document. Open with default viewer to read]"
                    .to_string(),
                None,
                Some(target_page as i32),
            ),
        }
    } else {
        (
            sift_core::read_file_preview(path, 65536)
                .unwrap_or_else(|e| format!("[Preview error: {}]", e)),
            None,
            None,
        )
    };

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

    let (ocr_regions, confidence, ocr_snippet) = if sift_core::is_ocr_supported_extension(&ext) {
        if let Ok(Some((full_text, conf, regions))) = state.db.get_ocr_data(&file.id) {
            (Some(regions), Some(conf), Some(full_text))
        } else {
            (None, None, None)
        }
    } else {
        (None, None, None)
    };

    let final_snippet = snippet.or(ocr_snippet);

    Ok(PreviewResponse {
        id: file.id,
        path: file.path,
        filename: file.filename,
        mime_type: mime_type.to_string(),
        size_bytes: file.size_bytes,
        preview_content,
        snippet: final_snippet,
        page_number,
        ocr_regions,
        confidence,
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSettings> {
    let old_settings = state.db.get_settings().unwrap_or_default();
    state.db.save_settings(&settings)?;

    if old_settings.global_shortcut != settings.global_shortcut {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let _ = app.global_shortcut().unregister(old_settings.global_shortcut.as_str());
        if let Err(e) = app.global_shortcut().register(settings.global_shortcut.as_str()) {
            tracing::warn!(
                "Failed to rebind global shortcut to '{}': {}",
                settings.global_shortcut,
                e
            );
        } else {
            tracing::info!("Rebound global shortcut to '{}'", settings.global_shortcut);
        }
        crate::ipc::sync_gnome_shortcut(&settings.global_shortcut);
    }

    Ok(settings)
}

#[tauri::command]
pub async fn hide_window(app: tauri::AppHandle) -> Result<()> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

