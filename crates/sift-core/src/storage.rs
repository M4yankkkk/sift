use crate::error::{Result, SiftError};
use crate::models::{AppSettings, FileRecord, MatchType, ModeCounts, RootFolder, SearchResultItem};
use crate::ranking::{self, QueryIntent, SimStats};
use crate::scanner::FileMeta;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::extract::extract_document_chunks;
use crate::tantivy_index::TantivySearchIndex;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub file_id: String,
    pub path: String,
    pub filename: String,
    /// Calibrated confidence in [0.5, 0.95].
    pub score: f32,
    /// Raw cosine similarity.
    pub similarity: f32,
    /// Standard deviations above this query's mean similarity (0 when the corpus is too small).
    pub z_score: f32,
    pub modality: String,
    pub snippet: String,
    pub page_number: Option<i32>,
    pub size_bytes: i64,
}
/// Safely truncates a string to at most `max_bytes` without slicing inside a multi-byte UTF-8 character.
pub fn safe_truncate_str(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s.to_string()
    } else {
        let mut end = max_bytes;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    pub text_index: Option<Arc<TantivySearchIndex>>,
    pub embedding_engine: Option<Arc<dyn crate::embeddings::EmbeddingEngine>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(&path)?;
        let index_dir = path
            .as_ref()
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join("sift_tantivy_index");

        let text_index = match TantivySearchIndex::open_or_create(&index_dir) {
            Ok(idx) => Some(Arc::new(idx)),
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize Tantivy search index at {:?}: {}",
                    index_dir,
                    e
                );
                None
            }
        };

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            text_index,
            embedding_engine: None,
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let text_index = match TantivySearchIndex::create_in_ram() {
            Ok(idx) => Some(Arc::new(idx)),
            Err(e) => {
                tracing::warn!("Failed to initialize in-RAM Tantivy index: {}", e);
                None
            }
        };

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            text_index,
            embedding_engine: None,
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn with_text_index(mut self, text_index: Option<Arc<TantivySearchIndex>>) -> Self {
        self.text_index = text_index;
        self
    }

    pub fn with_embedding_engine(
        mut self,
        embedding_engine: Option<Arc<dyn crate::embeddings::EmbeddingEngine>>,
    ) -> Self {
        self.embedding_engine = embedding_engine;
        self
    }

    /// Clears all files, chunks, OCR data, embeddings, and Tantivy index documents,
    /// while preserving configured roots and application settings.
    pub fn clear_all_index_data(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM chunk_embeddings", [])?;
        conn.execute("DELETE FROM chunks", [])?;
        conn.execute("DELETE FROM ocr_data", [])?;
        conn.execute("DELETE FROM files", [])?;
        let _ = conn.execute("VACUUM", []);
        drop(conn);

        if let Some(ref text_index) = self.text_index {
            let _ = text_index.clear();
        }
        Ok(())
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

            CREATE TABLE IF NOT EXISTS ocr_data (
                file_id TEXT PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
                full_text TEXT NOT NULL,
                confidence REAL NOT NULL,
                regions_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunk_embeddings (
                id TEXT PRIMARY KEY,
                file_id TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                chunk_index INTEGER NOT NULL,
                modality TEXT NOT NULL,
                page_number INTEGER,
                line_number INTEGER,
                snippet TEXT NOT NULL,
                embedding BLOB NOT NULL,
                created_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_embeddings_file_id ON chunk_embeddings(file_id);
            CREATE INDEX IF NOT EXISTS idx_embeddings_modality ON chunk_embeddings(modality);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;

        // Which embedding model produced each vector; vectors from different models are never compared.
        let has_model_column = conn
            .prepare("SELECT 1 FROM pragma_table_info('chunk_embeddings') WHERE name = 'model'")?
            .exists([])?;
        if !has_model_column {
            conn.execute("ALTER TABLE chunk_embeddings ADD COLUMN model TEXT", [])?;
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_embeddings_model ON chunk_embeddings(model)",
            [],
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
        if let Some(ref text_index) = self.text_index {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT f.id FROM files f JOIN roots r ON f.root_id = r.id WHERE r.path = ?1",
            ) {
                if let Ok(ids) = stmt.query_map(params![path], |row| row.get::<_, String>(0)) {
                    for id in ids.flatten() {
                        let _ = text_index.delete_file(&id);
                    }
                    let _ = text_index.commit();
                }
            }
        }
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
        let mut file_records_to_index: Vec<(String, String, String, String)> = Vec::new();
        let mut count = 0;

        {
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;
            let now_str = Utc::now().to_rfc3339();

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

                let mut check_existing = tx.prepare(
                    "SELECT id, fingerprint, is_deleted FROM files WHERE path = ?1",
                )?;
                let mut has_emb_stmt = tx.prepare(
                    "SELECT 1 FROM chunk_embeddings WHERE file_id = ?1 LIMIT 1",
                )?;
                let mut has_ocr_stmt = tx.prepare(
                    "SELECT 1 FROM ocr_data WHERE file_id = ?1 LIMIT 1",
                )?;

                for file in files {
                    let existing_opt = check_existing
                        .query_row(params![file.path], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        })
                        .optional()?;

                    let mut needs_indexing = true;
                    let file_id: String;

                    match existing_opt {
                        Some((existing_id, existing_fp, is_deleted)) => {
                            if is_deleted == 0 && existing_fp == file.fingerprint {
                                // Fingerprint matches exactly! The file has not changed on disk.
                                let is_image = crate::embeddings::is_image_supported_for_vision(&file.extension);
                                let is_ocr = crate::ocr::is_ocr_supported_extension(&file.extension);
                                let is_doc_or_code = crate::extract::is_text_or_code_extension(&file.extension) || file.extension == "pdf";

                                let already_has_embeddings = has_emb_stmt.exists(params![existing_id]).unwrap_or(false);
                                let already_has_ocr = if is_ocr {
                                    has_ocr_stmt.exists(params![existing_id]).unwrap_or(false)
                                } else {
                                    false
                                };

                                if (is_image && already_has_embeddings)
                                    || (is_ocr && (already_has_embeddings || already_has_ocr))
                                    || (is_doc_or_code && already_has_embeddings)
                                    || (!is_image && !is_ocr && !is_doc_or_code)
                                {
                                    // File is already indexed and completely up to date! Skip re-embedding.
                                    needs_indexing = false;
                                }
                            }

                            if needs_indexing {
                                let mtime_str = file.modified_at.to_rfc3339();
                                stmt.execute(params![
                                    existing_id,
                                    root_id,
                                    file.path,
                                    file.filename,
                                    file.extension,
                                    file.size_bytes,
                                    mtime_str,
                                    file.fingerprint,
                                    now_str,
                                ])?;
                            }
                            file_id = existing_id;
                        }
                        None => {
                            let new_id = Uuid::new_v4().to_string();
                            let mtime_str = file.modified_at.to_rfc3339();
                            stmt.execute(params![
                                new_id,
                                root_id,
                                file.path,
                                file.filename,
                                file.extension,
                                file.size_bytes,
                                mtime_str,
                                file.fingerprint,
                                now_str,
                            ])?;
                            file_id = new_id;
                        }
                    }

                    if needs_indexing {
                        file_records_to_index.push((
                            file.path.clone(),
                            file.filename.clone(),
                            file.extension.clone(),
                            file_id,
                        ));
                    }
                    count += 1;
                }
            }

            tx.commit()?;
        } // MutexGuard is dropped here before Tantivy chunk ingestion

        // Prioritize visual images first so image search is instantly responsive on launch
        file_records_to_index.sort_by(|a, b| {
            let is_img_a = crate::embeddings::is_image_supported_for_vision(&a.2);
            let is_img_b = crate::embeddings::is_image_supported_for_vision(&b.2);
            match (is_img_a, is_img_b) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });

        let settings = self.get_settings().unwrap_or_default();
        let is_eco = settings.resource_mode == crate::models::ResourceMode::Eco;
        let embeddings_on = settings.ml_enabled && self.embedding_engine.is_some();

        // Ingest and index document chunks into Tantivy and vector embeddings
        for (path, filename, extension, id) in file_records_to_index {
            let is_image = crate::embeddings::is_image_supported_for_vision(&extension);
            let is_ocr = crate::ocr::is_ocr_supported_extension(&extension);

            // 1. If it's an image, embed visual vector if embedding engine is active
            if is_image && embeddings_on {
                if is_eco {
                    // Eco mode throttles background heavy ML passes to conserve CPU and battery
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                if let Err(e) = self.store_image_embedding(&id, &path, &filename) {
                    tracing::warn!(path = %path, error = %e, "Failed to generate visual image embedding");
                }
            }

            // 2. If OCR is supported on this image, extract text
            if is_ocr {
                if !settings.ocr_enabled {
                    continue;
                }
                if let Ok(Some(ocr_res)) = crate::ocr::extract_image_ocr(Path::new(&path)) {
                    if !ocr_res.full_text.trim().is_empty() {
                        let regions_json =
                            serde_json::to_string(&ocr_res.regions).unwrap_or_default();
                        {
                            let conn = self.conn.lock().unwrap();
                            let _ = conn.execute(
                                r#"
                                INSERT INTO ocr_data (file_id, full_text, confidence, regions_json)
                                VALUES (?1, ?2, ?3, ?4)
                                ON CONFLICT(file_id) DO UPDATE SET
                                    full_text = excluded.full_text,
                                    confidence = excluded.confidence,
                                    regions_json = excluded.regions_json
                                "#,
                                params![
                                    id,
                                    ocr_res.full_text,
                                    ocr_res.average_confidence as f64,
                                    regions_json,
                                ],
                            );
                        }

                        // Index in Tantivy
                        if let Some(ref text_index) = self.text_index {
                            let chunk = crate::extract::DocumentChunk {
                                chunk_index: 0,
                                page_number: None,
                                line_number: None,
                                content: ocr_res.full_text.clone(),
                            };
                            let _ = text_index.upsert_file_chunks(
                                &id,
                                &path,
                                &filename,
                                "ocr",
                                &[chunk],
                            );
                        }

                        // Embed OCR text if embedding engine is active
                        let ocr_chunk = crate::extract::DocumentChunk {
                            chunk_index: 0,
                            page_number: None,
                            line_number: None,
                            content: ocr_res.full_text.clone(),
                        };
                        if embeddings_on {
                            self.store_text_embeddings(&id, &filename, "ocr", &[ocr_chunk]);
                        }
                    }
                }
            } else {
                // 3. Document or source code file
                let chunks = extract_document_chunks(Path::new(&path)).unwrap_or_default();
                if !chunks.is_empty() {
                    let modality = document_modality(&extension);

                    if let Some(ref text_index) = self.text_index {
                        let _ =
                            text_index.upsert_file_chunks(&id, &path, &filename, &modality, &chunks);
                    }

                    if embeddings_on {
                        self.store_text_embeddings(&id, &filename, &modality, &chunks);
                    }
                }
            }
        }

        if let Some(ref text_index) = self.text_index {
            let _ = text_index.commit();
        }

        Ok(count)
    }

    fn relevance(&self) -> ranking::RelevanceProfile {
        self.embedding_engine
            .as_ref()
            .map(|e| e.relevance_profile())
            .unwrap_or_default()
    }

    fn current_model(&self) -> Option<&str> {
        self.embedding_engine.as_ref().map(|e| e.model_name())
    }

    /// Replaces a file's image vector with a fresh one from the active engine.
    fn store_image_embedding(&self, file_id: &str, path: &str, filename: &str) -> Result<()> {
        let engine = match self.embedding_engine {
            Some(ref e) => e,
            None => return Ok(()),
        };
        let vector = engine.embed_image(Path::new(path))?;
        let blob = crate::embeddings::serialize_vector(&vector);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM chunk_embeddings WHERE file_id = ?1 AND modality = 'image'",
            params![file_id],
        )?;
        conn.execute(
            r#"
            INSERT INTO chunk_embeddings (id, file_id, chunk_index, modality, page_number, line_number, snippet, embedding, created_at, model)
            VALUES (?1, ?2, 0, 'image', NULL, NULL, ?3, ?4, ?5, ?6)
            "#,
            params![
                Uuid::new_v4().to_string(),
                file_id,
                filename,
                blob,
                Utc::now().timestamp(),
                engine.model_name()
            ],
        )?;
        Ok(())
    }

    /// Replaces a file's text/OCR vectors with fresh ones for `chunks`, embedded in one batch.
    fn store_text_embeddings(
        &self,
        file_id: &str,
        filename: &str,
        modality: &str,
        chunks: &[crate::extract::DocumentChunk],
    ) {
        let engine = match self.embedding_engine {
            Some(ref e) => e,
            None => return,
        };
        let contents: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
        let vectors = match engine.embed_documents(Some(filename), &contents) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(file = %filename, error = %e, "Failed to embed document chunks");
                return;
            }
        };
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "DELETE FROM chunk_embeddings WHERE file_id = ?1 AND modality != 'image'",
            params![file_id],
        );
        let now = Utc::now().timestamp();
        for (chunk, vector) in chunks.iter().zip(vectors) {
            let _ = conn.execute(
                r#"
                INSERT INTO chunk_embeddings (id, file_id, chunk_index, modality, page_number, line_number, snippet, embedding, created_at, model)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
                params![
                    Uuid::new_v4().to_string(),
                    file_id,
                    chunk.chunk_index,
                    modality,
                    chunk.page_number,
                    chunk.line_number,
                    safe_truncate_str(&chunk.content, 247),
                    crate::embeddings::serialize_vector(&vector),
                    now,
                    engine.model_name(),
                ],
            );
        }
    }

    /// Number of files whose vectors were produced by a different model than the active engine.
    pub fn count_stale_embedding_files(&self) -> Result<usize> {
        let model = match self.current_model() {
            Some(m) => m.to_string(),
            None => return Ok(0),
        };
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row(
            r#"
            SELECT COUNT(DISTINCT e.file_id) FROM chunk_embeddings e JOIN files f ON f.id = e.file_id
            WHERE f.is_deleted = 0 AND (e.model IS NULL OR e.model != ?1)
            "#,
            params![model],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// Re-embeds every file whose vectors came from another model (e.g. after upgrading to
    /// EmbeddingGemma 2), images first. Stored OCR text is reused and document chunks are
    /// re-extracted; keyword indexes are untouched. Progress is saved per file, so an interrupted
    /// run resumes where it stopped. `progress(done, total)` is called after each file; returning
    /// `false` stops early. Returns the number of files processed.
    pub fn reembed_stale<F: FnMut(usize, usize) -> bool>(&self, mut progress: F) -> Result<usize> {
        let model = match self.current_model() {
            Some(m) => m.to_string(),
            None => return Ok(0),
        };
        if !self.get_settings().map(|s| s.ml_enabled).unwrap_or(true) {
            return Ok(0);
        }
        let stale: Vec<(String, String, String, String)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                r#"
                SELECT f.id, f.path, f.filename, f.extension
                FROM chunk_embeddings e JOIN files f ON f.id = e.file_id
                WHERE f.is_deleted = 0 AND (e.model IS NULL OR e.model != ?1)
                GROUP BY f.id
                ORDER BY MAX(e.modality = 'image') DESC, f.path
                "#,
            )?;
            let rows = stmt.query_map(params![model], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?;
            rows.collect::<std::result::Result<_, _>>()?
        };

        let total = stale.len();
        let is_eco = self
            .get_settings()
            .map(|s| s.resource_mode == crate::models::ResourceMode::Eco)
            .unwrap_or(false);
        for (done, (id, path, filename, extension)) in stale.into_iter().enumerate() {
            if crate::embeddings::is_image_supported_for_vision(&extension) {
                if let Err(e) = self.store_image_embedding(&id, &path, &filename) {
                    tracing::warn!(path = %path, error = %e, "Re-embedding image failed");
                }
            }
            let ocr_text: Option<String> = {
                let conn = self.conn.lock().unwrap();
                conn.query_row(
                    "SELECT full_text FROM ocr_data WHERE file_id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()?
            };
            if let Some(text) = ocr_text.filter(|t| !t.trim().is_empty()) {
                let chunk = crate::extract::DocumentChunk {
                    chunk_index: 0,
                    page_number: None,
                    line_number: None,
                    content: text,
                };
                self.store_text_embeddings(&id, &filename, "ocr", &[chunk]);
            } else if !crate::embeddings::is_image_supported_for_vision(&extension) {
                let chunks = extract_document_chunks(Path::new(&path)).unwrap_or_default();
                if !chunks.is_empty() {
                    self.store_text_embeddings(&id, &filename, &document_modality(&extension), &chunks);
                }
            }
            // Anything still from the old model could not be regenerated; drop it so it is not
            // retried on every launch and never mixes with vectors from another space.
            {
                let conn = self.conn.lock().unwrap();
                conn.execute(
                    "DELETE FROM chunk_embeddings WHERE file_id = ?1 AND (model IS NULL OR model != ?2)",
                    params![id, model],
                )?;
            }
            if is_eco {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if !progress(done + 1, total) {
                return Ok(done + 1);
            }
        }
        Ok(total)
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

    pub fn search_vectors(
        &self,
        query_vec: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        self.search_vectors_by_modality(query_vec, limit, None)
    }

    /// Semantic vector search gated by per-query relevance (see [`crate::ranking`]).
    /// `is_image_only`: `Some(true)` = CLIP image vectors, `Some(false)` = text/OCR chunks, `None` = both.
    pub fn search_vectors_by_modality(
        &self,
        query_vec: &[f32],
        limit: usize,
        is_image_only: Option<bool>,
    ) -> Result<Vec<VectorSearchResult>> {
        Ok(self
            .vector_search(query_vec, limit, is_image_only, self.relevance().image_z_blended)?
            .hits)
    }

    /// Scores every stored vector of the requested modality against `query_vec`, keeps each file's
    /// best chunk, and admits files that stand out from this query's own similarity distribution.
    fn vector_search(
        &self,
        query_vec: &[f32],
        limit: usize,
        is_image_only: Option<bool>,
        image_min_z: f32,
    ) -> Result<VectorOutcome> {
        let modality_clause = match is_image_only {
            Some(true) => "AND e.modality = 'image'",
            Some(false) => "AND e.modality != 'image'",
            None => "",
        };
        let sql = format!(
            r#"
            SELECT e.id, e.file_id, e.modality = 'image', f.size_bytes, e.embedding,
                   CASE WHEN e.modality = 'ocr' THEN e.snippet END,
                   lower(f.extension) = 'svg' AND e.modality != 'image'
            FROM chunk_embeddings e
            JOIN files f ON e.file_id = f.id
            WHERE f.is_deleted = 0 AND e.model IS ?1 {}
            "#,
            modality_clause
        );
        let model = self.current_model().map(str::to_string);

        // Index 0 = text/OCR chunks, 1 = images: the two live in different embedding spaces.
        let mut sums = [(0usize, 0f64, 0f64); 2];
        // file_id -> (similarity, embedding row id, is_image)
        let mut best_by_file: HashMap<String, (f32, String, bool)> = HashMap::new();
        {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(params![model])?;
            while let Some(row) = rows.next()? {
                let is_image: bool = row.get(2)?;
                let size_bytes: i64 = row.get(3)?;
                // Exclude tiny theme assets / UI icon slices (< 2048 bytes) from visual image search
                #[cfg(not(test))]
                if is_image && size_bytes < 2048 {
                    continue;
                }
                if size_bytes < 32 {
                    continue;
                }
                // OCR of icons and UI sprites ("Dd", "<>") carries no meaning but still lands near
                // random queries in embedding space; keyword search keeps covering it.
                let ocr_text: Option<String> = row.get(5)?;
                if let Some(text) = ocr_text {
                    if size_bytes < 2048 || !has_meaningful_words(&text) {
                        continue;
                    }
                }
                // SVG markup embeds as noise; its text stays searchable by keyword.
                let svg_markup: bool = row.get(6)?;
                if svg_markup {
                    continue;
                }

                let vec = match row.get_ref(4)?.as_blob() {
                    Ok(blob) => crate::embeddings::deserialize_vector(blob),
                    Err(_) => continue,
                };
                let sim = crate::embeddings::cosine_similarity(query_vec, &vec);

                let group = &mut sums[is_image as usize];
                group.0 += 1;
                group.1 += sim as f64;
                group.2 += (sim as f64) * (sim as f64);

                let file_id: String = row.get(1)?;
                match best_by_file.get(&file_id) {
                    Some((best, _, _)) if *best >= sim => {}
                    _ => {
                        let emb_id: String = row.get(0)?;
                        best_by_file.insert(file_id, (sim, emb_id, is_image));
                    }
                }
            }
        }

        let stats = [
            SimStats::from_sums(sums[0].0, sums[0].1, sums[0].2),
            SimStats::from_sums(sums[1].0, sums[1].1, sums[1].2),
        ];

        let profile = self.relevance();
        let mut admitted: Vec<(String, String, f32, f32, f32)> = Vec::new(); // (file_id, emb_id, sim, z, conf)
        let mut top_z: Option<f32> = None;
        for (file_id, (sim, emb_id, is_image)) in best_by_file {
            let st = &stats[is_image as usize];
            let (passes, z, conf) = if st.is_reliable() {
                let z = st.z(sim);
                let (min_z, floor, span) = if is_image {
                    (image_min_z, profile.image_sim_floor, 2.0)
                } else {
                    (profile.text_z, profile.text_sim_floor, 3.0)
                };
                let conf = 0.55 + 0.40 * ranking::clamp01((z - min_z) / span);
                (z >= min_z && sim >= floor, z, conf)
            } else if is_image {
                let t = profile.image_sim_small_corpus;
                (sim >= t, 0.0, 0.55 + ranking::clamp01((sim - t) / 0.06) * 0.40)
            } else {
                let t = profile.text_sim_small_corpus;
                (sim >= t, 0.0, 0.50 + ranking::clamp01((sim - t) / 0.18) * 0.40)
            };
            if passes {
                if st.is_reliable() {
                    top_z = Some(top_z.map_or(z, |t: f32| t.max(z)));
                }
                admitted.push((file_id, emb_id, sim, z, conf));
            }
        }
        admitted.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));
        admitted.truncate(limit);

        let conn = self.conn.lock().unwrap();
        let mut detail = conn.prepare(
            r#"
            SELECT f.path, f.filename, e.modality, e.snippet, e.page_number, f.size_bytes
            FROM chunk_embeddings e JOIN files f ON e.file_id = f.id
            WHERE e.id = ?1
            "#,
        )?;
        let mut hits = Vec::with_capacity(admitted.len());
        for (file_id, emb_id, similarity, z_score, score) in admitted {
            let row = detail
                .query_row(params![emb_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<i32>>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                })
                .optional()?;
            if let Some((path, filename, modality, snippet, page_number, size_bytes)) = row {
                hits.push(VectorSearchResult {
                    file_id,
                    path,
                    filename,
                    score,
                    similarity,
                    z_score,
                    modality,
                    snippet,
                    page_number,
                    size_bytes,
                });
            }
        }
        Ok(VectorOutcome { hits, top_z })
    }

    /// Runs every retriever once for `intent`. Modes then fuse different subsets of the output.
    fn retrieve(&self, intent: &QueryIntent, limit: usize) -> Result<Retrieved> {
        let profile = self.relevance();
        let q = intent.text.as_str();
        let filename = self.search_filenames(q, limit.max(MAX_FILENAME_HITS))?;

        let keyword = match self.text_index {
            Some(ref text_index) => {
                group_keyword_hits(text_index.search(q, KEYWORD_CHUNK_FETCH).unwrap_or_default())
            }
            None => Vec::new(),
        };

        let ml_enabled = self.get_settings().map(|s| s.ml_enabled).unwrap_or(true);
        let (text_semantic, images) = match self.embedding_engine {
            Some(ref engine) if ml_enabled => {
                let query_vec = engine.embed_query(q).ok();
                let text_semantic = match query_vec {
                    Some(ref v) => self.vector_search(v, MAX_SEMANTIC_HITS, Some(false), 0.0)?,
                    None => VectorOutcome::default(),
                };
                let vision_vec = match query_vec {
                    Some(v) if engine.shares_text_image_space() => Ok(v),
                    _ => engine.embed_vision_query(q),
                };
                let images = if intent.allows_images() {
                    match vision_vec {
                        Ok(v) => self.vector_search(
                            &v,
                            MAX_VISUAL_IMAGE_HITS,
                            Some(true),
                            profile.image_z_visual,
                        )?,
                        Err(_) => VectorOutcome::default(),
                    }
                } else {
                    VectorOutcome::default()
                };
                (text_semantic, images)
            }
            _ => (VectorOutcome::default(), VectorOutcome::default()),
        };

        Ok(Retrieved {
            filename,
            keyword,
            text_semantic,
            images,
        })
    }

    /// Fuses retriever output for one mode with weighted reciprocal rank fusion, then applies the
    /// type filter, noise down-ranking, and duplicate collapsing.
    fn fuse(
        &self,
        intent: &QueryIntent,
        mode: SearchMode,
        retrieved: &Retrieved,
        records: &mut HashMap<String, Option<FileRecord>>,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>> {
        let is_image_name = |name: &str| {
            Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .map_or(false, crate::embeddings::is_image_supported_for_vision)
        };
        let visual = mode == SearchMode::Visual;
        let q_lower = intent.text.to_lowercase();
        let profile = self.relevance();

        let mut lists: Vec<(f32, Vec<String>)> = Vec::new();

        // 1. Filenames: whole-word matches are strong evidence, substrings inside words are weak,
        //    and for queries under four letters mostly accidental ("cat" in "communication").
        let short_query = q_lower.chars().count() < 4;
        if mode != SearchMode::Content {
            let (strong, weak): (Vec<&SearchResultItem>, Vec<&SearchResultItem>) = retrieved
                .filename
                .iter()
                .filter(|f| !visual || is_image_name(&f.filename))
                .take(MAX_FILENAME_HITS)
                .partition(|f| is_word_boundary_match(&f.filename.to_lowercase(), &q_lower));
            lists.push((1.2, strong.iter().map(|f| f.id.clone()).collect()));
            if !short_query {
                lists.push((0.5, weak.iter().map(|f| f.id.clone()).collect()));
            }
        }

        // 2. Keyword / OCR full-text hits.
        let keyword_cap = if mode == SearchMode::Content { 20 } else { 12 };
        lists.push((
            1.0,
            retrieved
                .keyword
                .iter()
                .filter(|k| !visual || is_image_name(&k.filename))
                .take(keyword_cap)
                .map(|k| k.file_id.clone())
                .collect(),
        ));

        // 3. Text-concept embeddings, weighted by how clearly the best chunk stood out.
        let text_weight = ranking::strength_weight(
            retrieved.text_semantic.top_z,
            profile.text_z,
            2.5,
            0.5,
            0.95,
        );
        lists.push((
            if visual { 0.7 } else { text_weight },
            retrieved
                .text_semantic
                .hits
                .iter()
                .filter(|h| !visual || h.modality == "ocr")
                .take(if mode == SearchMode::Content { 15 } else { 10 })
                .map(|h| h.file_id.clone())
                .collect(),
        ));

        // 4. CLIP images. Visual mode shows everything past the visual gate; the blended list only
        //    admits images that clearly stand out, weighted by that margin and the query's shape.
        if mode != SearchMode::Content {
            let (weight, ids): (f32, Vec<String>) = if visual {
                // Every clear match, plus a short tail of borderline ones.
                let mut weak = 0;
                let ids = retrieved
                    .images
                    .hits
                    .iter()
                    .filter(|h| {
                        let strong = h.z_score >= profile.image_z_blended
                            || retrieved.images.top_z.is_none();
                        weak += (!strong) as usize;
                        strong || weak <= MAX_VISUAL_WEAK_IMAGE_HITS
                    })
                    .map(|h| h.file_id.clone())
                    .collect();
                (1.0, ids)
            } else {
                let blended: Vec<String> = retrieved
                    .images
                    .hits
                    .iter()
                    .filter(|h| h.z_score >= profile.image_z_blended || retrieved.images.top_z.is_none())
                    .take(MAX_BLENDED_IMAGE_HITS)
                    .map(|h| h.file_id.clone())
                    .collect();
                let base = ranking::strength_weight(
                    retrieved.images.top_z,
                    profile.image_z_blended,
                    1.0,
                    0.55,
                    1.2,
                );
                (base * intent.image_shape_factor(), blended)
            };
            lists.push((weight, ids));
        }

        let fused = ranking::weighted_rrf(&lists);

        // Evidence lookups for building result items.
        let filename_by_id: HashMap<&str, &SearchResultItem> = retrieved
            .filename
            .iter()
            .map(|f| (f.id.as_str(), f))
            .collect();
        let keyword_by_id: HashMap<&str, &KeywordHit> = retrieved
            .keyword
            .iter()
            .map(|k| (k.file_id.as_str(), k))
            .collect();
        let semantic_by_id: HashMap<&str, &VectorSearchResult> = retrieved
            .text_semantic
            .hits
            .iter()
            .map(|h| (h.file_id.as_str(), h))
            .collect();
        let image_by_id: HashMap<&str, &VectorSearchResult> = retrieved
            .images
            .hits
            .iter()
            .map(|h| (h.file_id.as_str(), h))
            .collect();

        let mut scored: Vec<(f32, SearchResultItem, String)> = Vec::new();
        for (id, fused_score) in fused {
            if !records.contains_key(&id) {
                let rec = self.get_file_by_id(&id)?;
                records.insert(id.clone(), rec);
            }
            let rec = match records.get(&id).and_then(|r| r.as_ref()) {
                Some(r) if !r.is_deleted => r,
                _ => continue,
            };
            if !intent.allows(&rec.extension) {
                continue;
            }
            if visual && !crate::embeddings::is_image_supported_for_vision(&rec.extension) {
                continue;
            }

            let in_filename = filename_by_id.get(id.as_str()).copied();
            let in_keyword = keyword_by_id.get(id.as_str()).copied();
            let in_semantic = semantic_by_id
                .get(id.as_str())
                .copied()
                .filter(|h| !visual || h.modality == "ocr");
            let in_image = image_by_id.get(id.as_str()).copied().filter(|h| {
                visual || h.z_score >= profile.image_z_blended || retrieved.images.top_z.is_none()
            });
            if mode == SearchMode::Content && in_keyword.is_none() && in_semantic.is_none() {
                continue;
            }

            let mut multiplier = ranking::noise_factor(&rec.path, &rec.extension, rec.size_bytes);
            if intent
                .preferred_extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&rec.extension))
            {
                multiplier *= 1.3;
            }

            let item = build_result_item(
                rec,
                &intent.text,
                in_filename,
                in_keyword,
                in_semantic,
                in_image,
            );
            let dup_key = duplicate_key(rec, in_keyword, in_semantic, in_image);
            scored.push((fused_score * multiplier, item, dup_key));
        }

        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.filename.len().cmp(&b.1.filename.len()))
        });

        // Collapse identical copies (same type, size, and matched evidence) into the best-ranked one.
        let mut results: Vec<SearchResultItem> = Vec::new();
        let mut kept_by_key: HashMap<String, (usize, usize)> = HashMap::new(); // key -> (index, copies)
        let top = scored.first().map(|s| s.0).unwrap_or(1.0).max(1e-6);
        for (score, mut item, key) in scored {
            if let Some(entry) = kept_by_key.get_mut(&key) {
                entry.1 += 1;
                continue;
            }
            item.score = (score / top).min(1.0);
            kept_by_key.insert(key, (results.len(), 0));
            results.push(item);
        }
        for (idx, copies) in kept_by_key.into_values() {
            if copies > 0 {
                let label = if copies == 1 { "copy" } else { "copies" };
                results[idx]
                    .match_reasons
                    .push(format!("+{} identical {} elsewhere", copies, label));
            }
        }

        let cap = match mode {
            SearchMode::Visual => MAX_VISUAL_IMAGE_HITS,
            _ => MAX_BLENDED_RESULTS,
        };
        results.truncate(limit.min(cap));
        Ok(results)
    }

    fn search_mode(
        &self,
        query: &str,
        mode: SearchMode,
        limit: usize,
    ) -> Result<(Vec<SearchResultItem>, Option<ModeCounts>)> {
        let intent = QueryIntent::parse(query);
        if intent.text.trim().is_empty() {
            return Ok((Vec::new(), None));
        }
        let retrieved = self.retrieve(&intent, limit)?;
        let mut records: HashMap<String, Option<FileRecord>> = HashMap::new();
        let results = self.fuse(&intent, mode, &retrieved, &mut records, limit)?;

        // Chip counts: what each other mode would show for the same query (no extra retrieval).
        let mut counts = ModeCounts {
            filename: retrieved
                .filename
                .iter()
                .filter(|f| intent.allows(&f.extension))
                .count(),
            ..ModeCounts::default()
        };
        for other in [SearchMode::All, SearchMode::Content, SearchMode::Visual] {
            let n = if other == mode {
                results.len()
            } else {
                self.fuse(&intent, other, &retrieved, &mut records, limit)?.len()
            };
            match other {
                SearchMode::All => counts.all = n,
                SearchMode::Content => counts.content = n,
                SearchMode::Visual => counts.visual = n,
            }
        }
        Ok((results, Some(counts)))
    }

    /// Unified search coordinating fast SQLite filename lookup, Tantivy full-text index, and semantic vector similarity.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>> {
        Ok(self.search_mode(query, SearchMode::All, limit)?.0)
    }

    /// Content-only search (Tantivy full-text + text embeddings), excluding pure filename and visual matches.
    pub fn search_content(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>> {
        Ok(self.search_mode(query, SearchMode::Content, limit)?.0)
    }

    /// Visual/image-only search (CLIP multimodal image embeddings and OCR screenshot texts).
    pub fn search_visual(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>> {
        Ok(self.search_mode(query, SearchMode::Visual, limit)?.0)
    }

    /// Routes search according to explicit search_type filter ("filename", "content", "visual", or "hybrid"/"all").
    pub fn search_with_type(
        &self,
        query: &str,
        search_type: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>> {
        Ok(self.search_with_counts(query, search_type, limit)?.0)
    }

    /// Like [`Self::search_with_type`], also returning per-mode result counts for the filter chips.
    /// Filename mode stays a pure SQLite lookup and returns no counts.
    pub fn search_with_counts(
        &self,
        query: &str,
        search_type: Option<&str>,
        limit: usize,
    ) -> Result<(Vec<SearchResultItem>, Option<ModeCounts>)> {
        match search_type {
            Some("filename") => Ok((self.search_filenames(query, limit)?, None)),
            Some("content") => self.search_mode(query, SearchMode::Content, limit),
            Some("visual") | Some("image") | Some("images") => {
                self.search_mode(query, SearchMode::Visual, limit)
            }
            _ => self.search_mode(query, SearchMode::All, limit),
        }
    }
}

const MAX_FILENAME_HITS: usize = 10;
const KEYWORD_CHUNK_FETCH: usize = 60;
const MAX_SEMANTIC_HITS: usize = 15;
const MAX_VISUAL_IMAGE_HITS: usize = 24;
const MAX_VISUAL_WEAK_IMAGE_HITS: usize = 6;
const MAX_BLENDED_IMAGE_HITS: usize = 6;
const MAX_BLENDED_RESULTS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    All,
    Content,
    Visual,
}

#[derive(Default)]
struct VectorOutcome {
    hits: Vec<VectorSearchResult>,
    /// Best z-score among admitted hits; `None` when the corpus was too small for statistics.
    top_z: Option<f32>,
}

/// Best full-text chunk for one file.
struct KeywordHit {
    file_id: String,
    filename: String,
    snippet: String,
    page_number: Option<i32>,
    line_number: Option<i32>,
    is_ocr: bool,
}

struct Retrieved {
    filename: Vec<SearchResultItem>,
    keyword: Vec<KeywordHit>,
    text_semantic: VectorOutcome,
    images: VectorOutcome,
}

/// Minimal HTML escaping for text shown through the UI's HTML snippet renderer.
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// At least three alphabetic words of three or more letters: enough to carry meaning.
fn has_meaningful_words(text: &str) -> bool {
    text.split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() >= 3)
        .take(3)
        .count()
        >= 3
}

