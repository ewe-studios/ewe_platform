/// Error types for the IPC bus.

use thiserror::Error;

use super::version::Version;

/// Internal error — used throughout the IPC layer.
#[derive(Debug, Error)]
pub enum Error {
    #[error("encode error: {0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[error("decode error: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("type uuid not found")]
    TypeUuidNotFound,

    #[error("timeout")]
    Timeout,

    #[error("disconnected")]
    Disconnect,

    #[error("version mismatch: {0}")]
    VersionMismatch(Version, Option<String>),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("identifier in use")]
    IdentifierInUse,

    #[error("identifier not in use")]
    IdentifierNotInUse,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("memory region mapping error")]
    MemoryRegionMapping,

    #[error("permission denied")]
    PermissionDenied,

    #[error("unknown error")]
    Unknown,
}

/// Error from `join()` — endpoint couldn't connect to bus.
#[derive(Debug, Error)]
pub enum JoinError {
    #[error("version mismatch: {0}")]
    VersionMismatch(Version),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("timeout")]
    Timeout,

    #[error("permission denied")]
    PermissionDenied,
}

/// Error from `send()` — message couldn't be sent.
#[derive(Debug, Error)]
pub enum SendError {
    #[error("timeout")]
    Timeout,

    #[error("version mismatch: {0}")]
    VersionMismatch(Version),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("permission denied")]
    PermissionDenied,
}

impl From<JoinError> for SendError {
    fn from(value: JoinError) -> Self {
        match value {
            JoinError::VersionMismatch(v) => Self::VersionMismatch(v),
            JoinError::TokenMismatch => Self::TokenMismatch,
            JoinError::Timeout => Self::Timeout,
            JoinError::PermissionDenied => Self::PermissionDenied,
        }
    }
}

/// Error from `recv()` — message couldn't be received.
#[derive(Debug, Error)]
pub enum RecvError {
    #[error("decode error: {0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[error("timeout")]
    Timeout,

    #[error("version mismatch: {0}")]
    VersionMismatch(Version),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("permission denied")]
    PermissionDenied,
}

impl From<JoinError> for RecvError {
    fn from(value: JoinError) -> Self {
        match value {
            JoinError::VersionMismatch(v) => Self::VersionMismatch(v),
            JoinError::TokenMismatch => Self::TokenMismatch,
            JoinError::Timeout => Self::Timeout,
            JoinError::PermissionDenied => Self::PermissionDenied,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::IoError(_)));
    }

    #[test]
    fn join_error_to_send_error() {
        let join = JoinError::Timeout;
        let send: SendError = join.into();
        assert!(matches!(send, SendError::Timeout));
    }

    #[test]
    fn join_error_to_recv_error() {
        let join = JoinError::TokenMismatch;
        let recv: RecvError = join.into();
        assert!(matches!(recv, RecvError::TokenMismatch));
    }
}
