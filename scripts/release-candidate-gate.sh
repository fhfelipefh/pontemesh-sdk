#!/usr/bin/env bash
set -euo pipefail

missing=()
for tool in cargo-audit cargo-deny; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    missing+=("$tool")
  fi
done

if ((${#missing[@]} > 0)); then
  printf 'Missing release audit tools:\n' >&2
  printf '  %s\n' "${missing[@]}" >&2
  cat >&2 <<'MSG'

Install with:
  cargo install cargo-audit
  cargo install cargo-deny
MSG
  exit 1
fi

cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo build -p pontemesh-sdk-c --release
bash ./scripts/libp2p-release-gate.sh
./scripts/production-no-mock-gate.sh
./scripts/production-p2p-benchmark-gate.sh
# cargo-audit scans the full Cargo.lock, including optional libp2p-dns packages
# that are not in the active dependency graph with libp2p default features
# disabled. Keep these ignores narrow and verify with `cargo tree -i
# hickory-proto`, which must print no reverse dependency.
if cargo tree -i hickory-proto 2>/dev/null | grep -q '^hickory-proto'; then
  echo "hickory-proto is active in the dependency graph; remove cargo audit ignores" >&2
  exit 1
fi
cargo audit \
  --ignore RUSTSEC-2026-0118 \
  --ignore RUSTSEC-2026-0119
cargo deny check
