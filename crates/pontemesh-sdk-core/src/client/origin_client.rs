use serde_json::json;

use crate::contracts::{AccessPackage, CreateAccessPackageRequest, Manifest, SourceType};
use crate::errors::PontemeshError;
use crate::p2p::PeerAnnouncement;

#[derive(Debug, Clone)]
pub struct PontemeshClientConfig {
    pub origin_url: String,
    pub application_token: String,
    pub p2p: crate::p2p::P2pConfig,
}

impl PontemeshClientConfig {
    pub fn new(origin_url: String, application_token: String) -> Self {
        Self {
            origin_url,
            application_token,
            p2p: crate::p2p::P2pConfig::default(),
        }
    }
}

pub trait OriginClient: Send + Sync {
    fn create_access_package(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<AccessPackage, PontemeshError>;
    fn get_manifest(&self, bucket: &str, key: &str) -> Result<Manifest, PontemeshError>;
    #[allow(clippy::too_many_arguments)]
    fn record_event(
        &self,
        package_id: &str,
        package_token: &str,
        bucket: &str,
        key: &str,
        event_type: &str,
        fragment_index: Option<usize>,
        source_type: Option<&str>,
    ) -> Result<(), PontemeshError>;

    fn announce_peer_availability(
        &self,
        package: &AccessPackage,
        endpoint: &str,
        available_fragments: &[usize],
    ) -> Result<(), PontemeshError>;
}

pub struct HttpOriginClient {
    origin_url: String,
    application_token: String,
    http: reqwest::blocking::Client,
}

impl HttpOriginClient {
    pub fn new(config: PontemeshClientConfig) -> Self {
        Self {
            origin_url: config.origin_url.trim_end_matches('/').to_string(),
            application_token: config.application_token,
            http: reqwest::blocking::Client::builder()
                .no_proxy()
                .build()
                .expect("build HTTP client"),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.origin_url, path.trim_start_matches('/'))
    }

    fn normalize_origin_sources(&self, package: &mut AccessPackage) {
        let endpoint = self.url(&format!(
            "/pontemesh/access-packages/{}/objects/{}/{}",
            url_component(&package.id),
            url_component(&package.bucket),
            object_path(&package.key)
        ));
        for source in &mut package.authorized_sources {
            if source.source_type == SourceType::Origin {
                source.endpoint.clone_from(&endpoint);
            }
        }
        if package.fallback.source_type == "ORIGIN" {
            package.fallback.object_endpoint = endpoint;
        }
    }
}

impl OriginClient for HttpOriginClient {
    fn create_access_package(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<AccessPackage, PontemeshError> {
        let response = self
            .http
            .post(self.url("/pontemesh/access-packages"))
            .bearer_auth(&self.application_token)
            .json(&CreateAccessPackageRequest {
                bucket: bucket.to_string(),
                key: key.to_string(),
            })
            .send()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))?;

        if response.status() == reqwest::StatusCode::FORBIDDEN
            || response.status() == reqwest::StatusCode::UNAUTHORIZED
        {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(PontemeshError::AccessDenied(format_http_error(
                status.as_u16(),
                &body,
            )));
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(PontemeshError::OriginRequestFailed(format_http_error(
                status.as_u16(),
                &body,
            )));
        }
        let mut package: AccessPackage = response
            .json()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))?;
        self.normalize_origin_sources(&mut package);
        Ok(package)
    }

    fn get_manifest(&self, bucket: &str, key: &str) -> Result<Manifest, PontemeshError> {
        let bucket = url_component(bucket);
        let key = object_path(key);
        let response = self
            .http
            .get(self.url(&format!("/pontemesh/objects/{bucket}/manifest/{key}")))
            .bearer_auth(&self.application_token)
            .send()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(PontemeshError::OriginRequestFailed(format_http_error(
                status.as_u16(),
                &body,
            )));
        }
        response
            .json()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))
    }

    fn record_event(
        &self,
        package_id: &str,
        package_token: &str,
        bucket: &str,
        key: &str,
        event_type: &str,
        fragment_index: Option<usize>,
        source_type: Option<&str>,
    ) -> Result<(), PontemeshError> {
        let Some(fragment_index) = fragment_index else {
            return Ok(());
        };
        let manifest = self.get_manifest(bucket, key)?;
        let fragment = manifest
            .fragments
            .iter()
            .find(|fragment| fragment.index == fragment_index)
            .ok_or_else(|| {
                PontemeshError::OriginRequestFailed(format!(
                    "fragment {fragment_index} not found in manifest"
                ))
            })?;
        let source_type = source_type.unwrap_or("ORIGIN");
        if source_type == "PEER" {
            return Ok(());
        }
        let (event_type, outcome) = match event_type {
            "FRAGMENT_VALIDATED" => ("FRAGMENT_VALIDATED", "SUCCESS"),
            "SOURCE_FAILED" | "SOURCE_FAILURE" => ("SOURCE_FAILURE", "FAILURE"),
            "FRAGMENT_REJECTED" => ("FRAGMENT_REJECTED", "REJECTED"),
            "FALLBACK_DECISION" => ("FALLBACK_DECISION", "SUCCESS"),
            _ => return Ok(()),
        };
        let package_id = url_component(package_id);
        let bucket = url_component(bucket);
        let key = object_path(key);
        let response = self
            .http
            .post(self.url(&format!(
                "/pontemesh/access-packages/{package_id}/events/{bucket}/{key}"
            )))
            .bearer_auth(package_token)
            .json(&json!({
                "sourceType": source_type,
                "fragmentHash": fragment.sha256,
                "eventType": event_type,
                "fragmentIndex": fragment.index,
                "bytesTransferred": fragment.size_bytes,
                "outcome": outcome,
            }))
            .send()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(PontemeshError::OriginRequestFailed(format_http_error(
                status.as_u16(),
                &body,
            )));
        }
        Ok(())
    }

    fn announce_peer_availability(
        &self,
        package: &AccessPackage,
        endpoint: &str,
        available_fragments: &[usize],
    ) -> Result<(), PontemeshError> {
        let package_id = url_component(&package.id);
        let bucket = url_component(&package.bucket);
        let key = object_path(&package.key);
        let response = self
            .http
            .post(self.url(&format!(
                "/pontemesh/access-packages/{package_id}/peers/{bucket}/{key}"
            )))
            .bearer_auth(&package.package_token)
            .json(&PeerAnnouncement {
                endpoint: endpoint.to_string(),
                available_fragments: available_fragments.to_vec(),
            })
            .send()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(PontemeshError::OriginRequestFailed(format_http_error(
                status.as_u16(),
                &body,
            )));
        }
        Ok(())
    }
}

fn object_path(value: &str) -> String {
    value
        .split('/')
        .map(url_component)
        .collect::<Vec<_>>()
        .join("/")
}

fn url_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn format_http_error(status: u16, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        status.to_string()
    } else {
        format!("{status}: {body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_path_preserves_segments_and_encodes_components() {
        assert_eq!(
            object_path("objects/full stack/agent.bin"),
            "objects/full%20stack/agent.bin"
        );
    }
}
