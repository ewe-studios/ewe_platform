//! WHY: HTTP servers, SSE endpoints, WebSocket servers, CLI tools, and the WASM
//! runtime all need to turn `DomOp` batches into bytes. Putting the encoders in a
//! pure, dependency-free, `no_std` crate lets every one of those consumers share
//! the exact same wire format without dragging in WASM, FFI, or an allocator arena.
//!
//! WHAT: Layer 1 of the three-layer protocol architecture (decisions 014/022/028/030).
//! Defines the [`ProtocolEncoder`] trait plus three concrete encoders — [`ArrowEncoder`]
//! (Arrow-inspired columnar layout), [`JsonEncoder`] (UTF-8 JSON), and
//! and the 6-byte [`Envelope`] header. (Protocol byte 0 — Custom Binary — is the
//! `foundation_wasm` Instructions format; see the note above `PROTOCOL_CUSTOM_BINARY`.)
//!
//! HOW: Each encoder is a pure `Vec<DomOp> -> Vec<u8>` (and back) transform. No
//! `MemoryAllocations`, no FFI, no `host_apply`. The WASM transport layer
//! (`foundation_wasm`) and the composed protocol impls (`foundation_wasm_ui`) build
//! on top of these by adding arena memory and the FFI handoff.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use crate::DomOp;

// ─── Protocol bytes (decision 014/022) ────────────────────────────────────────

/// Protocol discriminant for the Custom Binary format.
pub const PROTOCOL_CUSTOM_BINARY: u8 = 0;
/// Protocol discriminant for the Arrow columnar format.
pub const PROTOCOL_ARROW: u8 = 1;
/// Protocol discriminant for the JSON format.
pub const PROTOCOL_JSON: u8 = 2;

/// Current version emitted by every encoder in this crate.
pub const PROTOCOL_VERSION: u8 = 0;

// ─── Operation codes (decision 010 — Arrow batch schema) ───────────────────────
//
// These `u8` operation codes are the canonical cross-language identity of a
// `DomOp`. Both the Arrow columnar layout and the Custom Binary tag stream use
// them, and the JS applicator switches on the same numbers.

const OP_CREATE_ELEMENT: u8 = 0;
const OP_SET_TEXT_CONTENT: u8 = 2;
const OP_SET_ATTRIBUTE: u8 = 3;
const OP_APPEND_CHILD: u8 = 8;
const OP_REMOVE_NODE: u8 = 10;
const OP_INSERT_BEFORE: u8 = 11;
const OP_REPLACE_NODE: u8 = 12;
const OP_SET_STYLE: u8 = 13;
const OP_ADD_CLASS: u8 = 14;
const OP_MORPH_NODE: u8 = 16;

// ─── DecodeError ───────────────────────────────────────────────────────────────

/// WHY: Decoding bytes that came off a socket, a fetch body, or an arena slot can
/// fail in many small ways; callers need a single typed error to branch on.
///
/// WHAT: Every failure mode shared by the three decoders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The byte slice ended before a fixed-width field could be read.
    UnexpectedEnd,
    /// An operation code (decision 010 `u8`) was not recognised.
    UnknownOperation(u8),
    /// A length/offset field pointed outside the payload.
    InvalidLength,
    /// Bytes that were expected to be UTF-8 were not.
    InvalidUtf8,
    /// A numeric reference (child/ref/new id) stored as text failed to parse.
    InvalidNumber,
    /// The JSON payload was malformed at the given byte position.
    MalformedJson(usize),
    /// A required JSON field was missing for an operation object.
    MissingField,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => f.write_str("unexpected end of payload"),
            Self::UnknownOperation(op) => write!(f, "unknown operation code: {op}"),
            Self::InvalidLength => f.write_str("length field exceeds payload bounds"),
            Self::InvalidUtf8 => f.write_str("payload contained invalid UTF-8"),
            Self::InvalidNumber => f.write_str("numeric reference failed to parse"),
            Self::MalformedJson(pos) => write!(f, "malformed JSON at byte {pos}"),
            Self::MissingField => f.write_str("JSON operation object missing a required field"),
        }
    }
}

