//! Placeholder for the production libp2p-backed transport.
//!
//! This SDK core is currently synchronous and exposes blocking download APIs.
//! Pulling libp2p into this slice would force an async runtime boundary through
//! the public API before the rest of the SDK is ready. The real P2P transport in
//! this release is `PeerClient`/`PeerServer`, an experimental native TCP
//! request/response implementation with the same `PeerTransport` contract.

pub const LIBP2P_TRANSPORT_STATUS: &str =
    "planned: production secure transport requires async runtime boundary plus Noise/TLS";
