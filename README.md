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
use pontemesh_sdk_core::{PontemeshClient, PontemeshClientConfig, SyncObjectRequest};

let client = PontemeshClient::new(PontemeshClientConfig {
    origin_url: "https://origin.example.com".to_string(),
    application_token: "application-token".to_string(),
});

client.sync_object(SyncObjectRequest {
    bucket: "game-assets".to_string(),
    key: "maps/desert-v3.pak".to_string(),
    destination: "./Game/Content/maps/desert-v3.pak".into(),
})?;
# Ok::<(), pontemesh_sdk_core::PontemeshError>(())
```

## Validate

```bash
cargo fmt
cargo test
cargo build --release
cargo build -p pontemesh-sdk-c --release
./scripts/check-no-dead-code.sh
```