/// Result of decoding a payload back into a `Vec<DomOp>`.
pub type DecodeResult = Result<Vec<DomOp>, DecodeError>;

// ─── ProtocolEncoder trait ─────────────────────────────────────────────────────

/// WHY: The runtime, HTTP servers, and tests all want to swap encoders behind one
/// interface so the choice of wire format is a configuration detail, not a
/// rewrite.
///
/// WHAT: A bidirectional, pure codec for a payload type `T`. Implementors carry no
/// state — they are zero-sized markers.
///
/// HOW: `encode` produces the protocol-specific payload bytes (no envelope);
/// `decode` reverses it. The `protocol_byte`/`version` pair identifies the format
/// inside an [`Envelope`].
pub trait ProtocolEncoder<T> {
    /// The protocol discriminant written into the envelope header.
    fn protocol_byte(&self) -> u8;
    /// The protocol version written into the envelope header.
    fn version(&self) -> u8;
    /// Encode `data` into protocol-specific payload bytes (envelope not included).
    fn encode(&self, data: T) -> Vec<u8>;
    /// Decode a protocol-specific payload back into the original data.
    ///
    /// # Errors
    /// Returns [`DecodeError`] when the bytes are truncated, malformed, or carry an
    /// unknown operation code.
    fn decode(&self, payload: &[u8]) -> Result<T, DecodeError>;
}

// ─── Envelope (6-byte, transport-agnostic) ─────────────────────────────────────

/// WHY: Every consumer outside WASM (HTTP, SSE, WebSocket) needs a tiny self-
/// describing header so the receiver can demux the format without out-of-band
/// metadata. WASM extends this with an arena id in `foundation_wasm::WasmEnvelope`.
///
/// WHAT: The 6-byte header `[protocol:1][version:1][length:4 LE]` that precedes a
/// protocol payload.
///
/// HOW: `write` prepends the header to the payload; `parse` splits it back apart.
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
    /// Header size in bytes.
    pub const HEADER_LEN: usize = 6;

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
    /// # Errors
    /// Returns [`DecodeError::UnexpectedEnd`] if `bytes` is shorter than the header
    /// or the declared `length` overruns the slice.
    pub fn parse(bytes: &[u8]) -> Result<(Envelope, &[u8]), DecodeError> {
        if bytes.len() < Self::HEADER_LEN {
            return Err(DecodeError::UnexpectedEnd);
        }
        let protocol = bytes[0];
        let version = bytes[1];
        let length = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        let end = Self::HEADER_LEN
            .checked_add(length as usize)
            .ok_or(DecodeError::InvalidLength)?;
        if bytes.len() < end {
            return Err(DecodeError::UnexpectedEnd);
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

// ─── Byte cursor helpers ───────────────────────────────────────────────────────

/// Minimal forward-only reader over a byte slice. Keeps the decoders readable and
/// keeps every bounds check in one place.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        let b = *self.bytes.get(self.pos).ok_or(DecodeError::UnexpectedEnd)?;
        self.pos += 1;
        Ok(b)
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        let end = self.pos.checked_add(4).ok_or(DecodeError::InvalidLength)?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(DecodeError::UnexpectedEnd)?;
        self.pos = end;
        Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(DecodeError::InvalidLength)?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(DecodeError::UnexpectedEnd)?;
        self.pos = end;
        Ok(slice)
    }

}

/// Convert a `usize` length / count / offset into the `u32` the wire format uses.
///
/// WHY: every length, row count, and string offset in these formats is a 4-byte
/// `u32` field — the same convention Apache Arrow IPC uses with its i32 offsets.
/// That caps a single batch, payload, or string column at `u32::MAX` (~4 GiB),
/// which is far beyond any real DOM update: batch sizes are bounded by the document,
/// not by the target's pointer width, so they're identical on wasm32 and wasm64.
/// Using a checked conversion means a value past the cap fails loudly instead of
/// silently truncating on a 64-bit target.
///
/// # Panics
/// Panics if `value > u32::MAX` — a >4 GiB DOM batch, which never occurs in practice
/// and would indicate a bug upstream.
#[inline]
fn to_u32(value: usize) -> u32 {
    u32::try_from(value).expect("wire-format field exceeds u32::MAX (~4 GiB)")
}


