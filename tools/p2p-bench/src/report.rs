use std::fs;
use std::path::Path;
use std::process::Command;

use crate::metrics::BenchmarkResult;
use crate::table::{best_throughput, markdown_table, worst_throughput};

pub fn write_outputs(output: &Path, results: &[BenchmarkResult]) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|error| error.to_string())?;
    write_json(&output.join("results.json"), results)?;
    write_csv(&output.join("results.csv"), results)?;
    write_report(&output.join("report.md"), results)?;
    Ok(())
}

fn write_json(path: &Path, results: &[BenchmarkResult]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(results).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())
}

fn write_csv(path: &Path, results: &[BenchmarkResult]) -> Result<(), String> {
    let mut writer = csv::Writer::from_path(path).map_err(|error| error.to_string())?;
    writer
        .write_record([
            "scenario",
            "object_size_bytes",
            "fragment_size_bytes",
            "downloaders",
            "run",
            "total_duration_ms",
            "throughput_mib_s",
            "bytes_from_peer",
            "bytes_from_origin",
            "bytes_from_replica",
            "fragments_from_peer",
            "fragments_from_origin",
            "fragments_from_replica",
            "peer_traffic_ratio",
            "origin_traffic_reduction_percent",
            "time_to_first_fragment_ms",
            "fragment_latency_avg_ms",
            "fragment_latency_p95_ms",
            "fragment_latency_p99_ms",
            "hash_validation_time_ms",
            "fallback_activations",
            "peer_failures",
            "peer_hash_failures",
            "object_hash_valid",
        ])
        .map_err(|error| error.to_string())?;
    for result in results {
        writer
            .write_record([
                result.scenario.clone(),
                result.object_size_bytes.to_string(),
                result.fragment_size_bytes.to_string(),
                result.downloaders.to_string(),
                result.run.to_string(),
                result.total_duration_ms.to_string(),
                format!("{:.6}", result.throughput_mib_s),
                result.bytes_from_peer.to_string(),
                result.bytes_from_origin.to_string(),
                result.bytes_from_replica.to_string(),
                result.fragments_from_peer.to_string(),
                result.fragments_from_origin.to_string(),
                result.fragments_from_replica.to_string(),
                format!("{:.6}", result.peer_traffic_ratio),
                format!("{:.6}", result.origin_traffic_reduction_percent),
                result
                    .time_to_first_fragment_ms
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                format!("{:.6}", result.fragment_latency_avg_ms),
                format!("{:.6}", result.fragment_latency_p95_ms),
                format!("{:.6}", result.fragment_latency_p99_ms),
                result.hash_validation_time_ms.to_string(),
                result.fallback_activations.to_string(),
                result.peer_failures.to_string(),
                result.peer_hash_failures.to_string(),
                result.object_hash_valid.to_string(),
            ])
            .map_err(|error| error.to_string())?;
    }
    writer.flush().map_err(|error| error.to_string())
}

