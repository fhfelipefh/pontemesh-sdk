#!/usr/bin/env bash
set -euo pipefail

FORBIDDEN_REGEX='mock|fake|placeholder|stub|dummy|planned|experimental|todo|fixme|temporary|simulation-only|test-only|not production|not secure|future implementation|partial implementation|marked partial|status.*partial|partial:'

echo "Checking forbidden production markers..."

SEARCH_PATHS="crates bindings examples docs README.md Cargo.toml"

forbidden_hits=$(grep -RInE "$FORBIDDEN_REGEX" $SEARCH_PATHS \
  --exclude-dir=target \
  --exclude-dir=.git \
  --exclude-dir=tests \
  --exclude='Cargo.lock' \
  | grep -v 'production-no-mock-gate.sh' || true)

if [[ -n "$forbidden_hits" ]]; then
  echo "$forbidden_hits"
  echo "Forbidden mock/placeholder/partial marker found."
  exit 1
fi

echo "Checking libp2p dependency..."
grep -RIn "libp2p" crates/pontemesh-sdk-core/Cargo.toml

echo "Checking secure transport docs..."
if grep -RIn "P2P secure encrypted channel.*Partial" docs; then
  echo "P2P secure encrypted channel is still marked partial."
  exit 1
fi

echo "Checking production build..."
cargo fmt -- --check
cargo test
cargo test -p pontemesh-sdk-core --test libp2p_mesh_simulation -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test libp2p_malicious_peer -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test libp2p_traffic_reduction -- --ignored --nocapture
cargo build --release
cargo build -p pontemesh-sdk-c --release

echo "Production no-mock gate passed."
