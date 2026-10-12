# ADR 002: Tauri 2 IPC Contract and Error Boundary

## Status
Accepted

## Context
Tauri bridges a React/TypeScript frontend and a Rust native backend.
Uncaught Rust panics or unhandled JavaScript Promise rejections cause application freezes or silent UI failures.
A unified contract and error boundary is required to:
1. Guarantee consistent types across Rust and TypeScript.
2. Provide predictable error codes to the user interface.
3. Prevent sensitive stack traces or internal paths from leaking to logs.

## Decision
1. **Explicit TypeScript Contract (`src/types/contract.ts`)**:
   All commands and events use shared interfaces mirroring Rust structs in `sift_core::models`.
2. **Unified Error Representation (`SiftError`)**:
   - `SiftError` derives `thiserror::Error` for idiomatic Rust error propagation.
   - It implements a custom `serde::Serialize` that always produces:
     ```json
     {
       "code": "PATH_NOT_FOUND",
       "message": "Path does not exist: /path/to/folder",
       "details": null
     }
     ```
   - Tauri commands return `Result<T, SiftError>`. If an error occurs, Tauri passes this structured JSON payload as a rejected promise, which the frontend catches and displays gracefully.
3. **Graceful Browser Fallback**:
   The frontend client in `src/services/tauriClient.ts` detects `isTauriEnvironment()`. In non-Tauri browser environments (e.g., standard Vite preview or unit tests), it provides safe mock responses rather than throwing missing IPC errors.

## Consequences
- Every new Tauri command must be added to both `src-tauri/src/commands/mod.rs` and `src/types/contract.ts`.
- No `unwrap()` or `expect()` calls are permitted in Tauri command handlers.
