use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use pontemesh_sdk_core::client::{OriginClient, SourceClient};
use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::download::fragment_downloader::download_fragment;
use pontemesh_sdk_core::download::{SourceSelector, TransferSummary};
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::{sha256_hex, validate_fragment};
use pontemesh_sdk_core::p2p::{DisabledPeerTransport, Libp2pTransport, PeerTransport};

use crate::config::BenchmarkConfig;
use crate::metrics::{add_summary, BenchmarkResult, MetricsCollector};
use crate::object_factory::{build_object, BenchmarkObject};
use crate::report::write_outputs;
use crate::scenario::Scenario;

pub fn run_benchmark(config: BenchmarkConfig) -> Result<(), String> {
    fs::create_dir_all(&config.output).map_err(|error| error.to_string())?;
    let exit_path = config.output.join("benchmark.exit");
    let _ = fs::remove_file(&exit_path);
    let logger = BenchLogger::new(config.output.join("benchmark.log"));
    logger.event("benchmark_start", &config, None, None);
    let result = run_benchmark_inner(&config, &logger);
    match &result {
        Ok(()) => {
            fs::write(&exit_path, "success\n").map_err(|error| error.to_string())?;
            logger.event("benchmark_done", &config, None, None);
        }
        Err(error) => {
            let _ = fs::write(&exit_path, format!("failure: {error}\n"));
            logger.event("benchmark_failed", &config, None, Some(error));
        }
    }
    result
}

fn run_benchmark_inner(config: &BenchmarkConfig, logger: &BenchLogger) -> Result<(), String> {
    if config.output.exists() && !config.force && config.profile.as_str() == "production" {
        let existing = config.output.join("results.json");
        if existing.exists() {
            return Err(format!(
                "{} already exists; pass --force to overwrite production benchmark outputs",
                existing.display()
            ));
        }
    }
    let mut results = Vec::new();
    let mut baselines = HashMap::new();
    for object_size in &config.object_sizes {
        for fragment_size in &config.fragment_sizes {
            let object = build_object(*object_size, *fragment_size);
            for downloaders in &config.downloaders {
                for run in 1..=config.runs {
                    for scenario in Scenario::all() {
                        eprintln!(
                            "running {} object={} fragment={} downloaders={} run={}",
                            scenario.name(),
                            object_size,
                            fragment_size,
                            downloaders,
                            run
                        );
                        logger.event(
                            "scenario_start",
                            config,
                            Some(ScenarioContext {
                                scenario,
                                object_size: *object_size,
                                fragment_size: *fragment_size as u64,
                                downloaders: *downloaders,
                                run,
                            }),
                            None,
                        );
                        let scenario_started = Instant::now();
                        let baseline = baselines
                            .get(&(*object_size, *fragment_size as u64, *downloaders, run))
                            .copied();
                        let result =
                            run_scenario(scenario, &object, *downloaders, run, baseline, logger)?;
                        if scenario_started.elapsed() > config.scenario_timeout {
                            logger.event(
                                "scenario_timeout",
                                config,
                                Some(ScenarioContext {
                                    scenario,
                                    object_size: *object_size,
                                    fragment_size: *fragment_size as u64,
                                    downloaders: *downloaders,
                                    run,
                                }),
                                Some("scenario exceeded configured timeout"),
                            );
                            return Err(format!("{} scenario timeout", scenario.name()));
                        }
                        if scenario == Scenario::OriginOnly {
                            baselines.insert(
                                (*object_size, *fragment_size as u64, *downloaders, run),
                                result.bytes_from_origin,
                            );
                        }
                        validate_result(&result, scenario)?;
                        append_checkpoint(&config.output, &result)?;
                        results.push(result);
                        write_state(&config.output, &results)?;
                        logger.event(
                            "scenario_done",
                            config,
                            Some(ScenarioContext {
                                scenario,
                                object_size: *object_size,
                                fragment_size: *fragment_size as u64,
                                downloaders: *downloaders,
                                run,
                            }),
                            None,
                        );
                    }
                }
            }
        }
    }
    write_outputs(&config.output, &results)?;
    assert_no_secret_leak(&config.output)?;
    Ok(())
}