/// Parse a stringified `u32` reference (child/ref/new id).
fn parse_ref(s: &str) -> Result<u32, DecodeError> {
    s.parse::<u32>().map_err(|_| DecodeError::InvalidNumber)
}

// ─── Canonical row view ────────────────────────────────────────────────────────
//
// Both the Arrow columnar layout and the Custom Binary tag stream serialise a
// `DomOp` through the same canonical `(operation, node_id, attribute, value,
// text_val)` shape defined by decision 010. Centralising the mapping keeps the two
// binary encoders bug-for-bug consistent with each other and with the JS side.

/// The canonical decision-010 row form of a [`DomOp`]: one operation byte, the
/// target node id, and the three string slots. Public so protocol impls (Arrow's
/// columns, the batch-instructions custom protocol) share ONE `DomOp` ⇄ row mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Operation code (decision 010 table).
    pub operation: u8,
    /// Target node id (primal id).
    pub node_id: u32,
    /// Attribute name slot (empty when unused).
    pub attribute: String,
    /// Value slot (empty when unused).
    pub value: String,
    /// Text-content slot (empty when unused).
    pub text_val: String,
}

impl Row {
    /// Map a [`DomOp`] to its row form.
    #[must_use]
    pub fn from_op(op: &DomOp) -> Self {
        let (operation, node_id, attribute, value, text_val) = match op {
            DomOp::CreateEl {
                node_id,
                tag,
                class,
            } => (
                OP_CREATE_ELEMENT,
                *node_id,
                tag.clone(),
                class.clone(),
                String::new(),
            ),
            DomOp::SetText { node_id, text } => (
                OP_SET_TEXT_CONTENT,
                *node_id,
                String::new(),
                String::new(),
                text.clone(),
            ),
            DomOp::SetAttr {
                node_id,
                name,
                value,
            } => (
                OP_SET_ATTRIBUTE,
                *node_id,
                name.clone(),
                value.clone(),
                String::new(),
            ),
            DomOp::SetStyle {
                node_id,
                prop,
                value,
            } => (
                OP_SET_STYLE,
                *node_id,
                prop.clone(),
                value.clone(),
                String::new(),
            ),
            DomOp::SetClass { node_id, class } => (
                OP_ADD_CLASS,
                *node_id,
                String::new(),
                class.clone(),
                String::new(),
            ),
            DomOp::AppendChild {
                parent_id,
                child_id,
            } => (
                OP_APPEND_CHILD,
                *parent_id,
                child_id.to_string(),
                String::new(),
                String::new(),
            ),
            DomOp::Remove { node_id } => (
                OP_REMOVE_NODE,
                *node_id,
                String::new(),
                String::new(),
                String::new(),
            ),
            DomOp::InsertBefore {
                parent_id,
                child_id,
                ref_id,
            } => (
                OP_INSERT_BEFORE,
                *parent_id,
                child_id.to_string(),
                String::new(),
                ref_id.to_string(),
            ),
            DomOp::Replace { old_id, new_id } => (
                OP_REPLACE_NODE,
                *old_id,
                String::new(),
                new_id.to_string(),
                String::new(),
            ),
            DomOp::Morph { node_id, html } => (
                OP_MORPH_NODE,
                *node_id,
                String::new(),
                String::new(),
                html.clone(),
            ),
        };
        Self {
            operation,
            node_id,
            attribute,
            value,
            text_val,
        }
    }

