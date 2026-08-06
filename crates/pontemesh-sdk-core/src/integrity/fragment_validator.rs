use crate::contracts::FragmentDescriptor;
use crate::errors::PontemeshError;
use crate::integrity::sha256_hex;

pub fn validate_fragment(
    fragment: &FragmentDescriptor,
    bytes: &[u8],
) -> Result<(), PontemeshError> {
    if bytes.len() != fragment.size_bytes {
        return Err(PontemeshError::HashMismatch(format!(
            "fragment {} size mismatch: expected {}, got {}",
            fragment.index,
            fragment.size_bytes,
            bytes.len()
        )));
    }
    let digest = sha256_hex(bytes);
    if !digest.eq_ignore_ascii_case(&fragment.sha256) {
        return Err(PontemeshError::HashMismatch(format!(
            "fragment {} sha256 mismatch",
            fragment.index
        )));
    }
    Ok(())
}
