use std::fs;
use std::path::PathBuf;

use crate::contracts::{FragmentDescriptor, Manifest};
use crate::errors::PontemeshError;

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
            .join(&manifest.manifest_id)
            .join(format!("{}.fragment", fragment.index))
    }
}

impl StorageAdapter for FilesystemStorage {
    fn fragment_state(&self, manifest: &Manifest, fragment: &FragmentDescriptor) -> FragmentState {
        if self.path(manifest, fragment).exists() {
            FragmentState::Validated
        } else {
            FragmentState::Missing
        }
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
        fs::write(path, bytes)?;
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
}
