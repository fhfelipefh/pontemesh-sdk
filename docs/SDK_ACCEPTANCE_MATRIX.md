# SDK Acceptance Matrix

| Requirement | Status |
| --- | --- |
| TypeScript is not the core | Done |
| Node is not required | Done |
| Rust core exists | Done |
| C ABI exists | Done |
| C header exists | Done |
| C# binding exists | Done |
| Unity binding documentation exists | Done |
| C example exists | Done |
| C++ wrapper exists | Done |
| Python binding is documented as future wrapper | Done |
| Node legacy code removed | Done |
| CI blocks Node/TypeScript artifacts | Done |
| CI runs clippy with warnings denied | Done |
| Ponte Mesh contracts are modeled | Done |
| SHA-256 validation exists | Done |
| Peer transport is pluggable | Done |
| Native Rust P2P transport exists | Done |
| Peer server local optional exists | Done |
| Peer client downloads fragments | Done |
| Two local peers are covered by integration test | Done |
| PEER is selected before Replica/Origin | Done |
| Invalid peer hashes are rejected | Done |
| Unauthorized peer sources are rejected | Done |
| Disabled peer transport falls back | Done |
| Peer availability announcement exists | Done |
| TransferSummary measures bytes/fragments by source | Done |
| Multi-SDK P2P mesh simulation exists | Done |
| Malicious peer tests exist | Done |
| P2P traffic reduction test exists | Done |
| PeerId validation exists for current contract shape | Done |
| Frame limit, timeout, and circuit breaker exist | Done |
| P2P secure encrypted channel | Partial: requires libp2p Noise/TLS |
| NAT traversal, DHT, relay, WebRTC | Future |
| S3/MCP/admin are forbidden in core tests | Done |

## Validation Commands

```bash
cargo fmt
cargo test
cargo test -p pontemesh-sdk-core --test p2p_mesh_simulation -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test p2p_malicious_peer -- --ignored --nocapture
cargo test -p pontemesh-sdk-core --test p2p_traffic_reduction -- --ignored --nocapture
cargo build --release
```
