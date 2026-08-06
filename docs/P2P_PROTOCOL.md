# P2P Protocol

The current production P2P protocol is libp2p request-response over authenticated encrypted channels. The SDK uses a real libp2p `PeerId`, Noise secure channel, Yamux multiplexing, and CBOR-framed fragment requests on `/pontemesh/fragment/1`.

```text
P2P funcional local: pronto
P2P seguro de produção: pronto
```

Endpoints use:

```text
/ip4/127.0.0.1/tcp/4001/p2p/<peerId>
```

Origin must authorize peers with `authorizedSources[].peerId`, `transport=libp2p`, and a libp2p multiaddr endpoint containing `/p2p/<PeerId>`.

## Request

CBOR payload:

- `protocolVersion`
- `packageId`
- `bucket`
- `key`
- `manifestId`
- `fragmentId`
- `fragmentIndex`
- `byteRangeStart`
- `byteRangeEnd`
- `requestNonce`

## Response

CBOR payload:

- `protocolVersion`
- `packageId`
- `manifestId`
- `fragmentId`
- `fragmentIndex`
- `sizeBytes`
- `sha256`
- `requestNonce`
- `bytes`

## Limits

- Request timeout: 5 seconds.
- libp2p idle connection timeout: 30 seconds.
- Origin and Replica/Edge fallback remains available.

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
- `transport=libp2p`.
- Source is not expired.
- Authenticated remote `PeerId` from the libp2p connection matches the authorized source.
- Protocol version and nonce match.
- Fragment metadata, size, and SHA-256 match the manifest.

No P2P message contains `applicationToken`, `packageToken`, credentials, or local file paths.
