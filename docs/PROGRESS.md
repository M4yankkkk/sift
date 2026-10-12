# Sift — Implementation Progress

## Current Phase: Phase 1 Completed

### 1. Completed in Phase 0 (Foundation and Decisions)
- [x] **Project Repository & Modular Scaffold**: Root Cargo workspace (`crates/sift-core` + `src-tauri` + React TS).
- [x] **Typed IPC Contract**: Fully typed bridge across Rust models and TypeScript.
- [x] **SQLite Storage**: Embedded SQLite via `rusqlite` (bundled) with WAL mode.
- [x] **Scanner Foundation & Exclusions**: Symlink loop prevention and default exclusion rules.
- [x] **Deterministic Test Fixtures**: Fixture documents in `tests/fixtures/`.
- [x] **Architecture Decision Records**: ADR 001, ADR 002, ADR 003.
- [x] **UI Polish**: Obsidian Dark minimal floating launcher inspired by Raycast and Linear.

---

### 2. Completed in Phase 1 (Fast Filename Search)
- [x] **Recursive Directory Scanner**:
  - Implemented in `crates/sift-core/src/scanner.rs` using `walkdir`.
  - Non-recursive exclusion pruning: Skips descending into `.git`, `node_modules`, `target`, `dist`, `.cache`, and hidden directories.
  - Avoids following symlinks to prevent traversal loops and infinite recursion.
- [x] **SQLite Metadata Ingestion**:
  - `upsert_files_batch`: High-throughput transactional batch insertion into SQLite with conflict handling on file path.
  - Stable identity generation (`Uuid`), file size, modified time, and compound fingerprint (`size:mtime_secs:mtime_nanos`).
- [x] **Fast Filename Search Engine**:
  - `search_filenames`: Substring, prefix, and exact match search powered by SQLite with relevance ranking (Exact = 1.0, Prefix = 0.85, Substring = 0.5–0.8).
  - Matches are sorted by relevance score descending and filename length ascending.
  - Completely non-mocked; queries execute directly against indexed SQLite records.
- [x] **Asynchronous Background Scanning**:
  - In `src-tauri/src/commands/mod.rs`, root addition triggers an asynchronous worker thread that traverses and indexes files without blocking the UI thread.
  - Live progress tracking via `AppState`: reports active operation (`Scanning <path>...`), file counts, and scan timestamps.
- [x] **Safe Text Preview Reader**:
  - `read_file_preview`: Reads up to 64KB of text/code files from disk.
  - Safeguards against binary files (detects null bytes) and invalid UTF-8 without crashing.
- [x] **Native Desktop Actions**:
  - `open_result`: Opens file with system default handler via `xdg-open`.
  - `reveal_result`: Opens parent directory in system file manager via `xdg-open`.
- [x] **End-to-End Integration Tests**:
  - `crates/sift-core/tests/phase1_indexing_search_test.rs`: Indexes fixture directories, tests duplicate filename resolution (`quarterly_report.txt` in separate folders), verifies excluded directories are omitted, verifies text and binary previews, and tests root cleanup.
- [x] **Synthetic Latency Benchmarking**:
  - `crates/sift-core/tests/benchmark_test.rs` and `benchmarks/run_benchmark.sh`:
  - Indexed a 10,000-file synthetic corpus.
  - Measured 100 warm queries:
    - **Throughput:** 98,705 files/second.
    - **Min Latency:** 0.115 ms.
    - **Average Latency:** 0.523 ms.
    - **P95 Latency:** 1.538 ms.
    - **Target Condition (< 100 ms): MET** (by a factor of > 50x).

---

### 3. Test Verification Results

| Suite | Command | Result | Notes |
|---|---|---|---|
| Rust Formatting | `cargo fmt --all -- --check` | **PASS** | 0 formatting issues |
| Rust Clippy | `cargo clippy -p sift-core -- -D warnings` | **PASS** | 0 warnings |
| Core Unit Tests | `cargo test -p sift-core` | **PASS** (10/10 passed) | Models, storage, scanner, preview, errors |
| Fixtures Tests | `cargo test -p sift-core --test fixtures_test` | **PASS** (2/2 passed) | Verifies fixture properties & exclusions |
| Phase 1 E2E Test | `cargo test -p sift-core --test phase1_indexing_search_test` | **PASS** (1/1 passed) | Full index, search, preview, and root cleanup |
| Synthetic Latency Benchmark | `cargo test -p sift-core --test benchmark_test` | **PASS** (1/1 passed) | Avg 0.52ms, p95 1.54ms on 10k files |
| Frontend Type Check | `npm run check` (`tsc --noEmit`) | **PASS** | 0 TypeScript errors |
| Frontend Vitest | `npm test` (`vitest run`) | **PASS** (5/5 passed) | Contract & client test suite |
| Frontend Production Build | `npm run build` | **PASS** | Production bundle built cleanly |

---

### 4. Known Limitations (Phase 1 Scope Boundary)
- Incremental file watching is not active yet; newly created or modified files appear upon manual scan or app restart (to be addressed in Phase 2).
- Search currently matches filename strings; full-text document content search via Tantivy is scheduled for Phase 3.
- Embeddings and semantic search remain disabled until Phase 5.

---

### 5. Next Phase: Phase 2 — Robust Incremental Indexing

### Scope for Phase 2:
1. **Filesystem Watchers (`notify`)**: Subscribe to approved root folders to capture file events in real-time.
2. **Event Debouncing & Coalescing**: Debounce event storms (e.g. `git checkout` or bulk saves).
3. **Change Handling**:
   - New files: Automatically scanned and indexed.
   - Modified files: Fingerprints checked; changed files re-indexed.
   - Deleted/Moved files: Old matches removed immediately; moved files update paths.
4. **Persistent Job Queue & Error Retries**: Bounded worker pool preventing CPU exhaustion during indexing.
5. **Periodic Reconciliation**: Catch missed filesystem events on application start and after sleep.
6. **Resource Optimization**: Near-zero CPU utilization at idle.