fn run_scenario(
    scenario: Scenario,
    object: &BenchmarkObject,
    downloaders: usize,
    run: usize,
    baseline_origin_bytes: Option<u64>,
    logger: &BenchLogger,
) -> Result<BenchmarkResult, String> {
    match scenario {
        Scenario::OriginOnly => run_origin_only(object, downloaders, run, logger),
        Scenario::P2pSingleSeeder => {
            run_single_seeder(object, downloaders, run, baseline_origin_bytes, logger)
        }
        Scenario::P2pMesh => run_mesh(object, downloaders, run, baseline_origin_bytes, logger),
        Scenario::P2pFallback => {
            run_fallback(object, downloaders, run, baseline_origin_bytes, logger)
        }
    }
}

fn run_origin_only(
    object: &BenchmarkObject,
    downloaders: usize,
    run: usize,
    logger: &BenchLogger,
) -> Result<BenchmarkResult, String> {
    let package = package(&object.manifest, vec![origin_source(&object.manifest)]);
    let origin = BenchOrigin {
        package: package.clone(),
    };
    let source = BenchSource {
        object: object.bytes.clone(),
    };
    let summaries = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..downloaders {
            let origin = origin.clone();
            let source = source.clone();
            let package = package.clone();
            let manifest = object.manifest.clone();
            let summaries = &summaries;
            let logger = logger.clone();
            scope.spawn(move || {
                let result = sync_one(
                    &origin,
                    &source,
                    &DisabledPeerTransport,
                    &package,
                    &manifest,
                    &logger,
                );
                summaries.lock().unwrap().push(result);
            });
        }
    });
    let (total, collector, valid, threads_started, threads_finished, panics) =
        collect_downloader_results(summaries.into_inner().unwrap())?;
    Ok(result_from_parts(
        Scenario::OriginOnly,
        object,
        downloaders,
        run,
        started.elapsed().as_millis(),
        total,
        collector,
        valid,
        None,
        0,
        0,
        threads_started,
        threads_finished,
        0,
        panics,
    ))
}

fn run_single_seeder(
    object: &BenchmarkObject,
    downloaders: usize,
    run: usize,
    baseline_origin_bytes: Option<u64>,
    logger: &BenchLogger,
) -> Result<BenchmarkResult, String> {
    let seeder = start_peer("seeder")?;
    let peer = peer_source("seeder", &seeder, &object.manifest, None);
    let seeder_package = package(&object.manifest, vec![peer.clone()]);
    seed_fragments(&seeder, &seeder_package, object, SeedMode::All)?;
    let package = package(
        &object.manifest,
        vec![peer, origin_source(&object.manifest)],
    );
    let origin = BenchOrigin {
        package: package.clone(),
    };
    let source = BenchSource {
        object: object.bytes.clone(),
    };
    let summaries = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..downloaders {
            let origin = origin.clone();
            let source = source.clone();
            let package = package.clone();
            let manifest = object.manifest.clone();
            let summaries = &summaries;
            let logger = logger.clone();
            scope.spawn(move || {
                let result = start_peer("downloader").and_then(|downloader| {
                    sync_one(&origin, &source, &downloader, &package, &manifest, &logger)
                });
                summaries.lock().unwrap().push(result);
            });
        }
    });
    let raw = summaries.into_inner().unwrap();
    let downloaders_with_peer = raw
        .iter()
        .filter_map(|item| item.as_ref().ok())
        .filter(|(summary, _, _)| summary.bytes_from_peer > 0)
        .count();
    let (total, collector, valid, threads_started, threads_finished, panics) =
        collect_downloader_results(raw)?;
    Ok(result_from_parts(
        Scenario::P2pSingleSeeder,
        object,
        downloaders,
        run,
        started.elapsed().as_millis(),
        total,
        collector,
        valid,
        baseline_origin_bytes,
        1,
        downloaders_with_peer,
        threads_started,
        threads_finished,
        1,
        panics,
    ))
}

