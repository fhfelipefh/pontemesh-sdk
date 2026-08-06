#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    pub peer_id: String,
}

impl PeerIdentity {
    pub fn local() -> Self {
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        Self {
            peer_id: format!("peer-{pid}-{nanos}"),
        }
    }
}

pub fn peer_id_from_endpoint(endpoint: &str) -> Option<&str> {
    endpoint.rsplit_once("/p2p/").map(|(_, peer_id)| peer_id)
}

pub fn socket_addr_from_endpoint(endpoint: &str) -> Option<&str> {
    let value = endpoint.strip_prefix("peer://")?;
    Some(
        value
            .split_once("/p2p/")
            .map(|(addr, _)| addr)
            .unwrap_or(value),
    )
}
