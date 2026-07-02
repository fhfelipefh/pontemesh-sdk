# SDK Requirements

The SDK must be embeddable in native and multiplatform applications, especially games and launchers.

Required consumers:

- C
- C++
- Rust
- C# and Unity
- Python wrapper
- future Node wrapper outside the native core, only if it calls the native library
- desktop native apps
- mobile apps in future platform work

The core must not require Node.js.

This repository must not keep a parallel TypeScript implementation or Node build pipeline.
