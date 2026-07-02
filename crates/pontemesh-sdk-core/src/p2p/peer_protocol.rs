use std::io::{BufRead, Read};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

use crate::errors::PontemeshError;
use crate::integrity::sha256_hex;

pub const P2P_PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FragmentRequest {
    #[serde(rename = "type")]
    pub message_type: String,
    pub protocol_version: u32,
    pub package_id: String,
    pub bucket: String,
    pub key: String,
    pub manifest_id: String,
    pub fragment_id: String,
    pub fragment_index: usize,
    pub byte_range_start: u64,
    pub byte_range_end: u64,
    pub request_nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FragmentResponse {
    #[serde(rename = "type")]
    pub message_type: String,
    pub protocol_version: u32,
    pub package_id: String,
    pub manifest_id: String,
    pub fragment_id: String,
    pub fragment_index: usize,
    pub size_bytes: usize,
    pub sha256: String,
    pub request_nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    pub bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerProtocolError {
    #[serde(rename = "type")]
    pub message_type: String,
    pub code: String,
    pub message: String,
}

impl FragmentResponse {
    pub fn from_bytes(
        package_id: &str,
        manifest_id: &str,
        fragment_id: &str,
        fragment_index: usize,
        request_nonce: &str,
        peer_id: Option<&str>,
        bytes: &[u8],
    ) -> Self {
        Self {
            message_type: "fragmentResponse".to_string(),
            protocol_version: P2P_PROTOCOL_VERSION,
            package_id: package_id.to_string(),
            manifest_id: manifest_id.to_string(),
            fragment_id: fragment_id.to_string(),
            fragment_index,
            size_bytes: bytes.len(),
            sha256: sha256_hex(bytes),
            request_nonce: request_nonce.to_string(),
            peer_id: peer_id.map(ToOwned::to_owned),
            bytes_base64: STANDARD.encode(bytes),
        }
    }

    pub fn decode_bytes(&self) -> Result<Vec<u8>, PontemeshError> {
        STANDARD
            .decode(&self.bytes_base64)
            .map_err(|error| PontemeshError::Internal(format!("invalid peer payload: {error}")))
    }
}

pub fn read_limited_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<String, PontemeshError> {
    let mut bytes = Vec::new();
    let read = reader
        .take((max_bytes + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if read == 0 {
        return Err(PontemeshError::NoSourceAvailable);
    }
    if bytes.len() > max_bytes {
        return Err(PontemeshError::InvalidArgument(
            "peer frame exceeds maximum size".to_string(),
        ));
    }
    String::from_utf8(bytes)
        .map_err(|error| PontemeshError::InvalidArgument(format!("invalid peer utf8: {error}")))
}

pub fn request_nonce() -> String {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{pid:x}{nanos:x}")
}