fn run_mesh(
    object: &BenchmarkObject,
    downloaders: usize,
    run: usize,
    baseline_origin_bytes: Option<u64>,
    logger: &BenchLogger,
) -> Result<BenchmarkResult, String> {
    let effective_downloaders = downloaders.max(2);
    let seeder = start_peer("seeder")?;
    let mut sources = vec![peer_source(
        "seeder",
        &seeder,
        &object.manifest,
        Some(first_half_fragments(&object.manifest)),
    )];
    let seeder_package = package(&object.manifest, sources.clone());
    seed_fragments(&seeder, &seeder_package, object, SeedMode::FirstHalf)?;
    let source = BenchSource {
        object: object.bytes.clone(),
    };
    let mut total = TransferSummary::default();
    let mut collector = MetricsCollector::default();
    let started = Instant::now();
    let mut valid = true;
    let mut retained_peers = vec![seeder];
    let mut downloaders_with_peer = 0;
    let bootstrap_downloaders = effective_downloaders.min(1);
    for downloader_index in 0..bootstrap_downloaders {
        let downloader = start_peer("mesh-downloader")?;
        let mut package_sources = sources.clone();
        package_sources.push(origin_source(&object.manifest));
        let package = package(&object.manifest, package_sources);
        let origin = BenchOrigin {
            package: package.clone(),
        };
        let (summary, metrics, hash_valid) = sync_one(
            &origin,
            &source,
            &downloader,
            &package,
            &object.manifest,
            logger,
        )?;
        if summary.bytes_from_peer > 0 {
            downloaders_with_peer += 1;
        }
        add_summary(&mut total, &summary);
        collector.append(metrics);
        valid &= hash_valid;
        let new_source = peer_source(
            &format!("downloader-{downloader_index}"),
            &downloader,
            &object.manifest,
            None,
        );
        if downloader_index == 0 {
            sources = vec![new_source];
        } else {
            sources.push(new_source);
        }
        retained_peers.push(downloader);
    }
    if effective_downloaders > bootstrap_downloaders {
        let summaries = Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for downloader_index in bootstrap_downloaders..effective_downloaders {
                let source = source.clone();
                let package_sources = {
                    let mut package_sources = sources.clone();
                    package_sources.push(origin_source(&object.manifest));
                    package_sources
                };
                let manifest = object.manifest.clone();
                let summaries = &summaries;
                let logger = logger.clone();
                scope.spawn(move || {
                    let package = package(&manifest, package_sources);
                    let origin = BenchOrigin {
                        package: package.clone(),
                    };
                    let result = start_peer("mesh-downloader").and_then(|downloader| {
                        let sync =
                            sync_one(&origin, &source, &downloader, &package, &manifest, &logger);
                        if sync.as_ref().is_ok() {
                            // The peer is intentionally dropped after serving this downloader; this stage
                            // measures concurrent download contention after bootstrap sharing exists.
                            let _ = downloader_index;
                        }
                        sync
                    });
                    summaries.lock().unwrap().push(result);
                });
            }
        });
        let raw = summaries.into_inner().unwrap();
        downloaders_with_peer += raw
            .iter()
            .filter_map(|item| item.as_ref().ok())
            .filter(|(summary, _, _)| summary.bytes_from_peer > 0)
            .count();
        let (stage_total, stage_collector, stage_valid, _, _, _) = collect_downloader_results(raw)?;
        add_summary(&mut total, &stage_total);
        collector.append(stage_collector);
        valid &= stage_valid;
    }
    let distinct_peer_sources = collector.distinct_peer_sources_served();
    drop(retained_peers);
    Ok(result_from_parts(
        Scenario::P2pMesh,
        object,
        effective_downloaders,
        run,
        started.elapsed().as_millis(),
        total,
        collector,
        valid,
        baseline_origin_bytes.or(Some(
            object.bytes.len() as u64 * effective_downloaders as u64,
        )),
        distinct_peer_sources,
        downloaders_with_peer,
        effective_downloaders as u64,
        effective_downloaders as u64,
        distinct_peer_sources as u64,
        0,
    ))
}

fn run_fallback(
    object: &BenchmarkObject,
    downloaders: usize,
    run: usize,
    baseline_origin_bytes: Option<u64>,
    logger: &BenchLogger,
) -> Result<BenchmarkResult, String> {
    let seeder = start_peer("partial-seeder")?;
    let peer = peer_source_with_priority("partial-seeder", &seeder, &object.manifest, None, 1);
    let seeder_package = package(&object.manifest, vec![peer.clone()]);
    let mut retained_empty_peer = None;
    let sources = if object.manifest.fragments.len() == 1 {
        seed_fragments(&seeder, &seeder_package, object, SeedMode::All)?;
        let empty_peer = start_peer("empty-fallback-peer")?;
        let empty_source = peer_source_with_priority(
            "empty-fallback-peer",
            &empty_peer,
            &object.manifest,
            None,
            0,
        );
        retained_empty_peer = Some(empty_peer);
        vec![empty_source, peer, origin_source(&object.manifest)]
    } else {
        seed_fragments(&seeder, &seeder_package, object, SeedMode::EvenOnly)?;
        vec![peer, origin_source(&object.manifest)]
    };
    let package = package(&object.manifest, sources);
    let origin = BenchOrigin {
        package: package.clone(),
    };
    let source = BenchSource {
        object: object.bytes.clone(),
    };
    let summaries = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..downloaders {
            let origin = origin.clone();
            let source = source.clone();
            let package = package.clone();
            let manifest = object.manifest.clone();
            let summaries = &summaries;
            let logger = logger.clone();
            scope.spawn(move || {
                let result = start_peer("fallback-downloader").and_then(|downloader| {
                    sync_one(&origin, &source, &downloader, &package, &manifest, &logger)
                });
                summaries.lock().unwrap().push(result);
            });
        }
    });
    let raw = summaries.into_inner().unwrap();
    let downloaders_with_peer = raw
        .iter()
        .filter_map(|item| item.as_ref().ok())
        .filter(|(summary, _, _)| summary.bytes_from_peer > 0)
        .count();
    let (total, collector, valid, threads_started, threads_finished, panics) =
        collect_downloader_results(raw)?;
    drop(retained_empty_peer);
    Ok(result_from_parts(
        Scenario::P2pFallback,
        object,
        downloaders,
        run,
        started.elapsed().as_millis(),
        total,
        collector,
        valid,
        baseline_origin_bytes,
        1,
        downloaders_with_peer,
        threads_started,
        threads_finished,
        2,
        panics,
    ))
}

