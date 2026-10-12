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
- **Frontend Contract Tests:** `npm test`
- **Type Checking:** `npm run check`
