use crate::error::{Result, SiftError};
use crate::models::{AppSettings, RootFolder};
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
}
