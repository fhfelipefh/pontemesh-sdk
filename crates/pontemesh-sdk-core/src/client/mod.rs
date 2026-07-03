pub mod origin_client;
pub mod source_client;

pub use origin_client::{HttpOriginClient, OriginClient, PontemeshClientConfig};
pub use source_client::{HttpSourceClient, SourceClient};

use std::fs;

use crate::download::{sync_object, ProgressCallback, SyncObjectRequest};
use crate::errors::PontemeshError;
use crate::p2p::{DisabledPeerTransport, Libp2pTransport, P2pTransportKind, PeerTransport};
use crate::storage::MemoryStorage;

pub struct PontemeshClient {
    origin: Box<dyn OriginClient>,
    source: Box<dyn SourceClient>,
    peer: Box<dyn PeerTransport>,
}

impl PontemeshClient {
    pub fn new(config: PontemeshClientConfig) -> Result<Self, PontemeshError> {
        let peer: Box<dyn PeerTransport> = if config.p2p.enabled {
            let started: Result<Box<dyn PeerTransport>, PontemeshError> = match config.p2p.transport
            {
                P2pTransportKind::Libp2p => {
                    Libp2pTransport::start(&config.p2p.listen_addrs, &config.p2p.announce_addrs)
                        .map(|peer| Box::new(peer) as Box<dyn PeerTransport>)
                }
                P2pTransportKind::Disabled => Err(PontemeshError::PeerTransportNotEnabled),
            };
            match started {
                Ok(peer) => peer,
                Err(error) if config.p2p.required => return Err(error),
                Err(error) => {
                    eprintln!(
                        "pontemesh-sdk: P2P transport disabled after startup failure: {error}"
                    );
                    Box::new(DisabledPeerTransport)
                }
            }
        } else if config.p2p.required {
            return Err(PontemeshError::PeerTransportNotEnabled);
        } else {
            Box::new(DisabledPeerTransport)
        };
        Ok(Self {
            origin: Box::new(HttpOriginClient::new(config)),
            source: Box::new(HttpSourceClient::new()),
            peer,
        })
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

    pub fn enable_p2p(&mut self, listen_addr: Option<&str>) -> Result<(), PontemeshError> {
        let listen_addrs = listen_addr
            .map(|addr| vec![addr.to_string()])
            .unwrap_or_else(|| vec!["/ip4/127.0.0.1/tcp/0".to_string()]);
        self.peer = Box::new(Libp2pTransport::start(&listen_addrs, &[])?);
        Ok(())
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
