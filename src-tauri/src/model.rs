//! First-run model setup. The window opens immediately with filename, keyword, and OCR search;
//! this worker downloads EmbeddingGemma 2 in the background (reporting progress to the UI),
//! loads it, and attaches it to the database so meaning search switches on.

use crate::app::AppState;
use serde::Serialize;
use sift_core::embeddings::{EmbeddingEngine, EmbeddingGemma2Engine, FastEmbedEngine};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const STATUS_EVENT: &str = "model-status";

/// Progress events are throttled so a fast connection doesn't flood the UI.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ModelStatus {
    Checking,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: u64,
        bytes_per_sec: u64,
        files_done: usize,
        files_total: usize,
    },
    Loading,
    Ready {
        model: String,
    },
    /// The user turned meaning search off in Settings.
    Disabled,
    Failed {
        message: String,
    },
}

fn publish(app: &AppHandle, status: ModelStatus) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.model_status.lock().unwrap() = status.clone();
    }
    let _ = app.emit(STATUS_EVENT, status);
}

/// Starts the setup worker unless one is running or the model is already attached.
pub fn start(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.db.embedding_engine().is_some() {
        let model = state
            .db
            .embedding_engine()
            .unwrap()
            .model_name()
            .to_string();
        publish(&app, ModelStatus::Ready { model });
        return;
    }
    if state.model_worker_running.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        run(&app);
        app.state::<AppState>()
            .model_worker_running
            .store(false, Ordering::SeqCst);
    });
}

fn run(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state
        .db
        .get_settings()
        .map(|s| s.ml_enabled)
        .unwrap_or(true)
    {
        publish(app, ModelStatus::Disabled);
        return;
    }
    publish(app, ModelStatus::Checking);

    let engine: Arc<dyn EmbeddingEngine> = if sift_core::embeddings::wants_legacy_engine() {
        publish(app, ModelStatus::Loading);
        match FastEmbedEngine::try_new() {
            Ok(engine) => Arc::new(engine),
            Err(e) => {
                return fail(
                    app,
                    "Couldn't load the legacy search models",
                    &e.to_string(),
                )
            }
        }
    } else {
        if !EmbeddingGemma2Engine::is_downloaded() {
            if let Err(e) = download(app) {
                return fail(
                    app,
                    "Couldn't download the search model. Check your internet connection and retry",
                    &e.to_string(),
                );
            }
        }
        publish(app, ModelStatus::Loading);
        match EmbeddingGemma2Engine::try_new() {
            Ok(engine) => Arc::new(engine),
            Err(e) => return fail(app, "Couldn't load the search model", &e.to_string()),
        }
    };

    let model = engine.model_name().to_string();
    state.db.set_embedding_engine(engine);
    tracing::info!(model = %model, "Embedding model ready");
    publish(app, ModelStatus::Ready { model });

    // Embed everything indexed while the model was unavailable.
    crate::commands::spawn_scan_worker(
        state.db.clone(),
        state.is_scanning.clone(),
        state.current_operation.clone(),
        state.last_scan_completed_at.clone(),
        state.needs_rescan.clone(),
    );
}

fn download(app: &AppHandle) -> sift_core::Result<()> {
    let started = Instant::now();
    let mut last_emit: Option<Instant> = None;
    EmbeddingGemma2Engine::download(&mut |p| {
        let finished = p.files_done == p.files_total;
        if !finished && last_emit.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) {
            return;
        }
        last_emit = Some(Instant::now());
        let secs = started.elapsed().as_secs_f64().max(0.001);
        publish(
            app,
            ModelStatus::Downloading {
                downloaded_bytes: p.downloaded_bytes,
                total_bytes: p.total_bytes,
                bytes_per_sec: (p.downloaded_bytes as f64 / secs) as u64,
                files_done: p.files_done,
                files_total: p.files_total,
            },
        );
    })
}

fn fail(app: &AppHandle, summary: &str, detail: &str) {
    tracing::warn!(error = %detail, "{}", summary);
    publish(
        app,
        ModelStatus::Failed {
            message: format!("{}. ({})", summary, detail),
        },
    );
}
