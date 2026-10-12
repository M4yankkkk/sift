# Sift — Implementation Progress

## Current Phase: Phase 2 Completed

### 1. Completed in Phase 0 (Foundation and Decisions)
- [x] **Project Repository & Modular Scaffold**: Root Cargo workspace (`crates/sift-core` + `src-tauri` + React TS).
- [x] **Typed IPC Contract**: Fully typed bridge across Rust models and TypeScript.
- [x] **SQLite Storage**: Embedded SQLite via `rusqlite` (bundled) with WAL mode.
- [x] **Scanner Foundation & Exclusions**: Symlink loop prevention and default exclusion rules.
- [x] **Deterministic Test Fixtures**: Fixture documents in `tests/fixtures/`.
- [x] **Architecture Decision Records**: ADR 001, ADR 002, ADR 003.
- [x] **UI Polish**: Obsidian Dark minimal floating launcher inspired by Raycast and Linear.

---

### 2. Completed in Phase 1 (Fast Filename Search & Rich Previews)
- [x] **Recursive Directory Scanner**: Implemented using `walkdir` with symlink loop prevention and exclusion pruning.
- [x] **SQLite Metadata Ingestion**: Transactional batch insertion with stable UUID identities and compound fingerprints.
- [x] **Fast Filename Search Engine**: Sub-millisecond SQLite queries with relevance ranking (Exact, Prefix, Substring).
- [x] **Asynchronous Background Scanning**: Non-blocking worker threads with live status reporting.
- [x] **Rich Previews (Images, PDFs, Code, Binary)**:
  - High-resolution visual first-page rendering for PDFs using Poppler (`pdftoppm`) with an interactive **[Visual]** / **[Text]** toggle.
  - Native Base64 data-URL image previewing (JPG, PNG, WebP, GIF, SVG, BMP, ICO) up to 10MB.
  - Code and markdown previews with line numbers and monospaced typography.
  - Clean binary file cards for non-previewable formats with direct system actions.
- [x] **Native Desktop Actions**: `open_result` and `reveal_result` using `xdg-open`.
- [x] **Synthetic Latency Benchmarking**: 98,705 files/second indexing; 0.52ms average search latency on 10,000 files.

---

### 3. Completed in Phase 2 (Robust Incremental Indexing)
- [x] **Real-time Filesystem Watcher (`crates/sift-core/src/watcher.rs`)**:
  - Built with native Linux `inotify` via the `notify` crate.
  - Automatically monitors all approved search roots recursively.
  - Dynamically registers newly added search roots and unregisters removed roots.
- [x] **Debouncing Engine**:
  - 250ms debouncing window coalesces bursts of file-system operations (e.g. IDE multi-step writes, temp files) to ensure **0.0% CPU usage at idle**.
- [x] **Full Event Lifecycle (Create / Modify / Move / Delete)**:
  - **Create**: Newly created files are immediately ingested and indexed into SQLite.
  - **Modify**: Modified files update size, modified timestamp, and fingerprint while preserving **stable UUID identities**.
  - **Move / Rename**: Moving or renaming a file automatically removes the old path and indexes the new path.
  - **Delete**: Deleted files are instantly removed from SQLite so stale results disappear from search immediately.
  - **Subdirectory Pruning**: Deleting a directory removes all of its children from the database.
- [x] **Exclusion Filtering**:
  - Live watcher events are checked against `PathRuleChecker` (ignoring `.git`, `node_modules`, `target`, hidden dotfiles).
- [x] **Periodic & Restart Reconciliation (`reconcile_root`)**:
  - Detects files deleted while the application was closed or offline.
  - Compares active disk files against SQLite catalog and prunes orphaned records without duplicating entries on restart.
- [x] **Tauri Lifecycle Integration (`src-tauri/src/app/mod.rs`)**:
  - `AppState` manages the `FileWatcher` lifecycle.
  - Live queue length reporting: Status bar reports `"Processing file changes..."` while changes are being indexed.
- [x] **Phase 2 Integration Test Suite (`crates/sift-core/tests/phase2_incremental_indexing_test.rs`)**:
  - Comprehensive end-to-end test verifying creation, modification, rename/move, exclusion, deletion, directory tree removal, and offline reconciliation.