    /// Map the row form back to a [`DomOp`].
    ///
    /// # Errors
    /// Returns [`DecodeError`] for an unknown operation code.
    pub fn into_op(self) -> Result<DomOp, DecodeError> {
        let op = match self.operation {
            OP_CREATE_ELEMENT => DomOp::CreateEl {
                node_id: self.node_id,
                tag: self.attribute,
                class: self.value,
            },
            OP_SET_TEXT_CONTENT => DomOp::SetText {
                node_id: self.node_id,
                text: self.text_val,
            },
            OP_SET_ATTRIBUTE => DomOp::SetAttr {
                node_id: self.node_id,
                name: self.attribute,
                value: self.value,
            },
            OP_SET_STYLE => DomOp::SetStyle {
                node_id: self.node_id,
                prop: self.attribute,
                value: self.value,
            },
            OP_ADD_CLASS => DomOp::SetClass {
                node_id: self.node_id,
                class: self.value,
            },
            OP_APPEND_CHILD => DomOp::AppendChild {
                parent_id: self.node_id,
                child_id: parse_ref(&self.attribute)?,
            },
            OP_REMOVE_NODE => DomOp::Remove {
                node_id: self.node_id,
            },
            OP_INSERT_BEFORE => DomOp::InsertBefore {
                parent_id: self.node_id,
                child_id: parse_ref(&self.attribute)?,
                ref_id: parse_ref(&self.text_val)?,
            },
            OP_REPLACE_NODE => DomOp::Replace {
                old_id: self.node_id,
                new_id: parse_ref(&self.value)?,
            },
            OP_MORPH_NODE => DomOp::Morph {
                node_id: self.node_id,
                html: self.text_val,
            },
            other => return Err(DecodeError::UnknownOperation(other)),
        };
        Ok(op)
    }
}

// ─── Protocol byte 0 (Custom Binary) ───────────────────────────────────────────
//
// Protocol byte 0 is the foundation_wasm INSTRUCTIONS format (decision 022) — the
// batch-operations system (`Operations` opcodes + quantized params + texts pool),
// implemented by `foundation_wasm::ops::Instructions` (encode) and the JS
// `BatchInstructions` runtime (execute). It is an instruction STREAM, not a value
// encoding, so it has no `ProtocolEncoder` here; `foundation_wasm_ui` exposes it
// through the transport traits as `BatchInstructionsV1`.

// ─── ArrowEncoder (Arrow-inspired columnar layout) ─────────────────────────────

/// WHY: A columnar layout lets the JS runtime build zero-copy `TypedArray` views
/// over each column (decision 010), which is far cheaper than parsing row-by-row.
/// Real Apache Arrow IPC needs `std`; this `no_std` layout keeps the same column
/// shape while staying dependency-free.
///
/// WHAT: Encodes a `DomOp` batch into six columns — `op_id`, `node_id`,
/// `operation`, `attribute`, `value`, `text_val`.
///
/// HOW: `[row_count:4]` then the fixed-width columns (`op_id` u32×N, `node_id`
/// u32×N, `operation` u8×N) followed by three string columns. Each string column is
/// `[(N+1) offsets: u32 LE][data_len: u32][utf8 data]` — the Arrow varlen layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArrowEncoder;

impl ArrowEncoder {
    /// Append an Arrow-style variable-length string column: an `N+1` offset buffer
    /// followed by the concatenated UTF-8 data.
    fn write_string_column(buf: &mut Vec<u8>, values: &[String]) {
        let mut offset: u32 = 0;
        // Offsets buffer: N+1 cumulative byte offsets.
        buf.extend_from_slice(&offset.to_le_bytes());
        for v in values {
            offset += to_u32(v.len());
            buf.extend_from_slice(&offset.to_le_bytes());
        }
        // Data buffer: total length then concatenated bytes.
        buf.extend_from_slice(&offset.to_le_bytes());
        for v in values {
            buf.extend_from_slice(v.as_bytes());
        }
    }

