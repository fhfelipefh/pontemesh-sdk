use crate::contracts::{AccessPackage, AuthorizedSource, FragmentDescriptor, SourceType};
use crate::errors::PontemeshError;

pub trait SourceClient: Send + Sync {
    fn download_fragment(
        &self,
        package: &AccessPackage,
        source: &AuthorizedSource,
        fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError>;
}

pub struct HttpSourceClient {
    http: reqwest::blocking::Client,
}

impl HttpSourceClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::blocking::Client::builder()
                .no_proxy()
                .build()
                .expect("build HTTP client"),
        }
    }
}

impl Default for HttpSourceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceClient for HttpSourceClient {
    fn download_fragment(
        &self,
        package: &AccessPackage,
        source: &AuthorizedSource,
        fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError> {
        let response = self
            .http
            .get(&source.endpoint)
            .bearer_auth(&package.package_token)
            .header(reqwest::header::RANGE, &fragment.fallback_range_header)
            .send()
            .map_err(|error| source_error(source.source_type, error.to_string()))?;
        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(source_error(
                source.source_type,
                response.status().to_string(),
            ));
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| source_error(source.source_type, error.to_string()))
    }
}

fn source_error(source_type: SourceType, message: String) -> PontemeshError {
    match source_type {
        SourceType::Origin => PontemeshError::OriginRequestFailed(message),
        SourceType::ReplicaEdge => PontemeshError::NoSourceAvailable,
        SourceType::Peer => PontemeshError::PeerTransportNotEnabled,
    }
}
