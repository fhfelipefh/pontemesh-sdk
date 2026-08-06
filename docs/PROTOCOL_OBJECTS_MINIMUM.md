# Protocol Objects Minimum

The Rust core models the SDK-facing contracts with `serde(rename_all = "camelCase")`.

Minimum objects:

- `AccessPackage`
- `Manifest`
- `FragmentDescriptor`
- `AuthorizedSource`
- `SourceSelectionContract`
- `FallbackContract`
- availability and policy fields for protocol growth

`SourceType` is serialized as `ORIGIN`, `REPLICA_EDGE` and `PEER`.
