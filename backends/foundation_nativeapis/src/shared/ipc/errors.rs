/// Error types for the IPC bus.
use derive_more::{Display, Error, From};

use super::version::Version;

/// Internal error — used throughout the IPC layer.
#[derive(Debug, Display, Error, From)]
pub enum Error {
    #[display("encode error: {_0}")]
    Encode(#[from] bincode::error::EncodeError),

    #[display("decode error: {_0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[display("type uuid not found")]
    TypeUuidNotFound,

    #[display("timeout")]
    Timeout,

    #[display("disconnected")]
    Disconnect,

    #[display("version mismatch: {_0}")]
    VersionMismatch(Version, Option<String>),

    #[display("token mismatch")]
    TokenMismatch,

    #[display("identifier in use")]
    IdentifierInUse,

    #[display("identifier not in use")]
    IdentifierNotInUse,

    #[display("I/O error: {_0}")]
    IoError(#[from] std::io::Error),

    #[display("memory region mapping error")]
    MemoryRegionMapping,

    #[display("permission denied")]
    PermissionDenied,

    #[display("unknown error")]
    Unknown,
}

/// Error from `join()` — endpoint couldn't connect to bus.
#[derive(Debug, Display, Error, From)]
pub enum JoinError {
    #[display("version mismatch: {_0}")]
    VersionMismatch(#[error(not(source))] Version),

    #[display("token mismatch")]
    TokenMismatch,

    #[display("timeout")]
    Timeout,

    #[display("permission denied")]
    PermissionDenied,
}

/// Error from `send()` — message couldn't be sent.
#[derive(Debug, Display, Error, From)]
pub enum SendError {
    #[display("timeout")]
    Timeout,

    #[display("version mismatch: {_0}")]
    VersionMismatch(#[error(not(source))] Version),

    #[display("token mismatch")]
    TokenMismatch,

    #[display("permission denied")]
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
#[derive(Debug, Display, Error, From)]
pub enum RecvError {
    #[display("decode error: {_0}")]
    Decode(#[from] bincode::error::DecodeError),

    #[display("timeout")]
    Timeout,

    #[display("version mismatch: {_0}")]
    VersionMismatch(#[error(not(source))] Version),

    #[display("token mismatch")]
    TokenMismatch,

    #[display("permission denied")]
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
