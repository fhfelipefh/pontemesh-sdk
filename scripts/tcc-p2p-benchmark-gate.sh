#!/usr/bin/env bash
set -euo pipefail

cargo build --release
cargo build -p pontemesh-sdk-c --release

./scripts/tcc-libp2p-gate.sh

cargo run --release -p p2p-bench -- \
  --output target/pontemesh-benchmarks \
  --object-sizes 1MiB,10MiB,100MiB \
  --fragment-sizes 64KiB,256KiB,1MiB \
  --downloaders 1,3,5 \
  --runs 3

test -f target/pontemesh-benchmarks/results.json
test -f target/pontemesh-benchmarks/results.csv
test -f target/pontemesh-benchmarks/report.md

grep -q "bytes_from_peer" target/pontemesh-benchmarks/results.csv
grep -q "Ponte Mesh SDK P2P Benchmark" target/pontemesh-benchmarks/report.md

