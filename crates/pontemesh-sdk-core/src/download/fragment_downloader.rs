use crate::client::SourceClient;
use crate::contracts::{AccessPackage, AuthorizedSource, FragmentDescriptor, SourceType};
use crate::errors::PontemeshError;
use crate::p2p::PeerTransport;

pub fn download_fragment(
    package: &AccessPackage,
    source_client: &dyn SourceClient,
    peer: &dyn PeerTransport,
    source: &AuthorizedSource,
    fragment: &FragmentDescriptor,
) -> Result<Vec<u8>, PontemeshError> {
    match source.source_type {
        SourceType::Peer => peer.download_fragment(source, fragment, &package.package_token),
        SourceType::ReplicaEdge | SourceType::Origin => {
            source_client.download_fragment(package, source, fragment)
        }
    }
}
