use sift_core::models::MatchType;
use sift_core::ocr::{extract_image_ocr, is_ocr_available, MAX_OCR_IMAGE_SIZE_BYTES};
use sift_core::scanner::scan_directory;
use sift_core::storage::Database;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::Builder;

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
fn test_phase4_ocr_extraction_and_bounding_boxes() {
    if !is_ocr_available() {
        eprintln!("Skipping OCR test: tesseract binary not installed on runner");
        return;
    }

    let fixture = fixtures_root()
        .join("images")
        .join("screenshot_error_404.png");
    assert!(
        fixture.exists(),
        "Image fixture must exist at {:?}",
        fixture
    );

    let ocr_res = extract_image_ocr(&fixture)
        .expect("extract OCR")
        .expect("must return OCR result for fixture");

    assert!(
        ocr_res.full_text.contains("SIGSEGV")
            || ocr_res.full_text.contains("interrupt")
            || ocr_res.full_text.contains("Process"),
        "Extracted text must contain screenshot keywords: got {}",
        ocr_res.full_text
    );

    assert!(
        !ocr_res.regions.is_empty(),
        "Must extract word bounding boxes"
    );

    let first_box = &ocr_res.regions[0].bbox;
    assert!(first_box.width > 0);
    assert!(first_box.height > 0);
    assert!(ocr_res.average_confidence > 0.0 && ocr_res.average_confidence <= 100.0);
}

#[test]
fn test_phase4_ocr_indexing_and_search_hit() {
    if !is_ocr_available() {
        eprintln!("Skipping OCR test: tesseract binary not installed on runner");
        return;
    }

    let root_dir = Builder::new()
        .prefix("sift_phase4_ocr_")
        .tempdir()
        .expect("tempdir");

    let db = Database::open_in_memory().expect("open in-memory db with tantivy");
    let root = db
        .add_root(root_dir.path().to_str().unwrap())
        .expect("add root");

    // Copy fixture image into root directory
    let src_img = fixtures_root()
        .join("images")
        .join("screenshot_error_404.png");
    let target_img = root_dir.path().join("crash_report_window.png");
    fs::copy(&src_img, &target_img).expect("copy image");

    let files = scan_directory(root_dir.path(), &[]).expect("scan directory");
    assert_eq!(files.len(), 1);

    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    // 1. Search by technical error string NOT present in filename ("crash_report_window.png")
    let results = db.search("SIGSEGV", 10).expect("search SIGSEGV");
    assert_eq!(
        results.len(),
        1,
        "Should find screenshot by internal error text"
    );
    assert_eq!(results[0].filename, "crash_report_window.png");
    assert_eq!(results[0].match_type, MatchType::Ocr);
    assert!(
        results[0].snippet.is_some(),
        "Must provide highlighted snippet"
    );
    let snippet = results[0].snippet.as_ref().unwrap();
    assert!(
        snippet.to_lowercase().contains("sigsegv"),
        "Snippet must contain matched word"
    );

    // 2. Verify stored OCR data
    let ocr_data = db
        .get_ocr_data(&results[0].id)
        .expect("get ocr data")
        .expect("ocr data must exist");
    let (text, conf, regions) = ocr_data;
    assert!(text.contains("SIGSEGV"));
    assert!(conf > 0.0);
    assert!(!regions.is_empty());
}

#[test]
fn test_phase4_ocr_second_fixture_security_memo() {
    if !is_ocr_available() {
        eprintln!("Skipping OCR test: tesseract binary not installed on runner");
        return;
    }

    let root_dir = Builder::new()
        .prefix("sift_phase4_memo_")
        .tempdir()
        .expect("tempdir");

    let db = Database::open_in_memory().expect("open in-memory db with tantivy");
    let root = db
        .add_root(root_dir.path().to_str().unwrap())
        .expect("add root");

    let src_img = fixtures_root()
        .join("images")
        .join("security_audit_memo.png");
    let target_img = root_dir.path().join("audit_screen.png");
    fs::copy(&src_img, &target_img).expect("copy memo");

    let files = scan_directory(root_dir.path(), &[]).expect("scan directory");
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert memo");

    // Search for security token
    let results = db.search("AUTH_KEY_99217", 10).expect("search auth token");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].filename, "audit_screen.png");
    assert_eq!(results[0].match_type, MatchType::Ocr);
}

#[test]
fn test_phase4_ocr_deletion_invalidation() {
    if !is_ocr_available() {
        eprintln!("Skipping OCR test: tesseract binary not installed on runner");
        return;
    }

    let root_dir = Builder::new()
        .prefix("sift_phase4_del_")
        .tempdir()
        .expect("tempdir");

    let db = Database::open_in_memory().expect("open in-memory db with tantivy");
    let root = db
        .add_root(root_dir.path().to_str().unwrap())
        .expect("add root");

    let src_img = fixtures_root()
        .join("images")
        .join("screenshot_error_404.png");
    let target_img = root_dir.path().join("temp_error.png");
    fs::copy(&src_img, &target_img).expect("copy image");

    let files = scan_directory(root_dir.path(), &[]).expect("scan directory");
    db.upsert_files_batch(&root.id, &files)
        .expect("upsert batch");

    // Confirm present
    let before = db.search("SIGSEGV", 10).expect("search before");
    assert_eq!(before.len(), 1);

    // Delete file
    let path_str = target_img.to_str().unwrap().to_string();
    let deleted = db.delete_file_by_path(&path_str).expect("delete file");
    assert!(deleted);

    // Search again -> must be 0 results
    let after = db.search("SIGSEGV", 10).expect("search after");
    assert_eq!(
        after.len(),
        0,
        "Deleted screenshot must be pruned from OCR index"
    );

    // SQLite ocr_data should also be cleaned
    let ocr_check = db.get_ocr_data(&before[0].id).expect("get ocr");
    assert!(ocr_check.is_none(), "Cascade deletion must clean ocr_data");
}

#[test]
fn test_phase4_ocr_corrupt_and_size_limits() {
    let dir = Builder::new()
        .prefix("sift_phase4_safety_")
        .tempdir()
        .expect("tempdir");

    // 1. Corrupt byte file with .png extension
    let corrupt_path = dir.path().join("corrupt_broken.png");
    {
        let mut f = File::create(&corrupt_path).expect("create corrupt file");
        f.write_all(b"NOT_A_VALID_PNG_FILE_HEADER")
            .expect("write bytes");
    }

    let corrupt_res = extract_image_ocr(&corrupt_path).expect("extract corrupt");
    assert!(
        corrupt_res.is_none(),
        "Corrupted image files must gracefully return None without panicking"
    );

    // 2. Giant image file exceeding 15MB limit
    let huge_img_path = dir.path().join("huge_screenshot.png");
    {
        let f = File::create(&huge_img_path).expect("create huge file");
        f.set_len(MAX_OCR_IMAGE_SIZE_BYTES + 1024)
            .expect("set huge size");
    }

    let huge_res = extract_image_ocr(&huge_img_path).expect("extract huge");
    assert!(
        huge_res.is_none(),
        "Images exceeding 15MB must be safely skipped for OCR"
    );
}
