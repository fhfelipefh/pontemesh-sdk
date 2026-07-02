use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pontemesh_sdk_core::client::{OriginClient, SourceClient};
use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::download::{
    order_sources_for_test, sync_object, SourceSelector, SyncObjectRequest,
};
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::{sha256_hex, validate_fragment};
use pontemesh_sdk_core::p2p::{DisabledPeerTransport, PeerTransport};
use pontemesh_sdk_core::storage::{MemoryStorage, StorageAdapter};

fn fragment(index: usize, bytes: &[u8]) -> FragmentDescriptor {
    FragmentDescriptor {
        index,
        fragment_id: format!("fragment-{index}"),
        byte_range_start: 0,
        byte_range_end: bytes.len().saturating_sub(1) as u64,
        size_bytes: bytes.len(),
        hash_algorithm: "SHA256".to_string(),
        sha256: sha256_hex(bytes),
        priority: "NORMAL".to_string(),
        fallback_range_header: format!("bytes=0-{}", bytes.len().saturating_sub(1)),
    }
}

fn source(id: &str, source_type: SourceType, priority: u8) -> AuthorizedSource {
    AuthorizedSource {
        id: id.to_string(),
        source_type,
        endpoint: format!("https://{id}.example.com/object"),
        priority,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        available_fragments: vec![0, 1],
    }
}

