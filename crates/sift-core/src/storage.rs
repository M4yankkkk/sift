use crate::error::{Result, SiftError};
use crate::models::{AppSettings, FileRecord, MatchType, RootFolder, SearchResultItem};
use crate::scanner::FileMeta;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Enable WAL mode and foreign keys for performance and data integrity
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS roots (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                added_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS files (
                id TEXT PRIMARY KEY,
                root_id TEXT NOT NULL REFERENCES roots(id) ON DELETE CASCADE,
                path TEXT NOT NULL UNIQUE,
                filename TEXT NOT NULL,
                extension TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                modified_at TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                indexed_at TEXT,
                is_deleted INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_files_filename ON files(filename);
            CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
            CREATE INDEX IF NOT EXISTS idx_files_root_id ON files(root_id);

            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                chunk_index INTEGER NOT NULL,
                page_number INTEGER,
                offset_start INTEGER,
                offset_end INTEGER,
                content TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_file_id ON chunks(file_id);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;

        Ok(())
    }

    pub fn add_root(&self, path: &str) -> Result<RootFolder> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let inserted = conn.execute(
            "INSERT INTO roots (id, path, added_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO NOTHING",
            params![id, path, now_str],
        )?;

        if inserted == 0 {
            // Already exists, return existing
            let mut stmt = conn.prepare("SELECT id, path, added_at FROM roots WHERE path = ?1")?;
            let root = stmt.query_row(params![path], |row| {
                let id: String = row.get(0)?;
                let path: String = row.get(1)?;
                let added_at_str: String = row.get(2)?;
                let added_at = chrono::DateTime::parse_from_rfc3339(&added_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                Ok(RootFolder {
                    id,
                    path,
                    added_at,
                    file_count: 0,
                })
            })?;
            return Ok(root);
        }

        Ok(RootFolder {
            id,
            path: path.to_string(),
            added_at: now,
            file_count: 0,
        })
    }

    pub fn remove_root(&self, path: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM roots WHERE path = ?1", params![path])?;
        Ok(rows > 0)
    }

    pub fn list_roots(&self) -> Result<Vec<RootFolder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT r.id, r.path, r.added_at, COUNT(f.id) as file_count
            FROM roots r
            LEFT JOIN files f ON f.root_id = r.id AND f.is_deleted = 0
            GROUP BY r.id
            ORDER BY r.added_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let path: String = row.get(1)?;
            let added_at_str: String = row.get(2)?;
            let file_count: i64 = row.get(3)?;

            let added_at = chrono::DateTime::parse_from_rfc3339(&added_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(RootFolder {
                id,
                path,
                added_at,
                file_count,
            })
        })?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn get_settings(&self) -> Result<AppSettings> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = 'app_settings'")?;
        let res = stmt.query_row([], |row| {
            let json_str: String = row.get(0)?;
            Ok(json_str)
        });

        match res {
            Ok(json_str) => {
                serde_json::from_str(&json_str).map_err(|e| SiftError::Config(e.to_string()))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AppSettings::default()),
            Err(e) => Err(SiftError::Database(e)),
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let json_str =
            serde_json::to_string(settings).map_err(|e| SiftError::Config(e.to_string()))?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('app_settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json_str],
        )?;
        Ok(())
    }

    pub fn upsert_files_batch(&self, root_id: &str, files: &[FileMeta]) -> Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339();
        let mut count = 0;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO files (
                    id, root_id, path, filename, extension,
                    size_bytes, modified_at, fingerprint, indexed_at, is_deleted
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)
                ON CONFLICT(path) DO UPDATE SET
                    filename = excluded.filename,
                    extension = excluded.extension,
                    size_bytes = excluded.size_bytes,
                    modified_at = excluded.modified_at,
                    fingerprint = excluded.fingerprint,
                    indexed_at = excluded.indexed_at,
                    is_deleted = 0
                "#,
            )?;

            for file in files {
                let id = Uuid::new_v4().to_string();
                let mtime_str = file.modified_at.to_rfc3339();
                stmt.execute(params![
                    id,
                    root_id,
                    file.path,
                    file.filename,
                    file.extension,
                    file.size_bytes,
                    mtime_str,
                    file.fingerprint,
                    now_str,
                ])?;
                count += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    pub fn search_filenames(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>> {
        let conn = self.conn.lock().unwrap();
        let q_lower = query.trim().to_lowercase();
        if q_lower.is_empty() {
            return Ok(Vec::new());
        }

        let like_pattern = format!("%{}%", query.trim());
        let mut stmt = conn.prepare(
            r#"
            SELECT id, path, filename, extension, size_bytes, modified_at
            FROM files
            WHERE is_deleted = 0 AND filename LIKE ?1
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![like_pattern, (limit * 2) as i64], |row| {
            let id: String = row.get(0)?;
            let path: String = row.get(1)?;
            let filename: String = row.get(2)?;
            let extension: String = row.get(3)?;
            let size_bytes: i64 = row.get(4)?;
            let modified_at_str: String = row.get(5)?;

            let modified_at = chrono::DateTime::parse_from_rfc3339(&modified_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok((id, path, filename, extension, size_bytes, modified_at))
        })?;

        let mut results = Vec::new();
        for r in rows {
            let (id, path, filename, extension, size_bytes, modified_at) = r?;
            let name_lower = filename.to_lowercase();

            let (score, match_reasons) = if name_lower == q_lower {
                (1.0, vec!["Exact filename match".to_string()])
            } else if name_lower.starts_with(&q_lower) {
                (0.85, vec!["Filename starts with query".to_string()])
            } else if name_lower.contains(&q_lower) {
                let ratio = q_lower.len() as f32 / name_lower.len().max(1) as f32;
                (
                    0.5 + 0.3 * ratio,
                    vec!["Filename contains query".to_string()],
                )
            } else {
                (0.3, vec!["Matched filename".to_string()])
            };

            results.push(SearchResultItem {
                id,
                path,
                filename,
                extension,
                size_bytes,
                modified_at,
                score,
                match_type: MatchType::Filename,
                match_reasons,
                snippet: None,
                page_number: None,
            });
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.filename.len().cmp(&b.filename.len()))
        });

        results.truncate(limit);
        Ok(results)
    }

    pub fn get_file_by_id(&self, file_id: &str) -> Result<Option<FileRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, root_id, path, filename, extension, size_bytes,
                   modified_at, fingerprint, indexed_at, is_deleted
            FROM files
            WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query_map(params![file_id], |row| {
            let id: String = row.get(0)?;
            let root_id: String = row.get(1)?;
            let path: String = row.get(2)?;
            let filename: String = row.get(3)?;
            let extension: String = row.get(4)?;
            let size_bytes: i64 = row.get(5)?;
            let modified_at_str: String = row.get(6)?;
            let fingerprint: String = row.get(7)?;
            let indexed_at_str: Option<String> = row.get(8)?;
            let is_deleted: i32 = row.get(9)?;

            let modified_at = chrono::DateTime::parse_from_rfc3339(&modified_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let indexed_at = indexed_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|d| d.with_timezone(&Utc))
                    .ok()
            });

            Ok(FileRecord {
                id,
                root_id,
                path,
                filename,
                extension,
                size_bytes,
                modified_at,
                fingerprint,
                indexed_at,
                is_deleted: is_deleted != 0,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn get_stats(&self) -> Result<(i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let total_roots: i64 = conn.query_row("SELECT COUNT(*) FROM roots", [], |r| r.get(0))?;
        let total_files: i64 =
            conn.query_row("SELECT COUNT(*) FROM files WHERE is_deleted = 0", [], |r| {
                r.get(0)
            })?;
        Ok((total_roots, total_files))
    }

    pub fn delete_file_by_path(&self, path: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM files WHERE path = ?1", params![path])?;
        Ok(affected > 0)
    }

    pub fn delete_files_under_path(&self, dir_prefix: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}/%", dir_prefix.trim_end_matches('/'));
        let affected = conn.execute("DELETE FROM files WHERE path LIKE ?1", params![pattern])?;
        Ok(affected)
    }

    pub fn reconcile_root(&self, root_id: &str, current_file_paths: &[String]) -> Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Retrieve all known indexed paths for this root
        let existing_paths: Vec<String> = {
            let mut stmt = tx.prepare("SELECT path FROM files WHERE root_id = ?1")?;
            let paths: Vec<String> = stmt
                .query_map(params![root_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            paths
        };

        let current_set: std::collections::HashSet<&str> =
            current_file_paths.iter().map(|s| s.as_str()).collect();

        let mut deleted_count = 0;
        {
            let mut del_stmt = tx.prepare("DELETE FROM files WHERE path = ?1")?;
            for old_path in existing_paths {
                if !current_set.contains(old_path.as_str()) {
                    del_stmt.execute(params![old_path])?;
                    deleted_count += 1;
                }
            }
        }

        tx.commit()?;
        Ok(deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_init_and_roots_crud() {
        let db = Database::open_in_memory().expect("in-memory db opens");

        let initial_roots = db.list_roots().expect("list roots");
        assert!(initial_roots.is_empty());

        let root1 = db.add_root("/home/user/Documents").expect("add root");
        assert_eq!(root1.path, "/home/user/Documents");

        let roots = db.list_roots().expect("list roots");
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].path, "/home/user/Documents");

        // Idempotent add
        let root1_again = db.add_root("/home/user/Documents").expect("re-add root");
        assert_eq!(root1_again.path, "/home/user/Documents");
        let roots_after_readd = db.list_roots().expect("list roots");
        assert_eq!(roots_after_readd.len(), 1);

        // Remove root
        let removed = db.remove_root("/home/user/Documents").expect("remove root");
        assert!(removed);
        let roots_empty = db.list_roots().expect("list roots");
        assert!(roots_empty.is_empty());
    }

    #[test]
    fn test_settings_persistence() {
        let db = Database::open_in_memory().expect("in-memory db opens");

        let default_settings = db.get_settings().expect("get default settings");
        assert_eq!(default_settings.global_shortcut, "Alt+Space");

        let mut custom_settings = default_settings;
        custom_settings.global_shortcut = "Ctrl+Shift+F".to_string();
        custom_settings.max_file_size_mb = 50;

        db.save_settings(&custom_settings)
            .expect("save custom settings");

        let retrieved = db.get_settings().expect("get custom settings");
        assert_eq!(retrieved.global_shortcut, "Ctrl+Shift+F");
        assert_eq!(retrieved.max_file_size_mb, 50);
    }

    #[test]
    fn test_batch_upsert_and_filename_search() {
        let db = Database::open_in_memory().expect("in-memory db opens");
        let root = db.add_root("/test/root").expect("add root");

        let files = vec![
            FileMeta {
                path: "/test/root/quarterly_report.pdf".to_string(),
                filename: "quarterly_report.pdf".to_string(),
                extension: "pdf".to_string(),
                size_bytes: 1024,
                modified_at: Utc::now(),
                fingerprint: "1024:100:0".to_string(),
            },
            FileMeta {
                path: "/test/root/notes.txt".to_string(),
                filename: "notes.txt".to_string(),
                extension: "txt".to_string(),
                size_bytes: 512,
                modified_at: Utc::now(),
                fingerprint: "512:100:0".to_string(),
            },
            FileMeta {
                path: "/test/root/quarterly_summary.md".to_string(),
                filename: "quarterly_summary.md".to_string(),
                extension: "md".to_string(),
                size_bytes: 256,
                modified_at: Utc::now(),
                fingerprint: "256:100:0".to_string(),
            },
        ];

        let count = db
            .upsert_files_batch(&root.id, &files)
            .expect("upsert batch");
        assert_eq!(count, 3);

        let (r_count, f_count) = db.get_stats().expect("get stats");
        assert_eq!(r_count, 1);
        assert_eq!(f_count, 3);

        // Search "quarterly" -> returns 2 items
        let search_results = db
            .search_filenames("quarterly", 10)
            .expect("search filenames");
        assert_eq!(search_results.len(), 2);
        assert!(search_results[0].filename.contains("quarterly"));

        // Exact match score check
        let notes_results = db.search_filenames("notes.txt", 10).expect("search notes");
        assert_eq!(notes_results.len(), 1);
        assert_eq!(notes_results[0].score, 1.0);

        // Lookup by ID
        let file_rec = db
            .get_file_by_id(&notes_results[0].id)
            .expect("get file by id");
        assert!(file_rec.is_some());
        assert_eq!(file_rec.unwrap().filename, "notes.txt");
    }
}
