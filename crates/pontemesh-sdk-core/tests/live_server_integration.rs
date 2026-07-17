use std::env;

use pontemesh_sdk_core::integrity::sha256_hex;
use pontemesh_sdk_core::{
    p2p::P2pConfig, PontemeshClient, PontemeshClientConfig, SyncObjectRequest,
};

struct LiveConfig {
    origin_url: String,
    application_token: String,
    bucket: String,
    key: String,
    expected_sha256: Option<String>,
}

#[test]
fn sdk_syncs_object_from_live_pontemesh_server() {
    let Some(config) = LiveConfig::from_env() else {
        return;
    };
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let destination = temp_dir.path().join("downloaded-object.bin");
    let client = PontemeshClient::new(PontemeshClientConfig {
        origin_url: config.origin_url,
        application_token: config.application_token,
        p2p: P2pConfig::default(),
    })
    .expect("create SDK client");
    let mut progress = Vec::new();

    let result = client
        .sync_object_with_summary_and_progress(
            SyncObjectRequest {
                bucket: config.bucket,
                key: config.key,
                destination: destination.clone(),
            },
            Some(
                &mut |fragment_index, bytes_downloaded, total_bytes, source_type| {
                    progress.push((
                        fragment_index,
                        bytes_downloaded,
                        total_bytes,
                        source_type.to_string(),
                    ));
                },
            ),
        )
        .expect("sync object from live Ponte Mesh server");

    let bytes = std::fs::read(destination).expect("read downloaded object");
    assert!(!bytes.is_empty());
    assert_eq!(result.bytes, bytes);
    if let Some(expected_sha256) = config.expected_sha256 {
        assert_eq!(sha256_hex(&bytes), expected_sha256);
    }
    assert!(!progress.is_empty());
    assert_eq!(successful_bytes(&result.summary), bytes.len() as u64);
    assert!(successful_fragments(&result.summary) > 0);
}

impl LiveConfig {
    fn from_env() -> Option<Self> {
        let required = env::var("PONTEMESH_LIVE_REQUIRED").as_deref() == Ok("1");
        let required_names = [
            "PONTEMESH_LIVE_ORIGIN_URL",
            "PONTEMESH_LIVE_APPLICATION_TOKEN",
            "PONTEMESH_LIVE_BUCKET",
            "PONTEMESH_LIVE_KEY",
        ];
        let missing = required_names
            .iter()
            .filter(|name| {
                env::var(name)
                    .map(|value| value.trim().is_empty())
                    .unwrap_or(true)
            })
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            if required {
                panic!("missing live Ponte Mesh env vars: {}", missing.join(", "));
            }
            eprintln!(
                "skipping live Ponte Mesh integration; set PONTEMESH_LIVE_REQUIRED=1 and {}",
                required_names.join(", ")
            );
            return None;
        }

        Some(Self {
            origin_url: env::var("PONTEMESH_LIVE_ORIGIN_URL").expect("origin url"),
            application_token: env::var("PONTEMESH_LIVE_APPLICATION_TOKEN")
                .expect("application token"),
            bucket: env::var("PONTEMESH_LIVE_BUCKET").expect("bucket"),
            key: env::var("PONTEMESH_LIVE_KEY").expect("key"),
            expected_sha256: env::var("PONTEMESH_LIVE_EXPECTED_SHA256")
                .ok()
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

fn successful_bytes(summary: &pontemesh_sdk_core::TransferSummary) -> u64 {
    summary.bytes_from_peer + summary.bytes_from_replica + summary.bytes_from_origin
}

fn successful_fragments(summary: &pontemesh_sdk_core::TransferSummary) -> u64 {
    summary.fragments_from_peer + summary.fragments_from_replica + summary.fragments_from_origin
}
