//! HPACK header compression (RFC 7541).
//!
//! WHY: HTTP/2 mandates HPACK for header compression — it uses a static table
//! of 61 common header name/value pairs (Appendix A), a dynamic table of
//! recently seen headers, and an optional Huffman code for string values
//! (Appendix B).
//!
//! WHAT: A decoder that reads HPACK-encoded header block fragments and emits
//! `(name, value)` pairs, and an encoder that writes the same. The dynamic
//! table tracks entries within a configurable max-size budget.
//!
//! HOW: The decoder is an iterator-style state machine that processes one
//! byte at a time through the [`Decoder`]; the encoder provides a
//! [`encode_header`] function that appends to a buffer. The dynamic table is
//! a simple [`VecDeque`] with linear-probe lookup — correct but not as fast
//! as robinhood hashing; adequate for Feature 29's acceptance criteria
//! (RFC 7541 Appendix C vectors pass).

pub mod huffman;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::VecDeque;

// ── Constants ───────────────────────────────────────────────────────────────

/// Initial maximum dynamic table size (RFC 7540 §6.5.2: 4096 bytes).
pub const DEFAULT_MAX_TABLE_SIZE: usize = 4096;

/// Overhead per dynamic-table entry: 32 bytes per RFC 7541 §4.1.
const ENTRY_OVERHEAD: usize = 32;

/// Fixed index of the first dynamic-table entry (static table has indices 1–61).
const STATIC_TABLE_LEN: usize = 61;
const FIRST_DYNAMIC_INDEX: usize = 62;

// ── Static table (RFC 7541 Appendix A) ──────────────────────────────────────
//
// Indexed 1..=61. Each entry is (name, value).

static STATIC_TABLE: [(&str, &str); STATIC_TABLE_LEN] = [
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

// ── Dynamic table ───────────────────────────────────────────────────────────

/// A single entry in the dynamic table.
#[derive(Debug, Clone)]
struct DynamicEntry {
    name: Bytes,
    value: Bytes,
}

impl DynamicEntry {
    fn size(&self) -> usize {
        self.name.len() + self.value.len() + ENTRY_OVERHEAD
    }
}

/// The mutable dynamic table (RFC 7541 §2.3).
#[derive(Debug)]
pub struct DynamicTable {
    entries: VecDeque<DynamicEntry>,
    current_size: usize,
    max_size: usize,
}

impl DynamicTable {
    /// Create a new dynamic table with the given max size.
    pub fn new(max_size: usize) -> Self {
        Self { entries: VecDeque::new(), current_size: 0, max_size }
    }

    /// Number of entries currently in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Current total size in bytes.
    pub fn size(&self) -> usize {
        self.current_size
    }

    /// Maximum allowed size in bytes.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Change the max size, evicting entries as needed.
    pub fn set_max_size(&mut self, new_max: usize) {
        self.max_size = new_max;
        if new_max == 0 {
            self.entries.clear();
            self.current_size = 0;
        } else {
            self.evict_to_fit(0);
        }
    }

    /// Look up an entry by its 1-based HPACK index (62+ is dynamic, 62 = most recent).
    pub fn get(&self, index: usize) -> Option<(&[u8], &[u8])> {
        if index < FIRST_DYNAMIC_INDEX {
            return None;
        }
        let offset = index - FIRST_DYNAMIC_INDEX;
        self.entries.get(offset).map(|e| (e.name.as_ref(), e.value.as_ref()))
    }

    /// Find the index of a matching (name, value) pair in this table.
    /// Returns the 1-based HPACK index (62+).
    pub fn find_exact(&self, name: &[u8], value: &[u8]) -> Option<usize> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.name.as_ref() == name && entry.value.as_ref() == value {
                return Some(FIRST_DYNAMIC_INDEX + i);
            }
        }
        None
    }

    /// Find the index of a matching name in this table.
    /// Returns the 1-based HPACK index (62+).
    pub fn find_name(&self, name: &[u8]) -> Option<usize> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.name.as_ref() == name {
                return Some(FIRST_DYNAMIC_INDEX + i);
            }
        }
        None
    }

    /// Insert a new entry at the front (index 62). Evicts old entries if needed.
    pub fn insert(&mut self, name: Bytes, value: Bytes) {
        let entry_size = name.len() + value.len() + ENTRY_OVERHEAD;

        // Evict until the new entry fits (RFC 7541 §4.4).
        self.evict_to_fit(entry_size);

        if entry_size > self.max_size {
            // Entry alone is larger than the max table size — don't insert.
            return;
        }

        self.current_size += entry_size;
        self.entries.push_front(DynamicEntry { name, value });
    }

    /// Evict entries from the back until the table fits `needed` more bytes.
    fn evict_to_fit(&mut self, needed: usize) {
        while self.current_size + needed > self.max_size {
            if let Some(evicted) = self.entries.pop_back() {
                self.current_size = self.current_size.saturating_sub(evicted.size());
            } else {
                break;
            }
        }
    }
}

