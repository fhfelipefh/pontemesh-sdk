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
        && !value.contains('\0')
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VersionScheme {
    Disabled,
    Semver,
    BuildNumber,
    Channel,
    Tag,
}

impl VersionScheme {
    pub fn parse(value: &str) -> Result<Self, PontemeshError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DISABLED" => Ok(VersionScheme::Disabled),
            "SEMVER" => Ok(VersionScheme::Semver),
            "BUILD_NUMBER" => Ok(VersionScheme::BuildNumber),
            "CHANNEL" => Ok(VersionScheme::Channel),
            "TAG" => Ok(VersionScheme::Tag),
            other => Err(PontemeshError::InvalidArgument(format!(
                "unsupported release versioning scheme: {other}"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            VersionScheme::Disabled => "DISABLED",
            VersionScheme::Semver => "SEMVER",
            VersionScheme::BuildNumber => "BUILD_NUMBER",
            VersionScheme::Channel => "CHANNEL",
            VersionScheme::Tag => "TAG",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemverParts {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

pub fn parse_semver(raw: &str) -> Option<SemverParts> {
    let raw = raw.trim();
    let s = raw
        .strip_prefix('v')
        .or_else(|| raw.strip_prefix('V'))
        .unwrap_or(raw);
    if s.is_empty() {
        return None;
    }
    let (ver_part, pre) = match s.split_once('-') {
        Some((v, p)) => {
            if p.trim().is_empty() {
                return None;
            }
            (v, Some(p.trim().to_owned()))
        }
        None => (s, None),
    };
    let parts: Vec<&str> = ver_part.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = if parts.len() > 1 {
        parts[1].parse::<u64>().ok()?
    } else {
        0
    };
    let patch = if parts.len() > 2 {
        parts[2].parse::<u64>().ok()?
    } else {
        0
    };
    Some(SemverParts {
        major,
        minor,
        patch,
        pre,
    })
}

pub fn compare_semver(a: &SemverParts, b: &SemverParts) -> std::cmp::Ordering {
    match (a.major, a.minor, a.patch).cmp(&(b.major, b.minor, b.patch)) {
        std::cmp::Ordering::Equal => match (&a.pre, &b.pre) {
            (None, None) => std::cmp::Ordering::Equal,
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(pre_a), Some(pre_b)) => pre_a.cmp(pre_b),
        },
        other => other,
    }
}

pub fn parse_build_number(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    let s = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .or_else(|| trimmed.strip_prefix('#'))
        .or_else(|| trimmed.strip_prefix('b'))
        .or_else(|| trimmed.strip_prefix('B'))
        .unwrap_or(trimmed);
    s.parse::<u64>().ok()
}

pub fn channel_rank(channel: &str) -> u32 {
    match channel.to_ascii_lowercase().as_str() {
        "stable" => 100,
        "latest" => 90,
        "rc" | "release-candidate" => 80,
        "beta" => 50,
        "alpha" => 30,
        "dev" => 20,
        "nightly" => 10,
        _ => 0,
    }
}

pub fn extract_version_from_key(object_key: &str, software_id: &str) -> Option<String> {
    let trimmed_key = object_key.trim();
    let trimmed_software = software_id.trim();
    if trimmed_key.is_empty() || trimmed_software.is_empty() {
        return None;
    }

    let remainder = {
        let suffix = trimmed_key.strip_prefix(trimmed_software)?;
        suffix
            .strip_prefix('/')
            .or_else(|| suffix.strip_prefix('-'))
            .unwrap_or(suffix)
    };

    if remainder.is_empty() {
        return None;
    }

    let version_part = if let Some((dir, _)) = remainder.split_once('/') {
        dir
    } else if let Some(stripped) = remainder
        .strip_suffix(".tar.gz")
        .or_else(|| remainder.strip_suffix(".tar.xz"))
        .or_else(|| remainder.strip_suffix(".tar.bz2"))
    {
        stripped
    } else if let Some((stem, ext)) = remainder.rsplit_once('.') {
        if !stem.is_empty() && ext.chars().any(|c| c.is_alphabetic()) {
            stem
        } else {
            remainder
        }
    } else {
        remainder
    };

    let cleaned = version_part.trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_owned())
    }
}

pub fn is_newer(
    latest: &str,
    current: &str,
    scheme: VersionScheme,
) -> Result<bool, PontemeshError> {
    if scheme == VersionScheme::Disabled {
        return Err(PontemeshError::InvalidArgument(
            "release versioning scheme is disabled".to_string(),
        ));
    }
    if latest.trim() == current.trim() {
        return Ok(false);
    }
    match scheme {
        VersionScheme::Semver => {
            let parsed_latest = parse_semver(latest).ok_or_else(|| {
                PontemeshError::InvalidArgument(format!(
                    "invalid semver in latest release: {latest}"
                ))
            })?;
            let parsed_current = parse_semver(current).ok_or_else(|| {
                PontemeshError::InvalidArgument(format!(
                    "invalid semver in current version: {current}"
                ))
            })?;
            Ok(compare_semver(&parsed_latest, &parsed_current) == std::cmp::Ordering::Greater)
        }
        VersionScheme::BuildNumber => {
            let parsed_latest = parse_build_number(latest).ok_or_else(|| {
                PontemeshError::InvalidArgument(format!(
                    "invalid build number in latest release: {latest}"
                ))
            })?;
            let parsed_current = parse_build_number(current).ok_or_else(|| {
                PontemeshError::InvalidArgument(format!(
                    "invalid build number in current version: {current}"
                ))
            })?;
            Ok(parsed_latest > parsed_current)
        }
        VersionScheme::Channel => {
            let rank_latest = channel_rank(latest);
            let rank_current = channel_rank(current);
            if rank_latest != rank_current {
                Ok(rank_latest > rank_current)
            } else {
                Ok(latest.trim() != current.trim())
            }
        }
        VersionScheme::Tag => Ok(latest.trim() != current.trim()),
        VersionScheme::Disabled => unreachable!(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckRequest {
    pub bucket: String,
    pub software_id: String,
    pub current_version: Option<String>,
    pub channel: Option<String>,
}

impl UpdateCheckRequest {
    pub fn new(bucket: impl Into<String>, software_id: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            software_id: software_id.into(),
            current_version: None,
            channel: None,
        }
    }

    pub fn with_current_version(mut self, version: impl Into<String>) -> Self {
        self.current_version = Some(version.into());
        self
    }

    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SoftwareUpdateInfo {
    pub bucket: String,
    pub software_id: String,
    pub versioning_scheme: String,
    pub current_version: Option<String>,
    pub latest_version: String,
    pub has_update: bool,
    #[serde(alias = "latestKey", alias = "target_object_key")]
    pub target_object_key: String,
    #[serde(alias = "size_bytes")]
    pub size_bytes: i64,
    #[serde(default, alias = "manifest_id")]
    pub manifest_id: Option<String>,
    #[serde(default)]
    pub mandatory: bool,
}

impl SoftwareUpdateInfo {
    pub fn to_sync_request(
        &self,
        destination: impl Into<std::path::PathBuf>,
    ) -> crate::download::SyncObjectRequest {
        crate::download::SyncObjectRequest {
            bucket: self.bucket.clone(),
            key: self.target_object_key.clone(),
            destination: destination.into(),
        }
    }
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

    #[test]
    fn semver_parsing_and_comparison_rules() {
        let v1 = parse_semver("1.0.0").expect("v1");
        let v2 = parse_semver("v1.0.1").expect("v2");
        let v3 = parse_semver("1.1.0").expect("v3");
        let v4 = parse_semver("2.0.0").expect("v4");
        let v_beta = parse_semver("1.0.0-beta.1").expect("v_beta");

        assert_eq!(compare_semver(&v1, &v1), std::cmp::Ordering::Equal);
        assert_eq!(compare_semver(&v1, &v2), std::cmp::Ordering::Less);
        assert_eq!(compare_semver(&v2, &v1), std::cmp::Ordering::Greater);
        assert_eq!(compare_semver(&v2, &v3), std::cmp::Ordering::Less);
        assert_eq!(compare_semver(&v3, &v4), std::cmp::Ordering::Less);
        assert_eq!(compare_semver(&v_beta, &v1), std::cmp::Ordering::Less);
        assert_eq!(compare_semver(&v1, &v_beta), std::cmp::Ordering::Greater);
    }

    #[test]
    fn is_newer_all_supported_schemes() {
        assert!(is_newer("1.1.0", "1.0.0", VersionScheme::Semver).unwrap());
        assert!(!is_newer("1.0.0", "1.1.0", VersionScheme::Semver).unwrap());
        assert!(!is_newer("1.0.0", "1.0.0", VersionScheme::Semver).unwrap());

        assert!(is_newer("1050", "1040", VersionScheme::BuildNumber).unwrap());
        assert!(!is_newer("1040", "1050", VersionScheme::BuildNumber).unwrap());
        assert!(is_newer("v200", "199", VersionScheme::BuildNumber).unwrap());

        assert!(is_newer("stable", "beta", VersionScheme::Channel).unwrap());
        assert!(is_newer("latest", "nightly", VersionScheme::Channel).unwrap());
        assert!(!is_newer("alpha", "stable", VersionScheme::Channel).unwrap());

        assert!(is_newer("hash-b", "hash-a", VersionScheme::Tag).unwrap());
        assert!(!is_newer("same-tag", "same-tag", VersionScheme::Tag).unwrap());

        assert!(is_newer("1.0.0", "1.0.0", VersionScheme::Disabled).is_err());
    }

    #[test]
    fn extracts_version_from_various_key_patterns() {
        assert_eq!(
            extract_version_from_key("game/v1.0.0/launcher.zip", "game"),
            Some("v1.0.0".to_string())
        );
        assert_eq!(
            extract_version_from_key("game-1.2.3.tar.gz", "game"),
            Some("1.2.3".to_string())
        );
        assert_eq!(
            extract_version_from_key("mygame/1050/bin/game.exe", "mygame"),
            Some("1050".to_string())
        );
        assert_eq!(
            extract_version_from_key("other/1.0.0/file.bin", "game"),
            None
        );
    }

    #[test]
    fn parses_software_update_info_with_aliases() {
        let json_target_key = r#"{
            "bucket": "games",
            "softwareId": "mygame",
            "versioningScheme": "SEMVER",
            "currentVersion": "1.0.0",
            "latestVersion": "1.1.0",
            "hasUpdate": true,
            "targetObjectKey": "mygame/releases/1.1.0/game.zip",
            "sizeBytes": 1024,
            "manifestId": "m-1",
            "mandatory": true
        }"#;

        let info: SoftwareUpdateInfo =
            serde_json::from_str(json_target_key).expect("parse targetObjectKey");
        assert_eq!(info.software_id, "mygame");
        assert_eq!(info.target_object_key, "mygame/releases/1.1.0/game.zip");
        assert_eq!(info.size_bytes, 1024);
        assert!(info.has_update);
        assert!(info.mandatory);

        let json_latest_key = r#"{
            "bucket": "games",
            "softwareId": "mygame",
            "versioningScheme": "SEMVER",
            "currentVersion": "1.0.0",
            "latestVersion": "1.1.0",
            "hasUpdate": true,
            "latestKey": "mygame/releases/1.1.0/game.zip",
            "size_bytes": 1024
        }"#;

        let info2: SoftwareUpdateInfo =
            serde_json::from_str(json_latest_key).expect("parse latestKey");
        assert_eq!(info2.target_object_key, "mygame/releases/1.1.0/game.zip");
        assert!(!info2.mandatory);
    }
}
