#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if rg -n "placeholder|planned" crates/pontemesh-sdk-core/src/p2p/libp2p_transport.rs >/dev/null; then
  echo "libp2p_transport.rs contains forbidden status wording" >&2
  exit 1
fi
rg -n "^libp2p =" crates/pontemesh-sdk-core/Cargo.toml >/dev/null
if rg -n "P2P secure encrypted channel \| Partial" docs/SDK_ACCEPTANCE_MATRIX.md >/dev/null; then
  echo "P2P secure encrypted channel is still Partial" >&2
  exit 1
fi
for test in libp2p_mesh_simulation libp2p_malicious_peer libp2p_traffic_reduction; do
  test -f "crates/pontemesh-sdk-core/tests/${test}.rs"
done
cargo fmt -- --check
cargo test
cargo test -p pontemesh-sdk-core --test libp2p_mesh_simulation -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test libp2p_malicious_peer -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test libp2p_traffic_reduction -- --ignored --nocapture
cargo build --release
cargo build -p pontemesh-sdk-c --release
