use crate::contracts::{FragmentDescriptor, Manifest};
use crate::errors::PontemeshError;
use std::io::Write;

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
    fn assemble_into(
        &self,
        manifest: &Manifest,
        writer: &mut dyn Write,
    ) -> Result<u64, PontemeshError> {
        let bytes = self.assemble(manifest)?;
        writer.write_all(&bytes)?;
        Ok(bytes.len() as u64)
    }
}
