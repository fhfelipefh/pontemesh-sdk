# Language Bindings

The stable native boundary is the C ABI.

## C

Use `bindings/c/include/pontemesh_sdk.h` and link against the compiled platform library.

## C++

`bindings/cpp/pontemesh_sdk.hpp` provides a small RAII wrapper. It does not replace the C ABI.

## C# and Unity

`bindings/csharp/PontemeshSdk.cs` and `bindings/unity/Assets/Plugins/PonteMesh/PontemeshSdk.cs` use P/Invoke.

Unity loads:

- Windows: `pontemesh_sdk.dll`
- Linux: `libpontemesh_sdk.so`
- macOS: `libpontemesh_sdk.dylib`

## Python

Python will be a wrapper, not the core. Acceptable future routes:

- `pyo3` with `maturin`
- `ctypes` loading the C ABI

## Node

Node is not implemented in this repository. A future Node package must be a wrapper over the native C ABI or Rust core, not a parallel implementation.

Acceptable future routes:

- `napi-rs`
- `neon`
- `ffi-napi`