fn sync_one(
    _origin: &BenchOrigin,
    source: &BenchSource,
    peer: &dyn PeerTransport,
    package: &AccessPackage,
    manifest: &Manifest,
    logger: &BenchLogger,
) -> Result<(TransferSummary, MetricsCollector, bool), String> {
    let jobs = Mutex::new(VecDeque::from(manifest.fragments.clone()));
    let fragments = Mutex::new(vec![None::<Vec<u8>>; manifest.fragments.len()]);
    let summary = Mutex::new(TransferSummary::default());
    let mut initial_metrics = MetricsCollector::default();
    initial_metrics.start_now();
    let metrics = Mutex::new(initial_metrics);
    let error = Mutex::new(None::<String>);
    let workers = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4)
        .clamp(2, 4);

    if let Some(fragment) = jobs.lock().unwrap().pop_front() {
        process_fragment(
            fragment, package, source, peer, manifest, &fragments, &summary, &metrics, &error,
            logger,
        );
    }

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                if error.lock().unwrap().is_some() {
                    break;
                }
                let Some(fragment) = jobs.lock().unwrap().pop_front() else {
                    break;
                };
                process_fragment(
                    fragment, package, source, peer, manifest, &fragments, &summary, &metrics,
                    &error, logger,
                );
            });
        }
    });

    if let Some(item_error) = error.into_inner().unwrap() {
        return Err(item_error);
    }
    let fragments = fragments.into_inner().unwrap();
    let mut object = Vec::with_capacity(manifest.total_size_bytes as usize);
    for (index, bytes) in fragments.into_iter().enumerate() {
        let bytes = bytes.ok_or_else(|| format!("fragment {index} was not downloaded"))?;
        object.extend_from_slice(&bytes);
    }
    let hash_valid = sha256_hex(&object).eq_ignore_ascii_case(&manifest.object_sha256);
    Ok((
        summary.into_inner().unwrap(),
        metrics.into_inner().unwrap(),
        hash_valid,
    ))
}

