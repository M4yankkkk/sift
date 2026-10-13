use sift_core::embeddings::{DeterministicMockEngine, EmbeddingEngine};
use sift_core::models::MatchType;
use sift_core::scanner::scan_directory;
use sift_core::storage::Database;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use tempfile::Builder;

#[test]
fn test_phase5_paraphrased_concept_retrieval() {
    let dir = Builder::new()
        .prefix("sift_phase5_semantic_")
        .tempdir()
        .expect("tempdir");

    // 1. Create a document describing connection retries and exponential backoff
    // Notice: The file filename and body DO NOT contain the words "reconnect" or "database"
    let doc_path = dir.path().join("network_worker.rs");
    {
        let mut f = File::create(&doc_path).expect("create file");
        f.write_all(
            b"// Connection pool implementation\n\
            pub fn handle_pool_failure() {\n\
                let max_retries = 5;\n\
                exponential_backoff_delay(max_retries);\n\
                client.retry();\n\
            }\n",
        )
        .expect("write");
    }

    // 2. Create an unrelated document
    let recipe_path = dir.path().join("baking_recipes.txt");
    {
        let mut f = File::create(&recipe_path).expect("create file");
        f.write_all(b"Chocolate chip cookies with sugar, flour, and butter.")
            .expect("write");
    }

    // 3. Initialize in-memory database with semantic embedding engine
    let engine: Arc<dyn EmbeddingEngine> = Arc::new(DeterministicMockEngine::new(768));
    let db = Database::open_in_memory()
        .expect("open in memory")
        .with_embedding_engine(Some(engine));

    let root = db.add_root(dir.path().to_str().unwrap()).expect("add root");
    let files = scan_directory(dir.path(), &[]).expect("scan");
    let upserted = db
        .upsert_files_batch(&root.id, &files)
        .expect("upsert files batch");
    assert_eq!(upserted, 2);

    // 4. Paraphrased query: "reconnect database"
    // Filename search for "reconnect" will find 0 matches
    let filename_hits = db
        .search_filenames("reconnect", 10)
        .expect("search filenames");
    assert_eq!(
        filename_hits.len(),
        0,
        "Filename search must not match paraphrased term"
    );

    // 5. Semantic unified search: should find network_worker.rs via concept embedding
    let results = db.search("reconnect database", 10).expect("search");
    assert!(
        !results.is_empty(),
        "Semantic search must retrieve conceptually related passage"
    );

    let top = &results[0];
    assert_eq!(top.filename, "network_worker.rs");
    assert_eq!(top.match_type, MatchType::Semantic);
    assert!(top.score >= 0.5);
    assert!(
        top.match_reasons[0].contains("Semantic"),
        "Match reasons must reflect semantic retrieval"
    );
}

#[test]
fn test_phase5_visual_multimodal_image_search() {
    let dir = Builder::new()
        .prefix("sift_phase5_vision_")
        .tempdir()
        .expect("tempdir");

    // Create image file representing scenery
    let image_path = dir.path().join("sunset_beach_photo.png");
    {
        let mut f = File::create(&image_path).expect("create image file");
        f.write_all(b"FAKE_PNG_HEADER_PHOTO_CONTENT_SUNSET")
            .expect("write bytes");
    }

    let engine: Arc<dyn EmbeddingEngine> = Arc::new(DeterministicMockEngine::new(768));
    let db = Database::open_in_memory()
        .expect("open db")
        .with_embedding_engine(Some(engine));

    let root = db.add_root(dir.path().to_str().unwrap()).expect("add root");
    let files = scan_directory(dir.path(), &[]).expect("scan");
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    // Search using conceptual phrase that is not an exact match
    let results = db.search("sunset ocean", 10).expect("search");
    assert!(
        !results.is_empty(),
        "Multimodal search must find the visual photo"
    );

    let hit = &results[0];
    assert_eq!(hit.filename, "sunset_beach_photo.png");
    assert_eq!(hit.match_type, MatchType::Semantic);
    assert!(
        hit.match_reasons
            .iter()
            .any(|r| r.contains("Visual semantic")),
        "Match reason must indicate visual semantic match: {:?}",
        hit.match_reasons
    );
}