    /// Read a string column written by [`write_string_column`].
    fn read_string_column(cur: &mut Cursor<'_>, count: usize) -> Result<Vec<String>, DecodeError> {
        let mut offsets = Vec::with_capacity(count + 1);
        for _ in 0..=count {
            offsets.push(cur.u32()?);
        }
        let data_len = cur.u32()? as usize;
        let data = cur.bytes(data_len)?;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let start = offsets[i] as usize;
            let end = offsets[i + 1] as usize;
            if start > end || end > data.len() {
                return Err(DecodeError::InvalidLength);
            }
            let s =
                core::str::from_utf8(&data[start..end]).map_err(|_| DecodeError::InvalidUtf8)?;
            out.push(s.to_string());
        }
        Ok(out)
    }
}

impl ProtocolEncoder<Vec<DomOp>> for ArrowEncoder {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_ARROW
    }

    fn version(&self) -> u8 {
        PROTOCOL_VERSION
    }

    fn encode(&self, data: Vec<DomOp>) -> Vec<u8> {
        let rows: Vec<Row> = data.iter().map(Row::from_op).collect();
        let n = rows.len();
        let mut buf = Vec::new();
        buf.extend_from_slice(&to_u32(n).to_le_bytes());

        // Fixed-width columns.
        for i in 0..n {
            buf.extend_from_slice(&to_u32(i).to_le_bytes()); // op_id = sequential index
        }
        for row in &rows {
            buf.extend_from_slice(&row.node_id.to_le_bytes());
        }
        for row in &rows {
            buf.push(row.operation);
        }

        // Variable-length string columns.
        let attribute: Vec<String> = rows.iter().map(|r| r.attribute.clone()).collect();
        let value: Vec<String> = rows.iter().map(|r| r.value.clone()).collect();
        let text_val: Vec<String> = rows.iter().map(|r| r.text_val.clone()).collect();
        Self::write_string_column(&mut buf, &attribute);
        Self::write_string_column(&mut buf, &value);
        Self::write_string_column(&mut buf, &text_val);
        buf
    }

    fn decode(&self, payload: &[u8]) -> DecodeResult {
        let mut cur = Cursor::new(payload);
        let count = cur.u32()? as usize;

        // Skip op_id column (sequential, redundant on decode but kept for the
        // columnar contract / JS zero-copy views).
        for _ in 0..count {
            let _ = cur.u32()?;
        }
        let mut node_ids = Vec::with_capacity(count);
        for _ in 0..count {
            node_ids.push(cur.u32()?);
        }
        let mut operations = Vec::with_capacity(count);
        for _ in 0..count {
            operations.push(cur.u8()?);
        }
        let attribute = Self::read_string_column(&mut cur, count)?;
        let value = Self::read_string_column(&mut cur, count)?;
        let text_val = Self::read_string_column(&mut cur, count)?;

        let mut ops = Vec::with_capacity(count);
        for i in 0..count {
            ops.push(
                Row {
                    operation: operations[i],
                    node_id: node_ids[i],
                    attribute: attribute[i].clone(),
                    value: value[i].clone(),
                    text_val: text_val[i].clone(),
                }
                .into_op()?,
            );
        }
        Ok(ops)
    }
}

// ─── JsonEncoder ───────────────────────────────────────────────────────────────

/// WHY: A human-readable format for debugging, SSE `text/event-stream-json`, and
/// servers that prefer JSON over binary.
///
/// WHAT: Encodes a `DomOp` batch as a JSON array of tagged objects, e.g.
/// `[{"op":"SetText","node_id":2,"text":"hi"}]`.
///
/// HOW: Hand-written encode/decode over the fixed `DomOp` shape — no external
/// serde dependency, so it stays `no_std`. Strings are escaped/unescaped per the
/// JSON spec subset needed here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JsonEncoder;