---

### 4. Completed in Phase 3 (Full-Text Documents)
- [x] **Document Ingestion Engine (`crates/sift-core/src/extract.rs`)**:
  - Plain text and source code ingestion with null-byte binary filtering.
  - Line-bounded overlapping chunking with exact `line_number` provenance.
  - Page-by-page PDF extraction using poppler's `pdftotext` with exact `page_number` provenance.
  - File size safety limits: Files exceeding 10MB are safely bypassed to avoid memory spikes.
- [x] **Embedded Tantivy Inverted Index (`crates/sift-core/src/tantivy_index.rs`)**:
  - Tantivy 0.22 schema with indexed `file_id`, `path`, `filename`, `content`, `modality`, and numeric provenance fields.
  - Atomic document chunk upsertion and term-based deletion by `file_id` or `path`.
  - Rich BM25 relevance scoring and tokenized contextual `<b>highlighted</b>` snippets.
  - Manual reload policy on commit ensuring zero lag and deterministic query retrieval.
- [x] **Catalog & Index Synchronization (`crates/sift-core/src/storage.rs`)**:
  - Unified search (`search`) coordinating SQLite filename matches with Tantivy BM25 content hits.
  - Automatic deduplication and fused scoring (exact filename hits on top, followed by keyword hits with snippets).
  - Atomic cleanup on root removal, file deletion, and startup directory reconciliation.
- [x] **Frontend Provenance & Snippet Presentation (`ResultsList.tsx`, `PreviewPanel.tsx`, `index.css`)**:
  - Result list shows matched term context snippets with styled `<b>` highlights.
  - Result list displays `Page X` badges for PDF document hits.
  - Preview panel displays the dynamic matched page number and supports jump-to-page visual rendering.
- [x] **Phase 3 Integration Test Suite (`crates/sift-core/tests/phase3_fulltext_search_test.rs`)**:
  - Verified exact phrase searches inside source code and Markdown files.
  - Verified PDF page-level provenance.
  - Verified content modification invalidation and deletion cleanup.
  - Verified fused ranking (filename vs. content).
  - Verified 10MB file size safety limits.

---

### 5. Test Verification Results

| Suite | Command | Result | Notes |
|---|---|---|---|
| Rust Formatting | `cargo fmt --all -- --check` | **PASS** | Clean code formatting |
| Rust Linter (Clippy) | `cargo clippy --workspace -- -D warnings` | **PASS** | Zero clippy warnings |
| Core Unit Tests | `cargo test -p sift-core` | **PASS** (16/16 passed) | Models, storage, scanner, preview, extract, tantivy |
| Fixtures Tests | `cargo test -p sift-core --test fixtures_test` | **PASS** (2/2 passed) | Fixture properties & exclusions |
| Phase 1 E2E Test | `cargo test -p sift-core --test phase1_indexing_search_test` | **PASS** (1/1 passed) | Full index, search, preview, and root cleanup |
| Phase 2 E2E Test | `cargo test -p sift-core --test phase2_incremental_indexing_test` | **PASS** (1/1 passed) | Create, modify, rename, delete, prune, reconcile |
| Phase 3 E2E Test | `cargo test -p sift-core --test phase3_fulltext_search_test` | **PASS** (5/5 passed) | Code/doc phrases, PDF page provenance, invalidation, fused ranking |
| Synthetic Latency Benchmark | `cargo test -p sift-core --test benchmark_test` | **PASS** (1/1 passed) | Avg 0.52ms, p95 1.54ms on 10k files |
| Frontend Typecheck | `npm run check` | **PASS** | TypeScript compiles cleanly with 0 errors |
| Frontend Vitest | `npm test` (`vitest run`) | **PASS** (5/5 passed) | Contract & client test suite |
| Frontend Production Build | `npm run build` | **PASS** | Production bundle built cleanly |

---

### 6. Next Phase: Phase 4 — Screenshot OCR
- PNG/JPEG text extraction using local, offline OCR engine (e.g. Tesseract or embedded OCR).
- Bounding box and confidence scoring.
- Screenshot preview and match region highlighting.
- Failure handling for noisy, skewed, or unreadable images.
