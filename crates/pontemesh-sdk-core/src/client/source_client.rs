use std::io::Read;

use crate::contracts::{AccessPackage, AuthorizedSource, FragmentDescriptor, SourceType};
use crate::errors::PontemeshError;

const MAX_ERROR_RESPONSE_BYTES: usize = 16 * 1024;

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
            let status = response.status();
            let (body, truncated) = read_body_limited(response, MAX_ERROR_RESPONSE_BYTES)
                .unwrap_or_else(|_| (Vec::new(), false));
            let mut body = String::from_utf8_lossy(&body).into_owned();
            if truncated {
                body.push_str(" [response body truncated]");
            }
            return Err(source_error(
                source.source_type,
                format_http_error(status.as_u16(), &body),
            ));
        }
        let declared_size = u64::try_from(fragment.size_bytes).unwrap_or(u64::MAX);
        if response
            .content_length()
            .is_some_and(|length| length > declared_size)
        {
            return Err(source_error(
                source.source_type,
                "response exceeds declared fragment size".to_string(),
            ));
        }
        let (bytes, truncated) = read_body_limited(response, fragment.size_bytes)
            .map_err(|error| source_error(source.source_type, error))?;
        if truncated {
            return Err(source_error(
                source.source_type,
                "response exceeds declared fragment size".to_string(),
            ));
        }
        Ok(bytes)
    }
}

fn read_body_limited(
    response: reqwest::blocking::Response,
    max_bytes: usize,
) -> Result<(Vec<u8>, bool), String> {
    let limit = u64::try_from(max_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    response
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let truncated = bytes.len() > max_bytes;
    if truncated {
        bytes.truncate(max_bytes);
    }
    Ok((bytes, truncated))
}

fn format_http_error(status: u16, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        status.to_string()
    } else {
        format!("{status}: {body}")
    }
}

fn source_error(source_type: SourceType, message: String) -> PontemeshError {
    match source_type {
        SourceType::Origin => PontemeshError::OriginRequestFailed(message),
        SourceType::ReplicaEdge => PontemeshError::NoSourceAvailable,
        SourceType::Peer => PontemeshError::PeerTransportNotEnabled,
    }
}