impl ProtocolEncoder<Vec<DomOp>> for JsonEncoder {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_JSON
    }

    fn version(&self) -> u8 {
        PROTOCOL_VERSION
    }

    fn encode(&self, data: Vec<DomOp>) -> Vec<u8> {
        let mut s = String::from("[");
        for (i, op) in data.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            encode_op_json(&mut s, op);
        }
        s.push(']');
        s.into_bytes()
    }

    fn decode(&self, payload: &[u8]) -> DecodeResult {
        let text = core::str::from_utf8(payload).map_err(|_| DecodeError::InvalidUtf8)?;
        let value = json::parse(text)?;
        let json::Value::Array(array) = value else {
            return Err(DecodeError::MalformedJson(0));
        };
        let mut ops = Vec::with_capacity(array.len());
        for item in array {
            ops.push(op_from_json(item)?);
        }
        Ok(ops)
    }
}

/// Append the JSON object for one `DomOp` to `out`.
fn encode_op_json(out: &mut String, op: &DomOp) {
    match op {
        DomOp::CreateEl {
            node_id,
            tag,
            class,
        } => {
            out.push_str("{\"op\":\"CreateEl\",\"node_id\":");
            out.push_str(&node_id.to_string());
            out.push_str(",\"tag\":");
            json::write_string(out, tag);
            out.push_str(",\"class\":");
            json::write_string(out, class);
            out.push('}');
        }
        DomOp::SetText { node_id, text } => {
            out.push_str("{\"op\":\"SetText\",\"node_id\":");
            out.push_str(&node_id.to_string());
            out.push_str(",\"text\":");
            json::write_string(out, text);
            out.push('}');
        }
        DomOp::SetAttr {
            node_id,
            name,
            value,
        } => {
            out.push_str("{\"op\":\"SetAttr\",\"node_id\":");
            out.push_str(&node_id.to_string());
            out.push_str(",\"name\":");
            json::write_string(out, name);
            out.push_str(",\"value\":");
            json::write_string(out, value);
            out.push('}');
        }
        DomOp::SetClass { node_id, class } => {
            out.push_str("{\"op\":\"SetClass\",\"node_id\":");
            out.push_str(&node_id.to_string());
            out.push_str(",\"class\":");
            json::write_string(out, class);
            out.push('}');
        }
        DomOp::SetStyle {
            node_id,
            prop,
            value,
        } => {
            out.push_str("{\"op\":\"SetStyle\",\"node_id\":");
            out.push_str(&node_id.to_string());
            out.push_str(",\"prop\":");
            json::write_string(out, prop);
            out.push_str(",\"value\":");
            json::write_string(out, value);
            out.push('}');
        }
        DomOp::AppendChild {
            parent_id,
            child_id,
        } => {
            out.push_str("{\"op\":\"AppendChild\",\"parent_id\":");
            out.push_str(&parent_id.to_string());
            out.push_str(",\"child_id\":");
            out.push_str(&child_id.to_string());
            out.push('}');
        }
        DomOp::Remove { node_id } => {
            out.push_str("{\"op\":\"Remove\",\"node_id\":");
            out.push_str(&node_id.to_string());
            out.push('}');
        }
        DomOp::InsertBefore {
            parent_id,
            child_id,
            ref_id,
        } => {
            out.push_str("{\"op\":\"InsertBefore\",\"parent_id\":");
            out.push_str(&parent_id.to_string());
            out.push_str(",\"child_id\":");
            out.push_str(&child_id.to_string());
            out.push_str(",\"ref_id\":");
            out.push_str(&ref_id.to_string());
            out.push('}');
        }
        DomOp::Replace { old_id, new_id } => {
            out.push_str("{\"op\":\"Replace\",\"old_id\":");
            out.push_str(&old_id.to_string());
            out.push_str(",\"new_id\":");
            out.push_str(&new_id.to_string());
            out.push('}');
        }
        DomOp::Morph { node_id, html } => {
            out.push_str("{\"op\":\"Morph\",\"node_id\":");
            out.push_str(&node_id.to_string());
            out.push_str(",\"html\":");
            json::write_string(out, html);
            out.push('}');
        }
    }
}

