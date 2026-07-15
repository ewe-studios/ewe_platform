//! QUIC variable-length integers (RFC 9000 §16), the encoding HTTP/3 is built on.
//!
//! WHY: every HTTP/3 frame type, frame length, setting identifier, setting value,
//! stream type, and error code is a QUIC varint. Nothing in the protocol can be
//! parsed without this, so it is the first thing HTTP/3 needs and the easiest
//! thing to get subtly wrong.
//!
//! WHAT: [`VarInt`] — a 62-bit unsigned integer, plus encode/decode.
//!
//! HOW: the top two bits of the first byte select the length; the remaining bits
//! are the value's most-significant bits.
//!
//! ```text
//!   2MSB  Length  Usable bits  Range
//!   00    1       6            0 .. 63
//!   01    2       14           0 .. 16_383
//!   10    4       30           0 .. 1_073_741_823
//!   11    8       62           0 .. 4_611_686_018_427_387_903
//! ```
//!
//! ## Non-canonical encodings are legal
//!
//! RFC 9000 §16 is explicit: *"the encoding is not required to use the minimum
//! number of bytes"*. `0`, `0x40 0x00`, and the 8-byte form all decode to zero and
//! a receiver must accept all of them. A decoder that rejects a long-form small
//! value is wrong, and an encoder that emits one is merely wasteful. We decode
//! permissively and encode minimally.

use std::io;

/// The largest value a QUIC varint can carry: `2^62 - 1`.
pub const MAX: u64 = (1 << 62) - 1;

/// A QUIC variable-length integer: an unsigned value below `2^62`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VarInt(u64);

/// A value that cannot be encoded as a QUIC varint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarIntOutOfRange(pub u64);

impl std::fmt::Display for VarIntOutOfRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} exceeds the QUIC varint maximum of {MAX}", self.0)
    }
}

impl std::error::Error for VarIntOutOfRange {}

impl VarInt {
    /// The largest representable varint.
    pub const MAX: VarInt = VarInt(MAX);

    /// WHY: values at or above `2^62` have no encoding, so the constructor must
    /// reject them rather than silently truncate.
    ///
    /// WHAT: wrap `value` if it fits in 62 bits.
    ///
    /// # Errors
    /// [`VarIntOutOfRange`] when `value > 2^62 - 1`.
    pub const fn new(value: u64) -> Result<Self, VarIntOutOfRange> {
        if value > MAX {
            return Err(VarIntOutOfRange(value));
        }
        Ok(Self(value))
    }

    /// Wrap a value known at compile time to fit.
    ///
    /// # Panics
    /// Panics if `value > 2^62 - 1`. Use [`VarInt::new`] for runtime values.
    #[must_use]
    pub const fn from_u32(value: u32) -> Self {
        Self(value as u64)
    }

    /// The underlying value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// How many bytes this value occupies in its **minimal** encoding.
    #[must_use]
    pub const fn encoded_len(self) -> usize {
        match self.0 {
            0..=63 => 1,
            64..=16_383 => 2,
            16_384..=1_073_741_823 => 4,
            _ => 8,
        }
    }

    /// WHY: HTTP/3 writes varints into frame headers constantly.
    ///
    /// WHAT: append the minimal encoding of `self` to `out`.
    ///
    /// HOW: pick the shortest length class that fits, then OR the class tag into
    /// the top two bits of the first byte.
    ///
    /// # Panics
    /// Never panics.
    pub fn encode(self, out: &mut Vec<u8>) {
        match self.encoded_len() {
            1 => out.push(self.0 as u8),
            2 => out.extend_from_slice(&((self.0 as u16) | 0b01 << 14).to_be_bytes()),
            4 => out.extend_from_slice(&((self.0 as u32) | 0b10 << 30).to_be_bytes()),
            _ => out.extend_from_slice(&(self.0 | 0b11 << 62).to_be_bytes()),
        }
    }

    /// WHY: a non-blocking transport hands us partial frames constantly, so
    /// "not enough bytes yet" must be distinguishable from "malformed".
    ///
    /// WHAT: decode a varint from the front of `src`.
    ///
    /// HOW: returns `Ok(None)` when `src` is too short to hold the value the
    /// length tag announces — the caller should retry with more bytes. Returns
    /// `Ok(Some((value, consumed)))` otherwise. Non-canonical (over-long)
    /// encodings are accepted, as RFC 9000 §16 requires.
    ///
    /// # Errors
    /// Never returns `Err`: every byte sequence long enough for its own length tag
    /// is a valid varint. The signature keeps `Result` for symmetry with the frame
    /// decoders and so the contract can tighten without a breaking change.
    ///
    /// # Panics
    /// Never panics.
    pub fn decode(src: &[u8]) -> Result<Option<(VarInt, usize)>, io::Error> {
        let Some(&first) = src.first() else {
            return Ok(None);
        };

        let len = 1usize << (first >> 6);
        if src.len() < len {
            return Ok(None);
        }

        // Mask off the two-bit length tag from the most significant byte.
        let mut value = u64::from(first & 0b0011_1111);
        for &byte in &src[1..len] {
            value = (value << 8) | u64::from(byte);
        }

        Ok(Some((VarInt(value), len)))
    }
}

impl std::fmt::Display for VarInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<VarInt> for u64 {
    fn from(v: VarInt) -> Self {
        v.0
    }
}

impl From<u32> for VarInt {
    fn from(v: u32) -> Self {
        Self(u64::from(v))
    }
}

impl From<u8> for VarInt {
    fn from(v: u8) -> Self {
        Self(u64::from(v))
    }
}

impl TryFrom<u64> for VarInt {
    type Error = VarIntOutOfRange;

    fn try_from(v: u64) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}

impl TryFrom<usize> for VarInt {
    type Error = VarIntOutOfRange;

    fn try_from(v: usize) -> Result<Self, Self::Error> {
        Self::new(v as u64)
    }
}
