pub mod origin_client;
pub mod path_guard;
pub mod source_client;

pub use origin_client::{HttpOriginClient, OriginClient, PontemeshClientConfig};
pub use path_guard::validate_path_against_allowed;
pub use source_client::{HttpSourceClient, SourceClient};

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crate::download::{
    sync_object_with_control, sync_object_with_control_to_writer, CancellationToken,
    ProgressCallback, SyncObjectRequest, SyncObjectResult, TransferSummary,
};
use crate::errors::PontemeshError;
use crate::p2p::{DisabledPeerTransport, Libp2pTransport, P2pTransportKind, PeerTransport};
use crate::release::{SoftwareUpdateInfo, UpdateCheckRequest};
use crate::storage::FilesystemStorage;

pub struct PontemeshClient {
    config: Option<PontemeshClientConfig>,
    origin: Box<dyn OriginClient>,
    source: Box<dyn SourceClient>,
    peer: Arc<RwLock<Box<dyn PeerTransport>>>,
    suspended: Arc<AtomicBool>,
    allowed_directories: Arc<RwLock<Vec<PathBuf>>>,
}

impl PontemeshClient {
    pub fn new(config: PontemeshClientConfig) -> Result<Self, PontemeshError> {
        let peer: Box<dyn PeerTransport> = if !config.suspended && config.p2p.enabled {
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
        } else if !config.suspended && config.p2p.required {
            return Err(PontemeshError::PeerTransportNotEnabled);
        } else {
            Box::new(DisabledPeerTransport)
        };
        let suspended = Arc::new(AtomicBool::new(config.suspended));
        let allowed_directories = Arc::new(RwLock::new(config.allowed_directories.clone()));
        Ok(Self {
            config: Some(config.clone()),
            origin: Box::new(HttpOriginClient::new(config)),
            source: Box::new(HttpSourceClient::new()),
            peer: Arc::new(RwLock::new(peer)),
            suspended,
            allowed_directories,
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
            peer: Arc::new(RwLock::new(peer)),
            suspended: Arc::new(AtomicBool::new(false)),
            allowed_directories: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended.load(Ordering::Acquire)
    }

    pub fn is_active(&self) -> bool {
        !self.is_suspended()
    }

    pub fn suspend(&self) -> Result<(), PontemeshError> {
        self.set_suspended(true)
    }

    pub fn resume(&self) -> Result<(), PontemeshError> {
        self.set_suspended(false)
    }

    pub fn set_active(&self, active: bool) -> Result<(), PontemeshError> {
        self.set_suspended(!active)
    }

    pub fn set_suspended(&self, suspended: bool) -> Result<(), PontemeshError> {
        self.suspended.store(suspended, Ordering::Release);
        if suspended {
            self.origin.clear_memory_cache();
            if let Ok(mut peer) = self.peer.write() {
                *peer = Box::new(DisabledPeerTransport);
            }
        } else if let Some(config) = &self.config {
            if config.p2p.enabled {
                if let Ok(mut peer) = self.peer.write() {
                    match config.p2p.transport {
                        P2pTransportKind::Libp2p => {
                            if let Ok(p2p) = Libp2pTransport::start(
                                &config.p2p.listen_addrs,
                                &config.p2p.announce_addrs,
                            ) {
                                *peer = Box::new(p2p);
                            }
                        }
                        P2pTransportKind::Disabled => {}
                    }
                }
            }
        }
        Ok(())
    }

    pub fn allowed_directories(&self) -> Vec<PathBuf> {
        self.allowed_directories.read().unwrap().clone()
    }

    pub fn set_allowed_directories(&self, dirs: Vec<PathBuf>) {
        let mut guard = self.allowed_directories.write().unwrap();
        *guard = dirs;
    }

    pub fn add_allowed_directory(&self, dir: impl Into<PathBuf>) {
        let mut guard = self.allowed_directories.write().unwrap();
        guard.push(dir.into());
    }

    pub fn clear_allowed_directories(&self) {
        let mut guard = self.allowed_directories.write().unwrap();
        guard.clear();
    }

    pub fn validate_destination_path(&self, destination: &Path) -> Result<PathBuf, PontemeshError> {
        let allowed = self.allowed_directories.read().unwrap();
        validate_path_against_allowed(destination, &allowed)
    }

    pub fn enable_p2p(&self, listen_addr: Option<&str>) -> Result<(), PontemeshError> {
        let listen_addrs = listen_addr
            .map(|addr| vec![addr.to_string()])
            .unwrap_or_else(|| vec!["/ip4/127.0.0.1/tcp/0".to_string()]);
        let mut peer = self
            .peer
            .write()
            .map_err(|e| PontemeshError::Internal(e.to_string()))?;
        *peer = Box::new(Libp2pTransport::start(&listen_addrs, &[])?);
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
        if self.is_suspended() {
            return Err(PontemeshError::Suspended);
        }
        let validated_dest = self.validate_destination_path(&request.destination)?;
        let cache_root = cache_root(&validated_dest);
        let _ = self.validate_destination_path(&cache_root)?;
        let mut storage = FilesystemStorage::new(cache_root);
        let peer_guard = self
            .peer
            .read()
            .map_err(|e| PontemeshError::Internal(e.to_string()))?;
        let result = sync_object_with_control(
            self.origin.as_ref(),
            self.source.as_ref(),
            peer_guard.as_ref(),
            &mut storage,
            &request,
            progress,
            None,
            &cancellation,
        )?;
        install_atomically(&validated_dest, &result.bytes)?;
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
        if self.is_suspended() {
            return Err(PontemeshError::Suspended);
        }
        let validated_dest = self.validate_destination_path(&request.destination)?;
        let validated_cache = self.validate_destination_path(&cache_directory)?;
        let parent = validated_dest
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        let mut storage = FilesystemStorage::new(validated_cache);
        let peer_guard = self
            .peer
            .read()
            .map_err(|e| PontemeshError::Internal(e.to_string()))?;
        let result = sync_object_with_control_to_writer(
            self.origin.as_ref(),
            self.source.as_ref(),
            peer_guard.as_ref(),
            &mut storage,
            &request,
            progress,
            None,
            &cancellation,
            temporary.as_file_mut(),
        )?;
        temporary.as_file().sync_all()?;
        persist_atomically(&validated_dest, temporary)?;
        Ok(result.summary)
    }

    pub async fn sync_object_async(
        &self,
        request: SyncObjectRequest,
        cancellation: CancellationToken,
    ) -> Result<SyncObjectResult, PontemeshError> {
        if self.is_suspended() {
            return Err(PontemeshError::Suspended);
        }
        let mut config = self.config.clone().ok_or_else(|| {
            PontemeshError::InvalidArgument(
                "async sync requires a client created from PontemeshClientConfig".to_string(),
            )
        })?;
        config.allowed_directories = self.allowed_directories();
        config.suspended = self.is_suspended();
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
        if self.is_suspended() {
            return Err(PontemeshError::Suspended);
        }
        let mut config = self.config.clone().ok_or_else(|| {
            PontemeshError::InvalidArgument(
                "async sync requires a client created from PontemeshClientConfig".to_string(),
            )
        })?;
        config.allowed_directories = self.allowed_directories();
        config.suspended = self.is_suspended();
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

    pub fn check_software_update(
        &self,
        request: &UpdateCheckRequest,
    ) -> Result<Option<SoftwareUpdateInfo>, PontemeshError> {
        if self.is_suspended() {
            return Err(PontemeshError::Suspended);
        }
        self.origin.check_software_update(request)
    }

    pub async fn check_software_update_async(
        &self,
        request: UpdateCheckRequest,
    ) -> Result<Option<SoftwareUpdateInfo>, PontemeshError> {
        if self.is_suspended() {
            return Err(PontemeshError::Suspended);
        }
        let mut config = self.config.clone().ok_or_else(|| {
            PontemeshError::InvalidArgument(
                "async update check requires a client created from PontemeshClientConfig"
                    .to_string(),
            )
        })?;
        config.allowed_directories = self.allowed_directories();
        config.suspended = self.is_suspended();
        tokio::task::spawn_blocking(move || {
            PontemeshClient::new(config)?.check_software_update(&request)
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
    if backup.is_symlink() || backup.exists() {
        let _ = fs::remove_file(&backup);
    }
    if destination.is_symlink() {
        // If destination is a symlink, remove it instead of writing through it
        let _ = fs::remove_file(destination);
    } else if destination.exists() {
        fs::rename(destination, &backup)?;
    }
    match temporary.persist(destination) {
        Ok(_) => {
            if backup.is_symlink() || backup.exists() {
                let _ = fs::remove_file(backup);
            }
            Ok(())
        }
        Err(error) => {
            if backup.is_symlink() || backup.exists() {
                let _ = fs::rename(backup, destination);
            }
            Err(PontemeshError::Io(error.error))
        }
    }
}
