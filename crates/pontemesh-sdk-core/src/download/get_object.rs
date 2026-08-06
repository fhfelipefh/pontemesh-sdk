use crate::errors::PontemeshError;

pub struct GetObjectResult {
    pub bytes: Vec<u8>,
}

pub fn get_object_from_bytes(bytes: Vec<u8>) -> Result<GetObjectResult, PontemeshError> {
    Ok(GetObjectResult { bytes })
}
