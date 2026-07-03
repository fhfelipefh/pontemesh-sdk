use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use pontemesh_sdk_core::contracts::SourceType;
use pontemesh_sdk_core::download::{TransferEvent, TransferSummary};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResult {
    pub scenario: String,
    pub object_size_bytes: u64,
    pub fragment_size_bytes: u64,
    pub downloaders: usize,
    pub run: usize,
    pub total_duration_ms: u128,
    pub throughput_mib_s: f64,
    pub bytes_from_peer: u64,
    pub bytes_from_origin: u64,
    pub bytes_from_replica: u64,
    pub fragments_from_peer: u64,
    pub fragments_from_origin: u64,
    pub fragments_from_replica: u64,
    pub peer_traffic_ratio: f64,
    pub origin_traffic_reduction_percent: f64,
    pub time_to_first_fragment_ms: Option<u128>,
    pub fragment_latency_avg_ms: f64,
    pub fragment_latency_p95_ms: f64,
    pub fragment_latency_p99_ms: f64,
    pub hash_validation_time_ms: u128,
    pub fallback_activations: u64,
    pub peer_failures: u64,
    pub peer_hash_failures: u64,
    pub object_hash_valid: bool,
    pub memory_peak_mb: u64,
    pub threads_started: u64,
    pub threads_finished: u64,
    pub open_connections_peak: u64,
    pub timeouts: u64,
    pub panics: u64,
    pub distinct_peer_sources_served: usize,
    pub downloaders_with_peer_bytes: usize,
    pub fairness_min_downloader_ms: u128,
    pub fairness_max_downloader_ms: u128,
}

#[derive(Debug, Default)]
pub struct MetricsCollector {
    transfer_started: Option<Instant>,
    first_fragment_at: Option<Duration>,
    fragment_latencies_ms: Vec<f64>,
    hash_validation_time_ms: u128,
    peer_sources: BTreeSet<String>,
}

impl MetricsCollector {
    pub fn record_download(
        &mut self,
        source_type: SourceType,
        source_id: String,
        duration_ms: u128,
    ) {
        if self.first_fragment_at.is_none() {
            if let Some(started) = self.transfer_started {
                self.first_fragment_at = Some(started.elapsed());
            }
        }
        self.fragment_latencies_ms.push(duration_ms as f64);
        if source_type == SourceType::Peer {
            self.peer_sources.insert(source_id);
        }
    }

    pub fn record_validation(&mut self, duration_ms: u128) {
        self.hash_validation_time_ms += duration_ms;
    }

    pub fn start_now(&mut self) {
        self.transfer_started = Some(Instant::now());
    }

    #[allow(dead_code)]
    pub fn observe(&mut self, event: TransferEvent) {
        match event {
            TransferEvent::TransferStarted => self.transfer_started = Some(Instant::now()),
            TransferEvent::FragmentDownloadFinished {
                source_type,
                source_id,
                duration_ms,
                ..
            } => {
                self.record_download(source_type, source_id, duration_ms);
            }
            TransferEvent::FragmentValidated { duration_ms, .. } => {
                self.record_validation(duration_ms);
            }
            _ => {}
        }
    }

    pub fn append(&mut self, other: MetricsCollector) {
        if self.first_fragment_at.is_none()
            || other.first_fragment_at.is_some_and(|other_first| {
                self.first_fragment_at
                    .is_some_and(|current| other_first < current)
            })
        {
            self.first_fragment_at = other.first_fragment_at;
        }
        self.fragment_latencies_ms
            .extend(other.fragment_latencies_ms);
        self.hash_validation_time_ms += other.hash_validation_time_ms;
        self.peer_sources.extend(other.peer_sources);
    }

    pub fn time_to_first_fragment_ms(&self) -> Option<u128> {
        self.first_fragment_at.map(|duration| duration.as_millis())
    }

    pub fn avg_latency_ms(&self) -> f64 {
        if self.fragment_latencies_ms.is_empty() {
            return 0.0;
        }
        self.fragment_latencies_ms.iter().sum::<f64>() / self.fragment_latencies_ms.len() as f64
    }

    pub fn percentile_ms(&self, percentile: f64) -> f64 {
        if self.fragment_latencies_ms.is_empty() {
            return 0.0;
        }
        let mut values = self.fragment_latencies_ms.clone();
        values.sort_by(|left, right| left.total_cmp(right));
        let rank = ((values.len() as f64 - 1.0) * percentile).ceil() as usize;
        values[rank.min(values.len() - 1)]
    }

    pub fn hash_validation_time_ms(&self) -> u128 {
        self.hash_validation_time_ms
    }

    pub fn distinct_peer_sources_served(&self) -> usize {
        self.peer_sources.len()
    }
}

pub fn add_summary(total: &mut TransferSummary, item: &TransferSummary) {
    total.bytes_from_peer += item.bytes_from_peer;
    total.bytes_from_origin += item.bytes_from_origin;
    total.bytes_from_replica += item.bytes_from_replica;
    total.fragments_from_peer += item.fragments_from_peer;
    total.fragments_from_origin += item.fragments_from_origin;
    total.fragments_from_replica += item.fragments_from_replica;
    total.fallback_activations += item.fallback_activations;
    total.peer_failures += item.peer_failures;
    total.peer_hash_failures += item.peer_hash_failures;
    total.peer_rejected_fragments += item.peer_rejected_fragments;
}
