#![cfg(feature = "legacy-tcp-dev")]

use pontemesh_sdk_core::client::{OriginClient, SourceClient};
use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::download::{sync_object_with_summary, SyncObjectRequest, TransferSummary};
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::sha256_hex;
use pontemesh_sdk_core::p2p::{DisabledPeerTransport, PeerServer};
use pontemesh_sdk_core::storage::MemoryStorage;

#[derive(Clone)]
struct TestOrigin {
    package: AccessPackage,
}

impl OriginClient for TestOrigin {
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

struct OriginSource {
    object: Vec<u8>,
}

impl SourceClient for OriginSource {
    fn download_fragment(
        &self,
        _package: &AccessPackage,
        _source: &AuthorizedSource,
        fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError> {
        Ok(
            self.object[fragment.byte_range_start as usize..=fragment.byte_range_end as usize]
                .to_vec(),
        )
    }
}

#[test]
#[ignore]
fn p2p_reduces_origin_traffic_against_origin_only_baseline() {
    let object = b"traffic-reduction-object-with-several-fragments".to_vec();
    let manifest = manifest(&object, 7);
    let origin_only = run_clients(
        &object,
        &manifest,
        vec![origin_source()],
        &DisabledPeerTransport,
        5,
    );

    let seeder = PeerServer::start(Some("127.0.0.1:0"), None).expect("start seeder");
    let seeder_package = package(
        manifest.clone(),
        vec![peer_source("seeder", seeder.endpoint(), seeder.peer_id())],
    );
    for fragment in &manifest.fragments {
        let bytes = &object[fragment.byte_range_start as usize..=fragment.byte_range_end as usize];
        seeder
            .add_validated_fragment(&seeder_package, &manifest, fragment, bytes)
            .expect("seed fragment");
    }
    let p2p_enabled = run_clients(
        &object,
        &manifest,
        vec![
            peer_source("seeder", seeder.endpoint(), seeder.peer_id()),
            origin_source(),
        ],
        &pontemesh_sdk_core::p2p::PeerClient::new(),
        5,
    );

    assert!(p2p_enabled.bytes_from_peer > 0);
    assert!(p2p_enabled.fragments_from_peer > 0);
    assert!(p2p_enabled.bytes_from_origin < origin_only.bytes_from_origin);
}

fn run_clients(
    object: &[u8],
    manifest: &Manifest,
    sources: Vec<AuthorizedSource>,
    peer: &dyn pontemesh_sdk_core::p2p::PeerTransport,
    clients: usize,
) -> TransferSummary {
    let mut total = TransferSummary::default();
    for _ in 0..clients {
        let package = package(manifest.clone(), sources.clone());
        let result = sync_object_with_summary(
            &TestOrigin {
                package: package.clone(),
            },
            &OriginSource {
                object: object.to_vec(),
            },
            peer,
            &mut MemoryStorage::new(),
            &SyncObjectRequest {
                bucket: package.bucket.clone(),
                key: package.key.clone(),
                destination: "unused".into(),
            },
            None,
        )
        .expect("client sync");
        assert_eq!(sha256_hex(&result.bytes), manifest.object_sha256);
        total.bytes_from_peer += result.summary.bytes_from_peer;
        total.bytes_from_origin += result.summary.bytes_from_origin;
        total.fragments_from_peer += result.summary.fragments_from_peer;
        total.fragments_from_origin += result.summary.fragments_from_origin;
    }
    total
}

fn manifest(bytes: &[u8], fragment_size: usize) -> Manifest {
    let mut fragments = Vec::new();
    for (index, chunk) in bytes.chunks(fragment_size).enumerate() {
        let start = index * fragment_size;
        let end = start + chunk.len() - 1;
        fragments.push(FragmentDescriptor {
            index,
            fragment_id: format!("fragment-{index}"),
            byte_range_start: start as u64,
            byte_range_end: end as u64,
            size_bytes: chunk.len(),
            hash_algorithm: "SHA256".to_string(),
            sha256: sha256_hex(chunk),
            priority: "NORMAL".to_string(),
            fallback_range_header: format!("bytes={start}-{end}"),
        });
    }
    Manifest {
        manifest_id: "manifest-traffic".to_string(),
        object_id: "object-traffic".to_string(),
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        version: "v1".to_string(),
        total_size_bytes: bytes.len() as i64,
        content_type: "application/octet-stream".to_string(),
        object_hash_algorithm: "SHA256".to_string(),
        object_sha256: sha256_hex(bytes),
        fragment_size_bytes: fragment_size,
        fragments,
        availability_state: "AVAILABLE".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn package(manifest: Manifest, sources: Vec<AuthorizedSource>) -> AccessPackage {
    AccessPackage {
        id: "pkg-traffic".to_string(),
        package_token: "package-token-secret".to_string(),
        bucket: manifest.bucket.clone(),
        key: manifest.key.clone(),
        version: manifest.version.clone(),
        manifest_id: manifest.manifest_id.clone(),
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        scope: vec!["object:read".to_string()],
        authorized_sources: sources,
        source_selection: SourceSelectionContract::default(),
        fallback: FallbackContract::default(),
        manifest,
    }
}

fn peer_source(id: &str, endpoint: &str, peer_id: &str) -> AuthorizedSource {
    AuthorizedSource {
        id: id.to_string(),
        source_type: SourceType::Peer,
        endpoint: endpoint.to_string(),
        peer_id: Some(peer_id.to_string()),
        transport: Some("experimental-tcp".to_string()),
        priority: 1,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        available_fragments: (0..64).collect(),
    }
}

fn origin_source() -> AuthorizedSource {
    AuthorizedSource {
        id: "origin".to_string(),
        source_type: SourceType::Origin,
        endpoint: "http://origin.invalid/object".to_string(),
        peer_id: None,
        transport: None,
        priority: 9,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        available_fragments: (0..64).collect(),
    }
}
