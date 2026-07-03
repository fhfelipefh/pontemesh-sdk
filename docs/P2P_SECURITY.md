# P2P Security

Peers are never trusted. Origin remains the authority for access packages, manifests, source authorization, expiration, revocation, and fallback.

## Current Transport Status

The current SDK production transport is libp2p request-response with a real identity key, `PeerId`, Noise authenticated encryption, Yamux multiplexing, request nonces, timeouts, strict fragment validation, and Origin fallback.

Current readiness:

```text
P2P funcional local: pronto
P2P seguro de produção: pronto
```

## Threat Model

| Threat | Mitigation |
| --- | --- |
| Malicious peer sends corrupted fragment | Receiver validates size and SHA-256 against manifest before persisting. |
| Peer tries to impersonate another peer | SDK validates `authorizedSources[].peerId` or `/p2p/<peerId>` endpoint identity against the authenticated libp2p connection `PeerId`. |
| Peer is not in `authorizedSources` | `SourceSelector` only uses sources supplied by Origin. |
| Peer tries to reuse `packageToken` | P2P request does not include `packageToken` or `applicationToken`. |
| Peer tries to download unauthorized object | Request is limited to package, manifest, fragment id, index, and byte range. Serving peer only serves locally validated fragments for that package/manifest. |
| Peer responds with wrong fragment index/id/range | Receiver checks response package, manifest, fragment id, index, size, nonce, and hash. |
| Peer sends invalid response | Receiver validates metadata, size, nonce, and SHA-256 before persisting. |
| Peer attempts path traversal | P2P protocol carries no local file path. |
| Peer attempts DoS with slow or stalled requests | Client request-response timeout and fallback prevent permanent blocking. |
| MITM between peers | libp2p Noise authenticates the remote `PeerId`; an attacker cannot satisfy an authorized `PeerId` without its identity key. |
| Replay of old message | Request nonce must be echoed by the response; mismatched nonce is rejected. |
| Revocation/expiration during download | Expired source/package is rejected; stronger live revocation requires Origin revalidation or peer transfer token support. |

## Token Rules

- `applicationToken` is never used in P2P.
- `packageToken` is not sent to peers.
- Credentials are not embedded in peer URLs.
- SDK events and transfer summaries do not include secrets.

## Server Contract

Origin emits peer-specific authorization:

```text
authorizedSources[].peerId
authorizedSources[].transport
authorizedSources[].endpoint
authorizedSources[].availableFragments
authorizedSources[].expiresAt
```

Optional future hardening:

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
