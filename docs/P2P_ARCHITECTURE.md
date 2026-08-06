# P2P Architecture

P2P in the SDK is a data-plane accelerator. The Origin remains the authority for access packages, manifests, policies, expiration, authorized sources, and fallback rules.

Peers never create manifests, issue access packages, discover other peers independently, or decide that bytes are trusted. A peer can only serve a fragment that the local SDK already downloaded, validated by SHA-256, persisted in local fragment storage, and marked shareable by Origin policy through `allowPeerSharing=true`.

The first native transport is a blocking TCP request/response transport implemented in Rust. It is intentionally independent of Node, browsers, S3, SigV4, MCP, admin APIs, and any database. The public `PeerTransport` contract is transport-neutral so a future libp2p or WebRTC implementation can replace the TCP client/server without changing source selection.

Download flow:

1. The SDK requests an access package from Origin.
2. The SDK reads the manifest and `authorizedSources`.
3. `SourceSelector` orders candidates as `PEER`, then `REPLICA_EDGE`, then `ORIGIN`.
4. A PEER source is used only when it is present in `authorizedSources`, advertises the requested fragment, and the active peer transport supports the endpoint.
5. Every received fragment is validated against manifest size and SHA-256.
6. Invalid peer bytes are discarded and the SDK continues to Replica/Edge or Origin.
7. Validated fragments are preserved, assembled, and the final object hash is checked.

Fallback preserves valid fragments. Replica/Edge is the stable reinforcement path, and Origin is the final guarantee.
