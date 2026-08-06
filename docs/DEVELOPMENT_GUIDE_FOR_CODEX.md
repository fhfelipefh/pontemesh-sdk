# Development Guide For Codex

Work from the native SDK boundary.

Do:

- edit `crates/pontemesh-sdk-core` for SDK behavior
- edit `bindings/c` for ABI changes
- keep FFI handles opaque
- catch panics before returning through FFI
- add Rust tests for protocol behavior
- run `cargo fmt`, `cargo test` and `cargo build --release`
- run `./scripts/check-no-dead-code.sh`
- run `cargo clippy --workspace --all-targets -- -D warnings`

Do not:

- reintroduce a TypeScript core
- add legacy TypeScript/JavaScript source, package locks or Node build steps
- require Node.js for native use
- use S3, MCP, admin APIs, database access or migrations in the SDK core
