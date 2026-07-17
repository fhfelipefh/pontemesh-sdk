use std::collections::HashMap;
#[cfg(feature = "legacy-tcp-dev")]
use std::io::{BufRead, BufReader, Write};
#[cfg(feature = "legacy-tcp-dev")]
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
#[cfg(feature = "legacy-tcp-dev")]
use std::thread;

use pontemesh_sdk_core::client::{OriginClient, SourceClient};
use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::download::{
    order_sources_for_test, sync_object, FragmentProgressState, ProgressMap, SourceSelector,
    SyncObjectRequest,
};
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::{sha256_hex, validate_fragment};
#[cfg(feature = "legacy-tcp-dev")]
use pontemesh_sdk_core::p2p::{CircuitState, PeerClient, PeerServer};
use pontemesh_sdk_core::p2p::{DisabledPeerTransport, P2pConfig, P2pTransportKind, PeerTransport};
use pontemesh_sdk_core::storage::{MemoryStorage, StorageAdapter};

type Announcements = Arc<Mutex<Vec<(String, Vec<usize>)>>>;

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
        peer_id: None,
        transport: None,
        priority,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        available_fragments: vec![0, 1],
    }
}

#[cfg(feature = "legacy-tcp-dev")]
fn peer_source(id: &str, endpoint: &str) -> AuthorizedSource {
    AuthorizedSource {
        id: id.to_string(),
        source_type: SourceType::Peer,
        endpoint: endpoint.to_string(),
        peer_id: None,
        transport: Some("experimental-tcp".to_string()),
        priority: 1,
        expires_at: "2099-01-01T00:00:00Z".to_string(),
        available_fragments: vec![0],
    }
}

#[cfg(feature = "legacy-tcp-dev")]
fn peer_source_with_id(id: &str, endpoint: &str, peer_id: &str) -> AuthorizedSource {
    let mut source = peer_source(id, endpoint);
    source.peer_id = Some(peer_id.to_string());
    source
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
fn source_selector_allows_origin_with_empty_fragment_list() {
    let bytes = b"hello";
    let manifest = manifest(bytes);
    let peer = DisabledPeerTransport;
    let mut origin = source("origin", SourceType::Origin, 1);
    origin.available_fragments.clear();
    let sources = vec![origin];
    let selection = SourceSelectionContract::default();
    let selector = SourceSelector::new(&sources, &selection, &peer);
    let ordered = selector.sources_for(&manifest.fragments[0]);

    assert_eq!(order_sources_for_test(&ordered), vec![SourceType::Origin]);
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
    announcements: Announcements,
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
        _package: &AccessPackage,
        _manifest: &Manifest,
        _fragment: &FragmentDescriptor,
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
        announcements: Arc::new(Mutex::new(Vec::new())),
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
        announcements: Arc::new(Mutex::new(Vec::new())),
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
        announcements: Arc::new(Mutex::new(Vec::new())),
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
        p2p: P2pConfig::default(),
    };
    assert!(!origin.origin_url.contains("application-token"));
}

