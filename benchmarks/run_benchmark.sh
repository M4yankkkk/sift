#!/usr/bin/env bash
set -e

echo "============================================="
echo "  Sift Benchmark: Synthetic Corpus Evaluation"
echo "============================================="
echo "OS: $(uname -s) $(uname -r) ($(uname -m))"
echo "Rust: $(rustc --version)"
echo "Date: $(date -u)"
echo "---------------------------------------------"

cargo test -p sift-core --test benchmark_test -- --nocapture
cargo test -p sift-core --test phase7_ml_benchmark_test -- --nocapture

echo "============================================="
echo "All benchmarks completed successfully."
