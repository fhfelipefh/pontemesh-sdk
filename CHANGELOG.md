# Changelog

## 0.3.0

### Added

- **Runtime Resource Suspension**: Added `suspend()`, `resume()`, `set_active(bool)`, `is_active()`, and `is_suspended()` to `Client` and `OriginClient`. Disables network operations, terminates libp2p background worker threads, releases listening sockets, and drops manifest and fragment memory caches, freeing memory and CPU when the client is idle.
- **Allowed Directory Sandboxing**: Added `add_allowed_directory` and `clear_allowed_directories` to configure path whitelists. Enforces strict boundary checks preventing any writes, temporary files, or downloads outside developer-authorized local folders.
- **Security Hardening & Backdoor Prevention**:
  - Full SHA-256 cryptographic verification for every fragment against Origin manifests.
  - Strict path normalization and path traversal rejection (`../`, absolute paths, Windows drive escapes, symlink traversal, and null byte injection).
  - Isolated state: suspended mode ensures zero listening ports and zero network activity.
- **Software Release Versioning**: Version comparison, semantic versioning rules, and launcher application credentials compatibility.
- **Language Bindings Updated**: C ABI (`pontemesh_sdk.h`), C++ wrapper (`pontemesh_sdk.hpp`), and C# / Unity (`PontemeshSdk.cs`) support suspension controls, directory whitelisting, and new status codes (`PONTEMESH_SUSPENDED`, `PONTEMESH_PATH_NOT_ALLOWED`).
- **Error Codes**: Added `PontemeshError::Suspended` and `PontemeshError::PathNotAllowed`.

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

