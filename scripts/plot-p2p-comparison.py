#!/usr/bin/env python3
import json
import os
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 4:
        print(
            "usage: plot-p2p-comparison.py COMPARISON_JSON RESULTS_JSON OUTPUT_DIR",
            file=sys.stderr,
        )
        return 2

    comparison_path = Path(sys.argv[1])
    results_path = Path(sys.argv[2])
    output_dir = Path(sys.argv[3])
    output_dir.mkdir(parents=True, exist_ok=True)
    os.environ.setdefault("MPLCONFIGDIR", str(output_dir / ".matplotlib"))

    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    with comparison_path.open("r", encoding="utf-8") as handle:
        comparison = json.load(handle)
    with results_path.open("r", encoding="utf-8") as handle:
        results = json.load(handle)

    comparisons = comparison["comparisons"]
    write_summary_chart(plt, comparison["totals"], output_dir / "01_origin_reduction_total.png")
    write_origin_reduction_by_scenario(
        plt, comparisons, output_dir / "02_origin_reduction_by_scenario.png"
    )
    write_peer_bytes_by_downloaders(
        plt, comparisons, output_dir / "03_peer_bytes_by_downloaders.png"
    )
    write_throughput_delta(
        plt, comparisons, output_dir / "04_throughput_delta_by_scenario.png"
    )
    write_latency_chart(plt, results, output_dir / "05_latency_p95_by_scenario.png")

    manifest = {
        "images": [
            "01_origin_reduction_total.png",
            "02_origin_reduction_by_scenario.png",
            "03_peer_bytes_by_downloaders.png",
            "04_throughput_delta_by_scenario.png",
            "05_latency_p95_by_scenario.png",
        ]
    }
    with (output_dir / "manifest.json").open("w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2)
        handle.write("\n")
    print(json.dumps(manifest, indent=2))
    return 0


def write_summary_chart(plt, totals, path):
    labels = ["Origin-only", "P2P Origin", "P2P Peer"]
    values = [
        totals["totalBaselineOriginBytes"],
        totals["totalP2pOriginBytes"],
        totals["totalBytesFromPeer"],
    ]
    fig, ax = plt.subplots(figsize=(9, 5))
    bars = ax.bar(labels, [value / 1024 / 1024 for value in values], color=["#3b4252", "#bf616a", "#2e8b57"])
    ax.set_title("Total traffic served by source")
    ax.set_ylabel("MiB")
    ax.bar_label(bars, fmt="%.1f")
    ax.text(
        0.5,
        0.92,
        f"Origin traffic reduction: {totals['totalOriginReductionPercent']:.2f}%",
        transform=ax.transAxes,
        ha="center",
        fontsize=11,
    )
    fig.tight_layout()
    fig.savefig(path, dpi=160)
    plt.close(fig)


def write_origin_reduction_by_scenario(plt, comparisons, path):
    grouped = group_values(comparisons, "scenario", "originReductionPercent")
    labels = sorted(grouped)
    means = [statistics.mean(grouped[label]) for label in labels]
    fig, ax = plt.subplots(figsize=(10, 5))
    bars = ax.bar(labels, means, color="#5e81ac")
    ax.set_title("Average Origin traffic reduction by P2P scenario")
    ax.set_ylabel("Reduction (%)")
    ax.set_ylim(0, 105)
    ax.bar_label(bars, fmt="%.1f")
    fig.tight_layout()
    fig.savefig(path, dpi=160)
    plt.close(fig)


def write_peer_bytes_by_downloaders(plt, comparisons, path):
    grouped = defaultdict(lambda: defaultdict(list))
    for item in comparisons:
        grouped[item["scenario"]][item["downloaders"]].append(item["bytesFromPeer"] / 1024 / 1024)
    downloaders = sorted({item["downloaders"] for item in comparisons})
    scenarios = sorted(grouped)
    fig, ax = plt.subplots(figsize=(11, 5))
    width = 0.8 / len(scenarios)
    offsets = [index - (len(scenarios) - 1) / 2 for index in range(len(scenarios))]
    for scenario_index, scenario in enumerate(scenarios):
        values = [
            statistics.mean(grouped[scenario][downloader])
            if grouped[scenario][downloader]
            else 0.0
            for downloader in downloaders
        ]
        positions = [index + offsets[scenario_index] * width for index in range(len(downloaders))]
        ax.bar(positions, values, width=width, label=scenario)
    ax.set_title("Average peer-served traffic by downloader count")
    ax.set_xlabel("Downloaders")
    ax.set_ylabel("MiB from peers")
    ax.set_xticks(range(len(downloaders)), [str(value) for value in downloaders])
    ax.legend()
    fig.tight_layout()
    fig.savefig(path, dpi=160)
    plt.close(fig)


def write_throughput_delta(plt, comparisons, path):
    grouped = group_values(comparisons, "scenario", "throughputDeltaPercent")
    labels = sorted(grouped)
    means = [statistics.mean(grouped[label]) for label in labels]
    fig, ax = plt.subplots(figsize=(10, 5))
    bars = ax.bar(labels, means, color="#b48ead")
    ax.axhline(0, color="#2e3440", linewidth=1)
    ax.set_title("Average throughput delta vs Origin-only")
    ax.set_ylabel("Delta (%)")
    ax.bar_label(bars, fmt="%.1f")
    fig.tight_layout()
    fig.savefig(path, dpi=160)
    plt.close(fig)


def write_latency_chart(plt, results, path):
    grouped = group_values(results, "scenario", "fragmentLatencyP95Ms")
    labels = sorted(grouped)
    means = [statistics.mean(grouped[label]) for label in labels]
    fig, ax = plt.subplots(figsize=(10, 5))
    bars = ax.bar(labels, means, color="#d08770")
    ax.set_title("Average fragment p95 latency by scenario")
    ax.set_ylabel("Milliseconds")
    ax.bar_label(bars, fmt="%.1f")
    fig.tight_layout()
    fig.savefig(path, dpi=160)
    plt.close(fig)


def group_values(items, group_key, value_key):
    grouped = defaultdict(list)
    for item in items:
        grouped[item[group_key]].append(item[value_key])
    return grouped


if __name__ == "__main__":
    raise SystemExit(main())
