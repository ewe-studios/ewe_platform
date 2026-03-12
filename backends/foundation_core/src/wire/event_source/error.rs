//! SSE error types.
//!
//! WHY: SSE operations can fail for various reasons (parse errors, connection issues, etc.).
//! WHAT: Defines [`EventSourceError`] enum covering all SSE-specific error cases.

use std::fmt;

/// [`EventSourceError`] represents all possible errors in SSE operations.
///
/// WHY: Need specific error types for different failure modes.
/// WHAT: Enum covering HTTP errors, parse errors, connection issues, and invalid values.
#[derive(Debug)]
pub enum EventSourceError {
    /// HTTP client error during connection or request.
    Http(String),

    /// SSE parse error (malformed input).
    ParseError(String),

    /// Connection was closed by server.
    ConnectionClosed,

    /// Invalid retry value (negative or non-integer).
    InvalidRetry(String),

    /// I/O read error.
    ReadError(String),

    /// I/O write error (server-side).
    WriteError(String),

    /// Invalid URL (wrong scheme or format).
    InvalidUrl(String),

    /// Connection timeout.
    Timeout,
}

impl fmt::Display for EventSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventSourceError::Http(msg) => write!(f, "HTTP error: {msg}"),
            EventSourceError::ParseError(msg) => write!(f, "Parse error: {msg}"),
            EventSourceError::ConnectionClosed => write!(f, "Connection closed"),
            EventSourceError::InvalidRetry(msg) => write!(f, "Invalid retry value: {msg}"),
            EventSourceError::ReadError(msg) => write!(f, "Read error: {msg}"),
            EventSourceError::WriteError(msg) => write!(f, "Write error: {msg}"),
            EventSourceError::InvalidUrl(msg) => write!(f, "Invalid URL: {msg}"),
            EventSourceError::Timeout => write!(f, "Connection timeout"),
        }
    }
}

impl std::error::Error for EventSourceError {}

impl From<std::io::Error> for EventSourceError {
    fn from(err: std::io::Error) -> Self {
        EventSourceError::ReadError(err.to_string())
    }
}
