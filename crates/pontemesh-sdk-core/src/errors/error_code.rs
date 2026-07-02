#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidArgument,
    OriginRequestFailed,
    AccessDenied,
    HashMismatch,
    NoSourceAvailable,
    IoError,
    Cancelled,
    PeerTransportNotEnabled,
    InternalError,
}
