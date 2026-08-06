use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::Duration;

use crate::contracts::{
    is_expired_utc, AccessPackage, AuthorizedSource, FragmentDescriptor, Manifest, SourceType,
};
use crate::errors::PontemeshError;
use crate::integrity::sha256_hex;

use super::peer_identity::{peer_id_from_endpoint, socket_addr_from_endpoint};
use super::peer_protocol::{
    read_limited_line, request_nonce, MAX_FRAME_BYTES, P2P_PROTOCOL_VERSION,
};
use super::peer_protocol::{FragmentRequest, FragmentResponse, PeerProtocolError};
use super::peer_server::PeerServer;
use super::peer_transport::PeerTransport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
struct PeerCircuit {
    failures: u32,
    state: CircuitState,
    opened_at: Option<std::time::Instant>,
}

pub struct PeerClient {
    server: Option<PeerServer>,
    timeout: Duration,
    circuits: Mutex<HashMap<String, PeerCircuit>>,
    failure_threshold: u32,
    open_duration: Duration,
}

impl PeerClient {
    pub fn new() -> Self {
        Self {
            server: None,
            timeout: Duration::from_secs(5),
            circuits: Mutex::new(HashMap::new()),
            failure_threshold: 2,
            open_duration: Duration::from_secs(2),
        }
    }

    pub fn with_server(server: PeerServer) -> Self {
        Self {
            server: Some(server),
            timeout: Duration::from_secs(5),
            circuits: Mutex::new(HashMap::new()),
            failure_threshold: 2,
            open_duration: Duration::from_secs(2),
        }
    }

    pub fn start(
        listen_addr: Option<&str>,
        announce_addr: Option<&str>,
    ) -> Result<Self, PontemeshError> {
        Ok(Self::with_server(PeerServer::start(
            listen_addr,
            announce_addr,
        )?))
    }

    pub fn circuit_state(&self, source_id: &str) -> CircuitState {
        let mut circuits = self.circuits.lock().unwrap();
        let circuit = circuits
            .entry(source_id.to_string())
            .or_insert_with(|| PeerCircuit {
                failures: 0,
                state: CircuitState::Closed,
                opened_at: None,
            });
        if circuit.state == CircuitState::Open
            && circuit
                .opened_at
                .is_some_and(|opened_at| opened_at.elapsed() >= self.open_duration)
        {
            circuit.state = CircuitState::HalfOpen;
        }
        circuit.state
    }

    fn record_success(&self, source_id: &str) {
        self.circuits.lock().unwrap().insert(
            source_id.to_string(),
            PeerCircuit {
                failures: 0,
                state: CircuitState::Closed,
                opened_at: None,
            },
        );
    }

    fn record_failure(&self, source_id: &str) {
        let mut circuits = self.circuits.lock().unwrap();
        let circuit = circuits
            .entry(source_id.to_string())
            .or_insert_with(|| PeerCircuit {
                failures: 0,
                state: CircuitState::Closed,
                opened_at: None,
            });
        circuit.failures += 1;
        if circuit.failures >= self.failure_threshold {
            circuit.state = CircuitState::Open;
            circuit.opened_at = Some(std::time::Instant::now());
        }
    }
}

impl Default for PeerClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerTransport for PeerClient {
    fn can_handle(&self, source: &AuthorizedSource) -> bool {
        source.source_type == SourceType::Peer
            && source.endpoint.starts_with("peer://")
            && !is_expired_utc(&source.expires_at)
            && self.circuit_state(&source.id) != CircuitState::Open
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
                "peer source is not authorized for P2P".to_string(),
            ));
        }
        if !source
            .available_fragments
            .contains(&(fragment.index as i64))
        {
            return Err(PontemeshError::AccessDenied(
                "peer source does not advertise fragment".to_string(),
            ));
        }
        let expected_peer_id = source
            .peer_id
            .as_deref()
            .or_else(|| peer_id_from_endpoint(&source.endpoint));
        let Some(addr) = socket_addr_from_endpoint(&source.endpoint) else {
            return Err(PontemeshError::AccessDenied(
                "peer endpoint is not authorized".to_string(),
            ));
        };
        let nonce = request_nonce();
        let result = self.download_fragment_inner(
            source,
            package,
            manifest,
            fragment,
            addr,
            expected_peer_id,
            &nonce,
        );
        match result {
            Ok(bytes) => {
                self.record_success(&source.id);
                Ok(bytes)
            }
            Err(error) => {
                self.record_failure(&source.id);
                Err(error)
            }
        }
    }

    fn record_validated_fragment(
        &self,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        bytes: &[u8],
    ) -> Result<Option<Vec<usize>>, PontemeshError> {
        match &self.server {
            Some(server) => server
                .add_validated_fragment(package, manifest, fragment, bytes)
                .map(Some),
            None => Ok(None),
        }
    }

    fn local_endpoint(&self) -> Option<String> {
        self.server
            .as_ref()
            .map(|server| server.endpoint().to_string())
    }
}

impl PeerClient {
    #[allow(clippy::too_many_arguments)]
    fn download_fragment_inner(
        &self,
        _source: &AuthorizedSource,
        package: &AccessPackage,
        manifest: &Manifest,
        fragment: &FragmentDescriptor,
        addr: &str,
        expected_peer_id: Option<&str>,
        nonce: &str,
    ) -> Result<Vec<u8>, PontemeshError> {
        let mut stream = TcpStream::connect(addr).map_err(|_| PontemeshError::NoSourceAvailable)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        let request = FragmentRequest {
            message_type: "fragmentRequest".to_string(),
            protocol_version: P2P_PROTOCOL_VERSION,
            package_id: package.id.clone(),
            bucket: manifest.bucket.clone(),
            key: manifest.key.clone(),
            manifest_id: manifest.manifest_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
            fragment_index: fragment.index,
            byte_range_start: fragment.byte_range_start,
            byte_range_end: fragment.byte_range_end,
            request_nonce: nonce.to_string(),
        };
        serde_json::to_writer(&mut stream, &request)
            .map_err(|error| PontemeshError::Internal(error.to_string()))?;
        stream.write_all(b"\n")?;

        let mut reader = BufReader::new(stream);
        let line = read_limited_line(&mut reader, MAX_FRAME_BYTES)?;
        if line.trim().is_empty() {
            return Err(PontemeshError::NoSourceAvailable);
        }
        if let Ok(error) = serde_json::from_str::<PeerProtocolError>(&line) {
            return Err(PontemeshError::AccessDenied(error.message));
        }
        let response: FragmentResponse = serde_json::from_str(&line)
            .map_err(|error| PontemeshError::Internal(format!("invalid peer response: {error}")))?;
        if response.message_type != "fragmentResponse"
            || response.protocol_version != P2P_PROTOCOL_VERSION
            || response.package_id != package.id
            || response.manifest_id != manifest.manifest_id
            || response.fragment_id != fragment.fragment_id
            || response.fragment_index != fragment.index
            || response.size_bytes != fragment.size_bytes
            || response.request_nonce != nonce
        {
            return Err(PontemeshError::HashMismatch(
                "peer response metadata mismatch".to_string(),
            ));
        }
        if expected_peer_id.is_some() && response.peer_id.as_deref() != expected_peer_id {
            return Err(PontemeshError::AccessDenied(
                "peer identity does not match authorized source".to_string(),
            ));
        }
        let bytes = response.decode_bytes()?;
        if bytes.len() != response.size_bytes || sha256_hex(&bytes) != response.sha256 {
            return Err(PontemeshError::HashMismatch(
                "peer response self hash mismatch".to_string(),
            ));
        }
        Ok(bytes)
    }
}
