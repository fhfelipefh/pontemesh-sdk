# Changelog

## 0.2.2

### Changed

- Migrated `libp2p` dependency from git (rev-pinned `v0.57.0-dev`) to published
  `v0.56.0` on crates.io, enabling crate publication.
- Added crates.io metadata (`description`, `keywords`, `categories`, `readme`)
  to `pontemesh-sdk-core`.
- Internal crates (`pontemesh-sdk-c`, `pontemesh-live-client`, `p2p-bench`)
  marked as `publish = false`.

### Added

- Automated `cargo publish` to crates.io in the SDK release workflow
  (`workflow_dispatch`). Pre-release versions are skipped.

## 0.1.0-rc.1

Initial public release candidate.

### Added

- Native Rust core for Ponte Mesh object synchronization.
- C ABI for native integration.
- C header for C/C++ consumers.
- C# P/Invoke binding.
- Unity binding documentation and wrapper layout.
- C++ RAII wrapper over the C ABI.
- libp2p P2P transport using real `PeerId`.
- Noise secure channel.
- Yamux multiplexing.
- request-response CBOR fragment transfer.
- SHA-256 fragment and object validation.
- Origin fallback for unavailable or invalid peer fragments.
- Production P2P benchmark with Origin-only, single-seeder, mesh, and fallback
  scenarios.
- Production, stress, and soak benchmark scripts.

