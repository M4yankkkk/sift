# Sift — Implementation Progress

## Current Phase: Phase 5 Completed

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

### 5. Completed in Phase 4 (Screenshot OCR & Visual Bounding Boxes)
- [x] **Offline OCR Extraction Pipeline (`crates/sift-core/src/ocr.rs`)**:
  - User-space Tesseract 5.3.4 engine with local English training data (`eng.traineddata`).
  - Strict 100% offline execution; zero cloud APIs or telemetry.
  - TSV stream parser extracting word text, confidence percentage, and pixel bounding boxes (`left`, `top`, `width`, `height`).
  - Defensive error handling: Skips empty images and files exceeding 15MB limit without panicking.
- [x] **Unified Catalog & Search Indexing (`crates/sift-core/src/storage.rs`)**:
  - SQLite `ocr_data` table storing extracted text, average confidence, and serialized word regions.
  - Ingestion into Tantivy index under `modality = "ocr"`.
  - Search engine tags OCR text matches with `match_type: "ocr"` and `"Screenshot OCR match"` reasons.
  - Cascading deletion and cleanup when screenshot files are modified or deleted.
- [x] **Interactive Frontend Overlay & Preview (`PreviewPanel.tsx`, `ResultsList.tsx`, `index.css`)**:
  - Amber `ocr` badge tag styling in results list.
  - Exact proportional percentage bounding boxes overlaid directly on image previews.
  - Word tooltip displaying confidence scores.
  - Interactive toggle button (`OCR Boxes (N)`) to toggle bounding boxes on/off.
  - OCR text snippet card displaying extracted text with highlighted match phrases.
- [x] **Phase 4 Integration Test Suite (`crates/sift-core/tests/phase4_ocr_search_test.rs`)**:
  - Tested word extraction and bounding box accuracy on deterministic fixtures (`screenshot_error_404.png`, `security_audit_memo.png`).
  - Tested OCR search ranking and snippet generation.
  - Tested deletion invalidation and cascade cleanup.
  - Tested corrupted image files and file size bounds.

### 6. Completed in Phase 5 (Semantic Retrieval & Multimodal Vision Search)
- [x] **Local Multimodal Embedding Engine (`crates/sift-core/src/embeddings.rs`)**:
  - Abstract `EmbeddingEngine` trait for zero-lock-in modularity.
  - Native ONNX Runtime vector inference via FastEmbed with CPU multi-threading.
  - Text & Code embeddings for documents, source code, PDFs, and OCR text.
  - Visual embeddings for photos and screenshots (`png`, `jpg`, `jpeg`, `webp`, `bmp`, `gif`, `svg`).
  - High-performance, deterministic mock encoder for instant, zero-network unit/CI testing.
- [x] **SQLite Vector Persistence (`crates/sift-core/src/storage.rs`)**:
  - `chunk_embeddings` table storing compact IEEE-754 vector blobs with modality tags (`text`, `code`, `pdf`, `ocr`, `image`).
  - Exact dot-product / cosine similarity retrieval across indexed vector catalog.
  - Automatic cascading deletion and cache invalidation on file modify/delete/prune events.
- [x] **Hybrid Reciprocal Rank Fusion (RRF)**:
  - Fuses exact filename matches (tier 1), Tantivy BM25 keyword matches (tier 2), and semantic vector nearest neighbors (tier 3).
  - Documents matching both keyword and semantic concepts receive fused boost scores.
  - Dedicated semantic provenance (`"Visual semantic match (92% similarity)"`, `"Semantic concept match (88% similarity)"`).
- [x] **Frontend Polish & Presentation (`ResultsList.tsx`, `PreviewPanel.tsx`, `index.css`)**:
  - Distinct purple `semantic` badge styling in the launcher result list.
  - Enhanced Preview Panel callout displaying matched conceptual passages and relevance percentages.
- [x] **Phase 5 Integration Test Suite (`crates/sift-core/tests/phase5_semantic_search_test.rs`)**:
  - Paraphrased concept query finds document without literal keyword overlap.
  - Visual multimodal search finds images based on visual content description.
  - Hybrid RRF test confirms fused ranking boost.
  - Deletion invalidation test confirms purged embeddings.
  - Graceful degradation test confirms 100% functionality with ML disabled.

---

