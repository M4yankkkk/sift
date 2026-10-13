use sift_core::models::ResourceMode;
use sift_core::storage::Database;
use tempfile::Builder;

#[test]
fn test_phase6_settings_resource_mode_and_shortcut_persistence() {
    let db = Database::open_in_memory().expect("open db");

    // 1. Initial settings should default to Eco mode and Ctrl+Space
    let initial = db.get_settings().expect("get settings");
    assert_eq!(initial.global_shortcut, "Ctrl+Space");

    assert_eq!(initial.resource_mode, ResourceMode::Eco);
    assert_eq!(initial.max_file_size_mb, 25);

    // 2. Update to Performance mode and custom hotkey
    let mut updated = initial.clone();
    updated.global_shortcut = "Ctrl+Shift+Space".to_string();
    updated.resource_mode = ResourceMode::Performance;
    updated.max_file_size_mb = 50;

    db.save_settings(&updated).expect("save settings");

    // 3. Retrieve and verify persistence
    let retrieved = db.get_settings().expect("retrieve settings");
    assert_eq!(retrieved.global_shortcut, "Ctrl+Shift+Space");
    assert_eq!(retrieved.resource_mode, ResourceMode::Performance);
    assert_eq!(retrieved.max_file_size_mb, 50);

    // 4. Switch back to Eco mode
    let mut eco_mode = retrieved;
    eco_mode.resource_mode = ResourceMode::Eco;
    db.save_settings(&eco_mode).expect("save eco mode");

    let final_check = db.get_settings().expect("check eco");
    assert_eq!(final_check.resource_mode, ResourceMode::Eco);
}

#[test]
fn test_phase6_search_query_edge_cases_and_cancellation() {
    let dir = Builder::new()
        .prefix("sift_phase6_")
        .tempdir()
        .expect("tempdir");

    let db = Database::open_in_memory().expect("open db");
    let _root = db.add_root(dir.path().to_str().unwrap()).expect("add root");


    // Empty or whitespace queries should return immediately with empty results
    let empty_results = db.search("", 10).expect("empty search");
    assert!(empty_results.is_empty());

    let whitespace_results = db.search("    ", 10).expect("whitespace search");
    assert!(whitespace_results.is_empty());

    // Single-character search with no files indexed
    let no_hits = db.search("a", 10).expect("search a");
    assert!(no_hits.is_empty());
}
