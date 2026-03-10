//! Response intro wrapper for HTTP client.
//!
//! This module provides `ResponseIntro` wrapper that holds the initial
//! response information from `IncomingResponseParts::Intro`.

use crate::wire::simple_http::{Proto, Status};

/// HTTP response intro (status line).
///
/// This wrapper holds the initial response parsing result:
/// status code, protocol version, and optional reason phrase.
///
/// Corresponds to `IncomingResponseParts::Intro(Status, Proto, Option<String>)`.
#[derive(Debug, Clone)]
pub struct ResponseIntro {
    pub status: Status,
    pub proto: Proto,
    pub reason: Option<String>,
}

impl From<(Status, Proto, Option<String>)> for ResponseIntro {
    fn from((status, proto, reason): (Status, Proto, Option<String>)) -> Self {
        Self {
            status,
            proto,
            reason,
        }
    }
}