/// Build a `DomOp` from a decoded JSON object value.
fn op_from_json(value: json::Value) -> Result<DomOp, DecodeError> {
    let json::Value::Object(obj) = value else {
        return Err(DecodeError::MalformedJson(0));
    };
    let get_str = |key: &str| -> Result<String, DecodeError> {
        for (k, v) in &obj {
            if k == key {
                if let json::Value::String(s) = v {
                    return Ok(s.clone());
                }
                return Err(DecodeError::MissingField);
            }
        }
        Err(DecodeError::MissingField)
    };
    let get_num = |key: &str| -> Result<u32, DecodeError> {
        for (k, v) in &obj {
            if k == key {
                if let json::Value::Number(n) = v {
                    return Ok(*n);
                }
                return Err(DecodeError::MissingField);
            }
        }
        Err(DecodeError::MissingField)
    };
    let op = get_str("op")?;
    let dom_op = match op.as_str() {
        "CreateEl" => DomOp::CreateEl {
            node_id: get_num("node_id")?,
            tag: get_str("tag")?,
            class: get_str("class")?,
        },
        "SetText" => DomOp::SetText {
            node_id: get_num("node_id")?,
            text: get_str("text")?,
        },
        "SetAttr" => DomOp::SetAttr {
            node_id: get_num("node_id")?,
            name: get_str("name")?,
            value: get_str("value")?,
        },
        "SetClass" => DomOp::SetClass {
            node_id: get_num("node_id")?,
            class: get_str("class")?,
        },
        "SetStyle" => DomOp::SetStyle {
            node_id: get_num("node_id")?,
            prop: get_str("prop")?,
            value: get_str("value")?,
        },
        "AppendChild" => DomOp::AppendChild {
            parent_id: get_num("parent_id")?,
            child_id: get_num("child_id")?,
        },
        "Remove" => DomOp::Remove {
            node_id: get_num("node_id")?,
        },
        "InsertBefore" => DomOp::InsertBefore {
            parent_id: get_num("parent_id")?,
            child_id: get_num("child_id")?,
            ref_id: get_num("ref_id")?,
        },
        "Replace" => DomOp::Replace {
            old_id: get_num("old_id")?,
            new_id: get_num("new_id")?,
        },
        "Morph" => DomOp::Morph {
            node_id: get_num("node_id")?,
            html: get_str("html")?,
        },
        _ => return Err(DecodeError::MissingField),
    };
    Ok(dom_op)
}

// ─── Minimal no_std JSON value parser ──────────────────────────────────────────
//
// Scoped to exactly what `JsonEncoder` emits: arrays of flat objects whose values
// are strings or non-negative integers. It correctly handles JSON string escapes
// so round-tripping arbitrary text content works. Not a general-purpose parser.
mod json {
    use super::DecodeError;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// A decoded JSON value (subset).
    pub enum Value {
        String(String),
        Number(u32),
        Object(Vec<(String, Value)>),
        Array(Vec<Value>),
    }

