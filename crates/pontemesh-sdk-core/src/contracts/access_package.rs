use serde::{Deserialize, Serialize};

use super::{AuthorizedSource, FallbackContract, Manifest, SourceSelectionContract};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccessPackage {
    pub id: String,
    pub package_token: String,
    pub bucket: String,
    pub key: String,
    pub version: String,
    pub manifest_id: String,
    pub expires_at: String,
    pub scope: Vec<String>,
    pub authorized_sources: Vec<AuthorizedSource>,
    pub source_selection: SourceSelectionContract,
    pub fallback: FallbackContract,
    pub manifest: Manifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccessPackageRequest {
    pub bucket: String,
    pub key: String,
}
