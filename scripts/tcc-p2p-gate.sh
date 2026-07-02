#!/usr/bin/env bash
set -euo pipefail

cargo fmt -- --check
cargo test
cargo test -p pontemesh-sdk-core --test p2p_mesh_simulation -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test p2p_malicious_peer -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test p2p_traffic_reduction -- --ignored --nocapture
cargo build --release
cargo build -p pontemesh-sdk-c --release

cat <<'EOF'
P2P bytes > 0
Origin traffic reduced
Malicious peer rejected
Object hash validated
C ABI build OK
EOF
