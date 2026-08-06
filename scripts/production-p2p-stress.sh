#!/usr/bin/env bash
set -euo pipefail

cargo run --release -p p2p-bench -- \
  --profile stress \
  --output target/pontemesh-benchmarks-stress \
  --force

