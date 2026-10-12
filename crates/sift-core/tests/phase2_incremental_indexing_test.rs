use sift_core::scanner::read_file_metadata;
use sift_core::storage::Database;
use sift_core::watcher::FileWatcher;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_phase2_incremental_watcher_create_modify_delete_move() {
    let temp_root = tempfile::Builder::new()
        .prefix("sift_phase2_")
        .tempdir()
        .expect("create temp dir");
    let root_path = temp_root.path();

    // 1. Setup in-memory database and register root
    let db = Arc::new(Database::open_in_memory().expect("open db"));
    let root = db.add_root(&root_path.to_string_lossy()).expect("add root");

    // 2. Initial state: empty directory
    let (_, initial_count) = db.get_stats().expect("get stats");
    assert_eq!(initial_count, 0);

    // 3. Initialize FileWatcher with 100ms debouncing for fast test execution
    let exclusions = vec![
        "node_modules".to_string(),
        ".git".to_string(),
        "target".to_string(),
    ];
    let watcher = FileWatcher::new(db.clone(), exclusions, 100, None).expect("create watcher");
    watcher.watch_root(root_path).expect("watch root");

    // Wait for inotify registration
    thread::sleep(Duration::from_millis(150));

    // A) TEST CREATE: Create document_v1.txt
    let doc_path = root_path.join("document_v1.txt");
    {
        let mut f = File::create(&doc_path).expect("create doc");
        f.write_all(b"Initial content for search testing.")
            .expect("write content");
    }

    // Wait for debounce (100ms) + processing time
    thread::sleep(Duration::from_millis(350));

    // Verify search finds document_v1
    let results = db.search_filenames("document_v1", 10).expect("search");
    assert_eq!(results.len(), 1, "document_v1 should be indexed");
    assert_eq!(results[0].filename, "document_v1.txt");
    let initial_file_id = results[0].id.clone();

    // B) TEST MODIFY: Modify document_v1.txt
    thread::sleep(Duration::from_millis(100));
    {
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(&doc_path)
            .expect("open for append");
        f.write_all(b"\nUpdated second line of content.")
            .expect("append");
    }

    thread::sleep(Duration::from_millis(350));

    let updated_results = db.search_filenames("document_v1", 10).expect("search");
    assert_eq!(updated_results.len(), 1);
    // Stable identity check: file ID remains identical after modification
    assert_eq!(
        updated_results[0].id, initial_file_id,
        "File ID must be stable across modifications"
    );
    assert!(
        updated_results[0].size_bytes > results[0].size_bytes,
        "Size must be updated in database"
    );

    // C) TEST MOVE / RENAME: Rename document_v1.txt to document_v2.txt
    let doc_v2_path = root_path.join("document_v2.txt");
    fs::rename(&doc_path, &doc_v2_path).expect("rename file");

    thread::sleep(Duration::from_millis(350));

    // Old name disappears
    let old_search = db.search_filenames("document_v1", 10).expect("search old");
    assert_eq!(
        old_search.len(),
        0,
        "Old file name must disappear from search"
    );

    // New name is found
    let new_search = db.search_filenames("document_v2", 10).expect("search new");
    assert_eq!(new_search.len(), 1, "Renamed file must be found in search");
    assert_eq!(new_search[0].filename, "document_v2.txt");

    // D) TEST EXCLUSIONS: File created inside excluded directory must NOT be indexed
    let nm_dir = root_path.join("node_modules");
    fs::create_dir_all(&nm_dir).expect("create node_modules");
    let ignored_file = nm_dir.join("package_cache.json");
    {
        let mut f = File::create(&ignored_file).expect("create ignored file");
        f.write_all(b"{\"version\":\"1.0.0\"}").expect("write json");
    }

    thread::sleep(Duration::from_millis(350));

    let ignored_search = db
        .search_filenames("package_cache", 10)
        .expect("search ignored");
    assert_eq!(
        ignored_search.len(),
        0,
        "Files inside excluded folders must be ignored by watcher"
    );

    // E) TEST DELETE: Delete document_v2.txt
    fs::remove_file(&doc_v2_path).expect("delete document_v2");

    thread::sleep(Duration::from_millis(350));

    let post_delete_search = db
        .search_filenames("document_v2", 10)
        .expect("search deleted");
    assert_eq!(
        post_delete_search.len(),
        0,
        "Deleted file must be removed from search results"
    );

    // F) TEST SUBDIRECTORY DELETION: Subtree cleanup
    let sub_dir = root_path.join("nested_folder");
    fs::create_dir_all(&sub_dir).expect("create nested_folder");
    let sub_file = sub_dir.join("nested_report.pdf");
    {
        let mut f = File::create(&sub_file).expect("create sub file");
        f.write_all(b"PDF header").expect("write");
    }

    thread::sleep(Duration::from_millis(350));
    let sub_search = db
        .search_filenames("nested_report", 10)
        .expect("search sub file");
    assert_eq!(sub_search.len(), 1);

    // Delete entire folder
    fs::remove_dir_all(&sub_dir).expect("remove nested folder");
    thread::sleep(Duration::from_millis(350));

    let post_dir_search = db
        .search_filenames("nested_report", 10)
        .expect("search after dir remove");
    assert_eq!(
        post_dir_search.len(),
        0,
        "All files in deleted directory must be removed from index"
    );

    // G) TEST RECONCILIATION: Offline additions and deletions reconciled accurately
    // Create two files on disk directly while simulating an offline state
    let rec_file_1 = root_path.join("reconcile_a.txt");
    let rec_file_2 = root_path.join("reconcile_b.txt");
    File::create(&rec_file_1).expect("create rec 1");
    File::create(&rec_file_2).expect("create rec 2");

    let meta1 = read_file_metadata(&rec_file_1).expect("meta 1");
    let meta2 = read_file_metadata(&rec_file_2).expect("meta 2");
    db.upsert_files_batch(&root.id, &[meta1, meta2])
        .expect("upsert batch");

    // Now delete rec_file_1 from disk behind the scenes
    fs::remove_file(&rec_file_1).expect("remove rec 1");

    // Run reconciliation for root with current active disk files: [rec_file_2]
    let active_paths = vec![rec_file_2.to_string_lossy().to_string()];
    let pruned = db
        .reconcile_root(&root.id, &active_paths)
        .expect("reconcile");
    assert_eq!(pruned, 1, "Must prune exactly the 1 missing file");

    let rec_search_1 = db.search_filenames("reconcile_a", 10).expect("search");
    assert_eq!(rec_search_1.len(), 0, "Missing file must be pruned");

    let rec_search_2 = db.search_filenames("reconcile_b", 10).expect("search");
    assert_eq!(rec_search_2.len(), 1, "Active file must be preserved");

    // Cleanup watcher
    drop(watcher);
}
