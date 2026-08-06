# Ponte Mesh SDK Release Artifacts

Release candidate artifacts should include:

- Linux shared library: `libpontemesh_sdk.so`
- Windows dynamic library: `pontemesh_sdk.dll`
- macOS Intel and ARM dynamic library: `libpontemesh_sdk.dylib`
- static library where applicable: `libpontemesh_sdk.a`
- C header: `bindings/c/include/pontemesh_sdk.h`
- C# binding: `bindings/csharp/PontemeshSdk.cs`
- Unity package documentation: `bindings/unity/README.md`
- C++ wrapper: `bindings/cpp/pontemesh_sdk.hpp`
- release manifest: `pontemesh-sdk-v<VERSION>-manifest.json`
- production benchmark report: `target/pontemesh-benchmarks-production/report.md`

Build commands:

```bash
cargo build -p pontemesh-sdk-c --release
```

The release workflow builds each artifact on a runner with the matching target
architecture and rejects a package when either its dynamic or static library is
missing.

The release manifest is mandatory for automated update staging. It must list
each downloadable package with its file name, byte size, and SHA-256 digest.
