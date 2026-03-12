//! WebSocket error types.
//!
//! WHY: WebSocket operations can fail for various reasons (protocol errors, I/O, invalid frames).
//! WHAT: Defines [`WebSocketError`] enum covering all WebSocket-specific error cases.

use std::fmt;

/// WHY: WebSocket operations produce specific failure modes that callers need to distinguish.
///
/// WHAT: Enum covering all WebSocket error cases including upgrade failures, frame errors,
/// protocol violations, and I/O errors.
///
/// HOW: Each variant wraps the relevant context for the error type.
///
/// # Panics
/// Never panics.
#[derive(Debug)]
pub enum WebSocketError {
    /// HTTP upgrade failed with the given status code.
    UpgradeFailed(u16),

    /// Server returned an invalid Sec-WebSocket-Accept key.
    InvalidAcceptKey,

    /// Server response missing Sec-WebSocket-Accept header.
    MissingAcceptKey,

    /// Request missing Sec-WebSocket-Key header.
    MissingKey,

    /// Invalid WebSocket frame data.
    InvalidFrame(String),

    /// Invalid UTF-8 in a text frame payload.
    InvalidUtf8(std::string::FromUtf8Error),

    /// Connection was closed.
    ConnectionClosed,

    /// WebSocket protocol violation.
    ProtocolError(String),

    /// Underlying I/O error.
    IoError(std::io::Error),

    /// Invalid WebSocket URL.
    InvalidUrl(String),
}

impl fmt::Display for WebSocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebSocketError::UpgradeFailed(code) => write!(f, "upgrade failed: HTTP {code}"),
            WebSocketError::InvalidAcceptKey => write!(f, "invalid Sec-WebSocket-Accept key"),
            WebSocketError::MissingAcceptKey => write!(f, "missing Sec-WebSocket-Accept header"),
            WebSocketError::MissingKey => write!(f, "missing Sec-WebSocket-Key header"),
            WebSocketError::InvalidFrame(msg) => write!(f, "invalid frame: {msg}"),
            WebSocketError::InvalidUtf8(err) => write!(f, "invalid UTF-8: {err}"),
            WebSocketError::ConnectionClosed => write!(f, "connection closed"),
            WebSocketError::ProtocolError(msg) => write!(f, "protocol error: {msg}"),
            WebSocketError::IoError(err) => write!(f, "I/O error: {err}"),
            WebSocketError::InvalidUrl(msg) => write!(f, "invalid URL: {msg}"),
        }
    }
}

impl std::error::Error for WebSocketError {}

impl From<std::io::Error> for WebSocketError {
    fn from(err: std::io::Error) -> Self {
        WebSocketError::IoError(err)
    }
}

impl From<std::string::FromUtf8Error> for WebSocketError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        WebSocketError::InvalidUtf8(err)
    }
}