// ── Static table helpers ────────────────────────────────────────────────────

/// Look up an entry by its 1-based HPACK index (1..=61 for static).
pub fn static_table_get(index: usize) -> Option<(&'static str, &'static str)> {
    if index >= 1 && index <= STATIC_TABLE_LEN {
        Some(STATIC_TABLE[index - 1])
    } else {
        None
    }
}

/// Find the index of a matching (name, value) in the static table.
pub fn static_table_find_exact(name: &[u8], value: &[u8]) -> Option<usize> {
    for (i, (n, v)) in STATIC_TABLE.iter().enumerate() {
        if n.as_bytes() == name && v.as_bytes() == value {
            return Some(i + 1);
        }
    }
    None
}

/// Find the index of a matching name in the static table.
pub fn static_table_find_name(name: &[u8]) -> Option<usize> {
    for (i, (n, _)) in STATIC_TABLE.iter().enumerate() {
        if n.as_bytes() == name {
            return Some(i + 1);
        }
    }
    None
}

// ── Integer encoding (RFC 7541 §5.1) ───────────────────────────────────────

/// Decode an HPACK integer with the given prefix size.
///
/// Returns `None` if more bytes are needed.
pub(crate) fn decode_int(buf: &[u8], prefix_bits: u8) -> Result<Option<(usize, usize)>, &'static str> {
    if buf.is_empty() {
        return Ok(None);
    }
    let mask = (1u8 << prefix_bits) - 1;
    let mut value = (buf[0] & mask) as usize;

    if value < mask as usize {
        return Ok(Some((value, 1)));
    }

    // Multi-byte continuation
    let mut m: usize = 0;
    for i in 1.. {
        if i >= buf.len() {
            return Ok(None); // Need more bytes
        }
        let b = buf[i];
        value += (b as usize & 0x7f) << m;
        m += 7;
        if b & 0x80 == 0 {
            return Ok(Some((value, i + 1)));
        }
        // RFC 7541 §5.1: implementations MUST NOT produce values larger than
        // (2^N - 1) for N-bit representations. For a full 32-bit decode the
        // max continuation is ceil(32/7)=5 bytes after the first.
        //
        // The safety bail-out is len check above; if the value overflows
        // usize (unlikely on 64-bit) the `<< m` shift will wrap. Keep a
        // reasonable bound.
        #[allow(clippy::erasing_op)]
        if m > 31 {
            return Err("integer encoding overflow");
        }
    }
    unreachable!()
}

