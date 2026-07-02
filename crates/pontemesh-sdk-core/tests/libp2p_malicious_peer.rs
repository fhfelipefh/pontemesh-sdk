use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use pontemesh_sdk_core::client::{OriginClient, SourceClient};
use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::download::{sync_object_with_summary, SyncObjectRequest};
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::sha256_hex;
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
}

struct OriginSource {
    bytes: Vec<u8>,
}

impl SourceClient for OriginSource {
    fn download_fragment(
        &self,
        _package: &AccessPackage,
        _source: &AuthorizedSource,
        fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError> {
        Ok(
            self.bytes[fragment.byte_range_start as usize..=fragment.byte_range_end as usize]
                .to_vec(),
        )
    }
}

#[test]
#[ignore]
fn malicious_peer_hash_mismatch_is_rejected_and_origin_fallback_completes() {
    run_malicious_case(MaliciousMode::WrongHash, |summary| {
        assert!(summary.peer_hash_failures > 0);
        assert!(summary.fallback_activations > 0);
    });
}

#[test]
#[ignore]
fn malicious_peer_wrong_index_is_rejected_and_origin_fallback_completes() {
    run_malicious_case(MaliciousMode::WrongIndex, |summary| {
        assert!(summary.peer_failures > 0);
        assert!(summary.fallback_activations > 0);
    });
}

#[test]
#[ignore]
fn malicious_peer_oversized_frame_is_rejected_and_origin_fallback_completes() {
    run_malicious_case(MaliciousMode::Oversized, |summary| {
        assert!(summary.peer_failures > 0);
        assert!(summary.fallback_activations > 0);
    });
}

#[test]
#[ignore]
fn malicious_peer_identity_mismatch_is_rejected_and_origin_fallback_completes() {
    run_malicious_case(MaliciousMode::WrongPeerId, |summary| {
        assert!(summary.peer_failures > 0);
        assert!(summary.fallback_activations > 0);
    });
}

#[test]
#[ignore]
fn replayed_nonce_is_rejected_and_origin_fallback_completes() {
    run_malicious_case(MaliciousMode::WrongNonce, |summary| {
        assert!(summary.peer_failures > 0);
        assert!(summary.fallback_activations > 0);
    });
}

#[test]
#[ignore]
fn expired_peer_is_ignored_and_origin_completes() {
    let bytes = b"secure-fragment".to_vec();
    let manifest = manifest(&bytes);
    let package = package(
        manifest.clone(),
        vec![
            peer_source(
                "expired-peer",
                "peer://127.0.0.1:9/p2p/expired-peer",
                "expired-peer",
                "2000-01-01T00:00:00Z",
            ),
            origin_source(),
        ],
    );
    let result = sync_object_with_summary(
        &TestOrigin {
            package: package.clone(),
        },
        &OriginSource {
            bytes: bytes.clone(),
        },
        &pontemesh_sdk_core::p2p::Libp2pTransport::new(),
        &mut MemoryStorage::new(),
        &SyncObjectRequest {
            bucket: package.bucket.clone(),
            key: package.key.clone(),
            destination: "unused".into(),
        },
        None,
    )
    .expect("origin fallback");
    assert_eq!(result.bytes, bytes);
    assert_eq!(result.summary.bytes_from_peer, 0);
    assert_eq!(result.summary.bytes_from_origin, bytes.len() as u64);
}

#[test]
#[ignore]
fn peer_request_never_contains_path_traversal_or_tokens() {
    let captured = Arc::new(Mutex::new(String::new()));
    let endpoint = malicious_peer(MaliciousMode::WrongHash, captured.clone());
    let bytes = b"secure-fragment".to_vec();
    let manifest = manifest(&bytes);
    let package = package(
        manifest.clone(),
        vec![
            peer_source("malicious", &endpoint, "malicious", "2099-01-01T00:00:00Z"),
            origin_source(),
        ],
    );
    let _ = sync_object_with_summary(
        &TestOrigin {
            package: package.clone(),
        },
        &OriginSource {
            bytes: bytes.clone(),
        },
        &pontemesh_sdk_core::p2p::Libp2pTransport::new(),
        &mut MemoryStorage::new(),
        &SyncObjectRequest {
            bucket: package.bucket.clone(),
            key: package.key.clone(),
            destination: "unused".into(),
        },
        None,
    )
    .expect("fallback after malicious peer");
    let request = captured.lock().unwrap();
    assert!(!request.contains("../"));
    assert!(!request.contains("package-token-secret"));
    assert!(!request.contains("application-token"));
}

fn run_malicious_case(
    mode: MaliciousMode,
    assert_summary: impl FnOnce(pontemesh_sdk_core::download::TransferSummary),
) {
    let captured = Arc::new(Mutex::new(String::new()));
    let endpoint = malicious_peer(mode, captured);
    let bytes = b"secure-fragment".to_vec();
    let manifest = manifest(&bytes);
    let package = package(
        manifest.clone(),
        vec![
            peer_source("malicious", &endpoint, "malicious", "2099-01-01T00:00:00Z"),
            origin_source(),
        ],
    );
    let result = sync_object_with_summary(
        &TestOrigin {
            package: package.clone(),
        },
        &OriginSource {
            bytes: bytes.clone(),
        },
        &pontemesh_sdk_core::p2p::Libp2pTransport::new(),
        &mut MemoryStorage::new(),
        &SyncObjectRequest {
            bucket: package.bucket.clone(),
            key: package.key.clone(),
            destination: "unused".into(),
        },
        None,
    )
    .expect("fallback completes object");
    assert_eq!(result.bytes, bytes);
    assert_eq!(sha256_hex(&result.bytes), manifest.object_sha256);
    assert_summary(result.summary);
}

#[derive(Debug, Clone, Copy)]
enum MaliciousMode {
    WrongHash,
    WrongIndex,
    Oversized,
    WrongPeerId,
    WrongNonce,
}

fn malicious_peer(mode: MaliciousMode, captured: Arc<Mutex<String>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind malicious peer");
    let endpoint = format!("peer://{}/p2p/malicious", listener.local_addr().unwrap());
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept peer request");
        let mut request_line = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut request_line)
            .unwrap();
        *captured.lock().unwrap() = request_line.clone();
        if matches!(mode, MaliciousMode::Oversized) {
            stream.write_all(&vec![b'x'; 2 * 1024 * 1024 + 2]).unwrap();
            stream.write_all(b"\n").unwrap();
            return;
        }
        let request: serde_json::Value = serde_json::from_str(&request_line).unwrap();
        let bytes = b"evil-fragment";
        let fragment_index = if matches!(mode, MaliciousMode::WrongIndex) {
            99
        } else {
            0
        };
        let sha256 = if matches!(mode, MaliciousMode::WrongHash) {
            "000000".to_string()
        } else {
            sha256_hex(bytes)
        };
        let nonce = if matches!(mode, MaliciousMode::WrongNonce) {
            "replayed-nonce"
        } else {
            request["requestNonce"].as_str().unwrap()
        };
        let peer_id = if matches!(mode, MaliciousMode::WrongPeerId) {
            "other-peer"
        } else {
            "malicious"
        };
        let payload = serde_json::json!({
            "type": "fragmentResponse",
            "protocolVersion": 1,
            "packageId": request["packageId"],
            "manifestId": request["manifestId"],
            "fragmentId": request["fragmentId"],
            "fragmentIndex": fragment_index,
            "sizeBytes": bytes.len(),
            "sha256": sha256,
            "requestNonce": nonce,
            "peerId": peer_id,
            "bytesBase64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
        });
        writeln!(stream, "{payload}").unwrap();
    });
    endpoint
}

