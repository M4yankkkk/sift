# Contributing to Sift

Thanks for helping. Bug reports, search-quality reports, and pull requests are all welcome.

## Development setup (Ubuntu / Debian)

```bash
git clone https://github.com/M4yankkkk/sift.git && cd sift
./scripts/setup-dev.sh        # GTK/WebKit dev libraries, Tesseract, poppler
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust (stable), if needed
npm install                   # Node.js 20+
npm run tauri dev
```

If linking fails with `unable to find library -lgtk-3` (or `-lwebkit2gtk-4.1`), the `-dev` packages are missing; rerun `./scripts/setup-dev.sh`.

The first launch downloads EmbeddingGemma 2 (≈435 MB) into `~/.cache/sift/models`, with progress in the status bar.

## Release build

```bash
npm run tauri build           # → target/release/bundle/deb/Sift_<version>_amd64.deb (~19 MB)
```

The package contains the app only; the model is downloaded on first launch.

## Project layout

| Path | What lives there |
| :--- | :--- |
| `crates/sift-core` | Everything that isn't UI: scanning, extraction, OCR, SQLite storage, Tantivy, embeddings, search |
| `crates/sift-core/src/gemma2.rs` | EmbeddingGemma 2 on ONNX Runtime (image preprocessing, text/vision inference) |
| `crates/sift-core/src/ranking.rs` | Query intent, per-query relevance gates, weighted rank fusion |
| `src-tauri` | Desktop shell: commands, background workers, tray, global shortcut |
| `src` | React UI |
| `docs/ADR` | Architecture decision records |

## Checks (the same ones CI runs)

```bash
cargo fmt --all -- --check
cargo clippy -p sift-core --all-targets -- -D warnings
cargo test -p sift-core          # offline: uses a deterministic mock encoder, no model download

npm run check                    # TypeScript
npm test                         # Vitest
npm run build
```

## Search-quality changes

Unit tests run on a mock encoder, so they can't tell you whether ranking got better. If you change scoring, gating, or fusion, also run the live evaluation against a real index you have built:

```bash
# Copy your index somewhere safe first; the harness works on its own copy.
SIFT_EVAL_DB=src-tauri/sift.db SIFT_EVAL_CASES=benchmarks/live_eval_cases.json \
  cargo test --release -p sift-core --test live_index_eval -- --ignored --nocapture
```

`benchmarks/live_eval_cases.json` is personal (it names your files) and is ignored by git. The format is documented at the top of [`live_index_eval.rs`](crates/sift-core/tests/live_index_eval.rs). Please include before/after results in your PR.

## Reporting a bad search result

Open an issue with:
- the query, and which filter chip was active,
- the file you expected, and roughly where it ranked (or that it was missing),
- what came up instead.

Don't attach files you wouldn't share publicly. Describing them is enough.

## Privacy rules for contributions

Sift indexes people's private files. Code must never log file contents, snippets, OCR text, or queries at `info` level or above, and must not make network requests beyond the one-time model download (see [ADR 003](docs/ADR/003-logging-and-privacy-boundary.md)).
