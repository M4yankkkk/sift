# ADR 001: SQLite Driver Selection — rusqlite vs sqlx

## Status
Accepted

## Context
Sift requires an embedded relational database to store:
- User-approved roots and exclusion preferences
- File metadata (path, filename, extension, size, mtime, fingerprint, status)
- Chunk provenance (file ID, chunk index, page number, byte offsets)
- Index state and synchronization markers

The two primary candidates in the Rust ecosystem are `rusqlite` and `sqlx`.

## Comparison

| Criteria | `rusqlite` (with `bundled`) | `sqlx` |
|---|---|---|
| **Architecture** | Synchronous embedded C library wrapper | Async-first connection pool wrapper |
| **System Dependencies** | Zero (compiles modern SQLite from source via `bundled`) | Requires external sqlite3 dynamic library or complex feature flags |
| **Runtime Overhead** | Ultra-lightweight, zero async executor dependency for DB calls | Requires Tokio async runtime tasks and channel message passing |
| **Extension Support** | Direct support for `sqlite-vec` or custom C extensions | Difficult / limited extension loading API in async pool |
| **Transaction Control** | Direct fine-grained control over transactions and savepoints | Pool-based transactions with async lifetime bounds |
| **Compile Time & Binary Size** | Fast compile time, compact binary | Heavy compile times due to macro expansions and async plumbing |

## Decision
We choose **`rusqlite` with `bundled` feature**.
1. **Self-contained deployment:** Compiling SQLite statically guarantees version consistency across all Ubuntu installations without relying on `libsqlite3-dev` being installed on the user's host machine.
2. **Concurrency fit:** SQLite on local disk operates best with WAL (Write-Ahead Logging) mode, where background indexing writes can proceed concurrently with search readers. A synchronous wrapper accessed via `Arc<Mutex<Connection>>` or a dedicated writer thread provides predictable, deterministic latency without async polling overhead.
3. **Future Extension Compatibility:** Loading `sqlite-vec` for local vector search (Phase 5) is natively supported by `rusqlite`.

## Consequences
- All SQLite queries are synchronous. Background workers will execute database transactions on background worker threads, keeping the Tauri main/UI thread responsive.
- Migrations will be managed deterministically in `crates/sift-core/src/storage.rs`.
