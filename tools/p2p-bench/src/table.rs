use crate::metrics::BenchmarkResult;

pub fn best_throughput(results: &[BenchmarkResult]) -> Option<&BenchmarkResult> {
    results
        .iter()
        .max_by(|left, right| left.throughput_mib_s.total_cmp(&right.throughput_mib_s))
}

pub fn worst_throughput(results: &[BenchmarkResult]) -> Option<&BenchmarkResult> {
    results
        .iter()
        .min_by(|left, right| left.throughput_mib_s.total_cmp(&right.throughput_mib_s))
}

pub fn markdown_table(results: &[BenchmarkResult]) -> String {
    let mut out = String::from("| scenario | object MiB | fragment KiB | downloaders | run | throughput MiB/s | peer bytes | origin bytes | P2P % | valid |\n");
    out.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---|\n");
    for result in results {
        out.push_str(&format!(
            "| {} | {:.1} | {:.1} | {} | {} | {:.2} | {} | {} | {:.1} | {} |\n",
            result.scenario,
            result.object_size_bytes as f64 / 1024.0 / 1024.0,
            result.fragment_size_bytes as f64 / 1024.0,
            result.downloaders,
            result.run,
            result.throughput_mib_s,
            result.bytes_from_peer,
            result.bytes_from_origin,
            result.peer_traffic_ratio * 100.0,
            result.object_hash_valid
        ));
    }
    out
}