### 7. Completed in Phase 6 (Polish, Global Shortcut, Resource Modes & Validation)
- [x] **Configurable Global System Shortcut (`tauri-plugin-global-shortcut`)**:
  - Integrated `tauri-plugin-global-shortcut 2.4.1` with native Ubuntu X11/Wayland support.
  - Configurable key combination (defaults to `Alt+Space`, editable in Preferences to `Ctrl+Space`, `Super+Space`, etc.).
  - Global toggle behavior: When the shortcut is pressed anywhere on the desktop, Sift is shown, unminimized, and focused; pressing it while focused hides the launcher.
  - Dynamic shortcut re-binding: Changing the hotkey in Preferences immediately rebinds the listener without restarting.
- [x] **Escape Window Dismiss & Quick Launcher UX**:
  - Pressing `Escape` closes open modal sheets first; if none are open and the search bar has text, it clears the query; if already empty, it smoothly hides the launcher.
  - `hide_window` native Tauri command for frontend window dismissal.
- [x] **In-Flight Query Cancellation & Race Condition Prevention**:
  - Sequence-tagged search calls (`currentSearchSeqRef`): If user types rapidly, responses from older in-flight requests that finish late are automatically dropped, ensuring current results always match the latest input.
- [x] **Resource Profile Throttling (Eco Mode vs. Performance Mode)**:
  - `ResourceMode::Eco`: Throttles heavy background ML/OCR passes with 20ms sleeps between document chunks, keeping idle/background CPU usage low and preserving laptop battery.
  - `ResourceMode::Performance`: Unthrottled multi-threaded background processing for high-throughput batch indexing.
  - Profile state persisted in SQLite `settings` table and editable in Preferences.
- [x] **Phase 6 Integration Test Suite (`crates/sift-core/tests/phase6_system_polish_test.rs`)**:
  - Verified default settings, shortcut configuration, and persistent switching between Eco and Performance modes.
  - Verified query edge cases (empty strings, whitespace-only, single-char bounds).

---

### 8. Test Verification Results

| Suite | Command | Result | Notes |
|---|---|---|---|
| Rust Formatting | `cargo fmt --all -- --check` | **PASS** | Clean code formatting |
| Rust Linter (Clippy) | `cargo clippy --workspace -- -D warnings` | **PASS** | Zero clippy warnings |
| Core Unit Tests | `cargo test -p sift-core` | **PASS** (24/24 passed) | Models, storage, scanner, preview, extract, tantivy, ocr, embeddings |
| Fixtures Tests | `cargo test -p sift-core --test fixtures_test` | **PASS** (2/2 passed) | Fixture properties & exclusions |
| Phase 1 E2E Test | `cargo test -p sift-core --test phase1_indexing_search_test` | **PASS** (1/1 passed) | Full index, search, preview, and root cleanup |
| Phase 2 E2E Test | `cargo test -p sift-core --test phase2_incremental_indexing_test` | **PASS** (1/1 passed) | Create, modify, rename, delete, prune, reconcile |
| Phase 3 E2E Test | `cargo test -p sift-core --test phase3_fulltext_search_test` | **PASS** (5/5 passed) | Code/doc phrases, PDF page provenance, invalidation, fused ranking |
| Phase 4 E2E Test | `cargo test -p sift-core --test phase4_ocr_search_test` | **PASS** (5/5 passed) | OCR extraction, bounding boxes, search hit, deletion cleanup, limits |
| Phase 5 E2E Test | `cargo test -p sift-core --test phase5_semantic_search_test` | **PASS** (5/5 passed) | Paraphrased retrieval, vision search, RRF fusion, invalidation, degradation |
| Phase 6 E2E Test | `cargo test -p sift-core --test phase6_system_polish_test` | **PASS** (2/2 passed) | Shortcut persistence, resource mode switching, query edge cases |
| Synthetic Latency Benchmark | `cargo test -p sift-core --test benchmark_test` | **PASS** (1/1 passed) | Avg 0.52ms, p95 1.54ms on 10k files |
| Frontend Typecheck | `npm run check` | **PASS** | TypeScript compiles cleanly with 0 errors |
| Frontend Vitest | `npm test` (`vitest run`) | **PASS** (5/5 passed) | Contract & client test suite |
| Frontend Production Build | `npm run build` | **PASS** | Production bundle built cleanly |

