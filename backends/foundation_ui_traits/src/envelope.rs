//! WHY: Every consumer outside WASM (HTTP, SSE, WebSocket) needs a tiny self-
//! describing header so the receiver can demux the format without out-of-band
//! metadata. WASM extends this with an arena id (`foundation_wasm`'s 14-byte
//! `WasmEnvelope`) for the WASM-JS boundary only — no `memory_id` here.
//!
//! WHAT: The 6-byte header `[protocol:1][version:1][length:4 LE]` that precedes a
//! protocol payload, plus [`encode_with_envelope`] to encode-and-wrap in one step.
//!
//! HOW: `write` prepends the header to the payload; `parse` splits it back apart.

use alloc::vec::Vec;

use crate::encoder::{to_u32, DecodeError, DecodeResult, ProtocolEncoder};

/// Envelope header size in bytes.
pub const ENVELOPE_SIZE: usize = 6;

/// The parsed 6-byte envelope header.
///
/// ```text
///  0   1   2   3   4   5   6            6+N
///  ┌───┬───┬───┬───┬───┬───┬────────────┐
///  │ P │ V │ L0│ L1│ L2│ L3│ payload... │
///  └───┴───┴───┴───┴───┴───┴────────────┘
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Envelope {
    /// Protocol discriminant (`0` custom binary, `1` arrow, `2` json).
    pub protocol: u8,
    /// Protocol version.
    pub version: u8,
    /// Byte length of the payload that follows the header.
    pub length: u32,
}

impl Envelope {
    /// Header size in bytes (alias of [`ENVELOPE_SIZE`]).
    pub const HEADER_LEN: usize = ENVELOPE_SIZE;

    /// Build `[protocol][version][length:4 LE][payload...]`.
    #[must_use]
    pub fn write(protocol: u8, version: u8, payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::HEADER_LEN + payload.len());
        buf.push(protocol);
        buf.push(version);
        buf.extend_from_slice(&to_u32(payload.len()).to_le_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    /// Parse the 6-byte header and return it alongside the payload slice.
    ///
    /// The feature-01 spec sketches an `Option` return; this keeps the richer
    /// `Result` (telling truncated-header apart from truncated-payload) — the
    /// same two-state contract with diagnostics attached.
    ///
    /// # Errors
    /// Returns [`DecodeError::TruncatedBuffer`] if `bytes` is shorter than the
    /// header or the declared `length` overruns the slice.
    pub fn parse(bytes: &[u8]) -> DecodeResult<(Envelope, &[u8])> {
        if bytes.len() < Self::HEADER_LEN {
            return Err(DecodeError::TruncatedBuffer {
                expected_min: Self::HEADER_LEN,
                actual: bytes.len(),
            });
        }
        let protocol = bytes[0];
        let version = bytes[1];
        let length = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        let end = Self::HEADER_LEN
            .checked_add(length as usize)
            .ok_or(DecodeError::SchemaMismatch {
                detail: alloc::string::String::from("envelope length overflows usize"),
            })?;
        if bytes.len() < end {
            return Err(DecodeError::TruncatedBuffer {
                expected_min: end,
                actual: bytes.len(),
            });
        }
        Ok((
            Envelope {
                protocol,
                version,
                length,
            },
            &bytes[Self::HEADER_LEN..end],
        ))
    }
}

/// Encode `data` with `encoder` and wrap the payload in an [`Envelope`] — the
/// one-step form servers use to produce a complete wire message.
pub fn encode_with_envelope<T>(encoder: &impl ProtocolEncoder<T>, data: T) -> Vec<u8> {
    let payload = encoder.encode(data);
    Envelope::write(encoder.protocol_byte(), encoder.version(), &payload)
}