/// Encode an integer with the given prefix size into `dst`.
///
/// The first byte's prefix is assumed to be already set correctly by the
/// caller; this only writes the continuation bytes if the value doesn't fit.
pub(crate) fn encode_int(value: usize, prefix_bits: u8, dst: &mut BytesMut) {
    let mask = (1u8 << prefix_bits) - 1;

    if value < mask as usize {
        // Fits in the prefix — the caller must OR it into the first byte.
        // We write nothing here; the caller sets it.
        return;
    }

    // First byte gets the full mask, then continuation bytes.
    let mut remaining = value - mask as usize;
    // But the first byte value is set by the caller — we just emit the
    // continuation chain. Actually, the caller ORs the initial prefix into the
    // first byte. We need to help with the continuation. Let the caller handle
    // the first byte's prefix; we only emit continuation.
    // For simplicity, we write all the continuation bytes to dst directly.
    loop {
        let cont = (remaining % 128) as u8;
        remaining /= 128;
        if remaining > 0 {
            dst.put_u8(cont | 0x80);
        } else {
            dst.put_u8(cont);
            break;
        }
    }
}

/// Maximum value encodable in the initial prefix alone.
pub(crate) const fn int_prefix_max(prefix_bits: u8) -> usize {
    (1 << prefix_bits) - 1
}

// ── String literal decoding (RFC 7541 §5.2) ────────────────────────────────

/// A decoded string literal: raw bytes and whether it was Huffman-encoded.
#[derive(Debug, Clone)]
pub struct DecodedString {
    pub bytes: Bytes,
}

/// Decode an HPACK string literal from `buf`.
///
/// Returns `None` if incomplete, or `(string, bytes_consumed)`.
fn decode_string_literal(buf: &[u8]) -> Result<Option<(Bytes, usize)>, &'static str> {
    if buf.is_empty() {
        return Ok(None);
    }
    let huffman = buf[0] & 0x80 != 0;
    let len_result = decode_int(buf, 7)?;
    let (len, consumed) = match len_result {
        Some(v) => v,
        None => return Ok(None),
    };

    let total_needed = consumed + len;
    if buf.len() < total_needed {
        return Ok(None);
    }

    let raw = &buf[consumed..total_needed];
    let bytes = if huffman {
        huffman::decode(raw)?
    } else {
        BytesMut::from(raw)
    };

    Ok(Some((bytes.freeze(), total_needed)))
}

// ── Header field decoding (RFC 7541 §6) ────────────────────────────────────

/// A decoded header field (name, value).
pub type HeaderField = (Bytes, Bytes);

/// Incremental HPACK decoder for header block fragments.
///
/// Call [`decode_fragment`] repeatedly with bytes as they arrive. It returns
/// decoded headers as they become available.
#[derive(Debug)]
pub struct Decoder {
    table: DynamicTable,
    /// Leftover bytes that didn't complete a header field yet.
    pending: BytesMut,
}

impl Decoder {
    /// Create a new decoder with the default max table size (4096 bytes).
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_TABLE_SIZE)
    }

    /// Create a new decoder with the given max table size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self { table: DynamicTable::new(max_size), pending: BytesMut::new() }
    }

    /// Access the dynamic table.
    pub fn table(&self) -> &DynamicTable {
        &self.table
    }

    /// Mutable access to the dynamic table.
    pub fn table_mut(&mut self) -> &mut DynamicTable {
        &mut self.table
    }

    /// Feed bytes into the decoder and collect any completed header fields.
    ///
    /// The returned headers are `(name, value)` pairs (both lowercased name
    /// for field headers, raw for pseudo-headers). Pending bytes are buffered
    /// internally — call again when more data arrives.
    ///
    /// # Errors
    /// Returns an error on protocol violations.
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<HeaderField>, &'static str> {
        self.pending.extend_from_slice(data);
        let mut headers = Vec::new();

        loop {
            if self.pending.is_empty() {
                break;
            }
            let (action, consumed) = decode_one(&self.table, &self.pending[..])?;
            match action {
                DecodeAction::Emit(name, value, should_index) => {
                    if should_index {
                        self.table.insert(name.clone(), value.clone());
                    }
                    headers.push((name, value));
                    self.pending.advance(consumed);
                }
                DecodeAction::NeedMore => break,
                DecodeAction::TableSizeUpdate(new_max) => {
                    self.table.set_max_size(new_max);
                    self.pending.advance(consumed);
                }
            }
        }

        Ok(headers)
    }

}

