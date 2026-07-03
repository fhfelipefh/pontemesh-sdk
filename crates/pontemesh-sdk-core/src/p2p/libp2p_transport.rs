use std::collections::{BTreeSet, HashMap};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use futures::prelude::*;
use libp2p::request_response::{
    Config as RequestResponseConfig, Event as RequestResponseEvent,
    Message as RequestResponseMessage, ProtocolSupport, ResponseChannel,
};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identity, noise, ping, request_response, yamux, Multiaddr, PeerId, StreamProtocol};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc as tokio_mpsc;

use crate::contracts::{
    is_expired_utc, AccessPackage, AuthorizedSource, FragmentDescriptor, Manifest, SourceType,
};
use crate::errors::PontemeshError;
use crate::integrity::sha256_hex;

use super::peer_protocol::{request_nonce, P2P_PROTOCOL_VERSION};
use super::peer_transport::PeerTransport;

pub const FRAGMENT_PROTOCOL: &str = "/pontemesh/fragment/1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Libp2pFragmentRequest {
    pub protocol_version: u16,
    pub package_id: String,
    pub bucket: String,
    pub key: String,
    pub manifest_id: String,
    pub fragment_id: String,
    pub fragment_index: usize,
    pub byte_range_start: u64,
    pub byte_range_end: u64,
    pub request_nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Libp2pFragmentResponse {
    pub protocol_version: u16,
    pub package_id: String,
    pub manifest_id: String,
    pub fragment_id: String,
    pub fragment_index: usize,
    pub size_bytes: usize,
    pub sha256: String,
    pub request_nonce: String,
    pub bytes: Vec<u8>,
}

#[derive(NetworkBehaviour)]
struct FragmentBehaviour {
    request_response:
        request_response::cbor::Behaviour<Libp2pFragmentRequest, Libp2pFragmentResponse>,
    ping: ping::Behaviour,
}

#[derive(Clone)]
struct SharedFragment {
    package_id: String,
    bucket: String,
    key: String,
    manifest_id: String,
    fragment: FragmentDescriptor,
    bytes: Vec<u8>,
    package_expires_at: String,
}

#[derive(Default)]
struct PeerStore {
    fragments: HashMap<String, SharedFragment>,
    available: BTreeSet<usize>,
}

impl PeerStore {
    fn key(package_id: &str, manifest_id: &str, fragment_index: usize) -> String {
        format!("{package_id}:{manifest_id}:{fragment_index}")
    }
}

enum WorkerCommand {
    AddFragment {
        shared: SharedFragment,
        reply: mpsc::Sender<Result<Vec<usize>, PontemeshError>>,
    },
    Download {
        peer_id: PeerId,
        addr: Multiaddr,
        request: Libp2pFragmentRequest,
        reply: mpsc::Sender<Result<(PeerId, Libp2pFragmentResponse), PontemeshError>>,
    },
    LocalEndpoint {
        reply: mpsc::Sender<Option<String>>,
    },
    Stop,
}

pub struct Libp2pTransport {
    peer_id: PeerId,
    command_tx: tokio_mpsc::UnboundedSender<WorkerCommand>,
    handle: Option<JoinHandle<()>>,
}

