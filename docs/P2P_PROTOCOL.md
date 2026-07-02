# P2P Protocol

The current protocol is `experimental-tcp` v1: newline-delimited JSON over TCP with base64 fragment bytes. It is acceptable for local SDK mesh simulation and transport contract validation, but less efficient than binary framing and not the final secure transport.

```text
P2P funcional local: pronto
P2P seguro de produção: parcial
```

Endpoints use:

```text
peer://127.0.0.1:4001/p2p/<peerId>
```

The SDK also accepts future Origin contracts with explicit `authorizedSources[].peerId`.

## Request

```json
{
  "type": "fragmentRequest",
  "protocolVersion": 1,
  "packageId": "pkg_...",
  "manifestId": "manifest_...",
  "bucket": "game-assets",
  "key": "maps/desert-v3.pak",
  "fragmentId": "fragment_...",
  "fragmentIndex": 0,
  "byteRangeStart": 0,
  "byteRangeEnd": 1048575,
  "requestNonce": "..."
}
```

## Response

```json
{
  "type": "fragmentResponse",
  "protocolVersion": 1,
  "packageId": "pkg_...",
  "manifestId": "manifest_...",
  "fragmentId": "fragment_...",
  "fragmentIndex": 0,
  "sizeBytes": 1048576,
  "sha256": "...",
  "requestNonce": "...",
  "peerId": "peer_...",
  "bytesBase64": "..."
}
```

## Limits

- Max request frame: 16 KiB.
- Max response frame: 2 MiB.
- Request timeout: 5 seconds.
- Concurrent server requests: 32.
- Circuit breaker opens after repeated peer failures.

## Validation

Before serving, the peer checks:

- `allowPeerSharing=true`.
- Package is not expired.
- Fragment is locally validated.
- Manifest, bucket, key, fragment id, index, and byte range match.
- Local bytes match the manifest SHA-256.

Before accepting, the downloader checks:

- Source is authorized by Origin.
- `sourceType=PEER`.
- Source is not expired.
- Endpoint/peer id matches the authorized source when available.
- Protocol version and nonce match.
- Fragment metadata, size, and SHA-256 match the manifest.

No P2P message contains `applicationToken`, `packageToken`, credentials, or local file paths.