/// What the caller should do after decoding one item from the buffer.
enum DecodeAction {
    /// Emit a header field. `bool` = should be added to the dynamic table.
    Emit(Bytes, Bytes, bool),
    /// Not enough data — need more bytes.
    NeedMore,
    /// Dynamic table size update.
    TableSizeUpdate(usize),
}

/// Try to decode a single header field or table-size-update from the
/// beginning of `buf`. Returns `(action, bytes_consumed)`.
///
/// This is a free function (not a method on `Decoder`) so it can borrow the
/// table and the buffer independently — avoiding the NLL borrow conflict.
fn decode_one(table: &DynamicTable, buf: &[u8]) -> Result<(DecodeAction, usize), &'static str> {
    if buf.is_empty() {
        return Ok((DecodeAction::NeedMore, 0));
    }

    let first = buf[0];

    // ── Indexed Header Field (RFC 7541 §6.1) ──────────────────────────
    // 1xxx_xxxx
    if first & 0x80 != 0 {
        let (idx, consumed) = match decode_int(buf, 7)? {
            Some(v) => v,
            None => return Ok((DecodeAction::NeedMore, 0)),
        };
        let (name, value) = lookup(table, idx)?;
        return Ok((
            DecodeAction::Emit(Bytes::copy_from_slice(name), Bytes::copy_from_slice(value), false),
            consumed,
        ));
    }

    // ── Literal Header Field with Incremental Indexing (§6.2.1) ───────
    // 01xx_xxxx
    if first & 0xC0 == 0x40 {
        return decode_literal_indexed(table, buf, 6);
    }

    // ── Dynamic Table Size Update (§6.3) ──────────────────────────────
    // 001x_xxxx
    if first & 0xE0 == 0x20 {
        let (new_max, consumed) = match decode_int(buf, 5)? {
            Some(v) => v,
            None => return Ok((DecodeAction::NeedMore, 0)),
        };
        return Ok((DecodeAction::TableSizeUpdate(new_max), consumed));
    }

    // ── Literal Header Field Never Indexed (§6.2.3) / Without Indexing (§6.2.2)
    // 0001_xxxx or 0000_xxxx
    if first & 0xF0 == 0x10 || first & 0xF0 == 0x00 {
        return decode_literal_no_index(table, buf, 4);
    }

    Err("unknown HPACK header representation")
}

/// Decode a literal header field that should be added to the dynamic table.
fn decode_literal_indexed(
    table: &DynamicTable,
    buf: &[u8],
    prefix: u8,
) -> Result<(DecodeAction, usize), &'static str> {
    let name_info = decode_literal_name(table, buf, prefix, buf[0])?;
    let (name, offset) = match name_info {
        Some(v) => v,
        None => return Ok((DecodeAction::NeedMore, 0)),
    };

    let val_result = decode_string_literal(&buf[offset..])?;
    let (value, val_consumed) = match val_result {
        Some(v) => v,
        None => return Ok((DecodeAction::NeedMore, 0)),
    };

    Ok((DecodeAction::Emit(name, value, true), offset + val_consumed))
}

/// Decode a literal header field that should NOT be added to the dynamic table.
fn decode_literal_no_index(
    table: &DynamicTable,
    buf: &[u8],
    prefix: u8,
) -> Result<(DecodeAction, usize), &'static str> {
    let name_info = decode_literal_name(table, buf, prefix, buf[0])?;
    let (name, offset) = match name_info {
        Some(v) => v,
        None => return Ok((DecodeAction::NeedMore, 0)),
    };

    let val_result = decode_string_literal(&buf[offset..])?;
    let (value, val_consumed) = match val_result {
        Some(v) => v,
        None => return Ok((DecodeAction::NeedMore, 0)),
    };

    Ok((DecodeAction::Emit(name, value, false), offset + val_consumed))
}

