use crate::contracts::{FragmentDescriptor, Manifest};
use crate::errors::PontemeshError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentState {
    Missing,
    Validated,
}

pub trait StorageAdapter: Send {
    fn fragment_state(&self, manifest: &Manifest, fragment: &FragmentDescriptor) -> FragmentState;
    fn read_fragment(
        &self,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
    ) -> Result<Option<Vec<u8>>, PontemeshError>;
    fn write_validated_fragment(
        &mut self,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        bytes: &[u8],
    ) -> Result<(), PontemeshError>;
    fn assemble(&self, manifest: &Manifest) -> Result<Vec<u8>, PontemeshError>;
}