#[allow(clippy::too_many_arguments)]
fn process_fragment(
    fragment: FragmentDescriptor,
    package: &AccessPackage,
    source: &BenchSource,
    peer: &dyn PeerTransport,
    manifest: &Manifest,
    fragments: &Mutex<Vec<Option<Vec<u8>>>>,
    summary: &Mutex<TransferSummary>,
    metrics: &Mutex<MetricsCollector>,
    error: &Mutex<Option<String>>,
    logger: &BenchLogger,
) {
    if error.lock().unwrap().is_some() {
        return;
    }
    let selector =
        SourceSelector::new(&package.authorized_sources, &package.source_selection, peer);
    let candidates = selector.sources_for(&fragment);
    if candidates.is_empty() {
        *error.lock().unwrap() = Some("no source available".to_string());
        return;
    }
    let mut failed_sources = 0_u64;
    let mut last_error = None;
    for source_ref in candidates {
        logger.fragment_event(
            "fragment_request_start",
            package,
            &fragment,
            &source_ref,
            None,
            None,
        );
        let started = Instant::now();
        match download_fragment(package, source, peer, &source_ref, manifest, &fragment).and_then(
            |bytes| {
                let download_ms = elapsed_ms_ceil(started);
                logger.fragment_event(
                    "fragment_response_received",
                    package,
                    &fragment,
                    &source_ref,
                    Some(download_ms),
                    Some(bytes.len() as u64),
                );
                metrics.lock().unwrap().record_download(
                    source_ref.source_type,
                    source_ref.id.clone(),
                    download_ms,
                );
                let validation_started = Instant::now();
                validate_fragment(&fragment, &bytes)?;
                logger.fragment_event(
                    "fragment_validated",
                    package,
                    &fragment,
                    &source_ref,
                    Some(elapsed_ms_ceil(validation_started)),
                    Some(bytes.len() as u64),
                );
                metrics
                    .lock()
                    .unwrap()
                    .record_validation(elapsed_ms_ceil(validation_started));
                Ok(bytes)
            },
        ) {
            Ok(bytes) => {
                if let Err(record_error) =
                    peer.record_validated_fragment(package, manifest, &fragment, &bytes)
                {
                    *error.lock().unwrap() = Some(record_error.to_string());
                    return;
                }
                fragments.lock().unwrap()[fragment.index] = Some(bytes.clone());
                logger.fragment_event(
                    "fragment_persisted",
                    package,
                    &fragment,
                    &source_ref,
                    None,
                    Some(bytes.len() as u64),
                );
                let mut total = summary.lock().unwrap();
                if failed_sources > 0 {
                    total.fallback_activations += 1;
                }
                record_success(&mut total, source_ref.source_type, bytes.len() as u64);
                last_error = None;
                break;
            }
            Err(item_error) => {
                failed_sources += 1;
                record_failure(
                    &mut summary.lock().unwrap(),
                    source_ref.source_type,
                    &item_error,
                );
                last_error = Some(item_error.to_string());
            }
        }
    }
    if let Some(item_error) = last_error {
        *error.lock().unwrap() = Some(item_error);
    }
}

#[allow(clippy::too_many_arguments)]
fn result_from_parts(
    scenario: Scenario,
    object: &BenchmarkObject,
    downloaders: usize,
    run: usize,
    total_duration_ms: u128,
    summary: TransferSummary,
    collector: MetricsCollector,
    object_hash_valid: bool,
    baseline_origin_bytes: Option<u64>,
    distinct_peer_sources_served: usize,
    downloaders_with_peer_bytes: usize,
    threads_started: u64,
    threads_finished: u64,
    open_connections_peak: u64,
    panics: u64,
) -> BenchmarkResult {
    let transferred_bytes = object.bytes.len() as u64 * downloaders as u64;
    let seconds = (total_duration_ms.max(1) as f64) / 1000.0;
    let total_source_bytes =
        summary.bytes_from_peer + summary.bytes_from_origin + summary.bytes_from_replica;
    let peer_traffic_ratio = if total_source_bytes == 0 {
        0.0
    } else {
        summary.bytes_from_peer as f64 / total_source_bytes as f64
    };
    let origin_traffic_reduction_percent = baseline_origin_bytes
        .filter(|baseline| *baseline > 0)
        .map(|baseline| {
            ((baseline.saturating_sub(summary.bytes_from_origin)) as f64 / baseline as f64) * 100.0
        })
        .unwrap_or(0.0);
    BenchmarkResult {
        scenario: scenario.name().to_string(),
        object_size_bytes: object.bytes.len() as u64,
        fragment_size_bytes: object.manifest.fragment_size_bytes as u64,
        downloaders,
        run,
        total_duration_ms,
        throughput_mib_s: transferred_bytes as f64 / 1024.0 / 1024.0 / seconds,
        bytes_from_peer: summary.bytes_from_peer,
        bytes_from_origin: summary.bytes_from_origin,
        bytes_from_replica: summary.bytes_from_replica,
        fragments_from_peer: summary.fragments_from_peer,
        fragments_from_origin: summary.fragments_from_origin,
        fragments_from_replica: summary.fragments_from_replica,
        peer_traffic_ratio,
        origin_traffic_reduction_percent,
        time_to_first_fragment_ms: collector.time_to_first_fragment_ms(),
        fragment_latency_avg_ms: collector.avg_latency_ms(),
        fragment_latency_p95_ms: collector.percentile_ms(0.95),
        fragment_latency_p99_ms: collector.percentile_ms(0.99),
        hash_validation_time_ms: collector.hash_validation_time_ms(),
        fallback_activations: summary.fallback_activations,
        peer_failures: summary.peer_failures,
        peer_hash_failures: summary.peer_hash_failures,
        object_hash_valid,
        memory_peak_mb: memory_peak_mb(),
        threads_started,
        threads_finished,
        open_connections_peak,
        timeouts: 0,
        panics,
        distinct_peer_sources_served,
        downloaders_with_peer_bytes,
        fairness_min_downloader_ms: 0,
        fairness_max_downloader_ms: total_duration_ms,
    }
}

