<div align="center">

# Sift

### Find files on Linux by what's in them.

A keyboard-first desktop launcher that searches your photos, screenshots, PDFs, notes, and code by meaning, by exact text, and by the words inside images. It runs entirely on your machine, with Google DeepMind's **EmbeddingGemma 2** and a Rust core.

[![Rust](https://img.shields.io/badge/core-Rust-orange?logo=rust&logoColor=white)](crates/sift-core)
[![Model](https://img.shields.io/badge/model-EmbeddingGemma_2-4285F4?logo=google&logoColor=white)](https://huggingface.co/google/embeddinggemma-2)
[![Search](https://img.shields.io/badge/keyword-Tantivy_BM25-8A2BE2)](https://github.com/quickwit-oss/tantivy)
[![Platform](https://img.shields.io/badge/platform-Linux_(X11_%26_Wayland)-E95420?logo=linux&logoColor=white)](#install)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<!-- Demo: add docs/demo.gif here after recording -->

</div>

---

## What it does

Press `Ctrl+Alt+Space` anywhere and type what you remember:

| You type | Sift finds |
| :--- | :--- |
| `sunset` | the sunset photo in `~/Documents/images (1).jpeg`, which is not in the filename |
| `country` | a poster of India's map made of people |
| `error message` | a screenshot of a crash dialog, matched by what's in the picture *and* its OCR text |
| `database retry` | the distributed-systems notes PDF that explains retries, plus your SQL schema |
| `resume pdf` | only your résumé PDFs, with duplicate copies folded into one result |
| `"exact phrase"` / `ERR_0x1A4` | exact keyword hits ranked first, with highlighted snippets |

Everything happens locally. After a one-time model download (≈435 MB), Sift makes no network requests.

## Why

Linux file search finds names, and grep finds exact strings. Neither finds "that photo of a sunset" or "the notes that explained retries". Sift answers those with a design built for a resident desktop tool:

- **One process, no runtime to install.** The model runs in-process on ONNX Runtime from Rust. There is no Python, Node, or llama.cpp sidecar.
- **Three kinds of search fused into one list:** meaning (EmbeddingGemma 2), exact keywords (Tantivy BM25), and the text inside images (Tesseract OCR, with word boxes drawn on the preview).
- **A launcher, not a window:** a global hotkey, `Ctrl+K` actions, an instant preview, and PDFs that open at the matching page.

## How search works

Each query runs four retrievers once, then fuses them:

1. **Filenames** (SQLite). Whole-word matches count as strong evidence, substrings inside words as weak.
2. **Keywords** (Tantivy BM25). Every word must match first; if nothing does, Sift falls back to any word and drops weak hits.
3. **Text meaning.** EmbeddingGemma 2 embeds document chunks and OCR text, prefixed with `title: … | text: …`.
4. **Image meaning.** EmbeddingGemma 2 embeds photos into the **same 768-d space** as text, so a text query compares directly against pictures.

**Per-query relevance gates instead of fixed cut-offs.** Cosine similarities drift with every query, so a fixed threshold either floods results or hides them. Sift scores a query against the whole index and keeps only results that stand out from that query's own distribution. These thresholds were calibrated on a real index: genuine image matches landed 4–6σ above the query's mean (`sunset` 5.9σ, `dog` 6.4σ), and queries with no matching photo topped out near 3σ.

**Fusion.** Weighted reciprocal-rank fusion merges the lists. Each list's weight depends on how clearly it found a winner, so `earth` blends globe photos with documents while `database retry` shows no images at all.

**Query intent.**
- Type words filter results: `sunset photo`, `pic of earth`, `resume pdf`, `slides on transformers`, or the explicit `type:pdf` / `ext:png`. A subject word like "sunset" is never treated as a filter.
- Identifier-shaped queries (`snake_case`, `CamelCase`, `foo.rs`) favour code over images.

**Noise control.** Theme assets, icon sprites, and tiny images rank lower. OCR fragments without real words are left out of meaning search, though keyword search still covers them. Identical copies collapse into one result marked "+N identical copies elsewhere".

The filter chips (`All · Filename · Content · Visual`) show how many results each mode has for the current query. Visual mode shows a thumbnail grid.

Click the status line in the footer to see Sift's live memory and CPU use. Memory covers the whole app, including the WebKit interface processes, and is measured as PSS so shared memory isn't counted twice.

## Measured performance

All numbers below are from one laptop: Intel i7-13620H (10 cores), 16 GB RAM, Ubuntu 24.04, CPU only. The index holds 5,471 files: 670 photos and screenshots, about 12k meaningful text chunks, and PDFs up to 1,300 pages.

| | |
| :--- | :--- |
| Search latency (release build) | **72–92 ms** per query, all four retrievers, fused |
| Search core memory, model loaded | **≈480 MB** |
| Search core memory after 5 idle minutes (model released) | **≈140 MB**; the next search reloads it in ≈0.6 s |
| Peak memory while indexing a 1,300-page PDF | ≈0.95 GB |
| Image indexing (one-time) | ≈0.6 s per image on 8 threads |
| Model download (first run) | ≈435 MB (q8 text model + q4 vision encoder) |
| Search-quality regression set (20 graded real queries) | **20 / 20** pass ([`live_index_eval.rs`](crates/sift-core/tests/live_index_eval.rs)) |

Model correctness: Sift's Rust port of the EmbeddingGemma 2 image pipeline reproduces the model card's reference similarities (0.744 vs 0.740 on its cat photo, with 0.460 and 0.497 for the non-matching queries).

## Install

Requirements: Linux with X11 or Wayland (tested on GNOME), Rust 1.75+, Node.js 18+.

```bash
# System libraries: WebKitGTK for the UI, Tesseract for OCR, poppler for PDFs
sudo apt install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf \
                    tesseract-ocr poppler-utils

git clone https://github.com/M4yankkkk/sift.git
cd sift
npm install

npm run tauri dev       # run in development
./scripts/install.sh    # or: build a release, install the .deb, enable autostart and a Super+Space shortcut on GNOME
```

On first launch, add folders to index (for example `~/Documents` and `~/Downloads`). Sift downloads EmbeddingGemma 2 once, indexes in the background with progress in the status bar, and then follows file changes live.

## Keyboard

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+Alt+Space` | Show / hide Sift (default; change it in Settings) |
| `↑` `↓` or `Ctrl+N` `Ctrl+P` | Move through results |
| `Enter` / `Ctrl+O` | Open the file (PDFs open at the matching page) |
| `Ctrl+R` | Reveal in file manager |
| `Ctrl+T` | Open a terminal in the file's folder |
| `Ctrl+K` | Quick actions (open, reveal, terminal, copy path, copy content) |
| `Tab` | Toggle the preview panel |
| `Ctrl+L` | Focus the search box |
| `Ctrl+,` / `Ctrl+Shift+F` | Settings / indexed folders |
| `Esc` | Close menu → clear query → hide window |

## Configuration

| Variable | Effect |
| :--- | :--- |
| `SIFT_EMBEDDING_MODEL=legacy` | Use the older text-only EmbeddingGemma + CLIP instead of EmbeddingGemma 2 |
| `SIFT_EMBED_THREADS=N` | ONNX Runtime threads (default: half your cores, 2–8) |
| `SIFT_DB_PATH=/path/sift.db` | Index location (default `~/.local/share/sift/`) |
| `SIFT_CACHE_DIR=/path` | Model cache (default `~/.cache/sift/models`) |

Settings also has switches for the semantic engine and OCR, plus Eco / Performance indexing modes.

Changing the embedding model is safe. Every vector records which model produced it, vectors from different models are never compared, and Sift re-embeds outdated files in the background (images first, resumable).

## Development

```bash
cargo test -p sift-core          # unit + integration tests (offline mock encoder, no download)

# Search-quality regression against your own index (real models; copies the DB first)
SIFT_EVAL_DB=src-tauri/sift.db SIFT_EVAL_CASES=benchmarks/live_eval_cases.json \
  cargo test --release -p sift-core --test live_index_eval -- --ignored --nocapture
```

Layout: [`crates/sift-core`](crates/sift-core) holds indexing, search, and models (`gemma2.rs` is the EmbeddingGemma 2 runtime and `ranking.rs` handles intent, gating, and fusion). [`src-tauri`](src-tauri) is the desktop shell, and [`src`](src) is the React UI.

## Limitations

- **No audio or video search yet.** EmbeddingGemma 2 supports both, so this is the obvious next step.
- **CPU only.** The first full index of a large photo library takes a while (≈0.6 s per image). Optional GPU indexing is planned.
- **Linux only**, developed and tested on Ubuntu/GNOME. Other desktops should work but are less tested.
- Thresholds were calibrated on one person's files. If a query surprises you, please open an issue with the query and the file you expected.

## Credits

- [Google DeepMind](https://deepmind.google/) for [EmbeddingGemma 2](https://huggingface.co/google/embeddinggemma-2) (Apache 2.0), and the [ONNX export](https://huggingface.co/onnx-community/embeddinggemma-2-ONNX) by onnx-community.
- [Tantivy](https://github.com/quickwit-oss/tantivy), [ONNX Runtime](https://onnxruntime.ai/) via [`ort`](https://github.com/pykeio/ort), [Tesseract](https://github.com/tesseract-ocr/tesseract), and [Tauri](https://tauri.app/).

## License

[MIT](LICENSE) © 2026 Mayank Tiwari
