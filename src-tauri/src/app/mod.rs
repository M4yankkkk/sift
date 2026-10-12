use chrono::{DateTime, Utc};
use sift_core::models::IndexStatus;
use sift_core::storage::Database;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub db: Arc<Database>,
    pub is_scanning: Arc<AtomicBool>,
    pub current_operation: Arc<Mutex<Option<String>>>,
    pub last_scan_completed_at: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self {
            db: Arc::new(db),
            is_scanning: Arc::new(AtomicBool::new(false)),
            current_operation: Arc::new(Mutex::new(None)),
            last_scan_completed_at: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_status(&self) -> IndexStatus {
        let is_scanning = self.is_scanning.load(Ordering::SeqCst);
        let current_op = self.current_operation.lock().unwrap().clone();
        let last_scan = *self.last_scan_completed_at.lock().unwrap();

        let (_roots_count, files_count) = self.db.get_stats().unwrap_or((0, 0));

        IndexStatus {
            total_files_discovered: files_count,
            files_indexed: files_count,
            queue_length: 0,
            is_scanning,
            is_indexing: is_scanning,
            last_scan_completed_at: last_scan,
            current_operation: current_op,
        }
    }
}
