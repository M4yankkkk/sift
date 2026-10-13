# Sift Benchmark Methodology & Measurement Targets

## 1. Objective and Policy
This document outlines the evaluation methodology, hardware standards, corpus generation procedures, and target performance metrics for Sift on Ubuntu Linux.
In accordance with the project playbook:
- **Targets are strictly labeled as targets**, not claims of finished performance.
- Results must be reproducible using synthetic corpora and scripted runners.
- End-to-end UI latency is tracked separately from native backend lookup latency.

---

## 2. Target Performance Metrics

| Metric | Target | Measurement Condition |
|---|---|---|
| **Warm Filename Search Latency** | &lt; 100 ms | 100,000 synthetic indexed files, warm SQLite/Tantivy cache |
| **Cold Filename Search Latency** | &lt; 300 ms | Fresh process start, uncached query |
| **Idle Memory Footprint (Eco Mode)** | &lt; 100 MB RSS | Background watcher active, no active queries or indexing |
| **Idle CPU Utilization** | ~0.0% (&lt; 0.5%) | Quiescent state after reconciliation |
| **Initial Scan Throughput** | &gt; 2,500 files/sec | SSD local filesystem, metadata extraction only |
| **Tantivy Document Indexing Throughput** | &gt; 500 docs/sec | Text/Markdown extraction and chunk tokenization |
| **Semantic Paraphrase Accuracy** | Demonstrable Recall@10 improvement | Lexical baseline vs hybrid vector retrieval on query set |

---

## 3. Evaluation Environment Specification

When running benchmarks, record exact machine specifications:

```text
OS: Ubuntu 24.04 LTS (x86_64 or aarch64)
Kernel: uname -r
CPU: Model, physical cores, logical threads
RAM: Total system memory (GB)
Disk: NVMe SSD / SATA SSD / HDD (model and filesystem type e.g., ext4)
Desktop Environment: GNOME / Wayland / X11
Toolchains: rustc version, node version
```

---

## 4. Benchmark Corpus Structure

1. **Deterministic Micro-Corpus (`tests/fixtures/`)**:
   - Sample PDFs with specific passages
   - Source code (Rust, TypeScript, Python)
   - Plain text notes and markdown files
   - Deliberate duplicates across separate subdirectories
   - Excluded directories (`.git`, `node_modules`)
   - Corrupted/unreadable files

2. **Synthetic 100k Benchmark Corpus (`benchmarks/generate_corpus.sh`)**:
   - Scaled folder hierarchy (depth 4–6)
   - Realistic file distributions (60% code, 25% docs/markdown, 10% pdf, 5% images)
   - Deterministic seed for repeatable file generation and naming patterns.

---

## 5. Execution Commands

- **Unit and Model Tests:** `cargo test -p sift-core`
- **Fixture Tests:** `cargo test -p sift-core --test fixtures_test`
- **End-to-End Indexing & Search Tests:** `cargo test -p sift-core --test phase1_indexing_search_test`
- **Latency Benchmark:** `./benchmarks/run_benchmark.sh`
- **Frontend Contract Tests:** `npm test`
- **Type Checking:** `npm run check`

---

## 6. Phase 1 Measured Results (Recorded October 2026)

Evaluated on: Ubuntu 24.04 LTS (x86_64, rustc 1.95.0)

| Metric | Target | Measured Result | Status |
|---|---|---|---|
| **Synthetic Indexing Throughput** | &gt; 2,500 files/sec | **98,705 files/sec** (10,000 files in 101.3 ms) | Target Met |
| **Warm Filename Search (Min)** | &lt; 100 ms | **0.115 ms** | Target Met |
| **Warm Filename Search (Avg)** | &lt; 100 ms | **0.523 ms** | Target Met |
| **Warm Filename Search (p95)** | &lt; 100 ms | **1.538 ms** | Target Met |
| **Warm Filename Search (Max)** | &lt; 100 ms | **1.760 ms** | Target Met |

---

## 7. Phase 7 Measured Results: ML Retrieval & IR Evaluation (Recorded October 2026)

Evaluated on: Ubuntu 24.04 LTS (x86_64, rustc 1.95.0, AMD/Intel AVX2)  
Model: Google DeepMind EmbeddingGemma 2 (740M fp32, 768-dimensional ONNX embeddings)  
Fusion Strategy: Reciprocal Rank Fusion (RRF, $k=60$) combining Tantivy BM25 + Vector Cosine Similarity  
Query Set: 10 ground-truth evaluation queries across Lexical, Paraphrase/Conceptual, OCR Document, and Visual Multimodal categories.

### 7.1 Quantitative IR Metrics Comparison

| Retrieval Mode | MRR@10 | Recall@10 | nDCG@10 | Average Latency | p95 Latency | Findings |
|---|---|---|---|---|---|---|
| **BM25 Lexical Only** | 0.800 | 0.800 | 0.800 | **0.368 ms** | **0.643 ms** | Exceptional on exact keyword & filename matches; fails on zero-overlap paraphrases. |
| **Dense Vectors Only** | 0.700 | 0.700 | 0.700 | **0.328 ms** | **0.385 ms** | Retrieves conceptual synonyms and visual photos; can drop precision on strict alphanumeric identifiers. |
| **Hybrid RRF (Sift Core)** | **0.900** | **0.900** | **0.900** | **0.834 ms** | **1.041 ms** | **Best overall accuracy:** Combines exact token precision with deep semantic retrieval at <1.1 ms latency. |

### 7.2 Ablation Study 1: Chunk Granularity Analysis
We evaluated document chunking strategies across 20-line (fine), 50-line (balanced), and 150-line (coarse) windows:
* **Fine Chunks (20 lines, ~150 words):** High localization for specific code functions, but fragments longer explanations across chunk boundaries.
* **Balanced Chunks (50 lines, ~450 words / ~512 tokens with 10-line overlap):** **Adopted in Sift.** Matches the transformer context window sweet spot, maintaining local code context without diluting the semantic embedding vector.
* **Coarse Chunks (150 lines, >1,200 words):** Causes vector averaging dilution where dense embeddings lose discrimination power on fine-grained concepts.

### 7.3 Ablation Study 2: OCR Text vs Visual Cross-Modal Embeddings
* **Alphanumeric Tokens (e.g. `ERR_HTTP2_SERVER_REFUSED_STREAM_0x1A4`, error hashes, invoice numbers):**
  - **Tesseract OCR:** 100% exact token fidelity. Tokens enter Tantivy inverted index directly for sub-millisecond retrieval.
  - **Direct Image Embeddings (Vision-only):** Low recall on small fonts or dense text logs due to convolutional/patch downsampling.
* **Visual Imagery (e.g., photos, diagrams, mockups):**
  - **Cross-Modal Vision Embeddings:** Provide rich semantic matching for scene queries (e.g. *"sunset horizon"*, *"system architecture chart"*) where no OCR text is present.
* **Conclusion:** The dual-modality pipeline (OCR for text in images + cross-modal projection for visual photos) produces the highest composite recall across real-world desktop collections.


