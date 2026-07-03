#!/usr/bin/env bash
set -euo pipefail

cargo run --release -p p2p-bench -- \
  --output target/pontemesh-benchmarks \
  --object-sizes 1MiB,10MiB,100MiB \
  --fragment-sizes 64KiB,256KiB,1MiB \
  --downloaders 1,3,5,10 \
  --runs 3

