use std::{env, path::PathBuf, process::ExitCode, time::Instant};

use pontemesh_sdk_core::{
    integrity::sha256_hex, PontemeshClient, PontemeshClientConfig, SyncObjectRequest,
    UpdateCheckRequest,
};
use serde_json::json;

#[derive(Debug)]
enum Mode {
    SyncObject {
        key: String,
        destination: PathBuf,
        expected_sha256: Option<String>,
    },
    CheckUpdate {
        software_id: String,
        current_version: Option<String>,
        channel: Option<String>,
        destination: Option<PathBuf>,
    },
}

#[derive(Debug)]
struct Config {
    origin_url: String,
    application_token: String,
    bucket: String,
    mode: Mode,
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
    let config = match Config::from_args(env::args().skip(1))? {
        Some(config) => config,
        None => return Ok(()),
    };
    let started = Instant::now();
    let client = PontemeshClient::new(PontemeshClientConfig::new(
        config.origin_url.clone(),
        config.application_token.clone(),
    ))
    .map_err(|error| error.to_string())?;

    match config.mode {
        Mode::SyncObject {
            key,
            destination,
            expected_sha256,
        } => {
            let result = client
                .sync_object_with_summary(SyncObjectRequest {
                    bucket: config.bucket.clone(),
                    key: key.clone(),
                    destination: destination.clone(),
                })
                .map_err(|error| error.to_string())?;
            let bytes = std::fs::read(&destination).map_err(|error| error.to_string())?;
            let sha256 = sha256_hex(&bytes);
            if result.bytes != bytes {
                return Err("downloaded file does not match SDK result bytes".to_owned());
            }
            if let Some(expected_sha256) = &expected_sha256 {
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
                    "mode": "syncObject",
                    "originUrl": config.origin_url,
                    "bucket": config.bucket,
                    "key": key,
                    "destination": destination,
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
        Mode::CheckUpdate {
            software_id,
            current_version,
            channel,
            destination,
        } => {
            let req = UpdateCheckRequest {
                bucket: config.bucket.clone(),
                software_id: software_id.clone(),
                current_version: current_version.clone(),
                channel: channel.clone(),
            };
            let update = client
                .check_software_update(&req)
                .map_err(|error| error.to_string())?;

            let mut out = json!({
                "ok": true,
                "mode": "checkUpdate",
                "originUrl": config.origin_url,
                "bucket": config.bucket,
                "softwareId": software_id,
                "currentVersion": current_version,
                "channel": channel,
                "updateFound": update.is_some(),
            });

            if let Some(info) = update {
                out["update"] = json!({
                    "bucket": info.bucket,
                    "softwareId": info.software_id,
                    "versioningScheme": info.versioning_scheme,
                    "currentVersion": info.current_version,
                    "latestVersion": info.latest_version,
                    "hasUpdate": info.has_update,
                    "targetObjectKey": info.target_object_key,
                    "sizeBytes": info.size_bytes,
                    "manifestId": info.manifest_id,
                    "mandatory": info.mandatory,
                });

                if let Some(dest) = destination {
                    if info.has_update {
                        let sync_res = client
                            .sync_object_with_summary(info.to_sync_request(&dest))
                            .map_err(|error| error.to_string())?;
                        let bytes = std::fs::read(&dest).map_err(|error| error.to_string())?;
                        let sha256 = sha256_hex(&bytes);

                        out["download"] = json!({
                            "destination": dest,
                            "bytes": bytes.len(),
                            "sha256": sha256,
                            "summary": {
                                "bytesFromPeer": sync_res.summary.bytes_from_peer,
                                "bytesFromReplica": sync_res.summary.bytes_from_replica,
                                "bytesFromOrigin": sync_res.summary.bytes_from_origin,
                                "fragmentsFromPeer": sync_res.summary.fragments_from_peer,
                                "fragmentsFromReplica": sync_res.summary.fragments_from_replica,
                                "fragmentsFromOrigin": sync_res.summary.fragments_from_origin,
                                "peerFailures": sync_res.summary.peer_failures,
                                "peerHashFailures": sync_res.summary.peer_hash_failures,
                                "peerRejectedFragments": sync_res.summary.peer_rejected_fragments,
                                "fallbackActivations": sync_res.summary.fallback_activations
                            }
                        });
                    }
                }
            }

            out["elapsedMs"] = json!(started.elapsed().as_millis());
            println!("{}", out);
            Ok(())
        }
    }
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut origin_url = env::var("PONTEMESH_LIVE_ORIGIN_URL").ok();
        let mut application_token = env::var("PONTEMESH_LIVE_APPLICATION_TOKEN").ok();
        let mut bucket = env::var("PONTEMESH_LIVE_BUCKET").ok();
        let mut key = env::var("PONTEMESH_LIVE_KEY").ok();
        let mut software_id = env::var("PONTEMESH_LIVE_SOFTWARE_ID").ok();
        let mut current_version = env::var("PONTEMESH_LIVE_CURRENT_VERSION").ok();
        let mut channel = env::var("PONTEMESH_LIVE_CHANNEL").ok();
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
                "--software-id" => software_id = Some(take_value(&mut args, "--software-id")?),
                "--current-version" => {
                    current_version = Some(take_value(&mut args, "--current-version")?)
                }
                "--channel" => channel = Some(take_value(&mut args, "--channel")?),
                "--destination" => {
                    destination = Some(PathBuf::from(take_value(&mut args, "--destination")?))
                }
                "--expected-sha256" => {
                    expected_sha256 = Some(take_value(&mut args, "--expected-sha256")?)
                }
                "--help" | "-h" => {
                    println!("{}", usage());
                    return Ok(None);
                }
                unknown => return Err(format!("unknown argument: {unknown}\n\n{}", usage())),
            }
        }

        let origin_url = required(origin_url, "origin url")?;
        let application_token = required(application_token, "application token")?;
        let bucket = required(bucket, "bucket")?;

        let mode = if let Some(software_id) = software_id {
            Mode::CheckUpdate {
                software_id,
                current_version,
                channel,
                destination,
            }
        } else if let Some(key) = key {
            Mode::SyncObject {
                key,
                destination: destination
                    .ok_or_else(|| "destination is required for sync".to_owned())?,
                expected_sha256: expected_sha256.filter(|value| !value.trim().is_empty()),
            }
        } else {
            return Err("either --key or --software-id must be specified".to_owned());
        };

        Ok(Some(Self {
            origin_url,
            application_token,
            bucket,
            mode,
        }))
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
    "Usage:\n  pontemesh-live-client --origin-url URL --application-token TOKEN --bucket BUCKET --key KEY --destination PATH [--expected-sha256 SHA256]\n  pontemesh-live-client --origin-url URL --application-token TOKEN --bucket BUCKET --software-id ID [--current-version VER] [--channel CHANNEL] [--destination PATH]".to_owned()
}
