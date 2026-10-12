# ADR 003: Logging and Offline Privacy Boundary

## Status
Accepted

## Context
Sift is designed as an offline-first desktop search engine that processes personal user documents, source code, and screenshots.
Non-negotiable security requirements dictate:
- No remote telemetry or network transmission of indexed content, embeddings, or queries.
- No accidental leakage of private document text into system logs (`syslog`, `journalctl`, or stderr).

## Decision
1. **Structured Tracing via `tracing-subscriber`**:
   - Rust components log using `tracing::info!`, `warn!`, and `error!`.
   - The log filter is configurable via `RUST_LOG` and defaults to `sift_core=info,sift_tauri=info,warn`.
2. **Strict Redaction Rules**:
   - File counts, operational elapsed times, and status transitions may be logged.
   - Document contents, snippets, OCR text extracts, and vector embeddings must **never** be logged under any log level.
   - User file paths are logged only when a critical file I/O error occurs, with permission checks to ensure no sensitive path parameter leak.
3. **Network Isolation**:
   - No external HTTP clients (e.g., `reqwest`) are included in `sift-core`. Normal indexing and search operate with complete network isolation.
   - Future model downloads in Phase 5 will require explicit user consent and separate confirmation dialogs.

## Consequences
- Compliance with local privacy guarantees is strictly verifiable in code audits.
- Automated tests verify that no private text is printed to stdout or logs.
