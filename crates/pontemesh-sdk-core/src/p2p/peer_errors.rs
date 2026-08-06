use crate::errors::PontemeshError;

pub fn peer_error(message: impl Into<String>) -> PontemeshError {
    PontemeshError::Internal(format!("peer transport error: {}", message.into()))
}
