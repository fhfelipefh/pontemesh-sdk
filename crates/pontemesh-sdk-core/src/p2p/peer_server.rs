use std::collections::{BTreeSet, HashMap};
use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::contracts::{is_expired_utc, AccessPackage, FragmentDescriptor, Manifest};
use crate::errors::PontemeshError;
use crate::integrity::sha256_hex;

use super::peer_identity::PeerIdentity;
use super::peer_protocol::{
    read_limited_line, FragmentRequest, FragmentResponse, PeerProtocolError, MAX_REQUEST_BYTES,
    P2P_PROTOCOL_VERSION,
};

const MAX_CONCURRENT_REQUESTS: usize = 32;

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

pub struct PeerServer {
    endpoint: String,
    peer_id: String,
    store: Arc<Mutex<PeerStore>>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl PeerServer {
    pub fn start(
        listen_addr: Option<&str>,
        announce_addr: Option<&str>,
    ) -> Result<Self, PontemeshError> {
        let addr = listen_addr
            .and_then(|addr| addr.strip_prefix("tcp://").or(Some(addr)))
            .unwrap_or("127.0.0.1:0");
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let peer_id = PeerIdentity::local().peer_id;
        let endpoint = announce_addr
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("peer://{local_addr}/p2p/{peer_id}"));
        let store = Arc::new(Mutex::new(PeerStore::default()));
        let running = Arc::new(AtomicBool::new(true));
        let active_requests = Arc::new(AtomicUsize::new(0));
        let thread_store = Arc::clone(&store);
        let thread_running = Arc::clone(&running);
        let thread_active_requests = Arc::clone(&active_requests);
        let thread_peer_id = peer_id.clone();
        let handle = thread::spawn(move || {
            while thread_running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if thread_active_requests.load(Ordering::SeqCst) >= MAX_CONCURRENT_REQUESTS
                        {
                            let _ = reject_overloaded(stream);
                            continue;
                        }
                        thread_active_requests.fetch_add(1, Ordering::SeqCst);
                        let store = Arc::clone(&thread_store);
                        let active_requests = Arc::clone(&thread_active_requests);
                        let peer_id = thread_peer_id.clone();
                        thread::spawn(move || {
                            let _ = handle_connection(stream, store, peer_id);
                            active_requests.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            endpoint,
            peer_id,
            store,
            running,
            handle: Some(handle),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    pub fn add_validated_fragment(
        &self,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        bytes: &[u8],
    ) -> Result<Vec<usize>, PontemeshError> {
        if !package.source_selection.allow_peer_sharing {
            return Ok(self.available_fragments());
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
        let mut store = self.store.lock().unwrap();
        store.available.insert(fragment.index);
        store.fragments.insert(
            PeerStore::key(&package.id, &manifest.manifest_id, fragment.index),
            shared,
        );
        Ok(store.available.iter().copied().collect())
    }

    pub fn available_fragments(&self) -> Vec<usize> {
        self.store
            .lock()
            .unwrap()
            .available
            .iter()
            .copied()
            .collect()
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(addr) = super::peer_identity::socket_addr_from_endpoint(&self.endpoint) {
            let _ = TcpStream::connect(addr);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PeerServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn handle_connection(
    stream: TcpStream,
    store: Arc<Mutex<PeerStore>>,
    peer_id: String,
) -> Result<(), PontemeshError> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let line = read_limited_line(&mut reader, MAX_REQUEST_BYTES)?;
    let response = match serde_json::from_str::<FragmentRequest>(&line) {
        Ok(request) => handle_fragment_request(&request, store, &peer_id),
        Err(error) => Err(PontemeshError::InvalidArgument(error.to_string())),
    };
    let mut stream = stream;
    match response {
        Ok(response) => {
            serde_json::to_writer(&mut stream, &response)
                .map_err(|error| PontemeshError::Internal(error.to_string()))?;
        }
        Err(error) => {
            let protocol_error = PeerProtocolError {
                message_type: "error".to_string(),
                code: "PEER_REJECTED".to_string(),
                message: error.to_string(),
            };
            serde_json::to_writer(&mut stream, &protocol_error)
                .map_err(|error| PontemeshError::Internal(error.to_string()))?;
        }
    }
    stream.write_all(b"\n")?;
    Ok(())
}

fn handle_fragment_request(
    request: &FragmentRequest,
    store: Arc<Mutex<PeerStore>>,
    peer_id: &str,
) -> Result<FragmentResponse, PontemeshError> {
    if request.message_type != "fragmentRequest" || request.protocol_version != P2P_PROTOCOL_VERSION
    {
        return Err(PontemeshError::InvalidArgument(
            "unexpected peer message type".to_string(),
        ));
    }
    let key = PeerStore::key(
        &request.package_id,
        &request.manifest_id,
        request.fragment_index,
    );
    let shared = store
        .lock()
        .unwrap()
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
    {
        return Err(PontemeshError::AccessDenied(
            "peer fragment request does not match validated manifest".to_string(),
        ));
    }
    if is_expired_utc(&shared.package_expires_at) {
        return Err(PontemeshError::AccessDenied(
            "access package is expired".to_string(),
        ));
    }
    Ok(FragmentResponse::from_bytes(
        &shared.package_id,
        &shared.manifest_id,
        &shared.fragment.fragment_id,
        shared.fragment.index,
        &request.request_nonce,
        Some(peer_id),
        &shared.bytes,
    ))
}

fn reject_overloaded(mut stream: TcpStream) -> Result<(), PontemeshError> {
    let protocol_error = PeerProtocolError {
        message_type: "error".to_string(),
        code: "PEER_OVERLOADED".to_string(),
        message: "peer request limit reached".to_string(),
    };
    serde_json::to_writer(&mut stream, &protocol_error)
        .map_err(|error| PontemeshError::Internal(error.to_string()))?;
    stream.write_all(b"\n")?;
    Ok(())
}