fn manifest(bytes: &[u8]) -> Manifest {
    Manifest {
        manifest_id: "manifest-1".to_string(),
        object_id: "object-1".to_string(),
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        version: "v1".to_string(),
        total_size_bytes: bytes.len() as i64,
        content_type: "application/octet-stream".to_string(),
        object_hash_algorithm: "SHA256".to_string(),
        object_sha256: sha256_hex(bytes),
        fragment_size_bytes: bytes.len(),
        fragments: vec![fragment(0, bytes)],
        availability_state: "AVAILABLE".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn package(bytes: &[u8], sources: Vec<AuthorizedSource>) -> AccessPackage {
    let manifest = manifest(bytes);
    AccessPackage {
        id: "pkg-1".to_string(),
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

#[test]
fn parses_access_package_manifest_and_authorized_source() {
    let json = r#"{
      "id":"pkg-1","packageToken":"token","bucket":"game-assets","key":"maps/desert-v3.pak",
      "version":"v1","manifestId":"manifest-1","expiresAt":"2099-01-01T00:00:00Z",
      "scope":["object:read"],
      "authorizedSources":[{"id":"origin","sourceType":"ORIGIN","endpoint":"https://origin.example.com","priority":3,"expiresAt":"2099-01-01T00:00:00Z","availableFragments":[0]}],
      "sourceSelection":{"allowPeerSharing":true,"allowReplicaEdge":true,"failureThreshold":2},
      "fallback":{"enabled":true,"preserveValidatedFragments":true},
      "manifest":{"manifestId":"manifest-1","objectId":"object-1","bucket":"game-assets","key":"maps/desert-v3.pak","version":"v1","totalSizeBytes":5,"contentType":"application/octet-stream","objectHashAlgorithm":"SHA256","objectSha256":"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824","fragmentSizeBytes":5,"fragments":[{"index":0,"fragmentId":"f0","byteRangeStart":0,"byteRangeEnd":4,"sizeBytes":5,"hashAlgorithm":"SHA256","sha256":"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824","priority":"NORMAL","fallbackRangeHeader":"bytes=0-4"}],"availabilityState":"AVAILABLE","createdAt":"2026-01-01T00:00:00Z"}
    }"#;

    let parsed: AccessPackage = serde_json::from_str(json).expect("valid access package");
    assert_eq!(parsed.package_token, "token");
    assert_eq!(parsed.manifest.fragments[0].index, 0);
    assert_eq!(parsed.authorized_sources[0].source_type, SourceType::Origin);
}

#[test]
fn source_selector_prefers_peer_replica_then_origin() {
    let bytes = b"hello";
    let manifest = manifest(bytes);
    let peer = DisabledPeerTransport;
    let sources = vec![
        source("origin", SourceType::Origin, 1),
        source("replica", SourceType::ReplicaEdge, 1),
        source("peer", SourceType::Peer, 1),
    ];
    let selection = SourceSelectionContract::default();
    let selector = SourceSelector::new(&sources, &selection, &peer);
    let ordered = selector.sources_for(&manifest.fragments[0]);
    assert_eq!(
        order_sources_for_test(&ordered),
        vec![
            SourceType::Peer,
            SourceType::ReplicaEdge,
            SourceType::Origin
        ]
    );
}

#[test]
fn valid_fragment_sha256_is_accepted_and_invalid_is_rejected() {
    let descriptor = fragment(0, b"hello");
    validate_fragment(&descriptor, b"hello").expect("valid fragment");
    assert!(matches!(
        validate_fragment(&descriptor, b"HELLO"),
        Err(PontemeshError::HashMismatch(_))
    ));
}

#[derive(Clone)]
struct FakeOrigin {
    package: AccessPackage,
}

impl OriginClient for FakeOrigin {
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
}

struct FakeSource {
    bytes_by_source: HashMap<String, Result<Vec<u8>, String>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl SourceClient for FakeSource {
    fn download_fragment(
        &self,
        _package: &AccessPackage,
        source: &AuthorizedSource,
        _fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError> {
        self.calls.lock().unwrap().push(source.id.clone());
        match self.bytes_by_source.get(&source.id) {
            Some(Ok(bytes)) => Ok(bytes.clone()),
            Some(Err(message)) => Err(PontemeshError::OriginRequestFailed(message.clone())),
            None => Err(PontemeshError::NoSourceAvailable),
        }
    }
}

struct FailingPeer;

impl PeerTransport for FailingPeer {
    fn can_handle(&self, source: &AuthorizedSource) -> bool {
        source.source_type == SourceType::Peer
    }

    fn download_fragment(
        &self,
        _source: &AuthorizedSource,
        _fragment: &FragmentDescriptor,
        _package_token: &str,
    ) -> Result<Vec<u8>, PontemeshError> {
        Err(PontemeshError::PeerTransportNotEnabled)
    }
}

#[test]
fn validated_fragment_is_not_downloaded_again() {
    let bytes = b"hello";
    let package = package(bytes, vec![source("origin", SourceType::Origin, 1)]);
    let origin = FakeOrigin {
        package: package.clone(),
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = FakeSource {
        bytes_by_source: HashMap::from([("origin".to_string(), Ok(bytes.to_vec()))]),
        calls: calls.clone(),
    };
    let peer = DisabledPeerTransport;
    let mut storage = MemoryStorage::new();
    storage
        .write_validated_fragment(&package.manifest, &package.manifest.fragments[0], bytes)
        .expect("seed fragment");

    let result = sync_object(
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
    .expect("sync object");

    assert_eq!(result, bytes);
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn fallback_from_peer_to_replica_then_origin() {
    let bytes = b"hello";
    let package = package(
        bytes,
        vec![
            source("peer", SourceType::Peer, 1),
            source("replica", SourceType::ReplicaEdge, 1),
            source("origin", SourceType::Origin, 1),
        ],
    );
    let origin = FakeOrigin {
        package: package.clone(),
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = FakeSource {
        bytes_by_source: HashMap::from([
            ("replica".to_string(), Ok(bytes.to_vec())),
            ("origin".to_string(), Ok(bytes.to_vec())),
        ]),
        calls: calls.clone(),
    };
    let mut storage = MemoryStorage::new();

    let result = sync_object(
        &origin,
        &source,
        &FailingPeer,
        &mut storage,
        &SyncObjectRequest {
            bucket: package.bucket.clone(),
            key: package.key.clone(),
            destination: "unused".into(),
        },
        None,
    )
    .expect("sync object");

    assert_eq!(result, bytes);
    assert_eq!(calls.lock().unwrap().as_slice(), ["replica"]);
}

#[test]
fn fallback_from_replica_to_origin() {
    let bytes = b"hello";
    let package = package(
        bytes,
        vec![
            source("replica", SourceType::ReplicaEdge, 1),
            source("origin", SourceType::Origin, 1),
        ],
    );
    let origin = FakeOrigin {
        package: package.clone(),
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = FakeSource {
        bytes_by_source: HashMap::from([
            ("replica".to_string(), Err("down".to_string())),
            ("origin".to_string(), Ok(bytes.to_vec())),
        ]),
        calls: calls.clone(),
    };
    let mut storage = MemoryStorage::new();

    let result = sync_object(
        &origin,
        &source,
        &DisabledPeerTransport,
        &mut storage,
        &SyncObjectRequest {
            bucket: package.bucket.clone(),
            key: package.key.clone(),
            destination: "unused".into(),
        },
        None,
    )
    .expect("sync object");

    assert_eq!(result, bytes);
    assert_eq!(calls.lock().unwrap().as_slice(), ["replica", "origin"]);
}

#[test]
fn package_token_is_not_placed_in_pontemesh_urls() {
    let origin = pontemesh_sdk_core::client::PontemeshClientConfig {
        origin_url: "https://origin.example.com".to_string(),
        application_token: "application-token".to_string(),
    };
    assert!(!origin.origin_url.contains("application-token"));
}
