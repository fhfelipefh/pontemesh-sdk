# P2P Security

Peers are never trusted. Origin remains the authority for access packages, manifests, source authorization, expiration, revocation, and fallback.

## Current Transport Status

The current SDK transport is `experimental-tcp`: native Rust TCP with strict fragment validation, peer identity checks from authorized endpoints, request nonces, frame limits, timeouts, circuit breaker, and fallback.

It is not marked as the final secure transport because it does not yet provide a libp2p Noise/TLS authenticated encrypted channel. The production target remains libp2p with a stable `PeerId`, identity key, secure channel, request-response protocol, timeouts, backpressure, and rate limits.

## Threat Model

| Threat | Mitigation |
| --- | --- |
| Malicious peer sends corrupted fragment | Receiver validates size and SHA-256 against manifest before persisting. |
| Peer tries to impersonate another peer | SDK validates `authorizedSources[].peerId` or `/p2p/<peerId>` endpoint identity against the response `peerId`. |
| Peer is not in `authorizedSources` | `SourceSelector` only uses sources supplied by Origin. |
| Peer tries to reuse `packageToken` | P2P request does not include `packageToken` or `applicationToken`. |
| Peer tries to download unauthorized object | Request is limited to package, manifest, fragment id, index, and byte range. Serving peer only serves locally validated fragments for that package/manifest. |
| Peer responds with wrong fragment index/id/range | Receiver checks response package, manifest, fragment id, index, size, nonce, and hash. |
| Peer sends oversized response | Receiver enforces `MAX_FRAME_BYTES`; server enforces `MAX_REQUEST_BYTES`. |
| Peer attempts path traversal | P2P protocol carries no local file path. |
| Peer attempts DoS with many requests | Server enforces concurrent request limit; client has timeout and circuit breaker. |
| MITM between peers | Not fully mitigated in `experimental-tcp`; final transport must use libp2p Noise/TLS. |
| Replay of old message | Request nonce must be echoed by the response; mismatched nonce is rejected. |
| Revocation/expiration during download | Expired source/package is rejected; stronger live revocation requires Origin revalidation or peer transfer token support. |

## Token Rules

- `applicationToken` is never used in P2P.
- `packageToken` is not sent to peers.
- Credentials are not embedded in peer URLs.
- SDK events and transfer summaries do not include secrets.

## Required Server Contract Evolution

For a production secure transport, Origin should emit peer-specific authorization:

```text
authorizedSources[].peerId
authorizedSources[].transport
authorizedSources[].endpoint
authorizedSources[].availableFragments
authorizedSources[].expiresAt
```

Recommended next contract:

```text
peerTransferToken
audiencePeerId
packageId
bucket
key
manifestId
fragmentIndexes
expiresAt
```

The token should be short-lived, scoped to fragments, revocable, and verifiable by the serving peer without exposing administrative or application credentials.
