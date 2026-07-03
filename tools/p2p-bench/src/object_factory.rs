use pontemesh_sdk_core::contracts::{FragmentDescriptor, Manifest};
use pontemesh_sdk_core::integrity::sha256_hex;

const SEED: &[u8] = b"pontemesh-benchmark-seed";

#[derive(Debug, Clone)]
pub struct BenchmarkObject {
    pub bytes: Vec<u8>,
    pub manifest: Manifest,
}

pub fn build_object(size: u64, fragment_size: usize) -> BenchmarkObject {
    let mut bytes = Vec::with_capacity(size as usize);
    let mut counter = 0_u64;
    while bytes.len() < size as usize {
        let mut block = Vec::with_capacity(SEED.len() + 8);
        block.extend_from_slice(SEED);
        block.extend_from_slice(&counter.to_le_bytes());
        bytes.extend_from_slice(sha256_hex(&block).as_bytes());
        counter += 1;
    }
    bytes.truncate(size as usize);
    let manifest = manifest(&bytes, fragment_size);
    BenchmarkObject { bytes, manifest }
}

fn manifest(bytes: &[u8], fragment_size: usize) -> Manifest {
    let mut fragments = Vec::new();
    for (index, chunk) in bytes.chunks(fragment_size).enumerate() {
        let start = index * fragment_size;
        let end = start + chunk.len() - 1;
        fragments.push(FragmentDescriptor {
            index,
            fragment_id: format!("fragment-{index}"),
            byte_range_start: start as u64,
            byte_range_end: end as u64,
            size_bytes: chunk.len(),
            hash_algorithm: "SHA256".to_string(),
            sha256: sha256_hex(chunk),
            priority: "NORMAL".to_string(),
            fallback_range_header: format!("bytes={start}-{end}"),
        });
    }
    Manifest {
        manifest_id: format!("manifest-{}-{fragment_size}", bytes.len()),
        object_id: format!("object-{}", bytes.len()),
        bucket: "pontemesh-benchmark".to_string(),
        key: format!("objects/{}-{fragment_size}.bin", bytes.len()),
        version: "v1".to_string(),
        total_size_bytes: bytes.len() as i64,
        content_type: "application/octet-stream".to_string(),
        object_hash_algorithm: "SHA256".to_string(),
        object_sha256: sha256_hex(bytes),
        fragment_size_bytes: fragment_size,
        fragments,
        availability_state: "AVAILABLE".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
    }
}