#[test]
fn test_phase5_hybrid_reciprocal_rank_fusion() {
    let dir = Builder::new()
        .prefix("sift_phase5_rrf_")
        .tempdir()
        .expect("tempdir");

    // File A matches BOTH keyword and semantic concept
    let file_a = dir.path().join("auth_service.rs");
    {
        let mut f = File::create(&file_a).expect("create file");
        f.write_all(b"fn authenticate_user() { database pool retry(); }")
            .expect("write");
    }

    // File B matches ONLY semantically (no exact keyword match in name or text)
    let file_b = dir.path().join("connection_helper.rs");
    {
        let mut f = File::create(&file_b).expect("create file");
        f.write_all(b"fn handle_backoff() { exponential_backoff_delay(); }")
            .expect("write");
    }

    let engine: Arc<dyn EmbeddingEngine> = Arc::new(DeterministicMockEngine::new(768));
    let db = Database::open_in_memory()
        .expect("open db")
        .with_embedding_engine(Some(engine));

    let root = db.add_root(dir.path().to_str().unwrap()).expect("add root");
    let files = scan_directory(dir.path(), &[]).expect("scan");
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    let results = db.search("database retry", 10).expect("search");
    assert!(results.len() >= 2);

    // File A should outrank File B due to hybrid keyword + semantic fusion boost
    assert_eq!(results[0].filename, "auth_service.rs");
    assert!(results[0].score > results[1].score);


}

#[test]
fn test_phase5_embedding_invalidation_on_deletion() {
    let dir = Builder::new()
        .prefix("sift_phase5_del_")
        .tempdir()
        .expect("tempdir");

    let doc_path = dir.path().join("critical_crash.rs");
    {
        let mut f = File::create(&doc_path).expect("create file");
        f.write_all(b"Fatal SIGSEGV exception in task interrupt switch")
            .expect("write");
    }

    let engine: Arc<dyn EmbeddingEngine> = Arc::new(DeterministicMockEngine::new(768));
    let db = Database::open_in_memory()
        .expect("open db")
        .with_embedding_engine(Some(engine.clone()));

    let root = db.add_root(dir.path().to_str().unwrap()).expect("add root");
    let files = scan_directory(dir.path(), &[]).expect("scan");
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    // Check it matches semantically before deletion
    let before = db.search("crash memory", 10).expect("search before");
    assert_eq!(before.len(), 1);

    // Delete the file and reconcile
    fs::remove_file(&doc_path).expect("remove file");
    let remaining_paths: Vec<String> = scan_directory(dir.path(), &[])
        .unwrap()
        .into_iter()
        .map(|f| f.path)
        .collect();
    let reconciled = db
        .reconcile_root(&root.id, &remaining_paths)
        .expect("reconcile");
    assert_eq!(reconciled, 1);

    // Query again: should return 0 results
    let after = db.search("crash memory", 10).expect("search after");
    assert_eq!(
        after.len(),
        0,
        "Deleted file embeddings must be invalidated and purged"
    );
}

#[test]
fn test_phase5_graceful_degradation_when_ml_disabled() {
    let dir = Builder::new()
        .prefix("sift_phase5_disabled_")
        .tempdir()
        .expect("tempdir");

    let file_path = dir.path().join("simple_notes.md");
    {
        let mut f = File::create(&file_path).expect("create file");
        f.write_all(b"# Meeting notes\nAgenda item 1: release date schedule")
            .expect("write");
    }

    // Initialize database WITHOUT embedding engine (ml_enabled: false)
    let db = Database::open_in_memory().expect("open db");
    assert!(db.embedding_engine.is_none());

    let root = db.add_root(dir.path().to_str().unwrap()).expect("add root");
    let files = scan_directory(dir.path(), &[]).expect("scan");
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    // Filename search works seamlessly
    let hits = db.search("simple_notes", 10).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].match_type, MatchType::Filename);

    // Keyword search works seamlessly
    let keyword_hits = db.search("Agenda", 10).expect("search");
    assert_eq!(keyword_hits.len(), 1);
    assert_eq!(keyword_hits[0].match_type, MatchType::Keyword);
}
