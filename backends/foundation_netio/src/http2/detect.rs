//! HTTP/2 connection detection — protocol negotiation entry paths.
//!
//! WHY: Before the HTTP/2 connection state machine can start, we must
//! determine whether the peer speaks HTTP/1.1 or HTTP/2. This module
//! implements the three entry paths from Decision 12 §Decided Details #4:
//!
//! - **h2c prior-knowledge**: peek ≤ 24 bytes for the client preface magic
//!   (`PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`). If found, the connection is h2.
//! - **TLS-ALPN**: rustls `set_protocols([b"h2", b"http/1.1"])`; the
//!   negotiated protocol is available from the TLS session after the
//!   handshake — no application bytes are needed.
//! - **Upgrade: h2c**: the HTTP/1.1 Upgrade handshake (`Connection: Upgrade,
//!   HTTP2-Settings` + `Upgrade: h2c`) → server responds `101 Switching
//!   Protocols`, both sides switch to h2 framing, and the initiating request
//!   replays as stream 1.
//!
//! WHAT: [`is_h2c_preface`] checks a byte buffer for the magic string.
//! The ALPN and Upgrade paths are wired into `foundation_http`'s
//! `ConnectionHandler` — this module provides the detection primitives.

use super::connection::CLIENT_PREFACE;

/// Check whether `buf` starts with the HTTP/2 client connection preface.
///
/// The preface is the 24-byte magic string `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`
/// (RFC 7540 §3.5). Callers should peek at least 24 bytes from the socket
/// before calling this.
#[must_use]
pub fn is_h2c_preface(buf: &[u8]) -> bool {
    buf.len() >= CLIENT_PREFACE.len() && &buf[..CLIENT_PREFACE.len()] == CLIENT_PREFACE
}

/// The minimum number of bytes to peek to detect the h2c preface.
pub const H2C_PREFACE_PEEK_LEN: usize = 24;

/// Result of protocol detection from a socket peek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedProtocol {
    /// The peer is sending the HTTP/2 client preface — proceed to h2 handshake.
    H2,
    /// Not HTTP/2 (likely HTTP/1.1) — proceed to the HTTP/1.1 parser.
    Http11,
    /// Not enough bytes peeked to decide — try again or fall back.
    NeedMore,
}

/// Detect the protocol from peeked bytes.
#[must_use]
pub fn detect_protocol(peeked: &[u8]) -> DetectedProtocol {
    if peeked.len() >= H2C_PREFACE_PEEK_LEN && is_h2c_preface(peeked) {
        DetectedProtocol::H2
    } else if peeked.len() >= H2C_PREFACE_PEEK_LEN {
        // Got enough bytes and it's not the h2c preface — it's HTTP/1.1.
        DetectedProtocol::Http11
    } else {
        DetectedProtocol::NeedMore
    }
}
