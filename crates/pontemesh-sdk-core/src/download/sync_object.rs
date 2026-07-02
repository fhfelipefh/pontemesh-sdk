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
    mut progress: Option<ProgressCallback<'_>>,
) -> Result<Vec<u8>, PontemeshError> {
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

        for source in candidate_sources {
            match download_fragment(&package, source_client, peer, &source, &fragment).and_then(
                |bytes| {
                    validate_fragment(&fragment, &bytes)?;
                    Ok(bytes)
                },
            ) {
                Ok(bytes) => {
                    storage.write_validated_fragment(&manifest, &fragment, &bytes)?;
                    origin.record_event(
                        &package.id,
                        &request.bucket,
                        &request.key,
                        "FRAGMENT_VALIDATED",
                        Some(fragment.index),
                        Some(source_type_name(source.source_type)),
                    )?;
                    progress_map.mark(fragment.index, bytes.len() as u64);
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
    Ok(object)
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
