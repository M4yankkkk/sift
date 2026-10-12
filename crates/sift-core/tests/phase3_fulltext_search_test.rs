use sift_core::models::MatchType;
use sift_core::scanner::{read_file_metadata, scan_directory};
use sift_core::storage::Database;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::Builder;

#[test]
fn test_phase3_fulltext_exact_phrase_and_snippets() {
    let dir = Builder::new()
        .prefix("sift_phase3_test_")
        .tempdir()
        .expect("create tempdir");
    let root_path = dir.path().to_str().unwrap();

    let db = Database::open_in_memory().expect("open in-memory db with tantivy");
    let root = db.add_root(root_path).expect("add root");

    // Create a Rust source code file with a unique technical implementation
    let code_file_path = dir.path().join("quantum_engine.rs");
    let mut code_file = File::create(&code_file_path).expect("create code file");
    writeln!(
        code_file,
        r#"
pub struct QuantumStateCoordinator {{
    pub qubit_count: usize,
}}

impl QuantumStateCoordinator {{
    pub fn execute_entanglement_protocol(&self) -> bool {{
        // Entangle superconducting qubits in topological circuit
        true
    }}
}}
"#
    )
    .expect("write code");

    // Create a markdown documentation file
    let doc_file_path = dir.path().join("specifications.md");
    let mut doc_file = File::create(&doc_file_path).expect("create doc file");
    writeln!(
        doc_file,
        r#"
# System Specifications

The Sift offline search engine performs local lexical retrieval without network calls.
Guaranteed sub-100ms warm search across tens of thousands of local files.
"#
    )
    .expect("write doc");

    // Scan and batch upsert
    let files = scan_directory(dir.path(), &[]).expect("scan directory");
    assert_eq!(files.len(), 2);
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    // 1. Search for exact method inside the code file: "execute_entanglement_protocol"
    let results = db
        .search("execute_entanglement_protocol", 10)
        .expect("search code phrase");
    assert_eq!(results.len(), 1, "Should find quantum_engine.rs");
    assert_eq!(results[0].filename, "quantum_engine.rs");
    assert_eq!(results[0].match_type, MatchType::Keyword);
    assert!(
        results[0].snippet.is_some(),
        "Should generate matched snippet"
    );
    let snippet = results[0].snippet.as_ref().unwrap();
    assert!(
        snippet.contains("<b>execute</b>")
            && snippet.contains("<b>entanglement</b>")
            && snippet.contains("<b>protocol</b>"),
        "Snippet must contain highlighted tokens in <b> tags"
    );

    // 2. Search for exact phrase in markdown: "sub-100ms warm search"
    let doc_results = db
        .search("sub-100ms warm search", 10)
        .expect("search markdown phrase");
    assert_eq!(doc_results.len(), 1, "Should find specifications.md");
    assert_eq!(doc_results[0].filename, "specifications.md");
    assert!(doc_results[0].snippet.is_some());
}

#[test]
fn test_phase3_pdf_page_provenance() {
    let db = Database::open_in_memory().expect("open in-memory db with tantivy");
    let root = db.add_root("/mock/docs").expect("add root");

    let fixture_pdf = Path::new("tests/fixtures/documents/minimal_passage.pdf");
    if !fixture_pdf.exists() {
        return;
    }

    let meta = read_file_metadata(fixture_pdf).expect("read pdf meta");
    db.upsert_files_batch(&root.id, &[meta])
        .expect("upsert pdf");

    // Search for passage text known to be inside minimal_passage.pdf
    let hits = db
        .search("Quantum indexing benchmark", 5)
        .expect("search pdf passage");
    assert_eq!(hits.len(), 1, "Should find minimal_passage.pdf");
    assert_eq!(hits[0].filename, "minimal_passage.pdf");
    assert_eq!(
        hits[0].page_number,
        Some(1),
        "Must retain Page 1 provenance"
    );
    assert!(hits[0].snippet.is_some());
}

