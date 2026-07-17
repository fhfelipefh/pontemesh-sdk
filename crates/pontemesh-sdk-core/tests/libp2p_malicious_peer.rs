use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use futures::StreamExt;
use libp2p::request_response::{
    Config as RequestResponseConfig, Event as RequestResponseEvent,
    Message as RequestResponseMessage, ProtocolSupport,
};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identity, noise, ping, request_response, yamux, Multiaddr, PeerId, StreamProtocol};
use pontemesh_sdk_core::client::{OriginClient, SourceClient};
use pontemesh_sdk_core::contracts::*;
use pontemesh_sdk_core::download::{sync_object_with_summary, SyncObjectRequest};
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::sha256_hex;
use pontemesh_sdk_core::p2p::{Libp2pFragmentRequest, Libp2pFragmentResponse, FRAGMENT_PROTOCOL};
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
    let (endpoint, peer_id) = malicious_peer(MaliciousMode::WrongHash, captured.clone());
    let bytes = b"secure-fragment".to_vec();
    let manifest = manifest(&bytes);
    let package = package(
        manifest.clone(),
        vec![
            peer_source("malicious", &endpoint, &peer_id, "2099-01-01T00:00:00Z"),
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
    let (endpoint, peer_id) = malicious_peer(mode, captured);
    let authorized_peer_id = if matches!(mode, MaliciousMode::WrongPeerId) {
        PeerId::from(identity::Keypair::generate_ed25519().public()).to_string()
    } else {
        peer_id
    };
    let bytes = b"secure-fragment".to_vec();
    let manifest = manifest(&bytes);
    let package = package(
        manifest.clone(),
        vec![
            peer_source(
                "malicious",
                &endpoint,
                &authorized_peer_id,
                "2099-01-01T00:00:00Z",
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

#[derive(NetworkBehaviour)]
struct AdversarialBehaviour {
    request_response:
        request_response::cbor::Behaviour<Libp2pFragmentRequest, Libp2pFragmentResponse>,
    ping: ping::Behaviour,
}

fn malicious_peer(mode: MaliciousMode, captured: Arc<Mutex<String>>) -> (String, String) {
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    let peer_id_string = peer_id.to_string();
    let (ready_tx, ready_rx) = mpsc::channel();
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("malicious libp2p runtime");
        runtime.block_on(async move {
            let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
                .with_tokio()
                .with_tcp(
                    Default::default(),
                    noise::Config::new,
                    yamux::Config::default,
                )
                .expect("malicious tcp transport")
                .with_behaviour(|_| {
                    let protocols = [(
                        StreamProtocol::new(FRAGMENT_PROTOCOL),
                        ProtocolSupport::Full,
                    )];
                    AdversarialBehaviour {
                        request_response: request_response::cbor::Behaviour::new(
                            protocols,
                            RequestResponseConfig::default()
                                .with_request_timeout(Duration::from_secs(5)),
                        ),
                        ping: ping::Behaviour::default(),
                    }
                })
                .expect("malicious behaviour")
                .with_swarm_config(|config| {
                    config.with_idle_connection_timeout(Duration::from_secs(30))
                })
                .build();
            swarm
                .listen_on("/ip4/127.0.0.1/tcp/0".parse::<Multiaddr>().unwrap())
                .expect("malicious listen");
            loop {
                match swarm.select_next_some().await {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let endpoint = address
                            .with_p2p(peer_id)
                            .map(|addr| addr.to_string())
                            .unwrap_or_else(|addr| format!("{addr}/p2p/{peer_id}"));
                        let _ = ready_tx.send(endpoint);
                    }
                    SwarmEvent::Behaviour(AdversarialBehaviourEvent::RequestResponse(
                        RequestResponseEvent::Message {
                            message:
                                RequestResponseMessage::Request {
                                    request, channel, ..
                                },
                            ..
                        },
                    )) => {
                        *captured.lock().unwrap() =
                            serde_json::to_string(&request).unwrap_or_default();
                        let byte_count =
                            (request.byte_range_end - request.byte_range_start + 1) as usize;
                        let mut bytes = vec![b'x'; byte_count];
                        if matches!(mode, MaliciousMode::Oversized) {
                            bytes = vec![b'x'; 2 * 1024 * 1024 + 2];
                        }
                        let response = Libp2pFragmentResponse {
                            protocol_version: request.protocol_version,
                            package_id: request.package_id,
                            manifest_id: request.manifest_id,
                            fragment_id: request.fragment_id,
                            fragment_index: if matches!(mode, MaliciousMode::WrongIndex) {
                                request.fragment_index + 99
                            } else {
                                request.fragment_index
                            },
                            size_bytes: bytes.len(),
                            sha256: if matches!(mode, MaliciousMode::WrongHash) {
                                "000000".to_string()
                            } else {
                                sha256_hex(&bytes)
                            },
                            request_nonce: if matches!(mode, MaliciousMode::WrongNonce) {
                                "replayed-nonce".to_string()
                            } else {
                                request.request_nonce
                            },
                            bytes,
                        };
                        let _ = swarm
                            .behaviour_mut()
                            .request_response
                            .send_response(channel, response);
                    }
                    _ => {}
                }
            }
        });
    });
    let endpoint = ready_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("malicious peer endpoint");
    (endpoint, peer_id_string)
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

fn peer_source(id: &str, endpoint: &str, peer_id: &str, expires_at: &str) -> AuthorizedSource {
    let endpoint = endpoint.split("/p2p/").next().unwrap_or(endpoint);
    AuthorizedSource {
        id: id.to_string(),
        source_type: SourceType::Peer,
        endpoint: format!("{endpoint}/p2p/{peer_id}"),
        peer_id: Some(peer_id.to_string()),
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
