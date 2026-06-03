/// Error types for the IPC bus.

use std::io;

use thiserror::Error;

use super::version::Version;

/// Internal error — used throughout the IPC layer.
#[derive(Error, Debug)]
pub enum IpcError {
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

    #[error("version mismatch: local={0} remote={1:?}")]
    VersionMismatch(Version, Option<String>),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("bus identifier already in use")]
    IdentifierInUse,

    #[error("bus identifier not in use")]
    IdentifierNotInUse,

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("memory region mapping failed")]
    MemoryRegionMapping,

    #[error("permission denied")]
    PermissionDenied,

    #[error("unknown error")]
    Unknown,
}

/// Public alias for backward compatibility.
pub type Error = IpcError;

/// Result type for internal IPC operations.
pub type IpcResult<T> = std::result::Result<T, IpcError>;

/// Public result alias.
pub type Result<T> = IpcResult<T>;

/// Error from `join()` — endpoint couldn't connect to bus.
#[derive(Error, Debug)]
pub enum JoinError {
    #[error("version mismatch: expected {0}")]
    VersionMismatch(Version),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("timeout")]
    Timeout,

    #[error("permission denied")]
    PermissionDenied,

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Error from `send()` — message couldn't be sent.
#[derive(Error, Debug)]
pub enum SendError {
    #[error("timeout")]
    Timeout,

    #[error("version mismatch: {0}")]
    VersionMismatch(Version),

    #[error("token mismatch")]
    TokenMismatch,

    #[error("permission denied")]
    PermissionDenied,

    #[error("disconnected")]
    Disconnect,

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Error from `recv()` — message couldn't be received.
#[derive(Error, Debug)]
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

    #[error("disconnected")]
    Disconnect,

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

impl From<JoinError> for SendError {
    fn from(err: JoinError) -> Self {
        match err {
            JoinError::VersionMismatch(v) => SendError::VersionMismatch(v),
            JoinError::TokenMismatch => SendError::TokenMismatch,
            JoinError::Timeout => SendError::Timeout,
            JoinError::PermissionDenied => SendError::PermissionDenied,
            JoinError::Io(e) => SendError::Io(e),
        }
    }
}

impl From<JoinError> for RecvError {
    fn from(err: JoinError) -> Self {
        match err {
            JoinError::VersionMismatch(v) => RecvError::VersionMismatch(v),
            JoinError::TokenMismatch => RecvError::TokenMismatch,
            JoinError::Timeout => RecvError::Timeout,
            JoinError::PermissionDenied => RecvError::PermissionDenied,
            JoinError::Io(e) => RecvError::Io(e),
        }
    }
}

impl From<IpcError> for JoinError {
    fn from(err: IpcError) -> Self {
        match err {
            IpcError::VersionMismatch(v, _) => JoinError::VersionMismatch(v),
            IpcError::TokenMismatch => JoinError::TokenMismatch,
            IpcError::Timeout => JoinError::Timeout,
            IpcError::PermissionDenied => JoinError::PermissionDenied,
            IpcError::Io(e) => JoinError::Io(e),
            IpcError::Disconnect => JoinError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "disconnected",
            )),
            _ => JoinError::Io(io::Error::new(io::ErrorKind::Other, format!("{err}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::Other, "test");
        let ipc: IpcError = io_err.into();
        assert!(matches!(ipc, IpcError::Io(_)));
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
