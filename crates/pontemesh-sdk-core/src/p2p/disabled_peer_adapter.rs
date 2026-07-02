use crate::contracts::{AuthorizedSource, FragmentDescriptor, SourceType};
use crate::errors::PontemeshError;

use super::PeerTransport;

pub struct DisabledPeerTransport;

impl PeerTransport for DisabledPeerTransport {
    fn can_handle(&self, source: &AuthorizedSource) -> bool {
        source.source_type == SourceType::Peer
    }

    fn download_fragment(
        &self,
        _source: &AuthorizedSource,
        _fragment: &FragmentDescriptor,
        _package_token: &str,
    ) -> Result<Vec<u8>, PontemeshError> {
        Err(PontemeshError::PeerTransportNotEnabled)
    }
}
