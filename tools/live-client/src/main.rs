use std::{env, path::PathBuf, process::ExitCode, time::Instant};

use pontemesh_sdk_core::{
    integrity::sha256_hex, p2p::P2pConfig, PontemeshClient, PontemeshClientConfig,
    SyncObjectRequest,
};
use serde_json::json;

#[derive(Debug)]
struct Config {
    origin_url: String,
    application_token: String,
    bucket: String,
    key: String,
    destination: PathBuf,
    expected_sha256: Option<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pontemesh-live-client failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let config = Config::from_args(env::args().skip(1))?;
    let started = Instant::now();
    let client = PontemeshClient::new(PontemeshClientConfig {
        origin_url: config.origin_url.clone(),
        application_token: config.application_token.clone(),
        p2p: P2pConfig::default(),
    })
    .map_err(|error| error.to_string())?;
    let result = client
        .sync_object_with_summary(SyncObjectRequest {
            bucket: config.bucket.clone(),
            key: config.key.clone(),
            destination: config.destination.clone(),
        })
        .map_err(|error| error.to_string())?;
    let bytes = std::fs::read(&config.destination).map_err(|error| error.to_string())?;
    let sha256 = sha256_hex(&bytes);
    if result.bytes != bytes {
        return Err("downloaded file does not match SDK result bytes".to_owned());
    }
    if let Some(expected_sha256) = &config.expected_sha256 {
        if !sha256.eq_ignore_ascii_case(expected_sha256) {
            return Err(format!(
                "downloaded sha256 mismatch: expected {expected_sha256}, got {sha256}"
            ));
        }
    }

    println!(
        "{}",
        json!({
            "ok": true,
            "originUrl": config.origin_url,
            "bucket": config.bucket,
            "key": config.key,
            "destination": config.destination,
            "bytes": bytes.len(),
            "sha256": sha256,
            "elapsedMs": started.elapsed().as_millis(),
            "summary": {
                "bytesFromPeer": result.summary.bytes_from_peer,
                "bytesFromReplica": result.summary.bytes_from_replica,
                "bytesFromOrigin": result.summary.bytes_from_origin,
                "fragmentsFromPeer": result.summary.fragments_from_peer,
                "fragmentsFromReplica": result.summary.fragments_from_replica,
                "fragmentsFromOrigin": result.summary.fragments_from_origin,
                "peerFailures": result.summary.peer_failures,
                "peerHashFailures": result.summary.peer_hash_failures,
                "peerRejectedFragments": result.summary.peer_rejected_fragments,
                "fallbackActivations": result.summary.fallback_activations
            }
        })
    );
    Ok(())
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut origin_url = env::var("PONTEMESH_LIVE_ORIGIN_URL").ok();
        let mut application_token = env::var("PONTEMESH_LIVE_APPLICATION_TOKEN").ok();
        let mut bucket = env::var("PONTEMESH_LIVE_BUCKET").ok();
        let mut key = env::var("PONTEMESH_LIVE_KEY").ok();
        let mut destination = env::var("PONTEMESH_LIVE_DESTINATION")
            .map(PathBuf::from)
            .ok();
        let mut expected_sha256 = env::var("PONTEMESH_LIVE_EXPECTED_SHA256").ok();

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--origin-url" => origin_url = Some(take_value(&mut args, "--origin-url")?),
                "--application-token" => {
                    application_token = Some(take_value(&mut args, "--application-token")?)
                }
                "--bucket" => bucket = Some(take_value(&mut args, "--bucket")?),
                "--key" => key = Some(take_value(&mut args, "--key")?),
                "--destination" => {
                    destination = Some(PathBuf::from(take_value(&mut args, "--destination")?))
                }
                "--expected-sha256" => {
                    expected_sha256 = Some(take_value(&mut args, "--expected-sha256")?)
                }
                "--help" | "-h" => return Err(usage()),
                unknown => return Err(format!("unknown argument: {unknown}\n\n{}", usage())),
            }
        }

        Ok(Self {
            origin_url: required(origin_url, "origin url")?,
            application_token: required(application_token, "application token")?,
            bucket: required(bucket, "bucket")?,
            key: required(key, "key")?,
            destination: destination.ok_or_else(|| "destination is required".to_owned())?,
            expected_sha256: expected_sha256.filter(|value| !value.trim().is_empty()),
        })
    }
}

fn take_value(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    name: &str,
) -> Result<String, String> {
    args.next()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} requires a value"))
}

fn required(value: Option<String>, label: &str) -> Result<String, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{label} is required"))
}

fn usage() -> String {
    "Usage: pontemesh-live-client --origin-url URL --application-token TOKEN --bucket BUCKET --key KEY --destination PATH [--expected-sha256 SHA256]".to_owned()
}
