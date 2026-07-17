#!/usr/bin/env bash
set -euo pipefail

required_vars=(
  PONTEMESH_LIVE_ORIGIN_URL
  PONTEMESH_LIVE_APPLICATION_TOKEN
  PONTEMESH_LIVE_BUCKET
  PONTEMESH_LIVE_KEY
)

missing=()
for name in "${required_vars[@]}"; do
  if [[ -z "${!name:-}" ]]; then
    missing+=("$name")
  fi
done

if (( ${#missing[@]} > 0 )); then
  printf 'Missing required live Ponte Mesh variables: %s\n' "${missing[*]}" >&2
  exit 2
fi

PONTEMESH_LIVE_REQUIRED=1 cargo test -p pontemesh-sdk-core --test live_server_integration -- --nocapture