fn document_modality(extension: &str) -> String {
    if extension.eq_ignore_ascii_case("pdf") {
        "pdf".to_string()
    } else if crate::extract::is_text_or_code_extension(extension) {
        "code".to_string()
    } else {
        "text".to_string()
    }
}

/// Collapses chunk-level BM25 hits (already sorted best-first) to one entry per file.
fn group_keyword_hits(hits: Vec<crate::tantivy_index::FullTextHit>) -> Vec<KeywordHit> {
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|h| seen.insert(h.file_id.clone()))
        .map(|h| KeywordHit {
            is_ocr: h.modality == "ocr",
            file_id: h.file_id,
            filename: h.filename,
            snippet: h.snippet,
            page_number: h.page_number,
            line_number: h.line_number,
        })
        .collect()
}

/// True when `query` appears in `name` delimited by non-alphanumerics ("cat.png", "my_cat_pic").
fn is_word_boundary_match(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    name.match_indices(query).any(|(start, m)| {
        let end = start + m.len();
        let before_ok = start == 0 || !(bytes[start - 1] as char).is_alphanumeric();
        let after_ok = end >= bytes.len() || !(bytes[end] as char).is_alphanumeric();
        before_ok && after_ok
    })
}

/// Identity used to collapse duplicate copies. File fingerprints are size+mtime, so copies made at
/// different times differ; instead compare type, size, and the evidence that matched.
fn duplicate_key(
    rec: &FileRecord,
    keyword: Option<&KeywordHit>,
    semantic: Option<&VectorSearchResult>,
    image: Option<&VectorSearchResult>,
) -> String {
    let evidence = if let Some(img) = image {
        format!("img:{:.5}", img.similarity)
    } else if let Some(k) = keyword {
        format!("kw:{}", k.snippet.chars().take(80).collect::<String>())
    } else if let Some(s) = semantic {
        format!("sem:{}", s.snippet.chars().take(80).collect::<String>())
    } else {
        format!("name:{}", rec.filename.to_lowercase())
    };
    format!("{}:{}:{}", rec.extension.to_lowercase(), rec.size_bytes, evidence)
}

