#!/usr/bin/env bash
set -euo pipefail

cat <<'MSG'
Optional Linux loopback shaping for local P2P benchmarks:

  sudo tc qdisc add dev lo root netem delay 20ms rate 100mbit
  ./scripts/benchmark-p2p-transfer.sh
  sudo tc qdisc del dev lo root

This script does not apply tc netem automatically because it requires sudo.
MSG

