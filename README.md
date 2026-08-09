# Ponte Mesh SDK

`pontemesh-sdk` is a native, embeddable SDK for applications that need to download Ponte Mesh objects without knowing about manifests, fragments, fallback, Replica/Edge, peers, hashes or access packages.

The SDK core is Rust. It does not require Node.js, Next.js, TypeScript or npm.

## Architecture

- `crates/pontemesh-sdk-core`: native Rust core.
- `bindings/c`: C ABI that builds `pontemesh_sdk.dll`, `libpontemesh_sdk.so` and `libpontemesh_sdk.dylib`.
- `bindings/csharp`: initial P/Invoke wrapper.
- `bindings/unity`: Unity-ready C# wrapper and loading notes.
- `bindings/cpp`: small C++ RAII wrapper over the C ABI.
- `bindings/python`: future Python binding plan.
- Future Node support must be implemented as a native wrapper in a separate task. No Node or TypeScript implementation is kept in this repository.

## Rust usage

```rust
use pontemesh_sdk_core::{p2p::P2pConfig, PontemeshClient, PontemeshClientConfig, SyncObjectRequest};

let client = PontemeshClient::new(PontemeshClientConfig {
    origin_url: "https://origin.example.com".to_string(),
    application_token: "application-token".to_string(),
    p2p: P2pConfig::default(),
})?;

let result = client.sync_object_with_summary(SyncObjectRequest {
    bucket: "game-assets".to_string(),
    key: "maps/desert-v3.pak".to_string(),
    destination: "./Game/Content/maps/desert-v3.pak".into(),
})?;
println!(
    "downloaded via peer={}, replica={}, origin={}",
    result.summary.bytes_from_peer,
    result.summary.bytes_from_replica,
    result.summary.bytes_from_origin
);
# Ok::<(), pontemesh_sdk_core::PontemeshError>(())
```

For large objects, use the disk-streaming API. It keeps validated fragments in a
persistent cache, revalidates cached hashes after a restart, assembles through a
temporary file, and restores the previous destination if installation fails:

```rust
use pontemesh_sdk_core::CancellationToken;

let cancellation = CancellationToken::default();
let summary = client.sync_object_to_disk_with_options(
    SyncObjectRequest {
        bucket: "game-assets".to_string(),
        key: "maps/desert-v3.pak".to_string(),
        destination: "./Game/Content/maps/desert-v3.pak".into(),
    },
    None,
    cancellation,
)?;
# Ok::<(), pontemesh_sdk_core::PontemeshError>(())
```

`sync_object_to_disk_async` provides the same behavior without blocking an async UI
runtime. Progress callbacks report validated bytes for the complete object, not only
the current fragment. `release::ReleaseManifest` validates version descriptors,
ordered multi-file installs, hashes, sizes, duplicate paths, and path traversal.

## Validate

Production P2P defaults to libp2p + Noise + Yamux + request-response CBOR. The old TCP peer client/server is isolated behind the `legacy-tcp-dev` feature, which is disabled by default and is not part of the production gate.

```bash
cargo fmt -- --check
cargo test
cargo build --release
cargo build -p pontemesh-sdk-c --release
bash ./scripts/libp2p-release-gate.sh
./scripts/production-no-mock-gate.sh
```

Run the SDK against a live Ponte Mesh server with:

```bash
PONTEMESH_LIVE_ORIGIN_URL=http://127.0.0.1:8080 \
PONTEMESH_LIVE_APPLICATION_TOKEN=pm_app_... \
PONTEMESH_LIVE_BUCKET=game-assets \
PONTEMESH_LIVE_KEY=maps/desert-v3.pak \
./scripts/sdk-server-integration-gate.sh
```

`PONTEMESH_LIVE_EXPECTED_SHA256` is optional and, when set, is checked against the downloaded object.
