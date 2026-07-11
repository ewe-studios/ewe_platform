//! QPACK field compression (RFC 9204).
//!
//! WHY: HTTP/3 headers are compressed. HPACK could not be reused directly because
//! its dynamic table assumes ordered delivery, and QUIC streams are independent —
//! that is the whole reason QPACK exists.
//!
//! WHAT: [`encode_field_section`] and [`decode_field_section`], plus the
//! [`static_table`].
//!
//! HOW: **the dynamic table is disabled.** We advertise
//! `SETTINGS_QPACK_MAX_TABLE_CAPACITY = 0` and `SETTINGS_QPACK_BLOCKED_STREAMS = 0`,
//! which RFC 9204 §3.2.3 explicitly allows: "an encoder that does not wish to use
//! the dynamic table can set the capacity to zero". This is a real, conforming
//! QPACK, not a subset — a peer that respects our SETTINGS can never send us a
//! dynamic reference, and we never emit one.
//!
//! ## What that buys, and what it costs
//!
//! It buys the elimination of the encoder and decoder streams, head-of-line
//! blocking on the dynamic table, insert-count accounting, and the entire
//! `vas`/`dynamic` machinery — roughly two thirds of a QPACK implementation, and
//! the two thirds where the subtle bugs live. It costs compression ratio on
//! repeated custom headers; the 99-entry static table still covers the common ones.
//!
//! Dynamic-table support is a later feature; the seam is the two representations
//! we currently reject, both marked below.
//!
//! ## Wire shape (RFC 9204 §4.5)
//!
//! ```text
//! Encoded Field Section {
//!   Required Insert Count (prefix int, 8-bit prefix),
//!   S bit + Delta Base    (prefix int, 7-bit prefix),
//!   Field Line ...,
//! }
//! ```
//!
//! With the dynamic table disabled both prefix values are always zero, but they
//! are still *present* — a decoder that skips them desynchronises immediately.

pub mod static_table;

use bytes::{Bytes, BytesMut};

use crate::http2::hpack::huffman;

/// A QPACK encoding or decoding failure.
///
/// These map onto the HTTP/3 error codes `QPACK_DECOMPRESSION_FAILED` (0x0200) and
/// `QPACK_ENCODER_STREAM_ERROR` (0x0201); the connection layer performs that
/// mapping, because only it can close the connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QpackError {
    /// The field section ended in the middle of a representation.
    Truncated,
    /// An index does not exist in the static table.
    BadStaticIndex(usize),
    /// The peer used the dynamic table, which our SETTINGS disabled.
    DynamicTableDisabled,
    /// A Huffman-coded string is malformed.
    BadHuffman(&'static str),
    /// A prefix integer does not terminate, or overflows.
    BadInteger,
    /// The decoded field section exceeds the caller's limit.
    TooLarge {
        /// The limit that was exceeded, in bytes.
        limit: usize,
    },
}

impl std::fmt::Display for QpackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(f, "QPACK field section is truncated"),
            Self::BadStaticIndex(i) => write!(f, "QPACK static table has no index {i}"),
            Self::DynamicTableDisabled => write!(
                f,
                "peer referenced the QPACK dynamic table, but we advertised \
                 QPACK_MAX_TABLE_CAPACITY = 0"
            ),
            Self::BadHuffman(why) => write!(f, "malformed Huffman string: {why}"),
            Self::BadInteger => write!(f, "malformed QPACK prefix integer"),
            Self::TooLarge { limit } => {
                write!(f, "decoded field section exceeds the {limit} byte limit")
            }
        }
    }
}

impl std::error::Error for QpackError {}

/// One decoded header field.
pub type Field = (Bytes, Bytes);

// ── Prefix integers (RFC 7541 §5.1, reused verbatim by RFC 9204 §4.1.1) ──────