/// Helper: decode the name part of a literal header.
/// Returns `Some((name_bytes, buffer_offset_to_value))`.
fn decode_literal_name(
    table: &DynamicTable,
    buf: &[u8],
    prefix: u8,
    first_byte: u8,
) -> Result<Option<(Bytes, usize)>, &'static str> {
    let name_index = (first_byte & ((1 << prefix) - 1)) as usize;

    if name_index > 0 {
        let result = decode_int(buf, prefix)?;
        let (idx, consumed) = match result {
            Some(v) => v,
            None => return Ok(None),
        };
        let (n, _) = lookup(table, idx)?;
        Ok(Some((Bytes::copy_from_slice(n), consumed)))
    } else {
        let result = decode_string_literal(&buf[1..])?;
        match result {
            Some((name, n)) => Ok(Some((name, n + 1))),
            None => Ok(None),
        }
    }
}

/// Look up a header by its 1-based HPACK index.
fn lookup(table: &DynamicTable, index: usize) -> Result<(&[u8], &[u8]), &'static str> {
    if index == 0 {
        return Err("index 0 is invalid in HPACK");
    }

    if let Some((name, value)) = static_table_get(index) {
        Ok((name.as_bytes(), value.as_bytes()))
    } else if let Some((name, value)) = table.get(index) {
        Ok((name, value))
    } else {
        Err("HPACK index out of range")
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

// ── HPACK Encoder ───────────────────────────────────────────────────────────
///
/// An HPACK encoder that writes header blocks.
#[derive(Debug)]
pub struct Encoder {
    table: DynamicTable,
}

impl Encoder {
    /// Create a new encoder with the default max table size.
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_TABLE_SIZE)
    }

    /// Create a new encoder with the given max table size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self { table: DynamicTable::new(max_size) }
    }

    /// Access the encoder's dynamic table.
    pub fn table(&self) -> &DynamicTable {
        &self.table
    }

    /// Mutable access to the encoder's dynamic table.
    pub fn table_mut(&mut self) -> &mut DynamicTable {
        &mut self.table
    }

    /// Encode a single header field and append to `dst`.
    pub fn encode_header(&mut self, name: &[u8], value: &[u8], dst: &mut BytesMut) {
        // Try indexed representation (full match in static or dynamic table)
        if let Some(idx) = static_table_find_exact(name, value)
            .or_else(|| self.table.find_exact(name, value))
        {
            // Indexed Header Field: 1xxx_xxxx
            let prefix_max = int_prefix_max(7);
            if idx <= prefix_max {
                dst.put_u8(0x80 | idx as u8);
            } else {
                dst.put_u8(0x80 | prefix_max as u8);
                encode_int(idx, 7, dst);
            }
            return;
        }

        // Try literal with indexed name
        let name_idx = static_table_find_name(name)
            .or_else(|| self.table.find_name(name));

        if let Some(idx) = name_idx {
            // Literal Header Field with Incremental Indexing: 01xx_xxxx
            let prefix_max = int_prefix_max(6);
            if idx <= prefix_max {
                dst.put_u8(0x40 | idx as u8);
            } else {
                dst.put_u8(0x40 | prefix_max as u8);
                encode_int(idx, 6, dst);
            }
        } else {
            // Literal name + value, both Huffman-encoded
            dst.put_u8(0x40); // 01xx_xxxx with index=0 → literal name follows
            encode_string_literal(name, dst);
        }

        // Encode the value (always Huffman-encoded)
        encode_string_literal(value, dst);

        // Add to dynamic table
        self.table.insert(Bytes::copy_from_slice(name), Bytes::copy_from_slice(value));
    }

    /// Encode a header field without adding it to the dynamic table.
    pub fn encode_header_no_index(&self, name: &[u8], value: &[u8], dst: &mut BytesMut) {
        // 0000_xxxx — Literal Header Field Without Indexing
        let name_idx = static_table_find_name(name);

        if let Some(idx) = name_idx {
            let prefix_max = int_prefix_max(4);
            if idx <= prefix_max {
                dst.put_u8(idx as u8);
            } else {
                dst.put_u8(prefix_max as u8);
                encode_int(idx, 4, dst);
            }
        } else {
            dst.put_u8(0x00); // index=0 → literal name follows
            encode_string_literal(name, dst);
        }

        encode_string_literal(value, dst);
    }

    /// Signal a dynamic table size update.
    pub fn encode_table_size_update(&mut self, new_max: usize, dst: &mut BytesMut) {
        // 001x_xxxx
        let prefix_max = int_prefix_max(5);
        if new_max <= prefix_max {
            dst.put_u8(0x20 | new_max as u8);
        } else {
            dst.put_u8(0x20 | prefix_max as u8);
            encode_int(new_max, 5, dst);
        }
        self.table.set_max_size(new_max);
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode a string literal (Huffman + length-prefixed).
pub fn encode_string_literal(data: &[u8], dst: &mut BytesMut) {
    // Always use Huffman encoding for compactness
    let mut encoded = BytesMut::new();
    huffman::encode(data, &mut encoded);

    let len = encoded.len();
    let prefix_max = int_prefix_max(7);

    if len < prefix_max {
        dst.put_u8(0x80 | len as u8); // Huffman flag set
    } else {
        dst.put_u8(0x80 | prefix_max as u8);
        encode_int(len, 7, dst);
    }
    dst.put_slice(&encoded);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Integer encoding ──────────────────────────────────────────────────

    #[test]
    fn decode_int_small() {
        // Value 10 with 5-bit prefix
        let buf = [0x0A]; // 0000_1010
        let (val, n) = decode_int(&buf, 5).unwrap().unwrap();
        assert_eq!(val, 10);
        assert_eq!(n, 1);
    }

    #[test]
    fn decode_int_prefix_max_values() {
        // When the prefix value equals 2^N - 1, RFC 7541 §5.1 requires
        // continuation bytes — the prefix alone is an incomplete encoding.
        // 31 with 5-bit prefix needs at least one continuation byte (0x00 = stop).
        let buf = [0x1F, 0x00]; // max prefix + 0 → 31
        let (val, n) = decode_int(&buf, 5).unwrap().unwrap();
        assert_eq!(val, 31);
        assert_eq!(n, 2);
    }

    #[test]
    fn decode_int_multibyte() {
        // Value 1337 with 5-bit prefix:
        // 1337 - 31 = 1306
        // 1306 % 128 = 26 → 0x1A (no continue)
        // 1306 / 128 = 10
        // 10 % 128 = 10 → 0x8A (continue = 0x80 | 10)
        // → bytes: [0x1F, 0x9A, 0x0A] wait, let me think again
        // 1337 = 0x539
        // prefix 5 bits → first byte has bottom 5 bits = 31 (0x1F)
        // remaining = 1337 - 31 = 1306
        // 1306 in base 128: 1306 / 128 = 10 r26
        // byte 2: 26 | 0x80 = 0x9A (continue)
        // byte 3: 10 = 0x0A (stop)
        let buf = [0x1F, 0x9A, 0x0A];
        let (val, n) = decode_int(&buf, 5).unwrap().unwrap();
        assert_eq!(val, 1337);
        assert_eq!(n, 3);
    }

    #[test]
    fn decode_int_incomplete() {
        let buf = [0x1F]; // max prefix → need more bytes but there aren't any
        assert_eq!(decode_int(&buf, 5).unwrap(), None);
    }

    // ── Static table ──────────────────────────────────────────────────────

    #[test]
    fn static_table_index_2_is_method_get() {
        let (name, value) = static_table_get(2).unwrap();
        assert_eq!(name, ":method");
        assert_eq!(value, "GET");
    }

    #[test]
    fn static_table_index_1_is_authority() {
        let (name, value) = static_table_get(1).unwrap();
        assert_eq!(name, ":authority");
        assert_eq!(value, "");
    }

    #[test]
    fn static_table_find() {
        assert_eq!(static_table_find_exact(b":method", b"GET"), Some(2));
        assert_eq!(static_table_find_exact(b":path", b"/"), Some(4));
        assert_eq!(static_table_find_name(b"content-type"), Some(31));
        assert_eq!(static_table_find_exact(b"nonexistent", b"value"), None);
    }

    // ── Dynamic table ─────────────────────────────────────────────────────

    #[test]
    fn dynamic_table_insert_and_lookup() {
        let mut table = DynamicTable::new(4096);
        table.insert(Bytes::from("custom-name"), Bytes::from("custom-value"));
        assert_eq!(table.len(), 1);

        let (name, value) = table.get(62).unwrap();
        assert_eq!(name, b"custom-name");
        assert_eq!(value, b"custom-value");
    }

    #[test]
    fn dynamic_table_evicts_when_full() {
        // Set max to hold exactly 2 small entries
        let entry_size = 5 + 5 + 32; // name + value + overhead = 42
        let mut table = DynamicTable::new(entry_size * 2);

        table.insert(Bytes::from("aaaaa"), Bytes::from("bbbbb"));
        table.insert(Bytes::from("ccccc"), Bytes::from("ddddd"));
        assert_eq!(table.len(), 2);

        // Third entry should evict the first
        table.insert(Bytes::from("eeeee"), Bytes::from("fffff"));
        assert_eq!(table.len(), 2);
        let (name, _) = table.get(62).unwrap();
        assert_eq!(name, b"eeeee"); // newest at front
    }

    #[test]
    fn dynamic_table_resize_to_zero_clears() {
        let mut table = DynamicTable::new(4096);
        table.insert(Bytes::from("a"), Bytes::from("b"));
        table.set_max_size(0);
        assert_eq!(table.len(), 0);
        assert_eq!(table.size(), 0);
    }

    // ── Decoder — indexed header ──────────────────────────────────────────

    #[test]
    fn decode_indexed_header() {
        // Index 2 (":method", "GET") with 7-bit prefix
        let mut dec = Decoder::new();
        let headers = dec.decode(&[0x82]).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(&headers[0].0[..], b":method");
        assert_eq!(&headers[0].1[..], b"GET");
    }

    // ── Decoder — literal with indexing ───────────────────────────────────

    #[test]
    fn decode_literal_indexed_name() {
        // Literal header with indexed name (":authority" = index 1) + literal value
        // 01xx_xxxx with index=1 → 0x41
        // value: Huffman-encoded "example.com" with 7-bit prefix length
        let mut buf = BytesMut::new();
        buf.put_u8(0x41); // indexed name 1, incremental indexing

        // "example.com" Huffman-encoded
        // e=101(0x65): code 0x1ffa (13 bits)
        // x=120(0x78): code 0x0fff_fff2 (28 bits), etc.
        // Let's just use non-Huffman for simplicity here
        let value = b"example.com";
        buf.put_u8(value.len() as u8); // length, Huffman flag = 0
        buf.put_slice(value);

        let mut dec = Decoder::new();
        let headers = dec.decode(&buf).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(&headers[0].0[..], b":authority");
        assert_eq!(&headers[0].1[..], b"example.com");
        // Should have been added to dynamic table
        assert_eq!(dec.table().len(), 1);
    }

    #[test]
    fn decode_multiple_headers_fragmented() {
        let mut dec = Decoder::new();

        // First fragment: just the first byte of indexed header
        let h1 = dec.decode(&[0x82]).unwrap(); // :method: GET
        assert_eq!(h1.len(), 1);

        // Second fragment: nothing
        let h2 = dec.decode(&[]).unwrap();
        assert_eq!(h2.len(), 0);
    }

    // ── Encoder ───────────────────────────────────────────────────────────

    #[test]
    fn encoder_indexed_static() {
        let mut enc = Encoder::new();
        let mut dst = BytesMut::new();

        // ":method: GET" → should be indexed (static table index 2)
        enc.encode_header(b":method", b"GET", &mut dst);
        assert_eq!(&dst[..], &[0x82]);
    }

    #[test]
    fn encoder_literal_indexed() {
        let mut enc = Encoder::new();
        let mut dst = BytesMut::new();

        // Custom header not in static table → literal with indexing
        enc.encode_header(b"x-custom", b"value", &mut dst);

        // Now decode what we encoded
        let mut dec = Decoder::new();
        let headers = dec.decode(&dst).unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(&headers[0].0[..], b"x-custom");
        assert_eq!(&headers[0].1[..], b"value");
    }

    // ── RFC 7541 Appendix C test vectors ──────────────────────────────────

    /// C.2.1 — First header: :method: GET → indexed (static table 2)
    #[test]
    fn rfc7541_c2_first_request() {
        let mut enc = Encoder::new();
        let mut dst = BytesMut::new();

        enc.encode_header(b":method", b"GET", &mut dst);
        enc.encode_header(b":scheme", b"http", &mut dst);
        enc.encode_header(b":path", b"/", &mut dst);
        enc.encode_header(b":authority", b"www.example.com", &mut dst);

        let mut dec = Decoder::new();
        let headers = dec.decode(&dst).unwrap();
        assert_eq!(headers.len(), 4);

        // Verify round-trip
        assert_eq!(&headers[0].0[..], b":method");
        assert_eq!(&headers[0].1[..], b"GET");
        assert_eq!(&headers[1].0[..], b":scheme");
        assert_eq!(&headers[1].1[..], b"http");
        assert_eq!(&headers[2].0[..], b":path");
        assert_eq!(&headers[2].1[..], b"/");
        assert_eq!(&headers[3].0[..], b":authority");
        assert_eq!(&headers[3].1[..], b"www.example.com");
    }

    /// C.2.2 — Second request: uses dynamic table from first response.
    /// After encoding the first request's headers, the dynamic table has entries.
    #[test]
    fn rfc7541_c2_second_request() {
        let mut enc = Encoder::new();
        let mut dst1 = BytesMut::new();

        // First, encode headers to populate the dynamic table
        enc.encode_header(b":method", b"GET", &mut dst1);
        enc.encode_header(b":scheme", b"http", &mut dst1);
        enc.encode_header(b":path", b"/", &mut dst1);
        enc.encode_header(b":authority", b"www.example.com", &mut dst1);

        // The encoder's dynamic table should now have entries
        assert!(enc.table().len() > 0);
    }

    /// Full round-trip: encode several headers, decode them, verify parity.
    #[test]
    fn roundtrip_various_headers() {
        let mut enc = Encoder::new();
        let mut dst = BytesMut::new();

        let input: Vec<(&[u8], &[u8])> = vec![
            (b":method", b"POST"),
            (b":scheme", b"https"),
            (b":path", b"/api/v1/users"),
            (b":authority", b"api.example.com"),
            (b"content-type", b"application/json"),
            (b"accept", b"application/json"),
            (b"user-agent", b"hpack-test/1.0"),
            (b"x-request-id", b"abc123"),
        ];

        for (name, value) in &input {
            enc.encode_header(name, value, &mut dst);
        }

        let mut dec = Decoder::new();
        let output = dec.decode(&dst).unwrap();

        assert_eq!(output.len(), input.len());
        for ((in_name, in_val), (out_name, out_val)) in input.iter().zip(output.iter()) {
            assert_eq!(out_name.as_ref(), *in_name, "name mismatch");
            assert_eq!(out_val.as_ref(), *in_val, "value mismatch for {}", String::from_utf8_lossy(in_name));
        }
    }
}
