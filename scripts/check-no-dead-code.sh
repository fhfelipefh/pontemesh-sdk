#!/usr/bin/env bash
set -euo pipefail

for path in package.json package-lock.json tsconfig.json bindings/node examples/node-basic dist node_modules; do
  if [ -e "$path" ]; then
    echo "Forbidden legacy Node/TypeScript artifact found: $path" >&2
    exit 1
  fi
done

if find . \
  -path ./target -prune -o \
  -path ./.git -prune -o \
  -name '*.ts' -print -quit | grep -q .; then
  echo "Forbidden TypeScript source found. SDK core and bindings in this repo must be native." >&2
  find . -path ./target -prune -o -path ./.git -prune -o -name '*.ts' -print >&2
  exit 1
fi

if find . \
  -path ./target -prune -o \
  -path ./.git -prune -o \
  \( -name '*.js' -o -name '*.mjs' -o -name '*.cjs' \) -print -quit | grep -q .; then
  echo "Forbidden JavaScript source found. Add a native binding instead of reviving Node code." >&2
  find . -path ./target -prune -o -path ./.git -prune -o \( -name '*.js' -o -name '*.mjs' -o -name '*.cjs' \) -print >&2
  exit 1
fi

for forbidden in legacy-src legacy-tests vitest "npm run" "setup-node"; do
  if grep -R --exclude=check-no-dead-code.sh --exclude-dir=.git --exclude-dir=target -n "$forbidden" . >/tmp/pontemesh-sdk-hygiene.txt; then
    echo "Forbidden legacy marker found: $forbidden" >&2
    cat /tmp/pontemesh-sdk-hygiene.txt >&2
    exit 1
  fi
done