/// Decode an `n`-bit prefix integer from `src`.
///
/// Returns `(value, bytes_consumed)`. The prefix bits live in `src[0]`.
fn decode_int(src: &[u8], prefix_bits: u8) -> Result<(u64, usize), QpackError> {
    let Some(&first) = src.first() else {
        return Err(QpackError::Truncated);
    };

    let mask = (1u16 << prefix_bits) - 1;
    let mut value = u64::from(u16::from(first) & mask);

    // A prefix that is not all-ones encodes the whole value.
    if value < u64::from(mask) {
        return Ok((value, 1));
    }

    let mut shift = 0u32;
    let mut consumed = 1usize;

    for &byte in &src[1..] {
        consumed += 1;
        value = value
            .checked_add(
                u64::from(byte & 0x7f)
                    .checked_shl(shift)
                    .ok_or(QpackError::BadInteger)?,
            )
            .ok_or(QpackError::BadInteger)?;
        if byte & 0x80 == 0 {
            return Ok((value, consumed));
        }
        shift += 7;
        if shift > 63 {
            return Err(QpackError::BadInteger);
        }
    }

    Err(QpackError::Truncated)
}

/// Encode `value` as an `n`-bit prefix integer, OR-ing `flags` into the first byte.
fn encode_int(value: u64, prefix_bits: u8, flags: u8, out: &mut Vec<u8>) {
    let mask = u64::from((1u16 << prefix_bits) - 1);

    if value < mask {
        out.push(flags | (value as u8));
        return;
    }

    out.push(flags | (mask as u8));
    let mut rest = value - mask;
    while rest >= 128 {
        out.push(((rest % 128) as u8) | 0x80);
        rest /= 128;
    }
    out.push(rest as u8);
}

// ── String literals (RFC 9204 §4.1.2) ───────────────────────────────────────

/// Decode a string literal whose Huffman bit sits at `huffman_bit` of `src[0]`.
fn decode_string(src: &[u8], prefix_bits: u8) -> Result<(Bytes, usize), QpackError> {
    let Some(&first) = src.first() else {
        return Err(QpackError::Truncated);
    };
    let huffman = first & (1 << prefix_bits) != 0;

    let (len, int_len) = decode_int(src, prefix_bits)?;
    let len = usize::try_from(len).map_err(|_| QpackError::BadInteger)?;

    let end = int_len.checked_add(len).ok_or(QpackError::BadInteger)?;
    if src.len() < end {
        return Err(QpackError::Truncated);
    }
    let raw = &src[int_len..end];

    let value = if huffman {
        // QPACK reuses HPACK's Huffman code verbatim (RFC 9204 §5).
        huffman::decode(raw)
            .map_err(QpackError::BadHuffman)?
            .freeze()
    } else {
        Bytes::copy_from_slice(raw)
    };

    Ok((value, end))
}

/// Encode a string literal, Huffman-coding it when that is actually smaller.
fn encode_string(value: &[u8], prefix_bits: u8, flags: u8, out: &mut Vec<u8>) {
    let mut huffed = BytesMut::new();
    huffman::encode(value, &mut huffed);

    // Huffman is only worth it when it shrinks the string; RFC 7541 §5.2 leaves
    // the choice to the encoder.
    if huffed.len() < value.len() {
        encode_int(
            huffed.len() as u64,
            prefix_bits,
            flags | (1 << prefix_bits),
            out,
        );
        out.extend_from_slice(&huffed);
    } else {
        encode_int(value.len() as u64, prefix_bits, flags, out);
        out.extend_from_slice(value);
    }
}

// ── Field sections ──────────────────────────────────────────────────────────

