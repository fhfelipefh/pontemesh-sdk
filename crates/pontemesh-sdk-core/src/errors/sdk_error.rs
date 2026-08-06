use thiserror::Error;

use super::ErrorCode;

#[derive(Debug, Error)]
pub enum PontemeshError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("origin request failed: {0}")]
    OriginRequestFailed(String),
    #[error("access denied: {0}")]
    AccessDenied(String),
    #[error("hash mismatch: {0}")]
    HashMismatch(String),
    #[error("no source available")]
    NoSourceAvailable,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("request cancelled")]
    Cancelled,
    #[error("peer transport is not enabled")]
    PeerTransportNotEnabled,
    #[error("internal error: {0}")]
    Internal(String),
}

impl PontemeshError {
    pub fn code(&self) -> ErrorCode {
        match self {
            PontemeshError::InvalidArgument(_) => ErrorCode::InvalidArgument,
            PontemeshError::OriginRequestFailed(_) => ErrorCode::OriginRequestFailed,
            PontemeshError::AccessDenied(_) => ErrorCode::AccessDenied,
            PontemeshError::HashMismatch(_) => ErrorCode::HashMismatch,
            PontemeshError::NoSourceAvailable => ErrorCode::NoSourceAvailable,
            PontemeshError::Io(_) => ErrorCode::IoError,
            PontemeshError::Cancelled => ErrorCode::Cancelled,
            PontemeshError::PeerTransportNotEnabled => ErrorCode::PeerTransportNotEnabled,
            PontemeshError::Internal(_) => ErrorCode::InternalError,
        }
    }
}