fn build_result_item(
    rec: &FileRecord,
    query_text: &str,
    filename: Option<&SearchResultItem>,
    keyword: Option<&KeywordHit>,
    semantic: Option<&VectorSearchResult>,
    image: Option<&VectorSearchResult>,
) -> SearchResultItem {
    let mut reasons: Vec<String> = Vec::new();
    if let Some(f) = filename {
        reasons.extend(f.match_reasons.iter().cloned());
    }
    if let Some(k) = keyword {
        reasons.push(if k.is_ocr {
            "Screenshot OCR match".to_string()
        } else if let Some(p) = k.page_number {
            format!("Full-text match (Page {})", p)
        } else if let Some(l) = k.line_number {
            format!("Full-text match (Line {})", l)
        } else {
            "Full-text match (Document body)".to_string()
        });
    }
    if let Some(s) = semantic {
        reasons.push(if s.modality == "ocr" {
            format!("OCR text match ({:.0}% relevance)", s.score * 100.0)
        } else {
            format!("Semantic concept match ({:.0}% relevance)", s.score * 100.0)
        });
    }
    if let Some(i) = image {
        reasons.push(format!(
            "Visual semantic match ({:.0}% relevance)",
            i.score * 100.0
        ));
    }

    let match_type = match (filename, keyword, semantic) {
        (_, Some(k), _) if k.is_ocr => MatchType::Ocr,
        (Some(_), _, _) => MatchType::Filename,
        (None, Some(_), _) => MatchType::Keyword,
        (None, None, Some(s)) if s.modality == "ocr" => MatchType::Ocr,
        _ => MatchType::Semantic,
    };

    // Snippets reach the UI as HTML (keyword highlights use <b>). Tantivy escapes its own
    // fragments; everything else is raw file or OCR text and must be escaped here, or a file
    // containing markup could inject script into the app.
    let snippet = keyword
        .map(|k| k.snippet.clone())
        .or_else(|| semantic.map(|s| escape_html(&s.snippet)))
        .or_else(|| {
            image.map(|_| format!("Visual contents match \"{}\"", escape_html(query_text)))
        });
    let page_number = keyword
        .and_then(|k| k.page_number)
        .or_else(|| semantic.and_then(|s| s.page_number));

    SearchResultItem {
        id: rec.id.clone(),
        path: rec.path.clone(),
        filename: rec.filename.clone(),
        extension: rec.extension.clone(),
        size_bytes: rec.size_bytes,
        modified_at: rec.modified_at,
        score: 0.0,
        match_type,
        match_reasons: reasons,
        snippet,
        page_number,
    }
}

