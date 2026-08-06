# Changelog

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