fn manifest(bytes: &[u8]) -> Manifest {
    Manifest {
        manifest_id: "manifest-sec".to_string(),
        object_id: "object-sec".to_string(),
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        version: "v1".to_string(),
        total_size_bytes: bytes.len() as i64,
        content_type: "application/octet-stream".to_string(),
        object_hash_algorithm: "SHA256".to_string(),
        object_sha256: sha256_hex(bytes),
        fragment_size_bytes: bytes.len(),
        fragments: vec![FragmentDescriptor {
            index: 0,
            fragment_id: "fragment-0".to_string(),
            byte_range_start: 0,
            byte_range_end: bytes.len().saturating_sub(1) as u64,
            size_bytes: bytes.len(),
            hash_algorithm: "SHA256".to_string(),
            sha256: sha256_hex(bytes),
            priority: "NORMAL".to_string(),
            fallback_range_header: format!("bytes=0-{}", bytes.len().saturating_sub(1)),
        }],
        availability_state: "AVAILABLE".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn package(manifest: Manifest, sources: Vec<AuthorizedSource>) -> AccessPackage {
    AccessPackage {
        id: "pkg-sec".to_string(),
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

fn peer_source(id: &str, endpoint: &str, _peer_id: &str, expires_at: &str) -> AuthorizedSource {
    let peer_id =
        libp2p::PeerId::from(libp2p::identity::Keypair::generate_ed25519().public()).to_string();
    let endpoint = endpoint.split("/p2p/").next().unwrap_or(endpoint);
    AuthorizedSource {
        id: id.to_string(),
        source_type: SourceType::Peer,
        endpoint: format!("{endpoint}/p2p/{peer_id}"),
        peer_id: Some(peer_id),
        transport: Some("libp2p".to_string()),
        priority: 1,
        expires_at: expires_at.to_string(),
        available_fragments: vec![0],
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
        available_fragments: vec![0],
    }
}
