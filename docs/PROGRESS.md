# Sift — Implementation Progress

## Current Phase: Phase 0 Completed

### 1. Completed in Phase 0
- [x] **Project Repository & Modular Scaffold**:
  - Cargo Workspace configured at repository root (`Cargo.toml`).
  - Pure domain core crate created at `crates/sift-core` (zero GUI dependencies for headless CI and unit tests).
  - Tauri 2 desktop application crate configured at `src-tauri` with modular module layout (`commands`, `scanner`, `watcher`, `jobs`, `extract`, `indexing`, `embeddings`, `search`, `storage`, `app`).
  - React 18 + Vite 6 + TypeScript frontend established under `src/` (`components/`, `pages/`, `services/`, `types/`).
- [x] **Typed IPC Contract**:
  - `src/types/contract.ts` defines all command payloads, status structures, query shapes, and result records.
  - Rust models in `crates/sift-core/src/models.rs` mirror the contract.
  - Safe error boundary: `SiftError` in `crates/sift-core/src/error.rs` serializes into structured JSON (`{ "code": "...", "message": "..." }`) preventing raw unhandled rejections or unhandled Rust panics.
- [x] **SQLite Embedded Metadata Storage**:
  - Integrated `rusqlite` with `bundled` feature.
  - Initial schema migrations creating tables for `roots`, `files`, `chunks`, and `settings` with WAL mode and indices.
  - CRUD operations implemented and unit tested for approved search roots and application settings.
- [x] **Scanner Foundation & Exclusions**:
  - Path exclusion rules implemented in `PathRuleChecker`: rejects hidden files (`.`), `.git`, `node_modules`, `target`, `dist`, `.cache`, and ignores symlinks to prevent traversal loops.
  - Metadata reader (`read_file_metadata`) generating stable identity fingerprints (`size:mtime_secs:mtime_nanos`).
- [x] **Deterministic Test Fixtures**:
  - Stored in `tests/fixtures/`:
    - `documents/sample_notes.md`
    - `documents/sample_readme.txt`
    - `documents/minimal_passage.pdf`
    - `code/example_service.rs`
    - `code/example_app.ts`
    - `code/database_pool.py`
    - `duplicates/folder_a/quarterly_report.txt` and `duplicates/folder_b/quarterly_report.txt`
    - `exclusions/.git/config` and `exclusions/node_modules/dummy_pkg/index.js`
    - `corrupted/corrupt_file.bin`
- [x] **Architecture Decision Records (ADRs)**:
  - `docs/ADR/001-sqlite-driver-selection.md`: Choice of `rusqlite` with `bundled` SQLite over `sqlx`.
  - `docs/ADR/002-ipc-contract-and-error-boundary.md`: Typed IPC and error serialization specification.
  - `docs/ADR/003-logging-and-privacy-boundary.md`: Structured `tracing` logging and offline zero-leakage privacy rules.
- [x] **Benchmarks Framework**:
  - `benchmarks/README.md` defining targets (warm filename search < 100ms on 100k files, idle RAM < 100MB, 0% idle CPU) and methodology.
- [x] **Documentation & Setup**:
  - `docs/SETUP.md` with complete Ubuntu 24.04 package prerequisites and test run commands.
- [x] **Continuous Integration**:
  - `.github/workflows/ci.yml` running format check, clippy, unit tests, fixture integration tests, frontend type-check, and bundle build.

---

## 2. Test Verification Results

| Suite | Command | Result | Notes |
|---|---|---|---|
| Rust Formatting | `cargo fmt --all -- --check` | **PASS** | 0 formatting issues |
| Rust Clippy | `cargo clippy -p sift-core -- -D warnings` | **PASS** | 0 warnings |
| Sift-Core Unit Tests | `cargo test -p sift-core` | **PASS** (7/7 passed) | Database, storage, exclusions, error serialization |
| Fixtures Integration Tests | `cargo test -p sift-core --test fixtures_test` | **PASS** (2/2 passed) | Verifies fixtures against exclusion rules and fingerprints |
| Frontend Type Check | `npm run check` (`tsc --noEmit`) | **PASS** | 0 TypeScript errors |
| Frontend Vitest | `npm test` (`vitest run`) | **PASS** (5/5 passed) | Validates typed contract, mock fallback, status reporting |
| Frontend Production Build | `npm run build` | **PASS** | Vite built clean bundle in 725ms |

---

## 3. Known Limitations (Phase 0 Scope Boundary)
- Desktop GUI window compilation requires GTK3 and WebKitGTK development packages (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`) installed on Ubuntu (see `docs/SETUP.md`).
- File search and indexing are not yet active; command handlers return foundation contracts.
- ML models and embeddings are disabled by design until Phase 5.

---

## 4. Next Phase: Phase 1 — Fast Filename Search

### Scope for Phase 1:
1. **Folder Picker & Approved Roots**: Connect UI folder picker to `add_root` / `remove_root` commands.
2. **Recursive Scanner**: Traverse approved roots using `walkdir`/`ignore`, applying default and user exclusions.
3. **SQLite File Metadata Catalog**: Bulk-insert discovered files with stable identities, modified timestamps, and fingerprints into `files` table.
4. **Lightweight Filename Search**: Instant case-insensitive prefix and substring matching on filenames directly from SQLite/memory.
5. **Desktop Actions**: Native `open_result` and `reveal_result` using `xdg-open` on Ubuntu.
6. **Progress UI**: Live reporting of discovered and indexed file counts.
7. **Benchmarks**: Synthetic benchmark runner measuring warm search latency on a synthetic 100k-file corpus.
