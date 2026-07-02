use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceType {
    Origin,
    ReplicaEdge,
    Peer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedSource {
    pub id: String,
    pub source_type: SourceType,
    pub endpoint: String,
    pub priority: u8,
    pub expires_at: String,
    pub available_fragments: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceSelectionContract {
    pub allow_peer_sharing: bool,
    pub allow_replica_edge: bool,
    pub failure_threshold: u32,
}

impl Default for SourceSelectionContract {
    fn default() -> Self {
        Self {
            allow_peer_sharing: true,
            allow_replica_edge: true,
            failure_threshold: 2,
        }
    }
}
