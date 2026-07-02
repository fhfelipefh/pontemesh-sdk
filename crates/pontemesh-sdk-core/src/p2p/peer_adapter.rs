use crate::contracts::{AuthorizedSource, FragmentDescriptor};
use crate::errors::PontemeshError;

pub trait PeerTransport: Send + Sync {
    fn can_handle(&self, source: &AuthorizedSource) -> bool;

    fn download_fragment(
        &self,
        source: &AuthorizedSource,
        fragment: &FragmentDescriptor,
        package_token: &str,
    ) -> Result<Vec<u8>, PontemeshError>;
}
