# TCC SDK Rules

- Rust is the core.
- TypeScript is not the core.
- Node.js is not required.
- Legacy TypeScript/Node code must not remain in-tree.
- The C ABI is the universal integration bridge.
- Wrappers must call the same native core.
- The SDK hides manifests, fragments, Replica/Edge, peer fallback, hashes, revalidation and access packages from consuming apps.
- The SDK only uses Ponte Mesh protocol endpoints.
- The SDK does not use S3, MCP, admin APIs, database access or migrations.
- P2P is a native Rust data-plane feature, not a Node or browser dependency.
- P2P can only use peers authorized by Origin.
- P2P can only share locally validated fragments when `allowPeerSharing=true`.
- Every fragment received from a peer must be validated by SHA-256 before use.
- Origin remains the final authority and fallback guarantee.
- The current TCP transport is explicitly experimental until replaced by libp2p Noise/TLS or equivalent secure transport.
- TCC status is `Parcial` for secure P2P until authenticated encrypted peer channels are implemented.
- P2P traffic proof must use `TransferSummary.bytes_from_peer > 0` and valid final object hashes.
