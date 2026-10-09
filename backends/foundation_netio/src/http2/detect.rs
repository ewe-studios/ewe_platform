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

/// The longest method token this server will entertain before concluding the
/// peer is not speaking HTTP at all.
///
/// RFC 9110 puts no ceiling on method length, but every registered method and
/// every WebDAV/extension method in practice fits well inside this. A "method"
/// that runs on without a space is not a request line — it is an SSH banner, a
/// TLS record, or noise.
const MAX_METHOD_LEN: usize = 16;

/// Result of protocol detection from a socket peek.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedProtocol {
    /// The peer is sending the HTTP/2 client preface — proceed to h2 handshake.
    H2,
    /// The bytes open like an HTTP/1.x request line — proceed to the HTTP/1.1
    /// parser. Note this says nothing about the *version* named later in that
    /// line; that is the HTTP/1.x handler's gate to apply.
    Http11,
    /// Not enough bytes peeked to decide — try again.
    NeedMore,
    /// The peer is not speaking any protocol this server serves. Refuse the
    /// connection; do not hand these bytes to the HTTP/1.1 parser.
    Unsupported,
}

/// Whether `b` is an RFC 9110 `tchar`, the character class a method token is
/// built from.
const fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

/// Decide whether `peeked` opens like an HTTP/1.x request line: a non-empty
/// method token followed by a space.
///
/// This is the check that keeps non-HTTP peers out of the HTTP/1.1 parser. A
/// TLS `ClientHello` starts `0x16 0x03`, and `0x16` is not a `tchar`; an SSH
/// banner (`SSH-2.0-...`) is all `tchar` but never reaches a space inside a
/// plausible method length. Both are refused here rather than being fed to a
/// parser that will try to read them as a request.
fn looks_like_request_line(peeked: &[u8]) -> DetectedProtocol {
    for (index, &byte) in peeked.iter().enumerate() {
        if byte == b' ' {
            // A leading space means there is no method at all.
            return if index == 0 {
                DetectedProtocol::Unsupported
            } else {
                DetectedProtocol::Http11
            };
        }
        if !is_tchar(byte) || index >= MAX_METHOD_LEN {
            return DetectedProtocol::Unsupported;
        }
    }

    // Every byte so far is a viable method token, but no space has arrived yet.
    DetectedProtocol::NeedMore
}

/// Detect the protocol from peeked bytes.
///
/// Decides as soon as the evidence allows, which is *not* the same as waiting
/// for 24 bytes. A complete HTTP/1.1 request can be shorter than the preface
/// (`GET / HTTP/1.1\r\n\r\n` is 18 bytes); demanding 24 before ruling out h2c
/// would stall such a request until the caller's detection timeout and then
/// drop it unanswered. Once the buffered bytes diverge from the preface at any
/// position, no continuation can make them h2c, so we answer immediately.
///
/// Divergence from the preface is *not* by itself evidence of HTTP/1.1 — that
/// inference would classify a TLS handshake, an SSH banner, and random noise as
/// HTTP/1.1 and feed them to the request parser. Having ruled out h2c, this
/// asks positively whether the bytes open like an HTTP/1.x request line, and
/// answers [`DetectedProtocol::Unsupported`] when they do not.
///
/// [`DetectedProtocol::NeedMore`] means the evidence so far is consistent with a
/// protocol we serve but does not yet single one out: either a viable prefix of
/// the h2c preface (a client whose preface was split across segments), or a
/// viable method token that has not yet reached its space.
#[must_use]
pub fn detect_protocol(peeked: &[u8]) -> DetectedProtocol {
    // The preface is checked first, because `PRI * HTTP/2.0…` would otherwise
    // satisfy the request-line shape below and be mistaken for HTTP/1.1.
    let common = peeked.len().min(CLIENT_PREFACE.len());
    if peeked[..common] == CLIENT_PREFACE[..common] {
        return if peeked.len() >= H2C_PREFACE_PEEK_LEN {
            DetectedProtocol::H2
        } else {
            DetectedProtocol::NeedMore
        };
    }

    looks_like_request_line(peeked)
}
