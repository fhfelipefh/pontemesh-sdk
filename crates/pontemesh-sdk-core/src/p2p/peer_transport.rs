use crate::contracts::{AccessPackage, AuthorizedSource, FragmentDescriptor, Manifest};
use crate::errors::PontemeshError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P2pTransportKind {
    Libp2p,
    ExperimentalTcp,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2pConfig {
    pub enabled: bool,
    pub required: bool,
    pub transport: P2pTransportKind,
    pub listen_addrs: Vec<String>,
    pub announce_addrs: Vec<String>,
    pub listen_addr: Option<String>,
    pub announce_addr: Option<String>,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            required: false,
            transport: P2pTransportKind::Disabled,
            listen_addrs: Vec::new(),
            announce_addrs: Vec::new(),
            listen_addr: None,
            announce_addr: None,
        }
    }
}

pub trait PeerTransport: Send + Sync {
    fn can_handle(&self, source: &AuthorizedSource) -> bool;

    fn download_fragment(
        &self,
        source: &AuthorizedSource,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError>;

    fn record_validated_fragment(
        &self,
        _package: &AccessPackage,
        _manifest: &Manifest,
        _fragment: &FragmentDescriptor,
        _bytes: &[u8],
    ) -> Result<Option<Vec<usize>>, PontemeshError> {
        Ok(None)
    }

    fn local_endpoint(&self) -> Option<String> {
        None
    }
}
