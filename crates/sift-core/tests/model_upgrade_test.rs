use sift_core::embeddings::{DeterministicMockEngine, EmbeddingEngine};
use sift_core::scanner::scan_directory;
use sift_core::storage::Database;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tempfile::Builder;

/// The mock encoder under a different model id, standing in for a model upgrade.
struct UpgradedEngine(DeterministicMockEngine);

impl EmbeddingEngine for UpgradedEngine {
    fn model_name(&self) -> &str {
        "upgraded-test-model"
    }
    fn dimension(&self) -> usize {
        self.0.dimension()
    }
    fn embed_texts(&self, texts: &[String]) -> sift_core::Result<Vec<Vec<f32>>> {
        self.0.embed_texts(texts)
    }
    fn embed_image(&self, image_path: &Path) -> sift_core::Result<Vec<f32>> {
        self.0.embed_image(image_path)
    }
}

#[test]
fn model_upgrade_reembeds_stale_vectors_and_never_mixes_spaces() {
    let dir = Builder::new().prefix("sift_upgrade_").tempdir().unwrap();
    File::create(dir.path().join("network_worker.rs"))
        .unwrap()
        .write_all(b"fn handle_pool_failure() { client.retry(); exponential_backoff_delay(); }")
        .unwrap();

    let old: Arc<dyn EmbeddingEngine> = Arc::new(DeterministicMockEngine::new(768));
    let db = Database::open_in_memory()
        .unwrap()
        .with_embedding_engine(Some(old));
    let root = db.add_root(dir.path().to_str().unwrap()).unwrap();
    db.upsert_files_batch(&root.id, &scan_directory(dir.path(), &[]).unwrap())
        .unwrap();
    assert_eq!(db.count_stale_embedding_files().unwrap(), 0);
    assert!(!db.search("reconnect database", 10).unwrap().is_empty());

    // Switch engines: every existing vector is now stale and must not be searched.
    let upgraded: Arc<dyn EmbeddingEngine> =
        Arc::new(UpgradedEngine(DeterministicMockEngine::new(768)));
    let db = db.with_embedding_engine(Some(upgraded));
    assert_eq!(db.count_stale_embedding_files().unwrap(), 1);
    let before = db.search_content("reconnect database", 10).unwrap();
    assert!(
        before.iter().all(|r| !r.match_reasons.iter().any(|m| m.contains("Semantic"))),
        "vectors from another model must not be compared: {:?}",
        before
    );

    let mut calls = 0;
    let processed = db
        .reembed_stale(|done, total| {
            calls += 1;
            assert!(done <= total);
            true
        })
        .unwrap();
    assert_eq!((processed, calls), (1, 1));
    assert_eq!(db.count_stale_embedding_files().unwrap(), 0);

    let after = db.search("reconnect database", 10).unwrap();
    assert_eq!(after[0].filename, "network_worker.rs");
    assert!(after[0].match_reasons.iter().any(|m| m.contains("Semantic")));
}

#[test]
fn ml_toggle_disables_semantic_retrieval() {
    let dir = Builder::new().prefix("sift_ml_toggle_").tempdir().unwrap();
    File::create(dir.path().join("network_worker.rs"))
        .unwrap()
        .write_all(b"fn handle_pool_failure() { client.retry(); }")
        .unwrap();
    let engine: Arc<dyn EmbeddingEngine> = Arc::new(DeterministicMockEngine::new(768));
    let db = Database::open_in_memory()
        .unwrap()
        .with_embedding_engine(Some(engine));
    let root = db.add_root(dir.path().to_str().unwrap()).unwrap();
    db.upsert_files_batch(&root.id, &scan_directory(dir.path(), &[]).unwrap())
        .unwrap();
    assert!(!db.search("reconnect database", 10).unwrap().is_empty());

    let mut settings = db.get_settings().unwrap();
    settings.ml_enabled = false;
    db.save_settings(&settings).unwrap();
    assert!(db.search("reconnect database", 10).unwrap().is_empty());
}
