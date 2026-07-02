use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FallbackContract {
    #[serde(default = "default_source_type")]
    pub source_type: String,
    #[serde(default)]
    pub object_endpoint: String,
    #[serde(default = "default_true")]
    pub supports_range: bool,
    #[serde(default = "default_true")]
    pub preserve_validated_fragments: bool,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub revalidate_endpoint: Option<String>,
}

impl Default for FallbackContract {
    fn default() -> Self {
        Self {
            source_type: default_source_type(),
            object_endpoint: String::new(),
            supports_range: true,
            preserve_validated_fragments: true,
            mode: default_mode(),
            revalidate_endpoint: None,
        }
    }
}

fn default_source_type() -> String {
    "ORIGIN".to_string()
}

fn default_mode() -> String {
    "RANGE".to_string()
}

fn default_true() -> bool {
    true
}
