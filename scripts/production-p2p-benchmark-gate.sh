#!/usr/bin/env bash
set -euo pipefail

OUT="target/pontemesh-benchmarks-production"

cargo fmt -- --check
cargo test
cargo build --release
cargo build -p pontemesh-sdk-c --release
bash ./scripts/libp2p-release-gate.sh
./scripts/production-no-mock-gate.sh

cargo run --release -p p2p-bench -- \
  --profile production \
  --output "$OUT" \
  --force

test -f "$OUT/benchmark.exit"
grep -qx "success" "$OUT/benchmark.exit"
test -f "$OUT/results.json"
test -f "$OUT/results.csv"
test -f "$OUT/report.md"
test -f "$OUT/results.jsonl"
test -f "$OUT/results.partial.csv"
test -f "$OUT/benchmark.state.json"

grep -q "Production-ready" "$OUT/report.md"
! grep -R -E "packageToken|applicationToken|bench-secret|Authorization|Bearer|S3 access key|S3 secret" "$OUT"

