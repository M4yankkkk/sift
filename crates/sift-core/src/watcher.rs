use crate::error::{Result, SiftError};
use crate::scanner::{read_file_metadata, PathRuleChecker};
use crate::storage::Database;
use crossbeam_channel::{bounded, select, tick, Receiver};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    Upsert(PathBuf),
    Delete(PathBuf),
}

/// Thread-safe FileWatcher that monitors approved search roots with debouncing
pub struct FileWatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    shutdown_flag: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl FileWatcher {
    pub fn new(
        db: Arc<Database>,
        exclusions: Vec<String>,
        debounce_ms: u64,
        status_updater: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    ) -> Result<Self> {
        let (raw_tx, raw_rx) = bounded::<notify::Result<Event>>(1000);
        let watched_paths = Arc::new(Mutex::new(HashSet::new()));
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = raw_tx.send(res);
            },
            Config::default(),
        )
        .map_err(|e| SiftError::Io(std::io::Error::other(e.to_string())))?;

        let shutdown_clone = shutdown_flag.clone();
        let db_clone = db.clone();
        let watched_paths_clone = watched_paths.clone();

        let worker_handle = thread::Builder::new()
            .name("sift-watcher-debouncer".into())
            .spawn(move || {
                Self::run_event_loop(
                    raw_rx,
                    db_clone,
                    watched_paths_clone,
                    exclusions,
                    Duration::from_millis(debounce_ms),
                    shutdown_clone,
                    status_updater,
                );
            })
            .map_err(SiftError::Io)?;

        Ok(Self {
            watcher: Arc::new(Mutex::new(Some(watcher))),
            watched_paths,
            shutdown_flag,
            worker_handle: Some(worker_handle),
        })
    }

    pub fn watch_root(&self, path: &Path) -> Result<()> {
        let mut watcher_guard = self.watcher.lock().unwrap();
        if let Some(ref mut watcher) = *watcher_guard {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| SiftError::Io(std::io::Error::other(e.to_string())))?;

            let mut paths_guard = self.watched_paths.lock().unwrap();
            paths_guard.insert(path.to_path_buf());
            tracing::info!(root = ?path, "Started watching root filesystem");
        }
        Ok(())
    }

    pub fn unwatch_root(&self, path: &Path) -> Result<()> {
        let mut watcher_guard = self.watcher.lock().unwrap();
        if let Some(ref mut watcher) = *watcher_guard {
            let _ = watcher.unwatch(path);
            let mut paths_guard = self.watched_paths.lock().unwrap();
            paths_guard.remove(path);
            tracing::info!(root = ?path, "Stopped watching root filesystem");
        }
        Ok(())
    }

    pub fn list_watched_roots(&self) -> Vec<PathBuf> {
        let paths_guard = self.watched_paths.lock().unwrap();
        paths_guard.iter().cloned().collect()
    }

    fn run_event_loop(
        raw_rx: Receiver<notify::Result<Event>>,
        db: Arc<Database>,
        watched_roots: Arc<Mutex<HashSet<PathBuf>>>,
        exclusions: Vec<String>,
        debounce_duration: Duration,
        shutdown: Arc<AtomicBool>,
        status_updater: Option<Arc<dyn Fn(usize) + Send + Sync>>,
    ) {
        let checker = PathRuleChecker::new(exclusions);
        let mut pending_changes: HashMap<PathBuf, Instant> = HashMap::new();
        let ticker = tick(Duration::from_millis(100));

        while !shutdown.load(Ordering::Relaxed) {
            select! {
                recv(raw_rx) -> msg => {
                    match msg {
                        Ok(Ok(event)) => {
                            Self::handle_raw_event(&event, &watched_roots, &checker, &mut pending_changes);
                            if let Some(ref updater) = status_updater {
                                updater(pending_changes.len());
                            }
                        }
                        Ok(Err(err)) => {
                            tracing::warn!(error = ?err, "Filesystem watch event error");
                        }
                        Err(_) => break, // Channel closed
                    }
                }
                recv(ticker) -> _ => {
                    // Flush debounced items whose timer expired
                    let now = Instant::now();
                    let mut ready_paths = Vec::new();

                    pending_changes.retain(|path, last_seen| {
                        if now.duration_since(*last_seen) >= debounce_duration {
                            ready_paths.push(path.clone());
                            false
                        } else {
                            true
                        }
                    });

                    if !ready_paths.is_empty() {
                        Self::apply_batch_changes(&ready_paths, &db, &watched_roots, &checker);
                        if let Some(ref updater) = status_updater {
                            updater(pending_changes.len());
                        }
                    }
                }
            }
        }

        // Flush remaining events before shutdown
        if !pending_changes.is_empty() {
            let paths: Vec<PathBuf> = pending_changes.into_keys().collect();
            Self::apply_batch_changes(&paths, &db, &watched_roots, &checker);
        }
    }

    fn handle_raw_event(
        event: &Event,
        watched_roots: &Arc<Mutex<HashSet<PathBuf>>>,
        checker: &PathRuleChecker,
        pending: &mut HashMap<PathBuf, Instant>,
    ) {
        let roots: Vec<PathBuf> = {
            let guard = watched_roots.lock().unwrap();
            guard.iter().cloned().collect()
        };

        for path in &event.paths {
            let matched_root = roots
                .iter()
                .find(|r| path.starts_with(r))
                .map(|p| p.as_path());
            if !checker.should_index_relative(matched_root, path) {
                continue;
            }

            match event.kind {
                EventKind::Create(_)
                | EventKind::Modify(_)
                | EventKind::Remove(_)
                | EventKind::Any => {
                    pending.insert(path.clone(), Instant::now());
                }
                _ => {}
            }
        }
    }

    fn apply_batch_changes(
        paths: &[PathBuf],
        db: &Arc<Database>,
        watched_roots: &Arc<Mutex<HashSet<PathBuf>>>,
        checker: &PathRuleChecker,
    ) {
        let roots: Vec<PathBuf> = {
            let guard = watched_roots.lock().unwrap();
            guard.iter().cloned().collect()
        };

        for path in paths {
            let path_str = path.to_string_lossy().to_string();

            if path.exists() {
                // If it's a directory that was created, we can index its files
                if path.is_dir() {
                    // Find matching root
                    if let Some(root) = roots.iter().find(|r| path.starts_with(r)) {
                        if let Ok(roots_list) = db.list_roots() {
                            if let Some(root_record) =
                                roots_list.iter().find(|r| r.path == root.to_string_lossy())
                            {
                                if let Ok(files) =
                                    crate::scanner::scan_directory(path, checker.exclusions())
                                {
                                    let _ = db.upsert_files_batch(&root_record.id, &files);
                                }
                            }
                        }
                    }
                } else if path.is_file() {
                    // Single file created or modified
                    if let Some(root) = roots.iter().find(|r| path.starts_with(r)) {
                        if let Ok(roots_list) = db.list_roots() {
                            if let Some(root_record) =
                                roots_list.iter().find(|r| r.path == root.to_string_lossy())
                            {
                                if let Ok(meta) = read_file_metadata(path) {
                                    let _ = db.upsert_files_batch(&root_record.id, &[meta]);
                                    tracing::info!(path = ?path_str, "Incrementally indexed file change");
                                }
                            }
                        }
                    }
                }
            } else {
                // File or directory was deleted or moved away
                let deleted = db.delete_file_by_path(&path_str).unwrap_or(false);
                let deleted_sub = db.delete_files_under_path(&path_str).unwrap_or(0);
                if deleted || deleted_sub > 0 {
                    tracing::info!(path = ?path_str, count = deleted_sub + (deleted as usize), "Incrementally removed deleted file(s) from index");
                }
            }
        }
    }

    pub fn shutdown(&mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
        let mut watcher_guard = self.watcher.lock().unwrap();
        *watcher_guard = None;

        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}
