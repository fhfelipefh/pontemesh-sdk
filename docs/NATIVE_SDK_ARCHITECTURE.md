# Native SDK Architecture

The Ponte Mesh SDK core is Rust.

TypeScript is not the core. Node.js, Next.js and npm are not required to use the SDK from native applications.

The C ABI in `bindings/c` is the universal bridge. C, C++, C#, Unity and future Python/Node wrappers call the same native library:

- Windows: `pontemesh_sdk.dll`
- Linux: `libpontemesh_sdk.so`
- macOS: `libpontemesh_sdk.dylib`

The consumer application provides:

- Origin URL
- application credential
- bucket
- object key
- destination path

The SDK handles:

- access package creation
- manifest parsing
- source selection
- fragment downloads using Range requests
- peer fallback preparation
- Replica/Edge fallback
- Origin fallback
- fragment SHA-256 validation
- final object SHA-256 validation
- preserving validated fragments during sync

The SDK must not expose Rust structs directly through FFI. It exposes opaque handles and status codes.

No legacy TypeScript or Node implementation is kept in-tree. CI runs `scripts/check-no-dead-code.sh` and `cargo clippy --workspace --all-targets -- -D warnings` to prevent dead code and Node artifacts from returning.
