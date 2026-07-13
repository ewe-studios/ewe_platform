//! WebTransport protocol constants and frame definitions (spec-55, F06).
//!
//! WHY: WebTransport runs over HTTP/3 Extended CONNECT, using QUIC streams for data
//! and capsule frames for session management. This module defines the constants,
//! capsule types, and error codes needed by the session layer.
//!
//! WHAT: Stream type, capsule frame types, WebTransport-specific H3 error codes,
//! and the [`WtProtocolError`] enum.
//!
//! HOW: Plain constants + a lightweight error type. No I/O — this is the protocol
//! vocabulary the session module is written in terms of.

// ---------------------------------------------------------------------------
// Stream type (RFC 9114 §11.2.4 extension)
// ---------------------------------------------------------------------------

/// WebTransport bidirectional stream (draft-ietf-webtrans-http3 §4.1).
pub const WEBTRANSPORT_STREAM: u64 = 0x54;

// ---------------------------------------------------------------------------
// HTTP/3 SETTINGS (draft-ietf-webtrans-http3 §7.2)
// ---------------------------------------------------------------------------

/// SETTINGS_ENABLE_WEBTRANSPORT — the peer supports WebTransport.
/// Value 1 = enabled, 0 = disabled.
pub const SETTINGS_ENABLE_WEBTRANSPORT: u64 = 0x2b603742;

/// Maximum number of WebTransport sessions the server is willing to accept
/// (draft-ietf-webtrans-http3 §7.2.2).
pub const SETTINGS_WEBTRANSPORT_MAX_SESSIONS: u64 = 0x2b603743;

// ---------------------------------------------------------------------------
// Capsule types (RFC 9297, draft-ietf-webtrans-http3 §4.3)
// ---------------------------------------------------------------------------

/// Well-known capsule types used in WebTransport session control.
pub mod capsule {
    /// Server → client: the session is being gracefully closed.
    /// Carries an application error code + reason.
    pub const CLOSE_WEBTRANSPORT_SESSION: u64 = 0x2843;

    /// Server → client: stop creating new streams, drain existing ones.
    pub const DRAIN_WEBTRANSPORT_SESSION: u64 = 0x2a05;

    /// An RFC 9297 datagram capsule (carries a QUIC datagram).
    pub const DATAGRAM: u64 = 0x00;

    /// A legacy capsule for backwards compat (not used).
    pub const LEGACY_DATAGRAM: u64 = 0xff37a0;
}

/// Capsule frame types, decoded from the session control stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapsuleType {
    /// An application datagram payload.
    Datagram(Vec<u8>),
    /// Close the session with `code` and `reason`.
    CloseSession { code: u32, reason: String },
    /// Drain the session (stop creating new streams).
    Drain,
    /// Unknown capsule type (forward-compatible).
    Unknown(u64, Vec<u8>),
}

/// Decode a capsule frame from bytes on the session control stream.
///
/// Capsule format (RFC 9297 §3.1):
///   type: varint
///   length: varint
///   payload: [u8; length]
pub fn decode_capsule(buf: &[u8]) -> Result<(CapsuleType, usize), WtProtocolError> {
    if buf.is_empty() {
        return Err(WtProtocolError::Truncated);
    }

    let (capsule_type, type_len) = read_varint(buf).ok_or(WtProtocolError::Truncated)?;
    let rest = &buf[type_len..];
    if rest.is_empty() {
        return Err(WtProtocolError::Truncated);
    }

    let (payload_len, len_len) = read_varint(rest).ok_or(WtProtocolError::Truncated)?;
    let payload_start = type_len + len_len;
    let payload_end = payload_start + payload_len as usize;

    if buf.len() < payload_end {
        return Err(WtProtocolError::Truncated);
    }

    let payload = buf[payload_start..payload_end].to_vec();
    let consumed = payload_end;

    let capsule = match capsule_type {
        capsule::DATAGRAM | capsule::LEGACY_DATAGRAM => CapsuleType::Datagram(payload),
        capsule::CLOSE_WEBTRANSPORT_SESSION => {
            let code = if payload.len() >= 4 {
                u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]])
            } else {
                0
            };
            let reason = if payload.len() > 4 {
                String::from_utf8_lossy(&payload[4..]).into_owned()
            } else {
                String::new()
            };
            CapsuleType::CloseSession { code, reason }
        }
        capsule::DRAIN_WEBTRANSPORT_SESSION => CapsuleType::Drain,
        other => CapsuleType::Unknown(other, payload),
    };

    Ok((capsule, consumed))
}

/// Encode a CLOSE_WEBTRANSPORT_SESSION capsule.
pub fn encode_close_session(code: u32, reason: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    write_varint(&mut buf, capsule::CLOSE_WEBTRANSPORT_SESSION);
    let payload_len = 4 + reason.len() as u64;
    write_varint(&mut buf, payload_len);
    buf.extend_from_slice(&code.to_be_bytes());
    buf.extend_from_slice(reason.as_bytes());
    buf
}

