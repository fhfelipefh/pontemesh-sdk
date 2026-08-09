use std::collections::HashSet;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::errors::PontemeshError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub product: String,
    pub version: String,
    pub files: Vec<ReleaseFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseFile {
    pub bucket: String,
    pub key: String,
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub order: u32,
}

impl ReleaseManifest {
    pub fn from_json(bytes: &[u8]) -> Result<Self, PontemeshError> {
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| PontemeshError::InvalidArgument(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), PontemeshError> {
        if self.schema_version != 1 {
            return Err(PontemeshError::InvalidArgument(format!(
                "unsupported release manifest schema: {}",
                self.schema_version
            )));
        }
        if self.product.trim().is_empty() || self.version.trim().is_empty() || self.files.is_empty()
        {
            return Err(PontemeshError::InvalidArgument(
                "release product, version, and files are required".to_string(),
            ));
        }

        let mut paths = HashSet::new();
        let mut total_size = 0_u64;
        for file in &self.files {
            if file.bucket.trim().is_empty() || file.key.trim().is_empty() || file.size_bytes == 0 {
                return Err(PontemeshError::InvalidArgument(
                    "release files require bucket, key, and a non-zero size".to_string(),
                ));
            }
            if !is_safe_relative_path(&file.path) {
                return Err(PontemeshError::InvalidArgument(format!(
                    "unsafe release path: {}",
                    file.path
                )));
            }
            if !is_sha256(&file.sha256) {
                return Err(PontemeshError::InvalidArgument(format!(
                    "invalid release sha256 for {}",
                    file.path
                )));
            }
            if !paths.insert(file.path.to_lowercase()) {
                return Err(PontemeshError::InvalidArgument(format!(
                    "duplicate or case-conflicting release path: {}",
                    file.path
                )));
            }
            total_size = total_size.checked_add(file.size_bytes).ok_or_else(|| {
                PontemeshError::InvalidArgument("release size exceeds u64".to_string())
            })?;
        }
        Ok(())
    }

    pub fn files_in_install_order(&self) -> Vec<&ReleaseFile> {
        let mut files = self.files.iter().collect::<Vec<_>>();
        files.sort_by_key(|file| (file.order, &file.path));
        files
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.files
            .iter()
            .fold(0_u64, |total, file| total.saturating_add(file.size_bytes))
    }
}

fn is_safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !value.contains('\\')
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, order: u32) -> ReleaseFile {
        ReleaseFile {
            bucket: "updates".to_string(),
            key: format!("releases/{path}"),
            path: path.to_string(),
            size_bytes: 10,
            sha256: "a".repeat(64),
            order,
        }
    }

    #[test]
    fn rejects_paths_that_escape_the_install_root() {
        let manifest = ReleaseManifest {
            schema_version: 1,
            product: "game".to_string(),
            version: "1.0.0".to_string(),
            files: vec![file("../game.exe", 1)],
        };

        assert!(manifest.validate().is_err());
    }

    #[test]
    fn sorts_files_by_declared_install_order() {
        let manifest = ReleaseManifest {
            schema_version: 1,
            product: "game".to_string(),
            version: "1.0.0".to_string(),
            files: vec![file("data.pak", 20), file("launcher.bin", 10)],
        };

        let paths = manifest
            .files_in_install_order()
            .into_iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["launcher.bin", "data.pak"]);
    }

    #[test]
    fn rejects_platform_dependent_and_case_conflicting_paths() {
        let platform_dependent = ReleaseManifest {
            schema_version: 1,
            product: "game".to_string(),
            version: "1.0.0".to_string(),
            files: vec![file("bin\\game.exe", 1)],
        };
        assert!(platform_dependent.validate().is_err());

        let case_conflict = ReleaseManifest {
            schema_version: 1,
            product: "game".to_string(),
            version: "1.0.0".to_string(),
            files: vec![file("Game.exe", 1), file("game.exe", 2)],
        };
        assert!(case_conflict.validate().is_err());
    }

    #[test]
    fn rejects_release_size_overflow() {
        let mut first = file("first.bin", 1);
        first.size_bytes = u64::MAX;
        let manifest = ReleaseManifest {
            schema_version: 1,
            product: "game".to_string(),
            version: "1.0.0".to_string(),
            files: vec![first, file("second.bin", 2)],
        };

        assert!(manifest.validate().is_err());
    }
}
