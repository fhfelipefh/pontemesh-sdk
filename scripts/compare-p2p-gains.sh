#!/usr/bin/env bash
set -euo pipefail

OUT="${PONTEMESH_P2P_COMPARISON_OUT:-target/pontemesh-p2p-comparison}"
OBJECT_SIZES="${PONTEMESH_P2P_COMPARISON_OBJECT_SIZES:-1MiB,10MiB}"
FRAGMENT_SIZES="${PONTEMESH_P2P_COMPARISON_FRAGMENT_SIZES:-256KiB,1MiB}"
DOWNLOADERS="${PONTEMESH_P2P_COMPARISON_DOWNLOADERS:-3,5}"
RUNS="${PONTEMESH_P2P_COMPARISON_RUNS:-2}"
CONCURRENT_DOWNLOADERS="${PONTEMESH_P2P_COMPARISON_CONCURRENT_DOWNLOADERS:-true}"

cargo run --release -p p2p-bench -- \
  --output "$OUT" \
  --object-sizes "$OBJECT_SIZES" \
  --fragment-sizes "$FRAGMENT_SIZES" \
  --downloaders "$DOWNLOADERS" \
  --runs "$RUNS" \
  --concurrent-downloaders "$CONCURRENT_DOWNLOADERS" \
  --force

python3 - "$OUT/results.json" "$OUT/comparison.json" "$OUT/comparison.md" <<'PY'
import json
import sys
from collections import defaultdict

results_path, json_path, md_path = sys.argv[1:4]
with open(results_path, "r", encoding="utf-8") as handle:
    results = json.load(handle)

by_key = defaultdict(dict)
for item in results:
    key = (
        item["objectSizeBytes"],
        item["fragmentSizeBytes"],
        item["downloaders"],
        item["run"],
    )
    by_key[key][item["scenario"]] = item

comparisons = []
for key, scenarios in sorted(by_key.items()):
    baseline = scenarios.get("origin-only")
    if not baseline:
        raise SystemExit(f"missing origin-only baseline for {key}")
    for scenario, item in sorted(scenarios.items()):
        if scenario == "origin-only":
            continue
        origin_reduction_bytes = baseline["bytesFromOrigin"] - item["bytesFromOrigin"]
        throughput_delta_percent = (
            ((item["throughputMibS"] - baseline["throughputMibS"]) / baseline["throughputMibS"]) * 100.0
            if baseline["throughputMibS"] > 0
            else 0.0
        )
        comparisons.append({
            "scenario": scenario,
            "objectSizeBytes": key[0],
            "fragmentSizeBytes": key[1],
            "downloaders": key[2],
            "run": key[3],
            "baselineOriginBytes": baseline["bytesFromOrigin"],
            "p2pOriginBytes": item["bytesFromOrigin"],
            "bytesFromPeer": item["bytesFromPeer"],
            "originReductionBytes": origin_reduction_bytes,
            "originReductionPercent": item["originTrafficReductionPercent"],
            "baselineThroughputMibS": baseline["throughputMibS"],
            "p2pThroughputMibS": item["throughputMibS"],
            "throughputDeltaPercent": throughput_delta_percent,
            "peerTrafficRatioPercent": item["peerTrafficRatio"] * 100.0,
            "objectHashValid": item["objectHashValid"],
            "timeouts": item["timeouts"],
            "panics": item["panics"],
        })

if not comparisons:
    raise SystemExit("no P2P scenarios were produced")
if any(not item["objectHashValid"] for item in comparisons):
    raise SystemExit("at least one P2P comparison has invalid object hash")
if any(item["timeouts"] or item["panics"] for item in comparisons):
    raise SystemExit("at least one P2P comparison has timeout or panic")
if all(item["bytesFromPeer"] == 0 for item in comparisons):
    raise SystemExit("P2P comparison produced zero peer bytes")
if all(item["originReductionBytes"] <= 0 for item in comparisons):
    raise SystemExit("P2P comparison did not reduce Origin traffic")

totals = {
    "comparisons": len(comparisons),
    "totalBaselineOriginBytes": sum(item["baselineOriginBytes"] for item in comparisons),
    "totalP2pOriginBytes": sum(item["p2pOriginBytes"] for item in comparisons),
    "totalBytesFromPeer": sum(item["bytesFromPeer"] for item in comparisons),
}
totals["totalOriginReductionBytes"] = totals["totalBaselineOriginBytes"] - totals["totalP2pOriginBytes"]
totals["totalOriginReductionPercent"] = (
    (totals["totalOriginReductionBytes"] / totals["totalBaselineOriginBytes"]) * 100.0
    if totals["totalBaselineOriginBytes"] > 0
    else 0.0
)

payload = {"ok": True, "totals": totals, "comparisons": comparisons}
with open(json_path, "w", encoding="utf-8") as handle:
    json.dump(payload, handle, indent=2)
    handle.write("\n")

lines = [
    "# Ponte Mesh SDK P2P Gain Comparison",
    "",
    "## Summary",
    "",
    f"- comparisons: `{totals['comparisons']}`",
    f"- totalBaselineOriginBytes: `{totals['totalBaselineOriginBytes']}`",
    f"- totalP2pOriginBytes: `{totals['totalP2pOriginBytes']}`",
    f"- totalBytesFromPeer: `{totals['totalBytesFromPeer']}`",
    f"- totalOriginReductionBytes: `{totals['totalOriginReductionBytes']}`",
    f"- totalOriginReductionPercent: `{totals['totalOriginReductionPercent']:.2f}`",
    "",
    "## Detailed Comparison",
    "",
    "| scenario | object | fragment | downloaders | run | origin-only bytes | p2p origin bytes | peer bytes | origin reduction % | throughput delta % |",
    "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
]
for item in comparisons:
    lines.append(
        "| {scenario} | {objectSizeBytes} | {fragmentSizeBytes} | {downloaders} | {run} | "
        "{baselineOriginBytes} | {p2pOriginBytes} | {bytesFromPeer} | "
        "{originReductionPercent:.2f} | {throughputDeltaPercent:.2f} |".format(**item)
    )
lines.extend([
    "",
    "## Verdict",
    "",
    "P2P reduced Origin traffic and transferred bytes from peers under the same object, fragment, downloader, and run matrix.",
    "",
    "## Artifacts",
    "",
    "- rawResults: `results.json`",
    "- rawCsv: `results.csv`",
    "- benchmarkReport: `report.md`",
    "- comparisonJson: `comparison.json`",
])
with open(md_path, "w", encoding="utf-8") as handle:
    handle.write("\n".join(lines))
    handle.write("\n")

print(json.dumps(payload["totals"], indent=2))
PY

printf '\nP2P gain comparison passed.\n'
printf 'Report: %s\n' "$OUT/comparison.md"