/// Encode a datagram capsule for sending over the control stream.
pub fn encode_datagram_capsule(data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_varint(&mut buf, capsule::DATAGRAM);
    write_varint(&mut buf, data.len() as u64);
    buf.extend_from_slice(data);
    buf
}

// ---------------------------------------------------------------------------
// WebTransport-specific error codes
// ---------------------------------------------------------------------------

/// WebTransport session error (draft-ietf-webtrans-http3 §5).
pub mod session_error {
    /// No error.
    pub const NO_ERROR: u32 = 0;
    /// Generic protocol violation.
    pub const GENERIC: u32 = 1;
    /// Internal error (implementation bug, not the peer's fault).
    pub const INTERNAL: u32 = 2;
}

// ---------------------------------------------------------------------------
// WtProtocolError
// ---------------------------------------------------------------------------

/// Errors at the WebTransport protocol layer.
#[derive(Debug)]
pub enum WtProtocolError {
    /// The capsule frame was truncated before the full header was received.
    Truncated,
    /// An unknown or invalid capsule type was received.
    InvalidCapsule(u64),
    /// The session was closed by the peer.
    SessionClosed { code: u32, reason: String },
    /// The Extended CONNECT handshake was rejected.
    ConnectRejected,
    /// The underlying QUIC connection failed.
    Quic(String),
    /// A stream-level error occurred.
    Stream(String),
}

impl std::fmt::Display for WtProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated capsule frame"),
            Self::InvalidCapsule(t) => write!(f, "invalid capsule type: {t:#x}"),
            Self::SessionClosed { code, reason } => {
                write!(f, "session closed: code={code} reason={reason}")
            }
            Self::ConnectRejected => write!(f, "extended CONNECT rejected"),
            Self::Quic(msg) => write!(f, "QUIC error: {msg}"),
            Self::Stream(msg) => write!(f, "stream error: {msg}"),
        }
    }
}

impl std::error::Error for WtProtocolError {}

// ---------------------------------------------------------------------------
// Varint helpers (QUIC-style, RFC 9000 §16)
// ---------------------------------------------------------------------------

fn read_varint(buf: &[u8]) -> Option<(u64, usize)> {
    if buf.is_empty() {
        return None;
    }
    let first = buf[0];
    let (val, len) = match first >> 6 {
        0 => (first as u64, 1),
        1 => {
            if buf.len() < 2 { return None; }
            (u64::from_be_bytes([0, 0, 0, 0, 0, 0, first & 0x3f, buf[1]]), 2)
        }
        2 => {
            if buf.len() < 4 { return None; }
            (u64::from_be_bytes([0, 0, 0, 0, first & 0x3f, buf[1], buf[2], buf[3]]), 4)
        }
        3 => {
            if buf.len() < 8 { return None; }
            (u64::from_be_bytes([first & 0x3f, buf[1], buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]]), 8)
        }
        _ => unreachable!(),
    };
    Some((val, len))
}

fn write_varint(buf: &mut Vec<u8>, value: u64) {
    if value <= 63 {
        buf.push(value as u8);
    } else if value <= 16383 {
        buf.push(0x40 | ((value >> 8) as u8 & 0x3f));
        buf.push(value as u8);
    } else if value <= 1_073_741_823 {
        buf.push(0x80 | ((value >> 24) as u8 & 0x3f));
        buf.push((value >> 16) as u8);
        buf.push((value >> 8) as u8);
        buf.push(value as u8);
    } else {
        buf.push(0xc0 | ((value >> 56) as u8 & 0x3f));
        buf.push((value >> 48) as u8);
        buf.push((value >> 40) as u8);
        buf.push((value >> 32) as u8);
        buf.push((value >> 24) as u8);
        buf.push((value >> 16) as u8);
        buf.push((value >> 8) as u8);
        buf.push(value as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip() {
        for val in [0, 1, 63, 64, 16383, 16384, 1073741823, 1073741824, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, val);
            let (decoded, len) = read_varint(&buf).expect("read");
            assert_eq!(decoded, val, "varint round-trip for {val}");
            assert_eq!(len, buf.len());
        }
    }

    #[test]
    fn decode_datagram_capsule() {
        let encoded = encode_datagram_capsule(b"hello");
        let (capsule, consumed) = decode_capsule(&encoded).expect("decode");
        assert_eq!(consumed, encoded.len());
        assert!(matches!(capsule, CapsuleType::Datagram(ref d) if d == b"hello"));
    }

    #[test]
    fn decode_close_session_capsule() {
        let encoded = encode_close_session(42, "gone");
        let (capsule, _) = decode_capsule(&encoded).expect("decode");
        assert!(matches!(capsule, CapsuleType::CloseSession { code: 42, ref reason } if reason == "gone"));
    }

    #[test]
    fn decode_truncated_returns_error() {
        assert!(matches!(decode_capsule(&[]), Err(WtProtocolError::Truncated)));
    }
}
