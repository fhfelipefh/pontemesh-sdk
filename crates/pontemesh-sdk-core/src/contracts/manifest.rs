use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub manifest_id: String,
    pub object_id: String,
    pub bucket: String,
    pub key: String,
    pub version: String,
    pub total_size_bytes: i64,
    pub content_type: String,
    pub object_hash_algorithm: String,
    pub object_sha256: String,
    pub fragment_size_bytes: usize,
    pub fragments: Vec<FragmentDescriptor>,
    pub availability_state: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FragmentDescriptor {
    pub index: usize,
    pub fragment_id: String,
    pub byte_range_start: u64,
    pub byte_range_end: u64,
    pub size_bytes: usize,
    pub hash_algorithm: String,
    pub sha256: String,
    pub priority: String,
    pub fallback_range_header: String,
}
