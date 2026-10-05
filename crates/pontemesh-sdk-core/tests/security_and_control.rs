use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use pontemesh_sdk_core::contracts::{
    AccessPackage, AuthorizedSource, FallbackContract, FragmentDescriptor, Manifest,
    SourceSelectionContract, SourceType,
};
use pontemesh_sdk_core::download::SyncObjectRequest;
use pontemesh_sdk_core::errors::PontemeshError;
use pontemesh_sdk_core::integrity::sha256_hex;
use pontemesh_sdk_core::p2p::{DisabledPeerTransport, PeerTransport};
use pontemesh_sdk_core::release::{ReleaseFile, ReleaseManifest, UpdateCheckRequest};
use pontemesh_sdk_core::{PontemeshClient, PontemeshClientConfig};

struct MockOriginClient {
    manifest: Manifest,
    package: AccessPackage,
    request_count: Arc<AtomicUsize>,
}

impl MockOriginClient {
    fn new(payload: &[u8]) -> Self {
        let digest = sha256_hex(payload);
        let manifest = Manifest {
            manifest_id: "m-1".to_string(),
            object_id: "obj-1".to_string(),
            bucket: "game-assets".to_string(),
            key: "game.bin".to_string(),
            version: "1.0.0".to_string(),
            total_size_bytes: payload.len() as i64,
            content_type: "application/octet-stream".to_string(),
            object_hash_algorithm: "SHA256".to_string(),
            object_sha256: digest.clone(),
            fragment_size_bytes: payload.len(),
            fragments: vec![FragmentDescriptor {
                index: 0,
                fragment_id: "f-0".to_string(),
                byte_range_start: 0,
                byte_range_end: payload.len().saturating_sub(1) as u64,
                size_bytes: payload.len(),
                hash_algorithm: "SHA256".to_string(),
                sha256: digest,
                priority: "NORMAL".to_string(),
                fallback_range_header: format!("bytes=0-{}", payload.len().saturating_sub(1)),
            }],
            availability_state: "AVAILABLE".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let package = AccessPackage {
            id: "pkg-1".to_string(),
            package_token: "token".to_string(),
            bucket: "game-assets".to_string(),
            key: "game.bin".to_string(),
            version: "1.0.0".to_string(),
            manifest_id: "m-1".to_string(),
            expires_at: "2036-01-01T00:00:00Z".to_string(),
            scope: vec!["read".to_string()],
            authorized_sources: vec![AuthorizedSource {
                id: "origin-1".to_string(),
                source_type: SourceType::Origin,
                endpoint: "http://mock-origin/download".to_string(),
                peer_id: None,
                transport: None,
                priority: 10,
                expires_at: "2036-01-01T00:00:00Z".to_string(),
                available_fragments: vec![0],
            }],
            source_selection: SourceSelectionContract {
                strategy: "PREFER_PEER".to_string(),
                fragment_priority: "SEQUENTIAL".to_string(),
                failure_threshold: 3,
                allow_peer_sharing: true,
                allow_replica_edge: true,
            },
            fallback: FallbackContract::default(),
            manifest: manifest.clone(),
        };

        Self {
            manifest,
            package,
            request_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl pontemesh_sdk_core::client::OriginClient for MockOriginClient {
    fn create_access_package(
        &self,
        _bucket: &str,
        _key: &str,
    ) -> Result<AccessPackage, PontemeshError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.package.clone())
    }

    fn get_manifest(&self, _bucket: &str, _key: &str) -> Result<Manifest, PontemeshError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.manifest.clone())
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

    fn check_software_update(
        &self,
        _request: &UpdateCheckRequest,
    ) -> Result<Option<pontemesh_sdk_core::release::SoftwareUpdateInfo>, PontemeshError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
}

struct MockSourceClient {
    data: Vec<u8>,
    request_count: Arc<AtomicUsize>,
}

impl pontemesh_sdk_core::client::SourceClient for MockSourceClient {
    fn download_fragment(
        &self,
        _package: &AccessPackage,
        _source: &AuthorizedSource,
        _fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError> {
        self.request_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.data.clone())
    }
}

#[test]
fn suspension_disables_all_network_traffic_and_frees_resources() {
    let payload = b"game-binary-content-v1";
    let origin = Box::new(MockOriginClient::new(payload));
    let request_count = origin.request_count.clone();
    let source_requests = Arc::new(AtomicUsize::new(0));
    let source = Box::new(MockSourceClient {
        data: payload.to_vec(),
        request_count: source_requests.clone(),
    });
    let peer = Box::new(DisabledPeerTransport);

    let client = PontemeshClient::with_clients(origin, source, peer);
    assert!(client.is_active());
    assert!(!client.is_suspended());

    let temp = tempfile::tempdir().expect("temp dir");
    let target = temp.path().join("game.bin");

    // 1. Initial sync works normally
    let result = client.sync_object(SyncObjectRequest {
        bucket: "game-assets".to_string(),
        key: "game.bin".to_string(),
        destination: target.clone(),
    });
    assert!(result.is_ok());
    assert!(target.exists());
    let origin_calls_before = request_count.load(Ordering::SeqCst);
    let source_calls_before = source_requests.load(Ordering::SeqCst);
    assert!(origin_calls_before > 0);
    assert!(source_calls_before > 0);

    // 2. Suspend client (e.g., game launcher backend signals no updates until next week)
    client.suspend().expect("suspend succeeds");
    assert!(client.is_suspended());
    assert!(!client.is_active());

    // 3. Any sync attempt while suspended MUST fail immediately with PontemeshError::Suspended
    let second_target = temp.path().join("game_second.bin");
    let err = client
        .sync_object(SyncObjectRequest {
            bucket: "game-assets".to_string(),
            key: "game.bin".to_string(),
            destination: second_target.clone(),
        })
        .expect_err("must be rejected while suspended");

    assert!(matches!(err, PontemeshError::Suspended));
    assert!(!second_target.exists());

    // 4. Update check while suspended MUST also fail immediately
    let update_req = UpdateCheckRequest::new("game-assets", "mygame");
    let update_err = client
        .check_software_update(&update_req)
        .expect_err("update check must fail while suspended");
    assert!(matches!(update_err, PontemeshError::Suspended));

    // 5. Zero network requests occurred during suspended state
    assert_eq!(request_count.load(Ordering::SeqCst), origin_calls_before);
    assert_eq!(source_requests.load(Ordering::SeqCst), source_calls_before);

    // 6. Resume client (backend signals update window opened)
    client.resume().expect("resume succeeds");
    assert!(client.is_active());
    assert!(!client.is_suspended());

    // 7. Operations succeed again
    let third_target = temp.path().join("game_third.bin");
    let resumed_result = client.sync_object(SyncObjectRequest {
        bucket: "game-assets".to_string(),
        key: "game.bin".to_string(),
        destination: third_target.clone(),
    });
    assert!(resumed_result.is_ok());
    assert!(third_target.exists());
}

#[test]
fn config_supports_initial_suspended_state() {
    let config =
        PontemeshClientConfig::new("https://origin.example.com", "token").with_suspended(true);

    assert!(config.suspended);
    let client = PontemeshClient::new(config).expect("client creation");
    assert!(client.is_suspended());
    assert!(!client.is_active());

    let err = client
        .sync_object(SyncObjectRequest {
            bucket: "b".to_string(),
            key: "k".to_string(),
            destination: PathBuf::from("dest.bin"),
        })
        .expect_err("must fail");
    assert!(matches!(err, PontemeshError::Suspended));

    client.set_active(true).expect("activate");
    assert!(client.is_active());
}

#[test]
fn allowed_directories_sandboxing_strictly_enforced() {
    let payload = b"safe-game-content";
    let origin = Box::new(MockOriginClient::new(payload));
    let source = Box::new(MockSourceClient {
        data: payload.to_vec(),
        request_count: Arc::new(AtomicUsize::new(0)),
    });
    let peer = Box::new(DisabledPeerTransport);

    let client = PontemeshClient::with_clients(origin, source, peer);

    let temp = tempfile::tempdir().expect("temp dir");
    let allowed_dir = temp.path().join("game_install_dir");
    let forbidden_dir = temp.path().join("forbidden_system_dir");
    fs::create_dir_all(&allowed_dir).expect("create allowed");
    fs::create_dir_all(&forbidden_dir).expect("create forbidden");

    // Developer tells SDK: ONLY write to allowed_dir!
    client.set_allowed_directories(vec![allowed_dir.clone()]);
    assert_eq!(client.allowed_directories(), vec![allowed_dir.clone()]);

    // 1. Valid destination inside allowed_dir: SUCCEEDS
    let safe_dest = allowed_dir.join("bin").join("game.exe");
    let ok_res = client.sync_object(SyncObjectRequest {
        bucket: "game-assets".to_string(),
        key: "game.bin".to_string(),
        destination: safe_dest.clone(),
    });
    assert!(ok_res.is_ok());
    assert!(safe_dest.exists());

    // 2. Destination in forbidden_dir: FAILS STRICTLY
    let evil_dest = forbidden_dir.join("backdoor.exe");
    let evil_err = client
        .sync_object(SyncObjectRequest {
            bucket: "game-assets".to_string(),
            key: "game.bin".to_string(),
            destination: evil_dest.clone(),
        })
        .expect_err("forbidden destination must fail");
    assert!(matches!(evil_err, PontemeshError::PathNotAllowed(_)));
    assert!(!evil_dest.exists());

    // 3. Parent traversal attempt escaping allowed_dir: FAILS STRICTLY
    let traversal_dest = allowed_dir
        .join("..")
        .join("forbidden_system_dir")
        .join("escape.bin");
    let traversal_err = client
        .sync_object(SyncObjectRequest {
            bucket: "game-assets".to_string(),
            key: "game.bin".to_string(),
            destination: traversal_dest.clone(),
        })
        .expect_err("path traversal must fail");
    assert!(matches!(traversal_err, PontemeshError::PathNotAllowed(_)));
    assert!(!traversal_dest.exists());

    // 4. Prefix trickery (e.g., game_install_dir_evil): FAILS STRICTLY
    let evil_prefix_dir = temp.path().join("game_install_dir_evil");
    fs::create_dir_all(&evil_prefix_dir).expect("create evil prefix");
    let evil_prefix_dest = evil_prefix_dir.join("malware.bin");
    let prefix_err = client
        .sync_object(SyncObjectRequest {
            bucket: "game-assets".to_string(),
            key: "game.bin".to_string(),
            destination: evil_prefix_dest.clone(),
        })
        .expect_err("prefix spoofing must fail");
    assert!(matches!(prefix_err, PontemeshError::PathNotAllowed(_)));
    assert!(!evil_prefix_dest.exists());
}

#[test]
fn custom_cache_directory_outside_allowed_is_strictly_rejected() {
    let payload = b"cached-content";
    let origin = Box::new(MockOriginClient::new(payload));
    let source = Box::new(MockSourceClient {
        data: payload.to_vec(),
        request_count: Arc::new(AtomicUsize::new(0)),
    });
    let peer = Box::new(DisabledPeerTransport);

    let client = PontemeshClient::with_clients(origin, source, peer);

    let temp = tempfile::tempdir().expect("temp dir");
    let allowed_dir = temp.path().join("game_dir");
    let forbidden_cache = temp.path().join("forbidden_cache");
    fs::create_dir_all(&allowed_dir).expect("create allowed");
    fs::create_dir_all(&forbidden_cache).expect("create forbidden cache");

    client.set_allowed_directories(vec![allowed_dir.clone()]);

    let safe_dest = allowed_dir.join("game.exe");

    // Passing forbidden_cache outside allowed directories must be rejected
    let err = client
        .sync_object_to_disk_with_cache(
            SyncObjectRequest {
                bucket: "game-assets".to_string(),
                key: "game.bin".to_string(),
                destination: safe_dest,
            },
            forbidden_cache.clone(),
            None,
            Default::default(),
        )
        .expect_err("forbidden cache directory must be rejected");

    assert!(matches!(err, PontemeshError::PathNotAllowed(_)));
}

#[test]
fn malicious_peer_trojan_injection_is_cryptographically_impossible() {
    // Malicious peer that tries to serve trojaned bytes instead of valid game code
    struct TrojanPeerTransport {
        trojan_bytes: Vec<u8>,
    }

    impl PeerTransport for TrojanPeerTransport {
        fn can_handle(&self, _source: &AuthorizedSource) -> bool {
            true
        }

        fn download_fragment(
            &self,
            _source: &AuthorizedSource,
            _package: &AccessPackage,
            _manifest: &Manifest,
            _fragment: &FragmentDescriptor,
        ) -> Result<Vec<u8>, PontemeshError> {
            // Peer sends trojan binary
            Ok(self.trojan_bytes.clone())
        }
    }

    let legitimate_code = b"GENUINE_GAME_EXECUTABLE_CODE_V1";
    let trojan_payload = b"MALICIOUS_BACKDOOR_TROJAN_PAYLOAD";

    let mut origin = MockOriginClient::new(legitimate_code);
    // Add peer as prefer-peer source
    origin.package.authorized_sources.insert(
        0,
        AuthorizedSource {
            id: "rogue-peer-1".to_string(),
            source_type: SourceType::Peer,
            endpoint: "/ip4/192.168.1.100/tcp/4001/p2p/QmPeer".to_string(),
            peer_id: Some("QmPeer".to_string()),
            transport: Some("libp2p".to_string()),
            priority: 100,
            expires_at: "2036-01-01T00:00:00Z".to_string(),
            available_fragments: vec![0],
        },
    );

    let clean_source = Box::new(MockSourceClient {
        data: legitimate_code.to_vec(),
        request_count: Arc::new(AtomicUsize::new(0)),
    });
    let rogue_peer = Box::new(TrojanPeerTransport {
        trojan_bytes: trojan_payload.to_vec(),
    });

    let client = PontemeshClient::with_clients(Box::new(origin), clean_source, rogue_peer);

    let temp = tempfile::tempdir().expect("temp dir");
    let game_exe = temp.path().join("Game.exe");

    // Sync object: the peer will attempt to send trojan bytes,
    // which MUST be rejected due to SHA-256 hash mismatch, falling back to clean origin!
    let summary = client
        .sync_object_with_summary(SyncObjectRequest {
            bucket: "game-assets".to_string(),
            key: "game.bin".to_string(),
            destination: game_exe.clone(),
        })
        .expect("sync should succeed via fallback");

    // Peer hash failure must be recorded
    assert!(summary.summary.peer_hash_failures > 0 || summary.summary.peer_failures > 0);
    assert_eq!(summary.summary.fragments_from_origin, 1);

    // Verify the installed file contains the GENUINE code, NEVER the trojan payload
    let installed_bytes = fs::read(&game_exe).expect("read installed file");
    assert_eq!(installed_bytes, legitimate_code);
    assert_ne!(installed_bytes, trojan_payload);
}

#[test]
fn release_manifest_strictly_rejects_backdoor_paths_and_null_bytes() {
    fn file(path: &str) -> ReleaseFile {
        ReleaseFile {
            bucket: "updates".to_string(),
            key: format!("releases/{path}"),
            path: path.to_string(),
            size_bytes: 100,
            sha256: "a".repeat(64),
            order: 1,
        }
    }

    // 1. Escaping install root via ..
    let escape_manifest = ReleaseManifest {
        schema_version: 1,
        product: "game".to_string(),
        version: "1.0.0".to_string(),
        files: vec![file("../evil.exe")],
    };
    assert!(escape_manifest.validate().is_err());

    // 2. Absolute path in Linux/Unix
    let absolute_unix = ReleaseManifest {
        schema_version: 1,
        product: "game".to_string(),
        version: "1.0.0".to_string(),
        files: vec![file("/etc/cron.d/backdoor")],
    };
    assert!(absolute_unix.validate().is_err());

    // 3. Absolute path / platform backslash in Windows
    let absolute_win = ReleaseManifest {
        schema_version: 1,
        product: "game".to_string(),
        version: "1.0.0".to_string(),
        files: vec![file("C:\\Windows\\System32\\backdoor.dll")],
    };
    assert!(absolute_win.validate().is_err());

    // 4. Null byte injection attempt
    let null_byte_manifest = ReleaseManifest {
        schema_version: 1,
        product: "game".to_string(),
        version: "1.0.0".to_string(),
        files: vec![file("safe.png\0.exe")],
    };
    assert!(null_byte_manifest.validate().is_err());
}
