use std::path::PathBuf;

use crate::client::{OriginClient, SourceClient};
use crate::contracts::{AuthorizedSource, SourceType};
use crate::errors::PontemeshError;
use crate::integrity::{sha256_hex, validate_fragment};
use crate::p2p::PeerTransport;
use crate::storage::{FragmentState, StorageAdapter};

use super::fragment_downloader::download_fragment;
use super::{ProgressMap, SourceSelector};

pub type ProgressCallback<'a> = &'a mut dyn FnMut(u32, u64, u64, &str);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferSummary {
    pub bytes_from_peer: u64,
    pub bytes_from_replica: u64,
    pub bytes_from_origin: u64,
    pub fragments_from_peer: u64,
    pub fragments_from_replica: u64,
    pub fragments_from_origin: u64,
    pub peer_failures: u64,
    pub peer_hash_failures: u64,
    pub peer_rejected_fragments: u64,
    pub fallback_activations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncObjectResult {
    pub bytes: Vec<u8>,
    pub summary: TransferSummary,
}

#[derive(Debug, Clone)]
pub struct SyncObjectRequest {
    pub bucket: String,
    pub key: String,
    pub destination: PathBuf,
}

pub fn sync_object(
    origin: &dyn OriginClient,
    source_client: &dyn SourceClient,
    peer: &dyn PeerTransport,
    storage: &mut dyn StorageAdapter,
    request: &SyncObjectRequest,
    progress: Option<ProgressCallback<'_>>,
) -> Result<Vec<u8>, PontemeshError> {
    sync_object_with_summary(origin, source_client, peer, storage, request, progress)
        .map(|result| result.bytes)
}

pub fn sync_object_with_summary(
    origin: &dyn OriginClient,
    source_client: &dyn SourceClient,
    peer: &dyn PeerTransport,
    storage: &mut dyn StorageAdapter,
    request: &SyncObjectRequest,
    mut progress: Option<ProgressCallback<'_>>,
) -> Result<SyncObjectResult, PontemeshError> {
    if request.bucket.trim().is_empty() || request.key.trim().is_empty() {
        return Err(PontemeshError::InvalidArgument(
            "bucket and key are required".to_string(),
        ));
    }

    let package = origin.create_access_package(&request.bucket, &request.key)?;
    origin.record_event(
        &package.id,
        &request.bucket,
        &request.key,
        "ACCESS_PACKAGE_CREATED",
        None,
        None,
    )?;
    let manifest = origin
        .get_manifest(&request.bucket, &request.key)
        .unwrap_or_else(|_| package.manifest.clone());
    let selector =
        SourceSelector::new(&package.authorized_sources, &package.source_selection, peer);
    let mut progress_map = ProgressMap::default();
    let mut summary = TransferSummary::default();

    let mut fragments = manifest.fragments.clone();
    fragments.sort_by_key(|fragment| fragment.index);
    for fragment in fragments {
        if storage.fragment_state(&manifest, &fragment) == FragmentState::Validated {
            progress_map.mark(fragment.index, fragment.size_bytes as u64);
            continue;
        }

        let mut last_error = None;
        let candidate_sources = selector.sources_for(&fragment);
        if candidate_sources.is_empty() {
            return Err(PontemeshError::NoSourceAvailable);
        }

        let mut failed_sources = 0_u64;
        for source in candidate_sources {
            match download_fragment(&package, source_client, peer, &source, &manifest, &fragment)
                .and_then(|bytes| {
                    validate_fragment(&fragment, &bytes)?;
                    Ok(bytes)
                }) {
                Ok(bytes) => {
                    if failed_sources > 0 {
                        summary.fallback_activations += 1;
                    }
                    storage.write_validated_fragment(&manifest, &fragment, &bytes)?;
                    if let Some(available_fragments) =
                        peer.record_validated_fragment(&package, &manifest, &fragment, &bytes)?
                    {
                        if let Some(endpoint) = peer.local_endpoint() {
                            origin.announce_peer_availability(
                                &package,
                                &endpoint,
                                &available_fragments,
                            )?;
                        }
                    }
                    origin.record_event(
                        &package.id,
                        &request.bucket,
                        &request.key,
                        "FRAGMENT_VALIDATED",
                        Some(fragment.index),
                        Some(source_type_name(source.source_type)),
                    )?;
                    progress_map.mark(fragment.index, bytes.len() as u64);
                    record_summary_success(&mut summary, source.source_type, bytes.len() as u64);
                    if let Some(callback) = progress.as_deref_mut() {
                        callback(
                            fragment.index as u32,
                            progress_map.bytes_downloaded(fragment.index),
                            fragment.size_bytes as u64,
                            source_type_name(source.source_type),
                        );
                    }
                    last_error = None;
                    break;
                }
                Err(error) => {
                    failed_sources += 1;
                    record_summary_failure(&mut summary, source.source_type, &error);
                    let _ = origin.record_event(
                        &package.id,
                        &request.bucket,
                        &request.key,
                        "SOURCE_FAILED",
                        Some(fragment.index),
                        Some(source_type_name(source.source_type)),
                    );
                    last_error = Some(error);
                }
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }
    }

    let object = storage.assemble(&manifest)?;
    let digest = sha256_hex(&object);
    if !digest.eq_ignore_ascii_case(&manifest.object_sha256) {
        return Err(PontemeshError::HashMismatch(
            "object sha256 mismatch".to_string(),
        ));
    }
    origin.record_event(
        &package.id,
        &request.bucket,
        &request.key,
        "OBJECT_SYNCED",
        None,
        None,
    )?;
    Ok(SyncObjectResult {
        bytes: object,
        summary,
    })
}

pub fn source_type_name(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::Origin => "ORIGIN",
        SourceType::ReplicaEdge => "REPLICA_EDGE",
        SourceType::Peer => "PEER",
    }
}

pub fn order_sources_for_test(sources: &[AuthorizedSource]) -> Vec<SourceType> {
    sources.iter().map(|source| source.source_type).collect()
}

fn record_summary_success(summary: &mut TransferSummary, source_type: SourceType, bytes: u64) {
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

fn record_summary_failure(
    summary: &mut TransferSummary,
    source_type: SourceType,
    error: &PontemeshError,
) {
    if source_type != SourceType::Peer {
        return;
    }
    summary.peer_failures += 1;
    if matches!(error, PontemeshError::HashMismatch(_)) {
        summary.peer_hash_failures += 1;
        summary.peer_rejected_fragments += 1;
    }
}
