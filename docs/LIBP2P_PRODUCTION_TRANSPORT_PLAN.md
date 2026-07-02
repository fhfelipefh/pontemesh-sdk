# libp2p Production Transport Plan

`experimental-tcp` remains the local functional transport for SDK mesh simulation. Production P2P requires a secure authenticated transport before the SDK is marked production-ready for hostile networks.

## Required Transport

- Use `libp2p`.
- Use Noise or TLS for encrypted authenticated channels.
- Use cryptographic `PeerId` derived from the peer identity key.
- Implement a request-response protocol for fragment transfer.
- Preserve existing frame limits for requests and responses.
- Preserve client and server timeouts.
- Preserve rate limiting and concurrent request limits.
- Preserve the peer circuit breaker.
- Never send `packageToken` or `applicationToken` over P2P.

## Identity And Authorization

- Origin remains authoritative for manifests, access packages, source authorization, expiration, and revocation.
- Peer identity must be verified by the libp2p secure channel, not by trusting a JSON `peerId` field.
- `authorizedSources[].peerId` must match the authenticated remote peer.
- Any future peer transfer token must be short-lived, scoped to package, manifest, peer, and fragment indexes.

## Acceptance Criteria

- MITM cannot impersonate an authorized peer without the peer identity key.
- Fragment responses are still validated against manifest SHA-256 before persistence.
- Invalid peer identity, expired source, oversized frame, timeout, replayed nonce, and corrupted fragment are rejected.
- Existing fallback to Replica/Edge or Origin remains available without preserving unvalidated bytes.
