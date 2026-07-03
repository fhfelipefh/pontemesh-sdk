# libp2p Production Transport Plan

Production P2P uses libp2p request-response with authenticated encrypted peer channels. Older TCP fixtures are retained only for legacy contract tests and are not the production data-plane.

## Required Transport

- Use `libp2p`.
- Use Noise for encrypted authenticated channels.
- Use cryptographic `PeerId` derived from the peer identity key.
- Implement request-response protocol `/pontemesh/fragment/1` for fragment transfer.
- Preserve client and server timeouts.
- Validate remote peer identity from the libp2p connection.
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