impl Libp2pTransport {
    pub fn start(
        listen_addrs: &[String],
        announce_addrs: &[String],
    ) -> Result<Self, PontemeshError> {
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        let listen = parse_multiaddrs(listen_addrs)?;
        let announce = parse_multiaddrs(announce_addrs)?;
        let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let handle =
            thread::spawn(move || run_worker(keypair, listen, announce, command_rx, ready_tx));
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                peer_id,
                command_tx,
                handle: Some(handle),
            }),
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(_) => Err(PontemeshError::PeerTransportNotEnabled),
        }
    }

    pub fn new() -> Self {
        Self::start(&["/ip4/127.0.0.1/tcp/0".to_string()], &[])
            .expect("start default libp2p transport")
    }

    pub fn endpoint(&self) -> String {
        self.local_endpoint()
            .unwrap_or_else(|| format!("/p2p/{}", self.peer_id))
    }

    pub fn peer_id_string(&self) -> String {
        self.peer_id.to_string()
    }

    pub fn add_validated_fragment(
        &self,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        bytes: &[u8],
    ) -> Result<Vec<usize>, PontemeshError> {
        self.record_validated_fragment(package, manifest, fragment, bytes)?
            .ok_or(PontemeshError::PeerTransportNotEnabled)
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn secure_channel(&self) -> &'static str {
        "Noise"
    }

    pub fn multiplexer(&self) -> &'static str {
        "Yamux"
    }

    fn expected_peer_id(source: &AuthorizedSource) -> Option<PeerId> {
        source
            .peer_id
            .as_deref()
            .and_then(|id| id.parse().ok())
            .or_else(|| {
                source
                    .endpoint
                    .split("/p2p/")
                    .nth(1)
                    .and_then(|tail| tail.split('/').next())
                    .and_then(|id| id.parse().ok())
            })
    }

    fn validate_response(
        &self,
        source: &AuthorizedSource,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        nonce: &str,
        response: &Libp2pFragmentResponse,
        remote_peer: PeerId,
    ) -> Result<(), PontemeshError> {
        let expected = Self::expected_peer_id(source).ok_or_else(|| {
            PontemeshError::AccessDenied("authorized libp2p source is missing PeerId".to_string())
        })?;
        if remote_peer != expected {
            return Err(PontemeshError::AccessDenied(
                "libp2p connection PeerId does not match authorized source".to_string(),
            ));
        }
        if source.source_type != SourceType::Peer
            || source.transport.as_deref() != Some("libp2p")
            || is_expired_utc(&source.expires_at)
        {
            return Err(PontemeshError::AccessDenied(
                "libp2p source is not authorized".to_string(),
            ));
        }
        if response.protocol_version != P2P_PROTOCOL_VERSION as u16
            || response.package_id != package.id
            || response.manifest_id != manifest.manifest_id
            || response.fragment_id != fragment.fragment_id
            || response.fragment_index != fragment.index
            || response.size_bytes != fragment.size_bytes
            || response.request_nonce != nonce
        {
            return Err(PontemeshError::HashMismatch(
                "libp2p fragment response metadata mismatch".to_string(),
            ));
        }
        if response.bytes.len() != response.size_bytes
            || sha256_hex(&response.bytes) != response.sha256
            || !response.sha256.eq_ignore_ascii_case(&fragment.sha256)
        {
            return Err(PontemeshError::HashMismatch(
                "libp2p fragment hash mismatch".to_string(),
            ));
        }
        Ok(())
    }
}

