# TCC SDK Rules

- Rust is the core.
- TypeScript is not the core.
- Node.js is not required.
- Legacy TypeScript/Node code must not remain in-tree.
- The C ABI is the universal integration bridge.
- Wrappers must call the same native core.
- The SDK hides manifests, fragments, Replica/Edge, peer fallback, hashes, revalidation and access packages from consuming apps.
- The SDK only uses Ponte Mesh protocol endpoints.
- The SDK does not use S3, MCP, admin APIs, database access or migrations.
