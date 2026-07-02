use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FallbackContract {
    pub enabled: bool,
    pub preserve_validated_fragments: bool,
}

impl Default for FallbackContract {
    fn default() -> Self {
        Self {
            enabled: true,
            preserve_validated_fragments: true,
        }
    }
}
