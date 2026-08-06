# Ponte Mesh SDK Production Tuning

This document summarizes production recommendations from the completed local
P2P production benchmark.

## Benchmark Basis

The production benchmark completed 432 results:

- object sizes: 1 MiB, 10 MiB, 100 MiB
- fragment sizes: 64 KiB, 256 KiB, 1 MiB
- downloaders: 1, 3, 5, 10
- runs: 3
- scenarios: origin-only, p2p-single-seeder, p2p-mesh, p2p-fallback
- result: 0 timeouts, 0 panics, valid SHA-256 for every object

The benchmark runs on local loopback. It proves SDK data-plane behavior and
integrity, but it does not model Internet latency, NAT traversal, relay, DHT, or
carrier/network policy.

## Memory Analysis

Observed peak:

```text
memory_peak_mb: 3779
```

The peak occurred in the 100 MiB, 1 MiB fragment, 10 downloader, P2P mesh
case. This is expected for the current benchmark architecture and transport
shape:

- the object factory keeps deterministic object bytes in memory as `Arc<[u8]>`
  so object generation is not repeated;
- every downloader assembles its own final object buffer for SHA-256 validation;
- P2P request-response CBOR carries each fragment response as one complete
  payload in memory;
- fallback scenarios can have peer and Origin buffers in flight across multiple
  concurrent downloaders;
- validated fragments are retained until object assembly completes.

The `Arc<[u8]>` optimization is active in the benchmark object factory and
prevents cloning the full source object for every source client. It does not
remove per-downloader assembly buffers or libp2p/CBOR response buffers.

Operational guidance:

- Budget memory for at least `object_size * concurrent_downloaders`, plus P2P
  response buffers, object assembly, and runtime overhead.
- For 100 MiB objects with 10 concurrent downloaders, reserve several GiB of
  memory. The local benchmark reached about 3.8 GiB peak.
- Prefer file-backed storage for large production objects when integrating the
  SDK into long-lived applications.
- Avoid unbounded concurrent object downloads; place a caller-side concurrency
  limit around `sync_object`.

## Throughput Analysis

Worst observed case:

```text
p2p-single-seeder
object size: 1 MiB
fragment size: 1 MiB
downloaders: 1
throughput: 27.777778 MiB/s
```

The current worst case is dominated by fixed libp2p setup and benchmark
scheduling cost amortized over a very small single-object transfer. Earlier
pre-upgrade runs also showed poor throughput with 64 KiB fragments, which is
consistent with per-fragment overhead rather than SHA-256 alone:

- 64 KiB creates many request-response round trips relative to payload size;
- each fragment has libp2p request-response framing and CBOR encode/decode cost;
- each fragment is individually validated and recorded;
- the benchmark uses real Noise/Yamux streams and real `PeerId` validation;
- small object plus small fragment size amplifies fixed setup and scheduling
  overhead.

Average P2P throughput by fragment size improved materially as fragment size
increased:

```text
64 KiB   p2p-single-seeder avg: 114.526 MiB/s
256 KiB  p2p-single-seeder avg: 169.895 MiB/s
1 MiB    p2p-single-seeder avg: 158.989 MiB/s
```

## Fragment Size Recommendation

Default recommendation:

```text
256 KiB to 1 MiB
```

Use 256 KiB when:

- you need finer fallback granularity;
- objects are small to medium;
- partial recovery is more important than maximum throughput.

Use 1 MiB when:

- objects are large;
- throughput is the primary concern;
- peers and Origin can afford larger in-flight buffers.

Avoid 64 KiB for production defaults unless there is a specific need for very
fine-grained recovery. It is useful as a stress case and for proving protocol
correctness under many fragments, but it has high per-request overhead.

## Downloader Concurrency

The SDK can support concurrent downloaders, but applications should cap
concurrency based on object size and memory budget.

Recommended starting points:

- 1 to 3 concurrent downloaders for large 100 MiB+ objects on memory-constrained
  clients;
- 5 to 10 concurrent downloaders on machines with several GiB available;
- load-test higher concurrency with `scripts/production-p2p-stress.sh` before
  release.

## When To Use P2P

Use P2P when:

- multiple clients in the same environment request the same object;
- Origin traffic reduction matters;
- clients can accept peer-to-peer local network behavior;
- fragments are authorized by Origin and validated by SHA-256.

Do not use P2P as a trust replacement. Origin remains the policy authority, and
every peer byte must validate against the manifest.

## When To Use Replica/Edge

Use Replica/Edge when:

- clients are not expected to see each other;
- NAT, firewall, enterprise network policy, or mobile networks make P2P
  unreliable;
- predictable latency matters more than Origin traffic reduction;
- a managed cache tier is available close to users.

## Expected Fallback

Fallback to Origin is normal when:

- a peer does not advertise a requested fragment;
- a peer is unavailable;
- a peer returns invalid metadata or bytes;
- the authorized peer identity does not match the libp2p connection `PeerId`;
- a fragment request times out.

Fallback is only acceptable when the final object SHA-256 validates.

## Local Benchmark Limitations

The production benchmark is local loopback. It verifies the SDK data path,
metrics, hashing, fallback, and peer traffic accounting. It does not measure:

- Internet latency and jitter;
- NAT traversal;
- relay behavior;
- DHT discovery;
- cross-region or mobile network performance.

Run production, stress, and soak profiles in representative environments before
publishing a customer-facing release.
