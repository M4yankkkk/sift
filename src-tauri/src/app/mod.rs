use chrono::{DateTime, Utc};
use sift_core::models::IndexStatus;
use sift_core::storage::Database;
use sift_core::watcher::FileWatcher;
use std::path::Path;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Free the embedding model after this long without searches or indexing.
const MODEL_IDLE_RELEASE: Duration = Duration::from_secs(5 * 60);
const THUMBNAIL_CACHE_SIZE: usize = 400;

/// Small LRU of thumbnail data URLs keyed by file id (~5 KB each).
#[derive(Default)]
pub struct ThumbnailCache {
    entries: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

impl ThumbnailCache {
    pub fn get(&self, id: &str) -> Option<Option<String>> {
        self.entries.get(id).cloned()
    }

    pub fn insert(&mut self, id: String, thumb: Option<String>) {
        if self.entries.insert(id.clone(), thumb).is_none() {
            self.order.push_back(id);
            while self.order.len() > THUMBNAIL_CACHE_SIZE {
                if let Some(old) = self.order.pop_front() {
                    self.entries.remove(&old);
                }
            }
        }
    }
}

pub struct AppState {
    pub db: Arc<Database>,
    pub is_scanning: Arc<AtomicBool>,
    pub current_operation: Arc<Mutex<Option<String>>>,
    pub last_scan_completed_at: Arc<Mutex<Option<DateTime<Utc>>>>,
    pub watcher: Arc<Mutex<Option<FileWatcher>>>,
    pub queue_length: Arc<AtomicUsize>,
    pub needs_rescan: Arc<AtomicBool>,
    pub cpu_sampler: crate::resources::CpuSampler,
    pub thumbnails: Mutex<ThumbnailCache>,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        let db_arc = Arc::new(db);
        let queue_length = Arc::new(AtomicUsize::new(0));
        let q_clone = queue_length.clone();

        let settings = db_arc.get_settings().unwrap_or_default();
        let exclusions = settings.default_exclusions;

        let status_updater = Arc::new(move |len: usize| {
            q_clone.store(len, Ordering::Relaxed);
        });

        let watcher_res = FileWatcher::new(
            db_arc.clone(),
            exclusions,
            250, // 250ms debouncing window
            Some(status_updater),
        );

        let watcher = match watcher_res {
            Ok(w) => {
                // Register existing roots
                if let Ok(roots) = db_arc.list_roots() {
                    for r in roots {
                        let _ = w.watch_root(Path::new(&r.path));
                    }
                }
                Arc::new(Mutex::new(Some(w)))
            }
            Err(e) => {
                tracing::warn!(error = ?e, "Failed to initialize filesystem watcher");
                Arc::new(Mutex::new(None))
            }
        };

        let needs_rescan = Arc::new(AtomicBool::new(false));
        let state = Self {
            db: db_arc.clone(),
            is_scanning: Arc::new(AtomicBool::new(false)),
            current_operation: Arc::new(Mutex::new(None)),
            last_scan_completed_at: Arc::new(Mutex::new(None)),
            watcher,
            queue_length,
            needs_rescan: needs_rescan.clone(),
            cpu_sampler: Default::default(),
            thumbnails: Default::default(),
        };

        // Free the embedding model when Sift sits idle; the next search reloads it (~0.6 s).
        {
            let db = state.db.clone();
            let is_scanning = state.is_scanning.clone();
            let queue_length = state.queue_length.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(30));
                let busy = is_scanning.load(Ordering::SeqCst)
                    || queue_length.load(Ordering::Relaxed) > 0;
                if !busy {
                    if let Some(ref engine) = db.embedding_engine {
                        engine.release_if_idle(MODEL_IDLE_RELEASE);
                    }
                }
            });
        }

        // Automatically discover new/modified files and reconcile on app startup
        crate::commands::spawn_scan_worker(
            db_arc,
            state.is_scanning.clone(),
            state.current_operation.clone(),
            state.last_scan_completed_at.clone(),
            state.needs_rescan.clone(),
        );

        state
    }

    pub fn watch_root(&self, path: &str) {
        if let Ok(guard) = self.watcher.lock() {
            if let Some(ref w) = *guard {
                let _ = w.watch_root(Path::new(path));
            }
        }
    }

    pub fn unwatch_root(&self, path: &str) {
        if let Ok(guard) = self.watcher.lock() {
            if let Some(ref w) = *guard {
                let _ = w.unwatch_root(Path::new(path));
            }
        }
    }

    pub fn get_status(&self) -> IndexStatus {
        let is_scanning = self.is_scanning.load(Ordering::SeqCst);
        let current_op = self.current_operation.lock().unwrap().clone();
        let last_scan = *self.last_scan_completed_at.lock().unwrap();
        let q_len = self.queue_length.load(Ordering::Relaxed);

        let (_roots_count, files_count) = self.db.get_stats().unwrap_or((0, 0));
        let total_embeddings = self.db.get_total_embeddings().unwrap_or(0);

        IndexStatus {
            total_files_discovered: files_count,
            files_indexed: files_count,
            total_embeddings,
            queue_length: q_len as i64,
            is_scanning,
            is_indexing: is_scanning || q_len > 0,
            last_scan_completed_at: last_scan,
            current_operation: if q_len > 0 && !is_scanning {
                Some("Processing file changes...".to_string())
            } else {
                current_op
            },
        }
    }
}
