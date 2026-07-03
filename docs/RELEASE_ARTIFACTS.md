# Ponte Mesh SDK Release Artifacts

Release candidate artifacts should include:

- Linux shared library: `libpontemesh_sdk.so`
- Windows dynamic library: `pontemesh_sdk.dll`
- macOS dynamic library: `libpontemesh_sdk.dylib`
- static library where applicable: `libpontemesh_sdk.a`
- C header: `bindings/c/include/pontemesh_sdk.h`
- C# binding: `bindings/csharp/PontemeshSdk.cs`
- Unity package documentation: `bindings/unity/README.md`
- C++ wrapper: `bindings/cpp/pontemesh_sdk.hpp`
- production benchmark report: `target/pontemesh-benchmarks-production/report.md`

Build commands:

```bash
cargo build -p pontemesh-sdk-c --release
```

Cross-platform artifacts require building on the target platform or using a
configured cross-compilation toolchain.

