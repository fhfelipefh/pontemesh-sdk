use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPolicyContract {
    pub allow_peer_sharing: bool,
    pub allow_replica_edge: bool,
    pub require_integrity_validation: bool,
}