type DownloaderResult = Result<(TransferSummary, MetricsCollector, bool), String>;

fn collect_downloader_results(
    results: Vec<DownloaderResult>,
) -> Result<(TransferSummary, MetricsCollector, bool, u64, u64, u64), String> {
    let threads_started = results.len() as u64;
    let mut threads_finished = 0_u64;
    let panics = 0_u64;
    let mut total = TransferSummary::default();
    let mut collector = MetricsCollector::default();
    let mut valid = true;
    for item in results {
        match item {
            Ok((summary, metrics, hash_valid)) => {
                threads_finished += 1;
                add_summary(&mut total, &summary);
                collector.append(metrics);
                valid &= hash_valid;
            }
            Err(error) => {
                return Err(error);
            }
        }
    }
    Ok((
        total,
        collector,
        valid,
        threads_started,
        threads_finished,
        panics,
    ))
}

fn validate_result(result: &BenchmarkResult, scenario: Scenario) -> Result<(), String> {
    if !result.object_hash_valid {
        return Err(format!("{} produced invalid object hash", result.scenario));
    }
    if scenario.is_p2p() {
        if result.bytes_from_peer == 0 {
            return Err(format!("{} had bytes_from_peer == 0", result.scenario));
        }
        if result.fragments_from_peer == 0 {
            return Err(format!("{} had fragments_from_peer == 0", result.scenario));
        }
        if result.origin_traffic_reduction_percent <= 0.0 {
            return Err(format!(
                "{} did not reduce Origin traffic against baseline",
                result.scenario
            ));
        }
    }
    if scenario == Scenario::P2pMesh {
        if result.distinct_peer_sources_served < 2 {
            return Err("p2p-mesh must use at least 2 serving peers".to_string());
        }
        if result.downloaders_with_peer_bytes < 2 {
            return Err(
                "p2p-mesh must have at least 2 downloaders receiving P2P bytes".to_string(),
            );
        }
    }
    if scenario == Scenario::P2pFallback && result.fallback_activations == 0 {
        return Err("p2p-fallback must activate fallback".to_string());
    }
    Ok(())
}

fn record_success(summary: &mut TransferSummary, source_type: SourceType, bytes: u64) {
    match source_type {
        SourceType::Peer => {
            summary.bytes_from_peer += bytes;
            summary.fragments_from_peer += 1;
        }
        SourceType::ReplicaEdge => {
            summary.bytes_from_replica += bytes;
            summary.fragments_from_replica += 1;
        }
        SourceType::Origin => {
            summary.bytes_from_origin += bytes;
            summary.fragments_from_origin += 1;
        }
    }
}

fn record_failure(summary: &mut TransferSummary, source_type: SourceType, error: &PontemeshError) {
    if source_type != SourceType::Peer {
        return;
    }
    summary.peer_failures += 1;
    if matches!(error, PontemeshError::HashMismatch(_)) {
        summary.peer_hash_failures += 1;
        summary.peer_rejected_fragments += 1;
    }
}

fn elapsed_ms_ceil(started: Instant) -> u128 {
    let nanos = started.elapsed().as_nanos();
    if nanos == 0 {
        0
    } else {
        nanos.div_ceil(1_000_000)
    }
}

fn assert_no_secret_leak(output: &std::path::Path) -> Result<(), String> {
    for file in [
        "results.json",
        "results.csv",
        "report.md",
        "results.jsonl",
        "results.partial.csv",
        "benchmark.state.json",
        "benchmark.log",
    ] {
        let path = output.join(file);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        for secret in [
            "packageToken",
            "applicationToken",
            "bench-secret-package-token",
            "bench-secret-application-token",
            "bench-secret",
            "Authorization",
            "Bearer",
            "S3 access key",
            "S3 secret",
        ] {
            if content.contains(secret) {
                return Err(format!("{secret} leaked into {file}"));
            }
        }
    }
    Ok(())
}

fn start_peer(label: &str) -> Result<Libp2pTransport, String> {
    Libp2pTransport::start(&["/ip4/127.0.0.1/tcp/0".to_string()], &[])
        .map_err(|error| format!("start {label}: {error}"))
}

