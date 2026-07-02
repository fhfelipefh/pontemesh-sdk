use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityContract {
    pub state: String,
    pub available_fragments: Vec<usize>,
}
