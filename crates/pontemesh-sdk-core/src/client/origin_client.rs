use serde_json::json;

use crate::contracts::{AccessPackage, CreateAccessPackageRequest, Manifest};
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
            http: reqwest::blocking::Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.origin_url, path.trim_start_matches('/'))
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
            return Err(PontemeshError::AccessDenied(response.status().to_string()));
        }
        if !response.status().is_success() {
            return Err(PontemeshError::OriginRequestFailed(
                response.status().to_string(),
            ));
        }
        response
            .json()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))
    }

    fn get_manifest(&self, bucket: &str, key: &str) -> Result<Manifest, PontemeshError> {
        let bucket = urlencoding::encode(bucket);
        let key = urlencoding::encode(key);
        let response = self
            .http
            .get(self.url(&format!("/pontemesh/objects/{bucket}/manifest/{key}")))
            .bearer_auth(&self.application_token)
            .send()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))?;
        if !response.status().is_success() {
            return Err(PontemeshError::OriginRequestFailed(
                response.status().to_string(),
            ));
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
        let package_id = urlencoding::encode(package_id);
        let bucket = urlencoding::encode(bucket);
        let key = urlencoding::encode(key);
        let response = self
            .http
            .post(self.url(&format!(
                "/pontemesh/access-packages/{package_id}/events/{bucket}/{key}"
            )))
            .bearer_auth(package_token)
            .json(&json!({
                "eventType": event_type,
                "fragmentIndex": fragment_index,
                "sourceType": source_type,
            }))
            .send()
            .map_err(|error| PontemeshError::OriginRequestFailed(error.to_string()))?;
        if !response.status().is_success() {
            return Err(PontemeshError::OriginRequestFailed(
                response.status().to_string(),
            ));
        }
        Ok(())
    }

    fn announce_peer_availability(
        &self,
        package: &AccessPackage,
        endpoint: &str,
        available_fragments: &[usize],
    ) -> Result<(), PontemeshError> {
        let package_id = urlencoding::encode(&package.id);
        let bucket = urlencoding::encode(&package.bucket);
        let key = urlencoding::encode(&package.key);
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
            return Err(PontemeshError::OriginRequestFailed(
                response.status().to_string(),
            ));
        }
        Ok(())
    }
}