    /// Escape and quote `s` into `out` per the JSON string grammar.
    pub fn write_string(out: &mut String, s: &str) {
        out.push('"');
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    out.push_str("\\u");
                    let code = c as u32;
                    for shift in [12, 8, 4, 0] {
                        let nibble = (code >> shift) & 0xF;
                        out.push(core::char::from_digit(nibble, 16).unwrap_or('0'));
                    }
                }
                c => out.push(c),
            }
        }
        out.push('"');
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    impl Parser<'_> {
        fn skip_ws(&mut self) {
            while let Some(&b) = self.bytes.get(self.pos) {
                if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }

        fn peek(&self) -> Result<u8, DecodeError> {
            self.bytes
                .get(self.pos)
                .copied()
                .ok_or(DecodeError::MalformedJson(self.pos))
        }

        fn expect(&mut self, b: u8) -> Result<(), DecodeError> {
            if self.peek()? == b {
                self.pos += 1;
                Ok(())
            } else {
                Err(DecodeError::MalformedJson(self.pos))
            }
        }

        fn value(&mut self) -> Result<Value, DecodeError> {
            self.skip_ws();
            match self.peek()? {
                b'"' => self.string().map(Value::String),
                b'{' => self.object(),
                b'[' => self.array(),
                b'0'..=b'9' => self.number().map(Value::Number),
                _ => Err(DecodeError::MalformedJson(self.pos)),
            }
        }

        fn string(&mut self) -> Result<String, DecodeError> {
            self.expect(b'"')?;
            let mut out = String::new();
            loop {
                let b = self.peek()?;
                self.pos += 1;
                match b {
                    b'"' => break,
                    b'\\' => {
                        let esc = self.peek()?;
                        self.pos += 1;
                        match esc {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'b' => out.push('\u{0008}'),
                            b'f' => out.push('\u{000C}'),
                            b'u' => out.push(self.unicode_escape()?),
                            _ => return Err(DecodeError::MalformedJson(self.pos)),
                        }
                    }
                    // UTF-8 continuation: collect the full multibyte sequence.
                    0x80..=0xFF => {
                        let start = self.pos - 1;
                        while let Some(&n) = self.bytes.get(self.pos) {
                            if (0x80..0xC0).contains(&n) {
                                self.pos += 1;
                            } else {
                                break;
                            }
                        }
                        let s = core::str::from_utf8(&self.bytes[start..self.pos])
                            .map_err(|_| DecodeError::InvalidUtf8)?;
                        out.push_str(s);
                    }
                    _ => out.push(b as char),
                }
            }
            Ok(out)
        }

        fn unicode_escape(&mut self) -> Result<char, DecodeError> {
            let mut code: u32 = 0;
            for _ in 0..4 {
                let b = self.peek()?;
                self.pos += 1;
                let digit = (b as char)
                    .to_digit(16)
                    .ok_or(DecodeError::MalformedJson(self.pos))?;
                code = code * 16 + digit;
            }
            core::char::from_u32(code).ok_or(DecodeError::MalformedJson(self.pos))
        }

        fn number(&mut self) -> Result<u32, DecodeError> {
            let start = self.pos;
            while let Some(&b) = self.bytes.get(self.pos) {
                if b.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let s = core::str::from_utf8(&self.bytes[start..self.pos])
                .map_err(|_| DecodeError::InvalidUtf8)?;
            s.parse::<u32>().map_err(|_| DecodeError::InvalidNumber)
        }

        fn object(&mut self) -> Result<Value, DecodeError> {
            self.expect(b'{')?;
            let mut pairs = Vec::new();
            self.skip_ws();
            if self.peek()? == b'}' {
                self.pos += 1;
                return Ok(Value::Object(pairs));
            }
            loop {
                self.skip_ws();
                let key = self.string()?;
                self.skip_ws();
                self.expect(b':')?;
                let val = self.value()?;
                pairs.push((key, val));
                self.skip_ws();
                match self.peek()? {
                    b',' => {
                        self.pos += 1;
                    }
                    b'}' => {
                        self.pos += 1;
                        break;
                    }
                    _ => return Err(DecodeError::MalformedJson(self.pos)),
                }
            }
            Ok(Value::Object(pairs))
        }

        fn array(&mut self) -> Result<Value, DecodeError> {
            self.expect(b'[')?;
            let mut items = Vec::new();
            self.skip_ws();
            if self.peek()? == b']' {
                self.pos += 1;
                return Ok(Value::Array(items));
            }
            loop {
                let val = self.value()?;
                items.push(val);
                self.skip_ws();
                match self.peek()? {
                    b',' => {
                        self.pos += 1;
                    }
                    b']' => {
                        self.pos += 1;
                        break;
                    }
                    _ => return Err(DecodeError::MalformedJson(self.pos)),
                }
            }
            Ok(Value::Array(items))
        }
    }

    /// Parse a JSON document into a [`Value`].
    ///
    /// # Errors
    /// Returns [`DecodeError::MalformedJson`] at the offending byte position.
    pub fn parse(text: &str) -> Result<Value, DecodeError> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        let value = parser.value()?;
        parser.skip_ws();
        Ok(value)
    }
}
