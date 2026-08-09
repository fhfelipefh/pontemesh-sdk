use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::contracts::{FragmentDescriptor, Manifest};
use crate::errors::PontemeshError;
use crate::integrity::{sha256_hex, validate_fragment};

use super::{FragmentState, StorageAdapter};

pub struct FilesystemStorage {
    root: PathBuf,
}

impl FilesystemStorage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path(&self, manifest: &Manifest, fragment: &FragmentDescriptor) -> PathBuf {
        self.root
            .join(sha256_hex(manifest.manifest_id.as_bytes()))
            .join(format!("{}.fragment", fragment.index))
    }
}

impl StorageAdapter for FilesystemStorage {
    fn fragment_state(&self, manifest: &Manifest, fragment: &FragmentDescriptor) -> FragmentState {
        self.read_fragment(manifest, fragment)
            .ok()
            .flatten()
            .filter(|bytes| validate_fragment(fragment, bytes).is_ok())
            .map_or(FragmentState::Missing, |_| FragmentState::Validated)
    }

    fn read_fragment(
        &self,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
    ) -> Result<Option<Vec<u8>>, PontemeshError> {
        let path = self.path(manifest, fragment);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(fs::read(path)?))
    }

    fn write_validated_fragment(
        &mut self,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        bytes: &[u8],
    ) -> Result<(), PontemeshError> {
        let path = self.path(manifest, fragment);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let parent = path.parent().ok_or_else(|| {
            PontemeshError::InvalidArgument("fragment cache path has no parent".to_string())
        })?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        temporary
            .persist(path)
            .map_err(|error| PontemeshError::Io(error.error))?;
        Ok(())
    }

    fn assemble(&self, manifest: &Manifest) -> Result<Vec<u8>, PontemeshError> {
        let mut output = Vec::new();
        let mut fragments = manifest.fragments.clone();
        fragments.sort_by_key(|fragment| fragment.index);
        for fragment in fragments {
            let bytes = self.read_fragment(manifest, &fragment)?.ok_or_else(|| {
                PontemeshError::Internal(format!("fragment {} is missing", fragment.index))
            })?;
            output.extend_from_slice(&bytes);
        }
        Ok(output)
    }

    fn assemble_into(
        &self,
        manifest: &Manifest,
        writer: &mut dyn Write,
    ) -> Result<u64, PontemeshError> {
        let mut total = 0_u64;
        let mut fragments = manifest.fragments.clone();
        fragments.sort_by_key(|fragment| fragment.index);
        for fragment in fragments {
            let bytes = self.read_fragment(manifest, &fragment)?.ok_or_else(|| {
                PontemeshError::Internal(format!("fragment {} is missing", fragment.index))
            })?;
            writer.write_all(&bytes)?;
            total += bytes.len() as u64;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrity::sha256_hex;

    fn manifest(bytes: &[u8]) -> Manifest {
        Manifest {
            manifest_id: "manifest/unsafe".to_string(),
            object_id: "object".to_string(),
            bucket: "updates".to_string(),
            key: "game.bin".to_string(),
            version: "1".to_string(),
            total_size_bytes: bytes.len() as i64,
            content_type: "application/octet-stream".to_string(),
            object_hash_algorithm: "SHA256".to_string(),
            object_sha256: sha256_hex(bytes),
            fragment_size_bytes: bytes.len(),
            fragments: vec![FragmentDescriptor {
                index: 0,
                fragment_id: "fragment".to_string(),
                byte_range_start: 0,
                byte_range_end: bytes.len().saturating_sub(1) as u64,
                size_bytes: bytes.len(),
                hash_algorithm: "SHA256".to_string(),
                sha256: sha256_hex(bytes),
                priority: "NORMAL".to_string(),
                fallback_range_header: format!("bytes=0-{}", bytes.len().saturating_sub(1)),
            }],
            availability_state: "AVAILABLE".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn corrupt_cached_fragment_is_not_treated_as_validated() {
        let directory = tempfile::tempdir().expect("cache directory");
        let mut storage = FilesystemStorage::new(directory.path().to_path_buf());
        let manifest = manifest(b"expected");
        let fragment = &manifest.fragments[0];
        storage
            .write_validated_fragment(&manifest, fragment, b"expected")
            .expect("write fragment");
        fs::write(storage.path(&manifest, fragment), b"corrupt").expect("corrupt cache");

        assert_eq!(
            storage.fragment_state(&manifest, fragment),
            FragmentState::Missing
        );
    }
}
