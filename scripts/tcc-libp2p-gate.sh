#!/usr/bin/env bash
set -euo pipefail

echo "Checking libp2p production transport markers..."
grep -RIn "libp2p" crates/pontemesh-sdk-core/Cargo.toml

if grep -RInE "placeholder|future implementation|not implemented" crates/pontemesh-sdk-core/src/p2p/libp2p_transport.rs; then
  echo "libp2p transport still contains non-production markers."
  exit 1
fi

if grep -RIn "P2P secure encrypted channel.*Partial" docs; then
  echo "P2P secure encrypted channel is still marked partial."
  exit 1
fi

cargo fmt -- --check
cargo test
cargo test -p pontemesh-sdk-core --test libp2p_traffic_reduction -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test libp2p_mesh_simulation -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test libp2p_malicious_peer -- --ignored --nocapture
cargo build --release
cargo build -p pontemesh-sdk-c --release
