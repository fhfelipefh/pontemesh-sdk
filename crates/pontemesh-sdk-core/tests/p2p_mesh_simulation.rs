#![cfg(feature = "legacy-tcp-dev")]

use std::sync::{Arc, Mutex};

use pontemesh_sdk_core::client::{OriginClient, SourceClient};
use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::download::{sync_object_with_summary, SyncObjectRequest, TransferSummary};
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::sha256_hex;
use pontemesh_sdk_core::p2p::{PeerClient, PeerServer, PeerTransport};
use pontemesh_sdk_core::storage::MemoryStorage;

type Announcements = Arc<Mutex<Vec<(String, Vec<usize>)>>>;

#[derive(Clone)]
struct MeshOrigin {
    package: AccessPackage,
    announcements: Announcements,
}

impl OriginClient for MeshOrigin {
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

    fn announce_peer_availability(
        &self,
        _package: &AccessPackage,
        endpoint: &str,
        available_fragments: &[usize],
    ) -> Result<(), PontemeshError> {
        self.announcements
            .lock()
            .unwrap()
            .push((endpoint.to_string(), available_fragments.to_vec()));
        Ok(())
    }
}

struct MeshSource {
    object: Vec<u8>,
}

impl SourceClient for MeshSource {
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

#[test]
#[ignore]
fn multi_sdk_mesh_downloads_from_peers_and_completes_objects() {
    let object = b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_vec();
    let manifest = manifest(&object, 8);
    let seeder_server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start seeder");
    let seeder_source = peer_source("seeder", seeder_server.endpoint(), seeder_server.peer_id());
    let seeder_package = package(&object, manifest.clone(), vec![seeder_source.clone()]);
    for fragment in &manifest.fragments {
        let bytes = &object[fragment.byte_range_start as usize..=fragment.byte_range_end as usize];
        seeder_server
            .add_validated_fragment(&seeder_package, &manifest, fragment, bytes)
            .expect("seed fragment");
    }

    let mut peer_sources = vec![seeder_source];
    let mut summaries = Vec::new();
    let mut completed = 0;
    let mut downloader_announcements = 0;
    for downloader in 0..5 {
        let announcements = Arc::new(Mutex::new(Vec::new()));
        let mut sources = peer_sources.clone();
        sources.push(origin_source());
        let package = package(&object, manifest.clone(), sources);
        let origin = MeshOrigin {
            package: package.clone(),
            announcements: announcements.clone(),
        };
        let source = MeshSource {
            object: object.clone(),
        };
        let peer = PeerClient::start(Some("127.0.0.1:0"), None).expect("start downloader peer");
        let mut storage = MemoryStorage::new();
        let result = sync_object_with_summary(
            &origin,
            &source,
            &peer,
            &mut storage,
            &SyncObjectRequest {
                bucket: package.bucket.clone(),
                key: package.key.clone(),
                destination: "unused".into(),
            },
            None,
        )
        .expect("sync downloader");
        assert_eq!(sha256_hex(&result.bytes), manifest.object_sha256);
        if result.summary.fragments_from_peer > 0 {
            completed += 1;
        }
        downloader_announcements += announcements.lock().unwrap().len();
        if let Some(endpoint) = peer.local_endpoint() {
            let peer_id = endpoint.rsplit_once("/p2p/").unwrap().1;
            peer_sources.push(peer_source(
                &format!("downloader-{downloader}"),
                &endpoint,
                peer_id,
            ));
        }
        summaries.push(result.summary);
    }

    let total = total_summary(&summaries);
    assert!(completed >= 3, "at least 3 SDKs must complete via peer");
    assert!(
        downloader_announcements > 0,
        "downloaders must become shareable"
    );
    assert!(total.bytes_from_peer > 0);
    assert!(total.fragments_from_peer > 0);
    assert_eq!(total.bytes_from_origin, 0);
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
        manifest_id: "manifest-mesh".to_string(),
        object_id: "object-mesh".to_string(),
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

fn package(_bytes: &[u8], manifest: Manifest, sources: Vec<AuthorizedSource>) -> AccessPackage {
    AccessPackage {
        id: "pkg-mesh".to_string(),
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

fn total_summary(summaries: &[TransferSummary]) -> TransferSummary {
    summaries
        .iter()
        .cloned()
        .fold(TransferSummary::default(), |mut total, item| {
            total.bytes_from_peer += item.bytes_from_peer;
            total.bytes_from_origin += item.bytes_from_origin;
            total.bytes_from_replica += item.bytes_from_replica;
            total.fragments_from_peer += item.fragments_from_peer;
            total.fragments_from_origin += item.fragments_from_origin;
            total.fragments_from_replica += item.fragments_from_replica;
            total.fallback_activations += item.fallback_activations;
            total
        })
}
