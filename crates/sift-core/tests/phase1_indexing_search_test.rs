use sift_core::scanner::{read_file_preview, scan_directory};
use sift_core::storage::Database;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

#[test]
fn test_phase1_end_to_end_indexing_and_search() {
    let db = Database::open_in_memory().expect("open database");
    let fixtures = fixtures_dir();
    assert!(fixtures.exists(), "Fixtures dir must exist");

    // 1. Add approved root
    let root_path_str = fixtures.to_str().unwrap();
    let root = db.add_root(root_path_str).expect("add root");
    assert_eq!(root.path, root_path_str);

    // 2. Scan directory applying default exclusions
    let exclusions = vec![
        ".git".to_string(),
        "node_modules".to_string(),
        "target".to_string(),
    ];
    let discovered_files = scan_directory(&fixtures, &exclusions).expect("scan fixtures directory");
    assert!(
        !discovered_files.is_empty(),
        "Should discover fixture files"
    );

    // Verify exclusions at scan time: no node_modules or .git
    for f in &discovered_files {
        assert!(
            !f.path.contains("node_modules"),
            "Must not index node_modules: {}",
            f.path
        );
        assert!(
            !f.path.contains("/.git/"),
            "Must not index .git: {}",
            f.path
        );
    }

    // 3. Batch upsert into database catalog
    let count = db
        .upsert_files_batch(&root.id, &discovered_files)
        .expect("upsert files");
    assert_eq!(count, discovered_files.len());

    let (roots_count, files_count) = db.get_stats().expect("get stats");
    assert_eq!(roots_count, 1);
    assert_eq!(files_count, discovered_files.len() as i64);

    // 4. Filename Search Tests (Non-mocked, real SQLite lookups)

    // A) Search "quarterly" -> returns 2 duplicate reports from folder_a and folder_b
    let quarterly_matches = db
        .search_filenames("quarterly", 10)
        .expect("search quarterly");
    assert_eq!(quarterly_matches.len(), 2, "Expected 2 quarterly reports");
    assert!(quarterly_matches[0].path.contains("quarterly_report.txt"));
    assert!(quarterly_matches[1].path.contains("quarterly_report.txt"));
    assert_ne!(quarterly_matches[0].path, quarterly_matches[1].path);

    // B) Search "passage" -> returns minimal_passage.pdf
    let passage_matches = db.search_filenames("passage", 10).expect("search passage");
    assert_eq!(passage_matches.len(), 1);
    assert_eq!(passage_matches[0].filename, "minimal_passage.pdf");

    // C) Search "notes" -> returns sample_notes.md
    let notes_matches = db.search_filenames("notes", 10).expect("search notes");
    assert_eq!(notes_matches.len(), 1);
    assert_eq!(notes_matches[0].filename, "sample_notes.md");

    // D) Search "dummy_pkg" (in node_modules) -> 0 matches
    let excluded_matches = db
        .search_filenames("dummy_pkg", 10)
        .expect("search excluded");
    assert_eq!(
        excluded_matches.len(),
        0,
        "Excluded files must not be found"
    );

    // 5. Preview Content Verification
    let notes_file_path = fixtures.join("documents").join("sample_notes.md");
    let preview = read_file_preview(&notes_file_path, 4096).expect("read notes preview");
    assert!(preview.contains("Sift is a high-performance local desktop search utility"));

    let corrupt_file_path = fixtures.join("corrupted").join("corrupt_file.bin");
    let corrupt_preview =
        read_file_preview(&corrupt_file_path, 4096).expect("read corrupt preview");
    assert!(!corrupt_preview.is_empty());

    // 6. Root Removal Cleans Up
    let removed = db.remove_root(root_path_str).expect("remove root");
    assert!(removed);
    let roots_after = db.list_roots().expect("list roots");
    assert!(roots_after.is_empty());
}