fn write_report(path: &Path, results: &[BenchmarkResult]) -> Result<(), String> {
    let best = best_throughput(results);
    let worst = worst_throughput(results);
    let mut report = String::new();
    report.push_str("# Ponte Mesh SDK P2P Benchmark\n\n");
    report.push_str("## Ambiente\n\n");
    report.push_str(&format!("- OS: {}\n", std::env::consts::OS));
    report.push_str(&format!(
        "- CPU: {}\n",
        command_output(
            "sh",
            &[
                "-c",
                "nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo unknown"
            ]
        )
    ));
    report.push_str(&format!(
        "- RAM: {}\n",
        command_output(
            "sh",
            &[
                "-c",
                "free -h 2>/dev/null | awk '/Mem:/ {print $2}' || echo unknown"
            ]
        )
    ));
    report.push_str(&format!(
        "- Rust version: {}\n",
        command_output("rustc", &["--version"])
    ));
    report.push_str("- build mode: release\n");
    report.push_str("- libp2p transport: request-response CBOR\n");
    report.push_str("- secure channel: Noise\n");
    report.push_str("- multiplexer: Yamux\n\n");
    report.push_str("## Cenários\n\n");
    report.push_str("- Origin-only baseline\n- P2P com 1 seeder\n- P2P em malha\n- P2P com falha parcial e fallback\n\n");
    report.push_str("## Resultados resumidos\n\n");
    report.push_str(&summary_table(results));
    report.push_str("\n## Tabela detalhada\n\n");
    report.push_str(&markdown_table(results));
    report.push_str("\n## Melhor caso\n\n");
    if let Some(result) = best {
        report.push_str(&format!(
            "- {}: {:.2} MiB/s, {:.1}% P2P\n",
            result.scenario,
            result.throughput_mib_s,
            result.peer_traffic_ratio * 100.0
        ));
    }
    report.push_str("\n## Pior caso\n\n");
    if let Some(result) = worst {
        report.push_str(&format!(
            "- {}: {:.2} MiB/s, {:.1}% P2P\n",
            result.scenario,
            result.throughput_mib_s,
            result.peer_traffic_ratio * 100.0
        ));
    }
    report.push_str("\n## Comparação Origin-only vs P2P\n\n");
    report.push_str("Os cenários P2P são comparados com o baseline Origin-only da mesma combinação de tamanho, fragmento, downloaders e run.\n\n");
    report.push_str("## Redução de tráfego do Origin\n\n");
    for result in results
        .iter()
        .filter(|result| result.scenario != "origin-only")
    {
        report.push_str(&format!(
            "- {} objeto={} fragmento={} downloaders={} run={}: {:.2}%\n",
            result.scenario,
            result.object_size_bytes,
            result.fragment_size_bytes,
            result.downloaders,
            result.run,
            result.origin_traffic_reduction_percent
        ));
    }
    report.push_str("\n## Observações\n\n");
    report.push_str("- O benchmark usa Libp2pTransport com PeerId real, Noise, Yamux e request-response CBOR.\n");
    report.push_str("- Cada fragmento e cada objeto final são validados com SHA-256.\n");
    report.push_str(
        "- Uso aproximado de memória é estimado a partir do objeto, fragmentos e downloaders.\n\n",
    );
    report.push_str("## Limitações\n\n");
    report.push_str("- Benchmark local em loopback não representa Internet real.\n");
    report.push_str("- NAT traversal, relay e DHT não são medidos aqui.\n");
    report.push_str("- Resultados variam por máquina.\n\n");
    report.push_str("## Como reproduzir\n\n");
    report.push_str("```bash\n./scripts/benchmark-p2p-transfer.sh\n```\n");
    fs::write(path, report).map_err(|error| error.to_string())
}

fn summary_table(results: &[BenchmarkResult]) -> String {
    let mut out =
        String::from("| scenario | runs | avg MiB/s | avg P2P % | avg origin reduction % |\n");
    out.push_str("|---|---:|---:|---:|---:|\n");
    let scenarios = [
        "origin-only",
        "p2p-single-seeder",
        "p2p-mesh",
        "p2p-fallback",
    ];
    for scenario in scenarios {
        let items: Vec<_> = results
            .iter()
            .filter(|result| result.scenario == scenario)
            .collect();
        if items.is_empty() {
            continue;
        }
        let len = items.len() as f64;
        out.push_str(&format!(
            "| {scenario} | {} | {:.2} | {:.1} | {:.1} |\n",
            items.len(),
            items
                .iter()
                .map(|result| result.throughput_mib_s)
                .sum::<f64>()
                / len,
            items
                .iter()
                .map(|result| result.peer_traffic_ratio)
                .sum::<f64>()
                * 100.0
                / len,
            items
                .iter()
                .map(|result| result.origin_traffic_reduction_percent)
                .sum::<f64>()
                / len
        ));
    }
    out
}

fn command_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
