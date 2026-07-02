use std::collections::HashMap;

use crate::contracts::{FragmentDescriptor, Manifest};
use crate::errors::PontemeshError;

use super::{FragmentState, StorageAdapter};

#[derive(Default)]
pub struct MemoryStorage {
    fragments: HashMap<String, Vec<u8>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(manifest: &Manifest, fragment: &FragmentDescriptor) -> String {
        format!("{}:{}", manifest.manifest_id, fragment.index)
    }
}

impl StorageAdapter for MemoryStorage {
    fn fragment_state(&self, manifest: &Manifest, fragment: &FragmentDescriptor) -> FragmentState {
        if self.fragments.contains_key(&Self::key(manifest, fragment)) {
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
        Ok(self.fragments.get(&Self::key(manifest, fragment)).cloned())
    }

    fn write_validated_fragment(
        &mut self,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        bytes: &[u8],
    ) -> Result<(), PontemeshError> {
        self.fragments
            .insert(Self::key(manifest, fragment), bytes.to_vec());
        Ok(())
    }

    fn assemble(&self, manifest: &Manifest) -> Result<Vec<u8>, PontemeshError> {
        let mut output = Vec::new();
        let mut fragments = manifest.fragments.clone();
        fragments.sort_by_key(|fragment| fragment.index);
        for fragment in fragments {
            let bytes = self
                .fragments
                .get(&Self::key(manifest, &fragment))
                .ok_or_else(|| {
                    PontemeshError::Internal(format!("fragment {} is missing", fragment.index))
                })?;
            output.extend_from_slice(bytes);
        }
        Ok(output)
    }
}
