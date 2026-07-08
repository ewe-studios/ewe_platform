//! HPACK Huffman codec (RFC 7541 Appendix B).
//!
//! WHY: HPACK uses a static Huffman code for string literal compression.
//! Every byte 0..=255 has a variable-length bit-sequence; the codec handles
//! encoding (byte → bits) and decoding (bits → byte) with the standard
//! tables.
//!
//! WHAT: [`encode`] writes Huffman-encoded bytes to a buffer; [`decode`]
//! reads Huffman-encoded bytes and produces the original string.
//!
//! HOW: The tables are in [`huffman_table`] (copied from the h2 crate's
//! generated code, which derives them from RFC 7541 Appendix B).

use bytes::{BufMut, BytesMut};

mod huffman_table;

/// Flags used in the decode table entries.
const MAYBE_EOS: u8 = 1;
const DECODED: u8 = 2;
const ERROR: u8 = 4;

/// Huffman-decode `src` into a new [`BytesMut`].
///
/// # Errors
/// Returns an error string if the input contains an invalid Huffman sequence.
pub fn decode(src: &[u8]) -> Result<BytesMut, &'static str> {
    let mut buf = BytesMut::new();
    decode_into(src, &mut buf)?;
    Ok(buf)
}

/// Huffman-decode `src`, appending decoded bytes to `buf`.
///
/// # Errors
/// Returns an error string if the input contains an invalid Huffman sequence.
pub fn decode_into(src: &[u8], buf: &mut BytesMut) -> Result<(), &'static str> {
    // The Huffman codec guarantees compression ratio ≤ 1:2 at worst, so
    // pre-allocating 2× the input size is always enough.
    buf.reserve(src.len() * 2);

    let mut state: usize = 0;
    let mut maybe_eos = false;

    for &b in src {
        // Decode high nibble
        let (next, byte, flags) = huffman_table::DECODE_TABLE[state][(b >> 4) as usize];
        if flags & ERROR == ERROR {
            return Err("invalid Huffman code (EOS followed by data)");
        }
        if flags & DECODED == DECODED {
            buf.put_u8(byte);
        }
        state = next;

        // Decode low nibble
        let (next, byte, flags) = huffman_table::DECODE_TABLE[state][(b & 0x0f) as usize];
        if flags & ERROR == ERROR {
            return Err("invalid Huffman code (EOS followed by data)");
        }
        if flags & DECODED == DECODED {
            buf.put_u8(byte);
        }
        state = next;
        maybe_eos = flags & MAYBE_EOS == MAYBE_EOS;
    }

    // Must end at state 0 or at a state that accepts EOS
    if state != 0 && !maybe_eos {
        return Err("incomplete Huffman sequence");
    }

    Ok(())
}

/// Huffman-encode `src`, appending to `dst`.
pub fn encode(src: &[u8], dst: &mut BytesMut) {
    let mut bits: u64 = 0;
    let mut bits_left: i32 = 40;

    for &b in src {
        let (nbits, code) = huffman_table::ENCODE_TABLE[b as usize];

        bits |= code << (bits_left - nbits as i32);
        bits_left -= nbits as i32;

        while bits_left <= 32 {
            dst.put_u8((bits >> 32) as u8);
            bits <<= 8;
            bits_left += 8;
        }
    }

    if bits_left != 40 {
        // Pad with the EOS prefix (all 1s) and flush the final byte.
        bits |= (1 << bits_left) - 1;
        dst.put_u8((bits >> 32) as u8);
    }
}