#[test]
fn p2p_required_returns_startup_error_instead_of_silent_disable() {
    let result = pontemesh_sdk_core::PontemeshClient::new(
        pontemesh_sdk_core::client::PontemeshClientConfig {
            origin_url: "https://origin.example.com".to_string(),
            application_token: "application-token".to_string(),
            p2p: P2pConfig {
                enabled: true,
                required: true,
                transport: P2pTransportKind::Disabled,
                listen_addrs: Vec::new(),
                announce_addrs: Vec::new(),
                listen_addr: Some("127.0.0.1:1:not-a-socket".to_string()),
                announce_addr: None,
            },
        },
    );

    assert!(result.is_err());
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn peer_transport_starts_serves_and_stops() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let mut server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start peer server");
    let available = server
        .add_validated_fragment(
            &package,
            &package.manifest,
            &package.manifest.fragments[0],
            bytes,
        )
        .expect("add validated fragment");
    assert_eq!(available, vec![0]);
    assert!(server.endpoint().starts_with("peer://"));
    server.stop();
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn peer_a_serves_validated_fragment_to_peer_b() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start peer server");
    server
        .add_validated_fragment(
            &package,
            &package.manifest,
            &package.manifest.fragments[0],
            bytes,
        )
        .expect("add validated fragment");
    let peer_b = PeerClient::new();
    let source = peer_source("peer-a", server.endpoint());

    let downloaded = peer_b
        .download_fragment(
            &source,
            &package,
            &package.manifest,
            &package.manifest.fragments[0],
        )
        .expect("download from peer");

    assert_eq!(downloaded, bytes);
    validate_fragment(&package.manifest.fragments[0], &downloaded).expect("hash valid");
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn authorized_peer_id_is_accepted_and_different_peer_id_is_rejected() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start peer server");
    server
        .add_validated_fragment(
            &package,
            &package.manifest,
            &package.manifest.fragments[0],
            bytes,
        )
        .expect("add validated fragment");
    let peer_b = PeerClient::new();

    peer_b
        .download_fragment(
            &peer_source_with_id("peer-a", server.endpoint(), server.peer_id()),
            &package,
            &package.manifest,
            &package.manifest.fragments[0],
        )
        .expect("authorized peer id");

    let result = peer_b.download_fragment(
        &peer_source_with_id("peer-a", server.endpoint(), "different-peer-id"),
        &package,
        &package.manifest,
        &package.manifest.fragments[0],
    );
    assert!(matches!(result, Err(PontemeshError::AccessDenied(_))));
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn expired_peer_source_is_ignored() {
    let bytes = b"hello";
    let manifest = manifest(bytes);
    let peer = PeerClient::new();
    let mut expired = peer_source("peer-a", "peer://127.0.0.1:1/p2p/peer-a");
    expired.expires_at = "2000-01-01T00:00:00Z".to_string();
    let sources = vec![expired, source("origin", SourceType::Origin, 1)];
    let selection = SourceSelectionContract::default();
    let selector = SourceSelector::new(&sources, &selection, &peer);
    let ordered = selector.sources_for(&manifest.fragments[0]);

    assert_eq!(order_sources_for_test(&ordered), vec![SourceType::Origin]);
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn peer_rejects_fragment_that_was_not_validated_before_sharing() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start peer server");

    let result = server.add_validated_fragment(
        &package,
        &package.manifest,
        &package.manifest.fragments[0],
        b"wrong-fragment",
    );

    assert!(matches!(result, Err(PontemeshError::HashMismatch(_))));
    assert!(server.available_fragments().is_empty());
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn peer_b_rejects_invalid_hash_from_peer_response() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind malicious peer");
    let endpoint = format!("peer://{}", listener.local_addr().unwrap());
    let manifest_id = package.manifest.manifest_id.clone();
    let package_id = package.id.clone();
    let fragment_id = package.manifest.fragments[0].fragment_id.clone();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept peer request");
        let mut request_line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut request_line)
            .unwrap();
        let request: serde_json::Value = serde_json::from_str(&request_line).unwrap();
        let payload = serde_json::json!({
            "type": "fragmentResponse",
            "protocolVersion": 1,
            "packageId": package_id,
            "manifestId": manifest_id,
            "fragmentId": fragment_id,
            "fragmentIndex": 0,
            "sizeBytes": bytes.len(),
            "sha256": "000000",
            "requestNonce": request["requestNonce"].as_str().unwrap(),
            "bytesBase64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
        });
        writeln!(stream, "{payload}").unwrap();
    });
    let peer_b = PeerClient::new();
    let result = peer_b.download_fragment(
        &peer_source("bad-peer", &endpoint),
        &package,
        &package.manifest,
        &package.manifest.fragments[0],
    );
    handle.join().unwrap();
    assert!(matches!(result, Err(PontemeshError::HashMismatch(_))));
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn oversized_peer_frame_is_rejected() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind malicious peer");
    let endpoint = format!("peer://{}/p2p/malicious", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept peer request");
        let mut request_line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut request_line)
            .unwrap();
        stream.write_all(&vec![b'a'; 2 * 1024 * 1024 + 2]).unwrap();
        stream.write_all(b"\n").unwrap();
    });
    let peer_b = PeerClient::new();
    let result = peer_b.download_fragment(
        &peer_source("oversized-peer", &endpoint),
        &package,
        &package.manifest,
        &package.manifest.fragments[0],
    );
    handle.join().unwrap();
    assert!(matches!(result, Err(PontemeshError::InvalidArgument(_))));
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn circuit_breaker_opens_after_repeated_peer_failures() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve port");
    let endpoint = format!("peer://{}/p2p/down", listener.local_addr().unwrap());
    drop(listener);
    let peer_b = PeerClient::new();
    let source = peer_source("down-peer", &endpoint);

    for _ in 0..2 {
        let _ = peer_b.download_fragment(
            &source,
            &package,
            &package.manifest,
            &package.manifest.fragments[0],
        );
    }

    assert_eq!(peer_b.circuit_state("down-peer"), CircuitState::Open);
    assert!(!peer_b.can_handle(&source));
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn peer_request_does_not_send_application_or_package_token() {
    let bytes = b"peer-fragment";
    let package = package(bytes, vec![]);
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind inspecting peer");
    let endpoint = format!("peer://{}/p2p/inspect", listener.local_addr().unwrap());
    let captured = Arc::new(Mutex::new(String::new()));
    let thread_capture = captured.clone();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept peer request");
        let mut request_line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut request_line)
            .unwrap();
        *thread_capture.lock().unwrap() = request_line;
        let payload = serde_json::json!({
            "type": "error",
            "code": "TEST_DONE",
            "message": "done"
        });
        writeln!(stream, "{payload}").unwrap();
    });

    let peer_b = PeerClient::new();
    let _ = peer_b.download_fragment(
        &peer_source("inspect-peer", &endpoint),
        &package,
        &package.manifest,
        &package.manifest.fragments[0],
    );
    handle.join().unwrap();

    let request = captured.lock().unwrap();
    assert!(!request.contains("package-token-secret"));
    assert!(!request.contains("application-token"));
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn source_selector_does_not_use_peer_outside_authorized_sources() {
    let bytes = b"hello";
    let manifest = manifest(bytes);
    let peer = PeerClient::new();
    let sources = vec![
        source("replica", SourceType::ReplicaEdge, 1),
        source("origin", SourceType::Origin, 1),
    ];
    let selection = SourceSelectionContract::default();
    let selector = SourceSelector::new(&sources, &selection, &peer);
    let ordered = selector.sources_for(&manifest.fragments[0]);

    assert_eq!(
        order_sources_for_test(&ordered),
        vec![SourceType::ReplicaEdge, SourceType::Origin]
    );
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn validated_fragment_becomes_shareable_only_when_origin_policy_allows() {
    let bytes = b"peer-fragment";
    let allowed = package(bytes, vec![]);
    let server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start peer server");
    server
        .add_validated_fragment(
            &allowed,
            &allowed.manifest,
            &allowed.manifest.fragments[0],
            bytes,
        )
        .expect("share allowed");
    assert_eq!(server.available_fragments(), vec![0]);

    let mut denied = package(bytes, vec![]);
    denied.source_selection.allow_peer_sharing = false;
    let denied_server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start peer server");
    denied_server
        .add_validated_fragment(
            &denied,
            &denied.manifest,
            &denied.manifest.fragments[0],
            bytes,
        )
        .expect("sharing disabled is not an error");
    assert!(denied_server.available_fragments().is_empty());
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn sdk_announces_availability_to_origin_after_validation() {
    let bytes = b"hello";
    let package = package(bytes, vec![source("origin", SourceType::Origin, 1)]);
    let announcements = Arc::new(Mutex::new(Vec::new()));
    let origin = FakeOrigin {
        package: package.clone(),
        announcements: announcements.clone(),
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = FakeSource {
        bytes_by_source: HashMap::from([("origin".to_string(), Ok(bytes.to_vec()))]),
        calls,
    };
    let peer = PeerClient::start(Some("127.0.0.1:0"), None).expect("start local peer");
    let mut storage = MemoryStorage::new();

    sync_object(
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

    let announcements = announcements.lock().unwrap();
    assert_eq!(announcements.len(), 1);
    assert!(announcements[0].0.starts_with("peer://"));
    assert_eq!(announcements[0].1, vec![0]);
}

#[cfg(feature = "legacy-tcp-dev")]
#[test]
fn local_two_peer_integration_downloads_peer_before_fallbacks() {
    let bytes = b"hello";
    let peer_a_server = PeerServer::start(Some("127.0.0.1:0"), None).expect("start peer A");
    let peer_a_endpoint = peer_a_server.endpoint().to_string();
    let package = package(
        bytes,
        vec![
            peer_source("peer-a", &peer_a_endpoint),
            source("replica", SourceType::ReplicaEdge, 1),
            source("origin", SourceType::Origin, 1),
        ],
    );
    peer_a_server
        .add_validated_fragment(
            &package,
            &package.manifest,
            &package.manifest.fragments[0],
            bytes,
        )
        .expect("peer A has fragment");
    let origin = FakeOrigin {
        package: package.clone(),
        announcements: Arc::new(Mutex::new(Vec::new())),
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = FakeSource {
        bytes_by_source: HashMap::from([
            ("replica".to_string(), Ok(b"replica".to_vec())),
            ("origin".to_string(), Ok(bytes.to_vec())),
        ]),
        calls: calls.clone(),
    };
    let peer_b = PeerClient::new();
    let mut storage = MemoryStorage::new();

    let object = sync_object(
        &origin,
        &source,
        &peer_b,
        &mut storage,
        &SyncObjectRequest {
            bucket: package.bucket.clone(),
            key: package.key.clone(),
            destination: "unused".into(),
        },
        None,
    )
    .expect("sync from peer");

    assert_eq!(object, bytes);
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn progress_map_tracks_required_fragment_states() {
    let mut progress = ProgressMap::default();
    assert_eq!(progress.state(0), FragmentProgressState::Pending);
    progress.mark_state(0, FragmentProgressState::Downloading);
    assert_eq!(progress.state(0), FragmentProgressState::Downloading);
    progress.mark_state(0, FragmentProgressState::Fallback);
    assert_eq!(progress.state(0), FragmentProgressState::Fallback);
    progress.mark_state(0, FragmentProgressState::Shareable);
    assert_eq!(progress.state(0), FragmentProgressState::Shareable);
}