impl Drop for Libp2pTransport {
    fn drop(&mut self) {
        let _ = self.command_tx.send(WorkerCommand::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl PeerTransport for Libp2pTransport {
    fn can_handle(&self, source: &AuthorizedSource) -> bool {
        source.source_type == SourceType::Peer
            && source.transport.as_deref() == Some("libp2p")
            && !is_expired_utc(&source.expires_at)
            && Self::expected_peer_id(source).is_some()
            && source_endpoint_to_multiaddr(&source.endpoint).is_some()
    }

    fn download_fragment(
        &self,
        source: &AuthorizedSource,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
    ) -> Result<Vec<u8>, PontemeshError> {
        if !self.can_handle(source) {
            return Err(PontemeshError::AccessDenied(
                "libp2p peer source is not authorized".to_string(),
            ));
        }
        if !source
            .available_fragments
            .contains(&(fragment.index as i64))
        {
            return Err(PontemeshError::AccessDenied(
                "libp2p peer does not advertise fragment".to_string(),
            ));
        }
        let peer_id = Self::expected_peer_id(source).unwrap();
        let addr = source_endpoint_to_multiaddr(&source.endpoint).unwrap();
        let nonce = request_nonce();
        let request = Libp2pFragmentRequest {
            protocol_version: P2P_PROTOCOL_VERSION as u16,
            package_id: package.id.clone(),
            bucket: manifest.bucket.clone(),
            key: manifest.key.clone(),
            manifest_id: manifest.manifest_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
            fragment_index: fragment.index,
            byte_range_start: fragment.byte_range_start,
            byte_range_end: fragment.byte_range_end,
            request_nonce: nonce.clone(),
        };
        let (reply, rx) = mpsc::channel();
        self.command_tx
            .send(WorkerCommand::Download {
                peer_id,
                addr,
                request,
                reply,
            })
            .map_err(|_| PontemeshError::PeerTransportNotEnabled)?;
        let (remote_peer, response) = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| PontemeshError::NoSourceAvailable)??;
        self.validate_response(
            source,
            package,
            manifest,
            fragment,
            &nonce,
            &response,
            remote_peer,
        )?;
        Ok(response.bytes)
    }

    fn record_validated_fragment(
        &self,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        bytes: &[u8],
    ) -> Result<Option<Vec<usize>>, PontemeshError> {
        if !package.source_selection.allow_peer_sharing {
            return Ok(Some(Vec::new()));
        }
        if bytes.len() != fragment.size_bytes {
            return Err(PontemeshError::HashMismatch(
                "shareable fragment size mismatch".to_string(),
            ));
        }
        if !sha256_hex(bytes).eq_ignore_ascii_case(&fragment.sha256) {
            return Err(PontemeshError::HashMismatch(
                "shareable fragment hash mismatch".to_string(),
            ));
        }
        let shared = SharedFragment {
            package_id: package.id.clone(),
            bucket: manifest.bucket.clone(),
            key: manifest.key.clone(),
            manifest_id: manifest.manifest_id.clone(),
            fragment: fragment.clone(),
            bytes: bytes.to_vec(),
            package_expires_at: package.expires_at.clone(),
        };
        let (reply, rx) = mpsc::channel();
        self.command_tx
            .send(WorkerCommand::AddFragment { shared, reply })
            .map_err(|_| PontemeshError::PeerTransportNotEnabled)?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| PontemeshError::PeerTransportNotEnabled)?
            .map(Some)
    }

    fn local_endpoint(&self) -> Option<String> {
        let (reply, rx) = mpsc::channel();
        self.command_tx
            .send(WorkerCommand::LocalEndpoint { reply })
            .ok()?;
        rx.recv_timeout(Duration::from_secs(2)).ok().flatten()
    }
}

fn run_worker(
    keypair: identity::Keypair,
    listen_addrs: Vec<Multiaddr>,
    announce_addrs: Vec<Multiaddr>,
    command_rx: tokio_mpsc::UnboundedReceiver<WorkerCommand>,
    ready_tx: mpsc::Sender<Result<(), PontemeshError>>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = ready_tx.send(Err(PontemeshError::Internal(error.to_string())));
            return;
        }
    };
    runtime.block_on(async move {
        let result = run_swarm(keypair, listen_addrs, announce_addrs, command_rx, ready_tx).await;
        if let Err(error) = result {
            let _ = error;
        }
    });
}

