#!/usr/bin/env bash
set -euo pipefail

cargo run --release -p p2p-bench -- \
  --profile soak \
  --output target/pontemesh-benchmarks-soak \
  --force

cp target/pontemesh-benchmarks-soak/results.json target/pontemesh-benchmarks-soak/soak-results.json
cp target/pontemesh-benchmarks-soak/report.md target/pontemesh-benchmarks-soak/soak-report.md