#[test]
fn test_phase3_content_modification_and_deletion_invalidation() {
    let dir = Builder::new()
        .prefix("sift_phase3_mod_")
        .tempdir()
        .expect("create tempdir");
    let root_path = dir.path().to_str().unwrap();

    let db = Database::open_in_memory().expect("open in-memory db with tantivy");
    let root = db.add_root(root_path).expect("add root");

    let file_path = dir.path().join("active_memo.txt");
    {
        let mut f = File::create(&file_path).expect("create file");
        writeln!(f, "Confidential codeword: zephyr_aurora_alpha_99")
            .expect("write initial content");
    }

    let meta1 = read_file_metadata(&file_path).expect("read meta");
    db.upsert_files_batch(&root.id, &[meta1])
        .expect("upsert file");

    // Search initial content
    let init_hits = db
        .search("zephyr_aurora_alpha_99", 5)
        .expect("search initial");
    assert_eq!(init_hits.len(), 1);

    // 1. Modify the file content on disk
    {
        let mut f = File::create(&file_path).expect("overwrite file");
        writeln!(f, "Updated codeword: hyperion_nebula_omega_77").expect("write updated content");
    }

    let meta2 = read_file_metadata(&file_path).expect("read updated meta");
    db.upsert_files_batch(&root.id, &[meta2])
        .expect("re-upsert modified file");

    // Old phrase must return 0 results
    let old_hits = db
        .search("zephyr_aurora_alpha_99", 5)
        .expect("search old phrase");
    assert_eq!(
        old_hits.len(),
        0,
        "Old phrase must be invalidated after file modification"
    );

    // New phrase must return the file
    let new_hits = db
        .search("hyperion_nebula_omega_77", 5)
        .expect("search new phrase");
    assert_eq!(
        new_hits.len(),
        1,
        "New phrase must be indexed and searchable"
    );

    // 2. Delete the file
    fs::remove_file(&file_path).expect("delete file");
    db.delete_file_by_path(file_path.to_str().unwrap())
        .expect("delete from db");

    // Both old and new phrases must return 0 results
    let del_hits = db
        .search("hyperion_nebula_omega_77", 5)
        .expect("search after deletion");
    assert_eq!(
        del_hits.len(),
        0,
        "Deleted file must yield 0 search results"
    );
}

#[test]
fn test_phase3_fused_ranking_filename_vs_content() {
    let dir = Builder::new()
        .prefix("sift_phase3_fused_")
        .tempdir()
        .expect("create tempdir");
    let root_path = dir.path().to_str().unwrap();

    let db = Database::open_in_memory().expect("open in-memory db with tantivy");
    let root = db.add_root(root_path).expect("add root");

    // File A: named "phoenix.txt" with generic content
    let file_a_path = dir.path().join("phoenix.txt");
    {
        let mut f = File::create(&file_a_path).expect("create file a");
        writeln!(f, "General meeting minutes and project updates.").expect("write file a");
    }

    // File B: named "weekly_report.txt" with content mentioning "phoenix"
    let file_b_path = dir.path().join("weekly_report.txt");
    {
        let mut f = File::create(&file_b_path).expect("create file b");
        writeln!(
            f,
            "Project phoenix deployment milestones and system architecture."
        )
        .expect("write file b");
    }

    let files = scan_directory(dir.path(), &[]).expect("scan directory");
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    let results = db.search("phoenix", 10).expect("search phoenix");
    assert_eq!(results.len(), 2, "Both files should match");

    // File A (exact filename match) ranks first with score 1.0
    assert_eq!(results[0].filename, "phoenix.txt");
    assert_eq!(results[0].match_type, MatchType::Filename);

    // File B (keyword/content match) follows with BM25 score and snippet
    assert_eq!(results[1].filename, "weekly_report.txt");
    assert_eq!(results[1].match_type, MatchType::Keyword);
    assert!(results[1].snippet.is_some());
}

#[test]
fn test_phase3_file_size_limit_safety() {
    let dir = Builder::new()
        .prefix("sift_phase3_limit_")
        .tempdir()
        .expect("create tempdir");

    // Create a 12MB file (exceeding 10MB limit)
    let huge_file_path = dir.path().join("huge_dump.log");
    {
        let f = File::create(&huge_file_path).expect("create huge file");
        // Fast sparse file allocation or write 11MB
        f.set_len(11 * 1024 * 1024).expect("set 11MB file length");
    }

    // Extraction should safely return empty chunks without panic or memory spike
    let chunks = sift_core::extract_document_chunks(&huge_file_path).expect("extract chunks");
    assert!(
        chunks.is_empty(),
        "Files exceeding 10MB must be safely skipped for full-text indexing"
    );
}