async fn run_swarm(
    keypair: identity::Keypair,
    listen_addrs: Vec<Multiaddr>,
    announce_addrs: Vec<Multiaddr>,
    mut command_rx: tokio_mpsc::UnboundedReceiver<WorkerCommand>,
    ready_tx: mpsc::Sender<Result<(), PontemeshError>>,
) -> Result<(), PontemeshError> {
    let local_peer_id = PeerId::from(keypair.public());
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            Default::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .map_err(|error| PontemeshError::Internal(error.to_string()))?
        .with_behaviour(|_| {
            let protocols = [(
                StreamProtocol::new(FRAGMENT_PROTOCOL),
                ProtocolSupport::Full,
            )];
            let config =
                RequestResponseConfig::default().with_request_timeout(Duration::from_secs(5));
            FragmentBehaviour {
                request_response: request_response::cbor::Behaviour::new(protocols, config),
                ping: ping::Behaviour::default(),
            }
        })
        .map_err(|error| PontemeshError::Internal(error.to_string()))?
        .with_swarm_config(|config| config.with_idle_connection_timeout(Duration::from_secs(30)))
        .build();

    let listen = if listen_addrs.is_empty() {
        vec!["/ip4/127.0.0.1/tcp/0".parse().unwrap()]
    } else {
        listen_addrs
    };
    for addr in listen {
        swarm
            .listen_on(addr)
            .map_err(|error| PontemeshError::Internal(error.to_string()))?;
    }

    let mut store = PeerStore::default();
    let mut local_endpoint: Option<String> = None;
    let mut pending: HashMap<
        request_response::OutboundRequestId,
        mpsc::Sender<Result<(PeerId, Libp2pFragmentResponse), PontemeshError>>,
    > = HashMap::new();
    let mut ready_tx = Some(ready_tx);

    let result = 'worker: loop {
        tokio::select! {
            command = command_rx.recv() => {
                match command {
                Some(WorkerCommand::AddFragment { shared, reply }) => {
                    store.available.insert(shared.fragment.index);
                    store.fragments.insert(
                        PeerStore::key(
                            &shared.package_id,
                            &shared.manifest_id,
                            shared.fragment.index,
                        ),
                        shared,
                    );
                    let _ = reply.send(Ok(store.available.iter().copied().collect()));
                }
                Some(WorkerCommand::Download {
                    peer_id,
                    addr,
                    request,
                    reply,
                }) => {
                    swarm.add_peer_address(peer_id, addr);
                    send_fragment_request(
                        &mut swarm.behaviour_mut().request_response,
                        &mut pending,
                        peer_id,
                        request,
                        reply,
                    );
                }
                Some(WorkerCommand::LocalEndpoint { reply }) => {
                    let _ = reply.send(local_endpoint.clone());
                }
                Some(WorkerCommand::Stop) | None => break 'worker Ok(()),
                }
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        local_endpoint = Some(address.with_p2p(local_peer_id).map(|addr| addr.to_string()).unwrap_or_else(|addr| format!("{addr}/p2p/{local_peer_id}")));
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(Ok(()));
                        }
                        for addr in &announce_addrs {
                            local_endpoint = Some(addr.clone().with_p2p(local_peer_id).map(|addr| addr.to_string()).unwrap_or_else(|addr| format!("{addr}/p2p/{local_peer_id}")));
                        }
                    }
                    SwarmEvent::Behaviour(FragmentBehaviourEvent::RequestResponse(event)) => {
                        handle_request_response_event(event, &mut store, &mut swarm.behaviour_mut().request_response, &mut pending);
                    }
                    _ => {}
                }
            }
        }
    };
    result
}

fn send_fragment_request(
    request_response: &mut request_response::cbor::Behaviour<
        Libp2pFragmentRequest,
        Libp2pFragmentResponse,
    >,
    pending: &mut HashMap<
        request_response::OutboundRequestId,
        mpsc::Sender<Result<(PeerId, Libp2pFragmentResponse), PontemeshError>>,
    >,
    peer_id: PeerId,
    request: Libp2pFragmentRequest,
    reply: mpsc::Sender<Result<(PeerId, Libp2pFragmentResponse), PontemeshError>>,
) {
    let request_id = request_response.send_request(&peer_id, request);
    pending.insert(request_id, reply);
}

fn handle_request_response_event(
    event: RequestResponseEvent<Libp2pFragmentRequest, Libp2pFragmentResponse>,
    store: &mut PeerStore,
    request_response: &mut request_response::cbor::Behaviour<
        Libp2pFragmentRequest,
        Libp2pFragmentResponse,
    >,
    pending: &mut HashMap<
        request_response::OutboundRequestId,
        mpsc::Sender<Result<(PeerId, Libp2pFragmentResponse), PontemeshError>>,
    >,
) {
    match event {
        RequestResponseEvent::Message { peer, message } => match message {
            RequestResponseMessage::Request {
                request, channel, ..
            } => {
                send_fragment_response(request_response, store, channel, request);
            }
            RequestResponseMessage::Response {
                request_id,
                response,
            } => {
                if let Some(reply) = pending.remove(&request_id) {
                    let _ = reply.send(Ok((peer, response)));
                }
            }
        },
        RequestResponseEvent::OutboundFailure {
            request_id, error, ..
        } => {
            if let Some(reply) = pending.remove(&request_id) {
                let _ = reply.send(Err(PontemeshError::Internal(format!(
                    "libp2p outbound request failed: {error}"
                ))));
            }
        }
        RequestResponseEvent::InboundFailure { .. } => {}
        RequestResponseEvent::ResponseSent { .. } => {}
    }
}

