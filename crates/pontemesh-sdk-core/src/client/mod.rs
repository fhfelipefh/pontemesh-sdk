pub mod origin_client;
pub mod source_client;

pub use origin_client::{HttpOriginClient, OriginClient, PontemeshClientConfig};
pub use source_client::{HttpSourceClient, SourceClient};

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::download::{
    sync_object_with_control, sync_object_with_control_to_writer, CancellationToken,
    ProgressCallback, SyncObjectRequest, SyncObjectResult, TransferSummary,
};
use crate::errors::PontemeshError;
use crate::p2p::{DisabledPeerTransport, Libp2pTransport, P2pTransportKind, PeerTransport};
use crate::storage::FilesystemStorage;

pub struct PontemeshClient {
    config: Option<PontemeshClientConfig>,
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
            config: Some(config.clone()),
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
            config: None,
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
        self.sync_object_with_summary_and_progress(request, progress)
            .map(|_| ())
    }

    pub fn sync_object_with_summary(
        &self,
        request: SyncObjectRequest,
    ) -> Result<SyncObjectResult, PontemeshError> {
        self.sync_object_with_summary_and_progress(request, None)
    }

    pub fn sync_object_with_summary_and_progress(
        &self,
        request: SyncObjectRequest,
        progress: Option<ProgressCallback<'_>>,
    ) -> Result<SyncObjectResult, PontemeshError> {
        self.sync_object_with_options(request, progress, CancellationToken::default())
    }

    pub fn sync_object_with_options(
        &self,
        request: SyncObjectRequest,
        progress: Option<ProgressCallback<'_>>,
        cancellation: CancellationToken,
    ) -> Result<SyncObjectResult, PontemeshError> {
        let cache_root = cache_root(&request.destination);
        let mut storage = FilesystemStorage::new(cache_root);
        let result = sync_object_with_control(
            self.origin.as_ref(),
            self.source.as_ref(),
            self.peer.as_ref(),
            &mut storage,
            &request,
            progress,
            None,
            &cancellation,
        )?;
        install_atomically(&request.destination, &result.bytes)?;
        Ok(result)
    }

    pub fn sync_object_to_disk_with_options(
        &self,
        request: SyncObjectRequest,
        progress: Option<ProgressCallback<'_>>,
        cancellation: CancellationToken,
    ) -> Result<TransferSummary, PontemeshError> {
        let cache_directory = cache_root(&request.destination);
        self.sync_object_to_disk_with_cache(request, cache_directory, progress, cancellation)
    }

    pub fn sync_object_to_disk_with_cache(
        &self,
        request: SyncObjectRequest,
        cache_directory: PathBuf,
        progress: Option<ProgressCallback<'_>>,
        cancellation: CancellationToken,
    ) -> Result<TransferSummary, PontemeshError> {
        let parent = request
            .destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        let mut storage = FilesystemStorage::new(cache_directory);
        let result = sync_object_with_control_to_writer(
            self.origin.as_ref(),
            self.source.as_ref(),
            self.peer.as_ref(),
            &mut storage,
            &request,
            progress,
            None,
            &cancellation,
            temporary.as_file_mut(),
        )?;
        temporary.as_file().sync_all()?;
        persist_atomically(&request.destination, temporary)?;
        Ok(result.summary)
    }

    pub async fn sync_object_async(
        &self,
        request: SyncObjectRequest,
        cancellation: CancellationToken,
    ) -> Result<SyncObjectResult, PontemeshError> {
        let config = self.config.clone().ok_or_else(|| {
            PontemeshError::InvalidArgument(
                "async sync requires a client created from PontemeshClientConfig".to_string(),
            )
        })?;
        tokio::task::spawn_blocking(move || {
            PontemeshClient::new(config)?.sync_object_with_options(request, None, cancellation)
        })
        .await
        .map_err(|error| PontemeshError::Internal(error.to_string()))?
    }

    pub async fn sync_object_to_disk_async(
        &self,
        request: SyncObjectRequest,
        cancellation: CancellationToken,
    ) -> Result<TransferSummary, PontemeshError> {
        let config = self.config.clone().ok_or_else(|| {
            PontemeshError::InvalidArgument(
                "async sync requires a client created from PontemeshClientConfig".to_string(),
            )
        })?;
        tokio::task::spawn_blocking(move || {
            PontemeshClient::new(config)?.sync_object_to_disk_with_options(
                request,
                None,
                cancellation,
            )
        })
        .await
        .map_err(|error| PontemeshError::Internal(error.to_string()))?
    }
}

fn cache_root(destination: &Path) -> PathBuf {
    destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(".pontemesh-cache")
}

fn install_atomically(destination: &Path, bytes: &[u8]) -> Result<(), PontemeshError> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let available = fs2::available_space(parent)?;
    let required = bytes.len() as u64;
    if available < required {
        return Err(PontemeshError::InsufficientDiskSpace {
            required,
            available,
        });
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;

    persist_atomically(destination, temporary)
}

fn persist_atomically(
    destination: &Path,
    temporary: tempfile::NamedTempFile,
) -> Result<(), PontemeshError> {
    let backup = destination.with_extension("pontemesh-rollback");
    if backup.exists() {
        fs::remove_file(&backup)?;
    }
    if destination.exists() {
        fs::rename(destination, &backup)?;
    }
    match temporary.persist(destination) {
        Ok(_) => {
            if backup.exists() {
                fs::remove_file(backup)?;
            }
            Ok(())
        }
        Err(error) => {
            if backup.exists() {
                fs::rename(backup, destination)?;
            }
            Err(PontemeshError::Io(error.error))
        }
    }
}