impl Database {

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

    pub fn get_ocr_data(
        &self,
        file_id: &str,
    ) -> Result<Option<(String, f32, Vec<crate::models::OcrWordRegion>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT full_text, confidence, regions_json FROM ocr_data WHERE file_id = ?1",
        )?;
        let res = stmt.query_row(params![file_id], |row| {
            let text: String = row.get(0)?;
            let conf: f64 = row.get(1)?;
            let json: String = row.get(2)?;
            Ok((text, conf as f32, json))
        });

        match res {
            Ok((text, conf, json)) => {
                let regions: Vec<crate::models::OcrWordRegion> =
                    serde_json::from_str(&json).unwrap_or_default();
                Ok(Some((text, conf, regions)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SiftError::Database(e)),
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

    pub fn get_total_embeddings(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |r| r.get(0))
            .unwrap_or(0);
        Ok(total)
    }

    pub fn delete_file_by_path(&self, path: &str) -> Result<bool> {
        if let Some(ref text_index) = self.text_index {
            let _ = text_index.delete_path(path);
            let _ = text_index.commit();
        }
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM files WHERE path = ?1", params![path])?;
        Ok(affected > 0)
    }

    pub fn delete_files_under_path(&self, dir_prefix: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}/%", dir_prefix.trim_end_matches('/'));
        if let Some(ref text_index) = self.text_index {
            if let Ok(mut stmt) = conn.prepare("SELECT id FROM files WHERE path LIKE ?1") {
                if let Ok(ids) = stmt.query_map(params![pattern], |row| row.get::<_, String>(0)) {
                    for id in ids.flatten() {
                        let _ = text_index.delete_file(&id);
                    }
                    let _ = text_index.commit();
                }
            }
        }
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
                    if let Some(ref text_index) = self.text_index {
                        let _ = text_index.delete_path(&old_path);
                    }
                    del_stmt.execute(params![old_path])?;
                    deleted_count += 1;
                }
            }
        }

        if let Some(ref text_index) = self.text_index {
            let _ = text_index.commit();
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
        assert_eq!(default_settings.global_shortcut, "Ctrl+Alt+Space");



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
