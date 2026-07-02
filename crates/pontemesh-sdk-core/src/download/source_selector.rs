use crate::contracts::{
    is_expired_utc, AuthorizedSource, FragmentDescriptor, SourceSelectionContract, SourceType,
};
use crate::p2p::PeerTransport;

pub struct SourceSelector<'a> {
    sources: &'a [AuthorizedSource],
    selection: &'a SourceSelectionContract,
    peer: &'a dyn PeerTransport,
}

impl<'a> SourceSelector<'a> {
    pub fn new(
        sources: &'a [AuthorizedSource],
        selection: &'a SourceSelectionContract,
        peer: &'a dyn PeerTransport,
    ) -> Self {
        Self {
            sources,
            selection,
            peer,
        }
    }

    pub fn sources_for(&self, fragment: &FragmentDescriptor) -> Vec<AuthorizedSource> {
        [
            SourceType::Peer,
            SourceType::ReplicaEdge,
            SourceType::Origin,
        ]
        .into_iter()
        .flat_map(|source_type| {
            let mut matches: Vec<_> = self
                .sources
                .iter()
                .filter(|source| self.is_allowed(source, source_type, fragment.index))
                .cloned()
                .collect();
            matches.sort_by_key(|source| source.priority);
            matches
        })
        .collect()
    }

    fn is_allowed(
        &self,
        source: &AuthorizedSource,
        source_type: SourceType,
        fragment_index: usize,
    ) -> bool {
        if source.source_type != source_type {
            return false;
        }
        if is_expired_utc(&source.expires_at) {
            return false;
        }
        if !source
            .available_fragments
            .contains(&(fragment_index as i64))
        {
            return false;
        }
        match source_type {
            SourceType::Peer => self.selection.allow_peer_sharing && self.peer.can_handle(source),
            SourceType::ReplicaEdge => self.selection.allow_replica_edge,
            SourceType::Origin => true,
        }
    }
}
