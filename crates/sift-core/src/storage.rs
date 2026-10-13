use crate::error::{Result, SiftError};
use crate::models::{AppSettings, FileRecord, MatchType, RootFolder, SearchResultItem};
use crate::scanner::FileMeta;
use chrono::Utc;
use rusqlite::{params, Connection};
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
    pub score: f32,
    pub modality: String,
    pub snippet: String,
    pub page_number: Option<i32>,
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

                let mut id_lookup = tx.prepare("SELECT id FROM files WHERE path = ?1")?;

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

                    if let Ok(actual_id) =
                        id_lookup.query_row(params![file.path], |row| row.get::<_, String>(0))
                    {
                        file_records_to_index.push((
                            file.path.clone(),
                            file.filename.clone(),
                            file.extension.clone(),
                            actual_id,
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

        let is_eco = self
            .get_settings()
            .map(|s| s.resource_mode == crate::models::ResourceMode::Eco)
            .unwrap_or(false);

        // Ingest and index document chunks into Tantivy and vector embeddings
        for (path, filename, extension, id) in file_records_to_index {
            let is_image = crate::embeddings::is_image_supported_for_vision(&extension);
            let is_ocr = crate::ocr::is_ocr_supported_extension(&extension);

            // 1. If it's an image, embed visual vector if embedding engine is active
            if is_image {
                if let Some(ref engine) = self.embedding_engine {
                    if is_eco {
                        // Eco mode throttles background heavy ML passes to conserve CPU and battery
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    match engine.embed_image(Path::new(&path)) {

                        Ok(img_vec) => {
                            let blob = crate::embeddings::serialize_vector(&img_vec);
                            let emb_id = Uuid::new_v4().to_string();
                            let now = Utc::now().timestamp();
                            let conn = self.conn.lock().unwrap();
                            let _ = conn.execute(
                                "DELETE FROM chunk_embeddings WHERE file_id = ?1 AND modality = 'image'",
                                params![id],
                            );
                            let _ = conn.execute(
                                r#"
                                INSERT INTO chunk_embeddings (id, file_id, chunk_index, modality, page_number, line_number, snippet, embedding, created_at)
                                VALUES (?1, ?2, 0, 'image', NULL, NULL, ?3, ?4, ?5)
                                "#,
                                params![emb_id, id, filename, blob, now],
                            );
                            tracing::info!(path = %path, "Visual vector generated successfully");
                        }
                        Err(e) => {
                            tracing::warn!(path = %path, error = %e, "Failed to generate visual image embedding");
                        }
                    }
                }
            }

            // 2. If OCR is supported on this image, extract text
            if is_ocr {
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
                        if let Some(ref engine) = self.embedding_engine {
                            if let Ok(ocr_vec) = engine.embed_text(&ocr_res.full_text) {
                                let blob = crate::embeddings::serialize_vector(&ocr_vec);
                                let emb_id = Uuid::new_v4().to_string();
                                let now = Utc::now().timestamp();
                                let snippet = safe_truncate_str(&ocr_res.full_text, 247);
                                let conn = self.conn.lock().unwrap();
                                let _ = conn.execute(
                                    "DELETE FROM chunk_embeddings WHERE file_id = ?1 AND modality = 'ocr'",
                                    params![id],
                                );
                                let _ = conn.execute(
                                    r#"
                                    INSERT INTO chunk_embeddings (id, file_id, chunk_index, modality, page_number, line_number, snippet, embedding, created_at)
                                    VALUES (?1, ?2, 0, 'ocr', NULL, NULL, ?3, ?4, ?5)
                                    "#,
                                    params![emb_id, id, snippet, blob, now],
                                );
                            }
                        }
                    }
                }
            } else {
                // 3. Document or source code file
                let chunks = extract_document_chunks(Path::new(&path)).unwrap_or_default();
                if !chunks.is_empty() {
                    let modality = if extension == "pdf" {
                        "pdf"
                    } else if crate::extract::is_text_or_code_extension(&extension) {
                        "code"
                    } else {
                        "text"
                    };

                    if let Some(ref text_index) = self.text_index {
                        let _ =
                            text_index.upsert_file_chunks(&id, &path, &filename, modality, &chunks);
                    }

                    if let Some(ref engine) = self.embedding_engine {
                        let conn = self.conn.lock().unwrap();
                        let _ = conn.execute(
                            "DELETE FROM chunk_embeddings WHERE file_id = ?1 AND modality != 'image'",
                            params![id],
                        );
                        drop(conn);

                        for chunk in &chunks {
                            if let Ok(chunk_vec) = engine.embed_text(&chunk.content) {
                                let blob = crate::embeddings::serialize_vector(&chunk_vec);
                                let emb_id = Uuid::new_v4().to_string();
                                let now = Utc::now().timestamp();
                                let snippet = safe_truncate_str(&chunk.content, 247);
                                let conn = self.conn.lock().unwrap();
                                let _ = conn.execute(
                                    r#"
                                    INSERT INTO chunk_embeddings (id, file_id, chunk_index, modality, page_number, line_number, snippet, embedding, created_at)
                                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                                    "#,
                                    params![
                                        emb_id,
                                        id,
                                        chunk.chunk_index,
                                        modality,
                                        chunk.page_number,
                                        chunk.line_number,
                                        snippet,
                                        blob,
                                        now,
                                    ],
                                );
                            }
                        }
                    }
                }
            }
        }

        if let Some(ref text_index) = self.text_index {
            let _ = text_index.commit();
        }

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

    pub fn search_vectors(
        &self,
        query_vec: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        self.search_vectors_by_modality(query_vec, limit, None)
    }

    pub fn search_vectors_by_modality(
        &self,
        query_vec: &[f32],
        limit: usize,
        is_image_only: Option<bool>,
    ) -> Result<Vec<VectorSearchResult>> {
        let conn = self.conn.lock().unwrap();
        let query_sql = match is_image_only {
            Some(true) => {
                r#"
                SELECT e.file_id, f.path, f.filename, e.modality, e.snippet, e.page_number, e.embedding
                FROM chunk_embeddings e
                JOIN files f ON e.file_id = f.id
                WHERE f.is_deleted = 0 AND e.modality = 'image'
                "#
            }
            Some(false) => {
                r#"
                SELECT e.file_id, f.path, f.filename, e.modality, e.snippet, e.page_number, e.embedding
                FROM chunk_embeddings e
                JOIN files f ON e.file_id = f.id
                WHERE f.is_deleted = 0 AND e.modality != 'image'
                "#
            }
            None => {
                r#"
                SELECT e.file_id, f.path, f.filename, e.modality, e.snippet, e.page_number, e.embedding
                FROM chunk_embeddings e
                JOIN files f ON e.file_id = f.id
                WHERE f.is_deleted = 0
                "#
            }
        };

        let mut stmt = conn.prepare(query_sql)?;
        let mut best_by_file: std::collections::HashMap<String, VectorSearchResult> =
            std::collections::HashMap::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let file_id: String = row.get(0)?;
            let path: String = row.get(1)?;
            let filename: String = row.get(2)?;
            let modality: String = row.get(3)?;
            let snippet: String = row.get(4)?;
            let page_number: Option<i32> = row.get(5)?;
            let blob: Vec<u8> = row.get(6)?;

            let vec = crate::embeddings::deserialize_vector(&blob);
            let sim = crate::embeddings::cosine_similarity(query_vec, &vec);
            let threshold = if modality == "image" { 0.18 } else { 0.42 };
            if sim > threshold {
                match best_by_file.entry(file_id.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut occ) => {
                        if sim > occ.get().score {
                            occ.insert(VectorSearchResult {
                                file_id,
                                path,
                                filename,
                                score: sim,
                                modality,
                                snippet,
                                page_number,
                            });
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(vac) => {
                        vac.insert(VectorSearchResult {
                            file_id,
                            path,
                            filename,
                            score: sim,
                            modality,
                            snippet,
                            page_number,
                        });
                    }
                }
            }
        }

        let mut candidates: Vec<VectorSearchResult> = best_by_file.into_values().collect();
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(limit);
        Ok(candidates)
    }

    /// Unified search coordinating fast SQLite filename lookup, Tantivy full-text index, and semantic vector similarity.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>> {
        let q_trimmed = query.trim();
        if q_trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // 1. Filename matches
        let filename_matches = self.search_filenames(q_trimmed, limit)?;

        // 2. Full-text hits from Tantivy
        let text_hits = if let Some(ref text_index) = self.text_index {
            text_index.search(q_trimmed, limit).unwrap_or_default()
        } else {
            Vec::new()
        };

        // 3. Semantic vector hits
        let vector_hits = if let Some(ref engine) = self.embedding_engine {
            let mut hits = Vec::new();
            // 3a. Search conceptual document chunks with text query vector
            if let Ok(q_vec) = engine.embed_text(q_trimmed) {
                if let Ok(text_hits) = self.search_vectors_by_modality(&q_vec, limit, Some(false)) {
                    hits.extend(text_hits);
                }
            }
            // 3b. Search visual image chunks with cross-modal vision query vector
            if let Ok(vision_q_vec) = engine.embed_vision_query(q_trimmed) {
                if let Ok(mut img_hits) = self.search_vectors_by_modality(&vision_q_vec, limit, Some(true)) {
                    // Rescale cross-modal visual similarity so visual matches compete fairly with text matches
                    for h in &mut img_hits {
                        h.score = (h.score * 2.2).min(1.0);
                    }
                    hits.extend(img_hits);
                }
            }
            hits.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            hits.truncate(limit);
            hits
        } else {
            Vec::new()
        };

        if text_hits.is_empty() && vector_hits.is_empty() {
            return Ok(filename_matches);
        }

        // 4. Deduplicate and merge
        let mut merged: HashMap<String, SearchResultItem> = HashMap::new();

        // Add filename matches
        for item in filename_matches {
            merged.insert(item.id.clone(), item);
        }

        // Merge full-text hits
        for hit in text_hits {
            let is_ocr = hit.modality == "ocr";
            if let Some(existing) = merged.get_mut(&hit.file_id) {
                // Both filename and full-text content matched
                existing.score += 0.25;
                if is_ocr {
                    existing.match_type = MatchType::Ocr;
                    existing
                        .match_reasons
                        .push("Screenshot OCR match".to_string());
                } else {
                    let provenance = if let Some(p) = hit.page_number {
                        format!("Content matched on Page {}", p)
                    } else if let Some(l) = hit.line_number {
                        format!("Content matched at line {}", l)
                    } else {
                        "Content matched query".to_string()
                    };
                    existing.match_reasons.push(provenance);
                }
                if existing.snippet.is_none() {
                    existing.snippet = Some(hit.snippet);
                }
                if existing.page_number.is_none() {
                    existing.page_number = hit.page_number;
                }
            } else {
                // Content matched, but filename didn't
                if let Ok(Some(file_rec)) = self.get_file_by_id(&hit.file_id) {
                    if file_rec.is_deleted {
                        continue;
                    }
                    let match_type = if is_ocr {
                        MatchType::Ocr
                    } else {
                        MatchType::Keyword
                    };
                    let provenance = if is_ocr {
                        "Screenshot OCR match".to_string()
                    } else if let Some(p) = hit.page_number {
                        format!("Page {}", p)
                    } else if let Some(l) = hit.line_number {
                        format!("Line {}", l)
                    } else {
                        "Document body".to_string()
                    };

                    merged.insert(
                        hit.file_id.clone(),
                        SearchResultItem {
                            id: hit.file_id,
                            path: hit.path,
                            filename: hit.filename,
                            extension: file_rec.extension,
                            size_bytes: file_rec.size_bytes,
                            modified_at: file_rec.modified_at,
                            score: 0.55 + (hit.score.min(10.0) / 25.0),
                            match_type,
                            match_reasons: vec![format!("Full-text match ({})", provenance)],
                            snippet: Some(hit.snippet),
                            page_number: hit.page_number,
                        },
                    );
                }
            }
        }

        // Merge semantic vector hits
        for hit in vector_hits {
            let is_visual = hit.modality == "image";
            if let Some(existing) = merged.get_mut(&hit.file_id) {
                // Both previous matches (filename/full-text) and semantic matched
                existing.score = (existing.score + hit.score * 0.35).min(1.0);
                let reason = if is_visual {
                    format!(
                        "Visual semantic match ({:.0}% relevance)",
                        hit.score * 100.0
                    )
                } else {
                    format!(
                        "Semantic concept match ({:.0}% relevance)",
                        hit.score * 100.0
                    )
                };
                if !existing
                    .match_reasons
                    .iter()
                    .any(|r| r.starts_with("Semantic") || r.starts_with("Visual"))
                {
                    existing.match_reasons.push(reason);
                }
                if existing.snippet.is_none() {
                    existing.snippet = Some(hit.snippet);
                }
                if existing.page_number.is_none() {
                    existing.page_number = hit.page_number;
                }
            } else {
                // Only semantic matched
                if let Ok(Some(file_rec)) = self.get_file_by_id(&hit.file_id) {
                    if file_rec.is_deleted {
                        continue;
                    }
                    let (reason, snippet, score) = if is_visual {
                        (
                            format!(
                                "Visual semantic match ({:.0}% similarity)",
                                hit.score * 100.0
                            ),
                            Some(format!("Visual contents match \"{}\"", q_trimmed)),
                            (0.65 + (hit.score * 0.35)).min(1.0),
                        )
                    } else {
                        (
                            format!(
                                "Semantic concept match ({:.0}% similarity)",
                                hit.score * 100.0
                            ),
                            Some(hit.snippet),
                            (0.50 + (hit.score * 0.40)).min(1.0),
                        )
                    };

                    merged.insert(
                        hit.file_id.clone(),
                        SearchResultItem {
                            id: hit.file_id,
                            path: hit.path,
                            filename: hit.filename,
                            extension: file_rec.extension,
                            size_bytes: file_rec.size_bytes,
                            modified_at: file_rec.modified_at,
                            score,
                            match_type: MatchType::Semantic,
                            match_reasons: vec![reason],
                            snippet,
                            page_number: hit.page_number,
                        },
                    );
                }
            }
        }


        let mut results: Vec<SearchResultItem> = merged.into_values().collect();

        // Sort by score descending, then filename length
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