/// WHY: this is what a `HEADERS` frame carries.
///
/// WHAT: encode `fields` as a QPACK field section.
///
/// HOW: emits a zero Required Insert Count and zero Delta Base (the dynamic table
/// is disabled), then one representation per field:
///
/// - an exact static-table hit becomes an *Indexed Field Line* — one byte for the
///   common cases,
/// - a static name match becomes a *Literal Field Line With Name Reference*,
/// - anything else becomes a *Literal Field Line With Literal Name*.
///
/// Header names must already be lowercase: RFC 9114 §4.1.1 makes an uppercase name
/// a malformed request, and this function does not fix it for you.
///
/// # Panics
/// Never panics.
pub fn encode_field_section<N: AsRef<[u8]>, V: AsRef<[u8]>>(fields: &[(N, V)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(fields.len() * 16);

    // Required Insert Count = 0: this section references no dynamic entries.
    encode_int(0, 8, 0, &mut out);
    // S = 0, Delta Base = 0.
    encode_int(0, 7, 0, &mut out);

    for (name, value) in fields {
        let (name, value) = (name.as_ref(), value.as_ref());

        if let Some(index) = static_table::find_exact(name, value) {
            // Indexed Field Line, static: pattern `1 T=1 index(6+)`.
            encode_int(index as u64, 6, 0b1100_0000, &mut out);
            continue;
        }

        if let Some(index) = static_table::find_name(name) {
            // Literal With Name Reference, static: `01 N=0 T=1 index(4+)`, then the value.
            encode_int(index as u64, 4, 0b0101_0000, &mut out);
            encode_string(value, 7, 0, &mut out);
            continue;
        }

        // Literal With Literal Name: `001 N=0 H len(3+)`, name, then the value.
        encode_string(name, 3, 0b0010_0000, &mut out);
        encode_string(value, 7, 0, &mut out);
    }

    out
}

/// WHY: the inverse. A `HEADERS` frame's payload arrives as opaque bytes; this is
/// what turns it back into header fields.
///
/// WHAT: decode a QPACK field section into its fields.
///
/// HOW: reads the two prefix integers, then dispatches on the top bits of each
/// representation. `max_size` caps the *decoded* size, so a small compressed
/// section cannot expand into an unbounded allocation (the QPACK analogue of a
/// zip bomb; RFC 9204 §4.5.1 provides `SETTINGS_MAX_FIELD_SECTION_SIZE` for this).
///
/// # Errors
/// [`QpackError::DynamicTableDisabled`] if the peer ignored our SETTINGS;
/// [`QpackError::Truncated`] on a short section; [`QpackError::BadStaticIndex`] on
/// an out-of-range index; [`QpackError::TooLarge`] past `max_size`.
///
/// # Panics
/// Never panics.
pub fn decode_field_section(src: &[u8], max_size: usize) -> Result<Vec<Field>, QpackError> {
    // Required Insert Count. Non-zero means the peer used the dynamic table.
    let (required_insert_count, mut pos) = decode_int(src, 8)?;
    if required_insert_count != 0 {
        return Err(QpackError::DynamicTableDisabled);
    }

    // S bit + Delta Base. With no dynamic entries this must be zero.
    let (delta_base, used) = decode_int(&src[pos..], 7)?;
    if delta_base != 0 {
        return Err(QpackError::DynamicTableDisabled);
    }
    pos += used;

    let mut fields: Vec<Field> = Vec::new();
    let mut decoded_size = 0usize;

    while pos < src.len() {
        let rest = &src[pos..];
        let first = rest[0];

        let (name, value, used) = if first & 0b1000_0000 != 0 {
            // Indexed Field Line. T bit (0b0100_0000) selects static vs dynamic.
            if first & 0b0100_0000 == 0 {
                return Err(QpackError::DynamicTableDisabled);
            }
            let (index, used) = decode_int(rest, 6)?;
            let index = usize::try_from(index).map_err(|_| QpackError::BadInteger)?;
            let (n, v) = static_table::get(index).ok_or(QpackError::BadStaticIndex(index))?;
            (Bytes::from_static(n), Bytes::from_static(v), used)
        } else if first & 0b1100_0000 == 0b0100_0000 {
            // Literal Field Line With Name Reference. T bit is 0b0001_0000.
            if first & 0b0001_0000 == 0 {
                return Err(QpackError::DynamicTableDisabled);
            }
            let (index, used) = decode_int(rest, 4)?;
            let index = usize::try_from(index).map_err(|_| QpackError::BadInteger)?;
            let (n, _) = static_table::get(index).ok_or(QpackError::BadStaticIndex(index))?;
            let (value, value_used) = decode_string(&rest[used..], 7)?;
            (Bytes::from_static(n), value, used + value_used)
        } else if first & 0b1110_0000 == 0b0010_0000 {
            // Literal Field Line With Literal Name.
            let (name, name_used) = decode_string(rest, 3)?;
            let (value, value_used) = decode_string(&rest[name_used..], 7)?;
            (name, value, name_used + value_used)
        } else if first & 0b1111_0000 == 0b0001_0000 {
            // Indexed Field Line With Post-Base Index — dynamic table only.
            return Err(QpackError::DynamicTableDisabled);
        } else {
            // Literal Field Line With Post-Base Name Reference — dynamic only.
            return Err(QpackError::DynamicTableDisabled);
        };

        // RFC 9204 §4.5.1: a field's size is name + value + 32 bytes of overhead.
        decoded_size += name.len() + value.len() + 32;
        if decoded_size > max_size {
            return Err(QpackError::TooLarge { limit: max_size });
        }

        fields.push((name, value));
        pos += used;
    }

    Ok(fields)
}
