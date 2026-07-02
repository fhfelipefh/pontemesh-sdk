pub mod origin_client;
pub mod source_client;

pub use origin_client::{HttpOriginClient, OriginClient, PontemeshClientConfig};
pub use source_client::{HttpSourceClient, SourceClient};

use std::fs;

use crate::download::{sync_object, ProgressCallback, SyncObjectRequest};
use crate::errors::PontemeshError;
use crate::p2p::{DisabledPeerTransport, PeerTransport};
use crate::storage::MemoryStorage;

pub struct PontemeshClient {
    origin: Box<dyn OriginClient>,
    source: Box<dyn SourceClient>,
    peer: Box<dyn PeerTransport>,
}

impl PontemeshClient {
    pub fn new(config: PontemeshClientConfig) -> Self {
        Self {
            origin: Box::new(HttpOriginClient::new(config)),
            source: Box::new(HttpSourceClient::new()),
            peer: Box::new(DisabledPeerTransport),
        }
    }

    pub fn with_clients(
        origin: Box<dyn OriginClient>,
        source: Box<dyn SourceClient>,
        peer: Box<dyn PeerTransport>,
    ) -> Self {
        Self {
            origin,
            source,
            peer,
        }
    }

    pub fn sync_object(&self, request: SyncObjectRequest) -> Result<(), PontemeshError> {
        self.sync_object_with_progress(request, None)
    }

    pub fn sync_object_with_progress(
        &self,
        request: SyncObjectRequest,
        progress: Option<ProgressCallback<'_>>,
    ) -> Result<(), PontemeshError> {
        let mut storage = MemoryStorage::new();
        let bytes = sync_object(
            self.origin.as_ref(),
            self.source.as_ref(),
            self.peer.as_ref(),
            &mut storage,
            &request,
            progress,
        )?;
        if let Some(parent) = request.destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&request.destination, bytes)?;
        Ok(())
    }
}
