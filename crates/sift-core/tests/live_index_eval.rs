//! Search-quality regression check against a real, already-indexed Sift database.
//!
//! Dev-only (ignored by default, needs real models and a real index):
//!
//! ```sh
//! SIFT_EVAL_DB=src-tauri/sift.db \
//! SIFT_EVAL_CASES=benchmarks/live_eval_cases.json \
//!   cargo test --release -p sift-core --test live_index_eval -- --ignored --nocapture
//! ```
//!
//! The database, its WAL, and the sibling `sift_tantivy_index/` are copied to a temp dir first, so
//! the running app is never touched. Cases file format (JSON array):
//!
//! ```json
//! [
//!   { "query": "sunset", "mode": "all", "expect": "images (1).jpeg", "within": 3 },
//!   { "query": "database retry", "mode": "all", "max_images": 0 },
//!   { "query": "country", "mode": "visual" }
//! ]
//! ```
//! `expect` is a path substring that must appear in the top `within` results (`expect_any`: any of
//! several substrings); `max_images` caps image results; a case with neither is report-only.

use serde::Deserialize;
use sift_core::embeddings::{create_default_embedding_engine, is_image_supported_for_vision};
use sift_core::storage::Database;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Deserialize)]
struct Case {
    query: String,
    #[serde(default = "default_mode")]
    mode: String,
    expect: Option<String>,
    #[serde(default)]
    expect_any: Vec<String>,
    #[serde(default = "default_within")]
    within: usize,
    max_images: Option<usize>,
}

fn default_mode() -> String {
    "all".to_string()
}

fn default_within() -> usize {
    5
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else if entry.file_name() != ".tantivy-writer.lock" {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
#[ignore = "needs a real index: set SIFT_EVAL_DB and SIFT_EVAL_CASES"]
fn live_index_search_quality() {
    let db_src = PathBuf::from(std::env::var("SIFT_EVAL_DB").expect("set SIFT_EVAL_DB"));
    let cases_path = std::env::var("SIFT_EVAL_CASES")
        .unwrap_or_else(|_| "benchmarks/live_eval_cases.json".to_string());
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string(&cases_path).unwrap_or_else(|e| panic!("{cases_path}: {e}")),
    )
    .expect("parse cases");

    let tmp = tempfile::tempdir().unwrap();
    let db_copy = tmp.path().join("sift.db");
    std::fs::copy(&db_src, &db_copy).unwrap();
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{}", db_src.display(), suffix));
        if side.exists() {
            std::fs::copy(&side, format!("{}{}", db_copy.display(), suffix)).unwrap();
        }
    }
    let index_src = db_src.parent().unwrap().join("sift_tantivy_index");
    if index_src.exists() {
        copy_dir(&index_src, &tmp.path().join("sift_tantivy_index"));
    }

    let db = Database::open(&db_copy)
        .expect("open copy")
        .with_embedding_engine(Some(create_default_embedding_engine()));
    // Warm the models so the first case's latency is not model loading.
    let _ = db.search("warmup", 5);

    let show: usize = std::env::var("SIFT_EVAL_SHOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    let only = std::env::var("SIFT_EVAL_ONLY").ok();
    let mut failures = Vec::new();
    for case in cases
        .iter()
        .filter(|c| only.as_deref().is_none_or(|q| c.query == q))
    {
        let t0 = Instant::now();
        let (results, counts) = db
            .search_with_counts(&case.query, Some(case.mode.as_str()), 50)
            .expect("search");
        let ms = t0.elapsed().as_secs_f64() * 1000.0;

        let images = results
            .iter()
            .filter(|r| is_image_supported_for_vision(&r.extension))
            .count();
        let mut expected: Vec<&str> = case.expect_any.iter().map(String::as_str).collect();
        expected.extend(case.expect.as_deref());
        let rank = results
            .iter()
            .position(|r| expected.iter().any(|e| r.path.contains(e)));

        let mut verdict = "info";
        if !expected.is_empty() {
            if rank.is_some_and(|r| r < case.within) {
                verdict = "PASS";
            } else {
                verdict = "FAIL";
                failures.push(format!(
                    "{:?}/{}: expected {:?} in top {}, got rank {:?}",
                    case.query,
                    case.mode,
                    expected,
                    case.within,
                    rank.map(|r| r + 1)
                ));
            }
        }
        if let Some(max) = case.max_images {
            if images > max {
                verdict = "FAIL";
                failures.push(format!(
                    "{:?}/{}: {} image results, max {}",
                    case.query, case.mode, images, max
                ));
            } else if expected.is_empty() {
                verdict = "PASS";
            }
        }

        println!(
            "\n[{verdict}] {:?} ({}) — {} results, {} images, rank {}, {:.0} ms, counts {:?}",
            case.query,
            case.mode,
            results.len(),
            images,
            rank.map_or("-".to_string(), |r| (r + 1).to_string()),
            ms,
            counts.map(|c| (c.all, c.filename, c.content, c.visual)),
        );
        for (i, r) in results.iter().take(show).enumerate() {
            let short = r.path.rsplit('/').take(2).collect::<Vec<_>>();
            println!(
                "   {:>2}. {:.2} {:<8} {} | {}",
                i + 1,
                r.score,
                format!("{:?}", r.match_type),
                short.iter().rev().cloned().collect::<Vec<_>>().join("/"),
                r.match_reasons.join("; ")
            );
        }
    }

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