fn send_fragment_response(
    request_response: &mut request_response::cbor::Behaviour<
        Libp2pFragmentRequest,
        Libp2pFragmentResponse,
    >,
    store: &mut PeerStore,
    channel: ResponseChannel<Libp2pFragmentResponse>,
    request: Libp2pFragmentRequest,
) {
    if let Ok(response) = handle_fragment_request(&request, store) {
        let _ = request_response.send_response(channel, response);
    }
}

fn handle_fragment_request(
    request: &Libp2pFragmentRequest,
    store: &PeerStore,
) -> Result<Libp2pFragmentResponse, PontemeshError> {
    if request.protocol_version != P2P_PROTOCOL_VERSION as u16 {
        return Err(PontemeshError::InvalidArgument(
            "unexpected libp2p protocol version".to_string(),
        ));
    }
    let key = PeerStore::key(
        &request.package_id,
        &request.manifest_id,
        request.fragment_index,
    );
    let shared = store
        .fragments
        .get(&key)
        .cloned()
        .ok_or(PontemeshError::NoSourceAvailable)?;
    if shared.package_id != request.package_id
        || shared.bucket != request.bucket
        || shared.key != request.key
        || shared.manifest_id != request.manifest_id
        || shared.fragment.fragment_id != request.fragment_id
        || shared.fragment.index != request.fragment_index
        || shared.fragment.byte_range_start != request.byte_range_start
        || shared.fragment.byte_range_end != request.byte_range_end
        || shared.bytes.len() != shared.fragment.size_bytes
        || !sha256_hex(&shared.bytes).eq_ignore_ascii_case(&shared.fragment.sha256)
    {
        return Err(PontemeshError::AccessDenied(
            "libp2p fragment request does not match validated manifest".to_string(),
        ));
    }
    if is_expired_utc(&shared.package_expires_at) {
        return Err(PontemeshError::AccessDenied(
            "access package is expired".to_string(),
        ));
    }
    Ok(Libp2pFragmentResponse {
        protocol_version: P2P_PROTOCOL_VERSION as u16,
        package_id: shared.package_id,
        manifest_id: shared.manifest_id,
        fragment_id: shared.fragment.fragment_id,
        fragment_index: shared.fragment.index,
        size_bytes: shared.bytes.len(),
        sha256: sha256_hex(&shared.bytes),
        request_nonce: request.request_nonce.clone(),
        bytes: shared.bytes,
    })
}

fn parse_multiaddrs(addrs: &[String]) -> Result<Vec<Multiaddr>, PontemeshError> {
    addrs
        .iter()
        .map(|addr| {
            addr.parse().map_err(|error| {
                PontemeshError::InvalidArgument(format!("invalid libp2p multiaddr {addr}: {error}"))
            })
        })
        .collect()
}

pub fn source_endpoint_to_multiaddr(endpoint: &str) -> Option<Multiaddr> {
    let without_peer_id = endpoint.split("/p2p/").next().unwrap_or(endpoint);
    without_peer_id.parse::<Multiaddr>().ok().or_else(|| {
        if !without_peer_id.starts_with("peer://") {
            return None;
        }
        let socket = without_peer_id.strip_prefix("peer://")?;
        let (host, port) = socket.rsplit_once(':')?;
        format!("/ip4/{host}/tcp/{port}").parse().ok()
    })
}