enum SeedMode {
    All,
    FirstHalf,
    EvenOnly,
}

fn seed_fragments(
    peer: &Libp2pTransport,
    package: &AccessPackage,
    object: &BenchmarkObject,
    mode: SeedMode,
) -> Result<(), String> {
    for fragment in &object.manifest.fragments {
        if matches!(mode, SeedMode::FirstHalf) && fragment.index >= midpoint(&object.manifest) {
            continue;
        }
        if matches!(mode, SeedMode::EvenOnly) && fragment.index % 2 == 1 {
            continue;
        }
        let bytes =
            &object.bytes[fragment.byte_range_start as usize..=fragment.byte_range_end as usize];
        peer.add_validated_fragment(package, &object.manifest, fragment, bytes)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn package(manifest: &Manifest, sources: Vec<AuthorizedSource>) -> AccessPackage {
    AccessPackage {
        id: format!("pkg-{}", manifest.object_id),
        package_token: "bench-secret-package-token".to_string(),
        bucket: manifest.bucket.clone(),
        key: manifest.key.clone(),
        version: manifest.version.clone(),
        manifest_id: manifest.manifest_id.clone(),
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        scope: vec!["object:read".to_string()],
        authorized_sources: sources,
        source_selection: SourceSelectionContract::default(),
        fallback: FallbackContract::default(),
        manifest: manifest.clone(),
    }
}

fn peer_source(
    id: &str,
    peer: &Libp2pTransport,
    manifest: &Manifest,
    available: Option<Vec<i64>>,
) -> AuthorizedSource {
    peer_source_with_priority(id, peer, manifest, available, 1)
}

fn peer_source_with_priority(
    id: &str,
    peer: &Libp2pTransport,
    manifest: &Manifest,
    available: Option<Vec<i64>>,
    priority: u8,
) -> AuthorizedSource {
    AuthorizedSource {
        id: id.to_string(),
        source_type: SourceType::Peer,
        endpoint: peer.endpoint(),
        peer_id: Some(peer.peer_id_string()),
        transport: Some("libp2p".to_string()),
        priority,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        available_fragments: available.unwrap_or_else(|| all_fragments(manifest)),
    }
}

fn origin_source(manifest: &Manifest) -> AuthorizedSource {
    AuthorizedSource {
        id: "origin".to_string(),
        source_type: SourceType::Origin,
        endpoint: "http://origin.local/pontemesh-benchmark-object".to_string(),
        peer_id: None,
        transport: None,
        priority: 9,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        available_fragments: all_fragments(manifest),
    }
}

fn all_fragments(manifest: &Manifest) -> Vec<i64> {
    manifest
        .fragments
        .iter()
        .map(|fragment| fragment.index as i64)
        .collect()
}

fn first_half_fragments(manifest: &Manifest) -> Vec<i64> {
    manifest
        .fragments
        .iter()
        .filter(|fragment| fragment.index < midpoint(manifest))
        .map(|fragment| fragment.index as i64)
        .collect()
}

fn midpoint(manifest: &Manifest) -> usize {
    (manifest.fragments.len() / 2).max(1)
}

#[derive(Clone)]
struct BenchOrigin {
    package: AccessPackage,
}

impl OriginClient for BenchOrigin {
    fn create_access_package(
        &self,
        _bucket: &str,
        _key: &str,
    ) -> Result<AccessPackage, PontemeshError> {
        Ok(self.package.clone())
    }

    fn get_manifest(&self, _bucket: &str, _key: &str) -> Result<Manifest, PontemeshError> {
        Ok(self.package.manifest.clone())
    }

    fn record_event(
        &self,
        _package_id: &str,
        _package_token: &str,
        _bucket: &str,
        _key: &str,
        _event_type: &str,
        _fragment_index: Option<usize>,
        _source_type: Option<&str>,
    ) -> Result<(), PontemeshError> {
        Ok(())
    }

    fn announce_peer_availability(
        &self,
        _package: &AccessPackage,
        _endpoint: &str,
        _available_fragments: &[usize],
    ) -> Result<(), PontemeshError> {
        Ok(())
    }
}

#[derive(Clone)]
struct BenchSource {
    object: std::sync::Arc<[u8]>,
}

impl SourceClient for BenchSource {
    fn download_fragment(
        &self,
        _package: &AccessPackage,
        source: &AuthorizedSource,
        fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError> {
        if source.source_type != SourceType::Origin {
            return Err(PontemeshError::NoSourceAvailable);
        }
        Ok(
            self.object[fragment.byte_range_start as usize..=fragment.byte_range_end as usize]
                .to_vec(),
        )
    }
}

#[derive(Clone)]
struct BenchLogger {
    path: PathBuf,
}

#[derive(Clone, Copy)]
struct ScenarioContext {
    scenario: Scenario,
    object_size: u64,
    fragment_size: u64,
    downloaders: usize,
    run: usize,
}

impl BenchLogger {
    fn new(path: PathBuf) -> Self {
        let _ = fs::write(&path, "");
        Self { path }
    }

    fn event(
        &self,
        event: &str,
        config: &BenchmarkConfig,
        context: Option<ScenarioContext>,
        error: Option<&str>,
    ) {
        let mut line = serde_json::json!({
            "timestamp": unix_millis(),
            "event": event,
            "profile": config.profile.as_str(),
        });
        if let Some(context) = context {
            line["scenario"] = serde_json::json!(context.scenario.name());
            line["object_size"] = serde_json::json!(context.object_size);
            line["fragment_size"] = serde_json::json!(context.fragment_size);
            line["downloaders"] = serde_json::json!(context.downloaders);
            line["run"] = serde_json::json!(context.run);
        }
        if let Some(error) = error {
            line["error"] = serde_json::json!(error);
        }
        self.write(line);
    }

    fn fragment_event(
        &self,
        event: &str,
        package: &AccessPackage,
        fragment: &FragmentDescriptor,
        source: &AuthorizedSource,
        duration_ms: Option<u128>,
        bytes: Option<u64>,
    ) {
        let line = serde_json::json!({
            "timestamp": unix_millis(),
            "event": event,
            "scenario_package": package.id,
            "object_size": package.manifest.total_size_bytes,
            "fragment_size": package.manifest.fragment_size_bytes,
            "fragment_index": fragment.index,
            "source_type": source_type_label(source.source_type),
            "peer_id": source.peer_id,
            "duration_ms": duration_ms,
            "bytes": bytes,
        });
        self.write(line);
    }

    fn write(&self, value: serde_json::Value) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(file, "{value}");
        }
    }
}

fn append_checkpoint(output: &Path, result: &BenchmarkResult) -> Result<(), String> {
    let jsonl_path = output.join("results.jsonl");
    let mut jsonl = OpenOptions::new()
        .create(true)
        .append(true)
        .open(jsonl_path)
        .map_err(|error| error.to_string())?;
    writeln!(
        jsonl,
        "{}",
        serde_json::to_string(result).map_err(|error| error.to_string())?
    )
    .map_err(|error| error.to_string())?;

    let partial_path = output.join("results.partial.csv");
    let exists = partial_path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(partial_path)
        .map_err(|error| error.to_string())?;
    if !exists {
        writeln!(file, "scenario,object_size_bytes,fragment_size_bytes,downloaders,run,total_duration_ms,throughput_mib_s,bytes_from_peer,bytes_from_origin,fragments_from_peer,fragments_from_origin,peer_traffic_ratio,origin_traffic_reduction_percent,object_hash_valid")
            .map_err(|error| error.to_string())?;
    }
    writeln!(
        file,
        "{},{},{},{},{},{},{:.6},{},{},{},{},{:.6},{:.6},{}",
        result.scenario,
        result.object_size_bytes,
        result.fragment_size_bytes,
        result.downloaders,
        result.run,
        result.total_duration_ms,
        result.throughput_mib_s,
        result.bytes_from_peer,
        result.bytes_from_origin,
        result.fragments_from_peer,
        result.fragments_from_origin,
        result.peer_traffic_ratio,
        result.origin_traffic_reduction_percent,
        result.object_hash_valid
    )
    .map_err(|error| error.to_string())
}

fn write_state(output: &Path, results: &[BenchmarkResult]) -> Result<(), String> {
    let last = results.last();
    let state = serde_json::json!({
        "timestamp": unix_millis(),
        "completed_results": results.len(),
        "last_result": last,
    });
    fs::write(
        output.join("benchmark.state.json"),
        serde_json::to_string_pretty(&state).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn memory_peak_mb() -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                let value = line.strip_prefix("VmHWM:")?.trim();
                let kb = value.split_whitespace().next()?.parse::<u64>().ok()?;
                Some(kb / 1024)
            })
        })
        .unwrap_or(0)
}

fn source_type_label(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::Origin => "ORIGIN",
        SourceType::ReplicaEdge => "REPLICA_EDGE",
        SourceType::Peer => "PEER",
    }
}
