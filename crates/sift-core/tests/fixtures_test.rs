use sift_core::scanner::{read_file_metadata, PathRuleChecker};
use std::path::PathBuf;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

#[test]
fn test_fixtures_exclusion_rules() {
    let root = fixtures_root();
    assert!(root.exists(), "Fixtures directory must exist at {:?}", root);

    let checker = PathRuleChecker::new(vec!["node_modules".to_string(), ".git".to_string()]);

    // Valid documents should be allowed
    let notes_md = root.join("documents").join("sample_notes.md");
    assert!(checker.should_index_relative(Some(&root), &notes_md));

    let sample_pdf = root.join("documents").join("minimal_passage.pdf");
    assert!(checker.should_index_relative(Some(&root), &sample_pdf));

    // Excluded items should be rejected
    let git_config = root.join("exclusions").join(".git").join("config");
    assert!(!checker.should_index_relative(Some(&root), &git_config));

    let node_pkg = root
        .join("exclusions")
        .join("node_modules")
        .join("dummy_pkg")
        .join("index.js");
    assert!(!checker.should_index_relative(Some(&root), &node_pkg));
}

#[test]
fn test_fixtures_duplicates_and_fingerprints() {
    let root = fixtures_root();
    let file_a = root
        .join("duplicates")
        .join("folder_a")
        .join("quarterly_report.txt");
    let file_b = root
        .join("duplicates")
        .join("folder_b")
        .join("quarterly_report.txt");

    let meta_a = read_file_metadata(&file_a).expect("read file A metadata");
    let meta_b = read_file_metadata(&file_b).expect("read file B metadata");

    assert_eq!(meta_a.filename, "quarterly_report.txt");
    assert_eq!(meta_b.filename, "quarterly_report.txt");
    assert_ne!(meta_a.path, meta_b.path);
    assert_ne!(meta_a.size_bytes, meta_b.size_bytes);
    assert_ne!(meta_a.fingerprint, meta_b.fingerprint);
}
