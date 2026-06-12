//! WHY: A columnar layout lets the JS runtime build ZERO-COPY `TypedArray`
//! views over each column (decision 010) — but views demand ALIGNMENT, the
//! discipline Apache Arrow enforces with 8-byte buffer padding (feature 19).
//! And the row→column transform belongs at QUEUE time, not flush time: the
//! builder writes every string byte exactly once, with no intermediate
//! `String`s or `Vec<DomOp>`.
//!
//! WHAT: The compact-columnar wire form v1.1 — [`ColumnarBatch`] (the
//! columnar-native accumulator) and [`ColumnarEncoder`] (the
//! `ProtocolEncoder` face over it). Protocol byte 1, wire VERSION 1; wire
//! version 2 is REAL Apache Arrow IPC (`foundation_arrow::ArrowIpcEncoder`).
//!
//! HOW — layout v1.1 (all integers little-endian):
//!
//! ```text
//! payload := [pad_len: u8] [0x00 × pad_len]        // alignment shim
//!            [row_count: u32] [flags: u32 = 0]     // 8-byte header
//!            [op_id:     u32 × N]                  // header-relative 8 — aligned
//!            [node_id:   u32 × N]
//!            [operation: u8  × N] [pad to 4]
//!            [attribute column] [value column] [text_val column]
//!
//! column  := [(N+1) offsets: u32] [data_len: u32] [utf8 bytes] [pad to 4]
//! ```
//!
//! ALIGNMENT CONTRACT: every producer pads so the 8-byte header lands on an
//! 8-byte boundary — the PURE encoder relative to the payload start
//! (`pad_len = 7`, so a payload held as its own buffer is aligned at offset
//! 0), and the wasm FRAMING layer (`ColumnarV1`) relative to the ABSOLUTE
//! arena address it knows. Consumers therefore expect aligned data as the
//! rule; the JS parser still verifies and falls back to copying if some
//! foreign producer breaks the contract.
//!
//! Absent cells encode as zero-length strings (the op code alone determines
//! which cells are meaningful — see `Row::into_op`); the columnar form needs
//! no validity bitmaps.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::encoder::{
    row_view, to_u32, Cursor, DecodeError, DecodeResult, ProtocolEncoder, Row, PROTOCOL_ARROW,
    PROTOCOL_VERSION,
};
use crate::DomOp;

/// Pad value the PURE encoder uses: `1 + 7 = 8`, so the header starts at
/// relative offset 8 and a standalone payload buffer is aligned at offset 0.
const RELATIVE_ALIGN_PAD: u8 = 7;

/// Column names reported in [`DecodeError`]s.
const STRING_COLUMNS: [&str; 3] = ["attribute", "value", "text_val"];

// ─── ColumnarBatch (columnar-native accumulation) ─────────────────────────────

/// One variable-length string column: Arrow-style `N+1` cumulative offsets
/// plus concatenated UTF-8 data.
#[derive(Clone, Debug, Default)]
struct StrColumn {
    offsets: Vec<u32>,
    data: Vec<u8>,
}

impl StrColumn {
    fn push(&mut self, cell: Option<&str>) {
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        if let Some(text) = cell {
            self.data.extend_from_slice(text.as_bytes());
        }
        self.offsets.push(to_u32(self.data.len()));
    }

    fn clear(&mut self) {
        self.offsets.clear();
        self.data.clear();
    }

    /// Serialized size: offsets + `data_len` field + data (NO trailing pad).
    fn byte_len(&self, rows: usize) -> usize {
        (rows + 1) * 4 + 4 + self.data.len()
    }
}

/// Columnar-native `DomOp` accumulator (feature 19 §3): `push` appends one op
/// straight into column buffers via [`row_view`] — no `Vec<DomOp>`, no
/// intermediate `String`s; every string byte is copied exactly once.
/// `serialize` emits the v1.1 body; buffers survive `clear()` for reuse
/// across flush cycles.
#[derive(Clone, Debug, Default)]
pub struct ColumnarBatch {
    op_id: Vec<u32>,
    node_id: Vec<u32>,
    operation: Vec<u8>,
    attribute: StrColumn,
    value: StrColumn,
    text_val: StrColumn,
}

impl ColumnarBatch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one op to the columns.
    pub fn push(&mut self, op: &DomOp) {
        row_view(op, |operation, node_id, attribute, value, text_val| {
            self.op_id.push(to_u32(self.operation.len()));
            self.node_id.push(node_id);
            self.operation.push(operation);
            self.attribute.push(attribute);
            self.value.push(value);
            self.text_val.push(text_val);
        });
    }

    /// Rows accumulated.
    #[must_use]
    pub fn len(&self) -> usize {
        self.operation.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operation.is_empty()
    }

    /// Empty the columns, KEEPING their capacity (receiver reuse across
    /// flushes — the same G22 rationale as the row queue).
    pub fn clear(&mut self) {
        self.op_id.clear();
        self.node_id.clear();
        self.operation.clear();
        self.attribute.clear();
        self.value.clear();
        self.text_val.clear();
    }

    /// Total serialized size for a given alignment shim.
    #[must_use]
    pub fn serialized_len(&self, pad_len: u8) -> usize {
        let rows = self.len();
        let mut size = 1 + pad_len as usize + 8; // marker + shim + header
        size += rows * 4 * 2; // op_id + node_id
        size += rows; // operation
        size += pad4(size - (1 + pad_len as usize));
        for column in [&self.attribute, &self.value, &self.text_val] {
            size += column.byte_len(rows);
            size += pad4(size - (1 + pad_len as usize));
        }
        size
    }

    /// Serialize with the standalone-payload shim (`pad_len = 7` — the header
    /// lands 8-aligned relative to the payload start, so EVERY standalone
    /// buffer is correctly aligned; see the module alignment contract).
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        self.serialize_with_pad(RELATIVE_ALIGN_PAD)
    }

    /// Serialize with an explicit shim — the wasm framing layer computes
    /// `pad_len` from the ABSOLUTE slot address so the header lands 8-aligned
    /// in linear memory.
    ///
    /// # Panics
    /// Panics if `pad_len > 7` (the shim is one alignment window).
    #[must_use]
    pub fn serialize_with_pad(&self, pad_len: u8) -> Vec<u8> {
        assert!(pad_len < 8, "alignment shim is 0..=7");
        let rows = self.len();
        let mut out = Vec::with_capacity(self.serialized_len(pad_len));

        out.push(pad_len);
        out.resize(out.len() + pad_len as usize, 0);
        let header_at = out.len(); // all column alignment is relative to here

        out.extend_from_slice(&to_u32(rows).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // flags (reserved: cached-string demux)

        for id in &self.op_id {
            out.extend_from_slice(&id.to_le_bytes());
        }
        for id in &self.node_id {
            out.extend_from_slice(&id.to_le_bytes());
        }
        out.extend_from_slice(&self.operation);
        pad_to_4(&mut out, header_at);

        for column in [&self.attribute, &self.value, &self.text_val] {
            if rows == 0 {
                // Zero rows still write the single leading 0 offset.
                out.extend_from_slice(&0u32.to_le_bytes());
            } else {
                for offset in &column.offsets {
                    out.extend_from_slice(&offset.to_le_bytes());
                }
            }
            out.extend_from_slice(&to_u32(column.data.len()).to_le_bytes());
            out.extend_from_slice(&column.data);
            pad_to_4(&mut out, header_at);
        }
        out
    }
}

/// Bytes needed to reach the next multiple of 4 from `len`.
fn pad4(len: usize) -> usize {
    (4 - (len % 4)) % 4
}

fn pad_to_4(out: &mut Vec<u8>, base: usize) {
    let pad = pad4(out.len() - base);
    out.resize(out.len() + pad, 0);
}

// ─── ColumnarEncoder (ProtocolEncoder face) ────────────────────────────────────

/// The compact-columnar [`ProtocolEncoder`] — protocol byte 1, wire VERSION 1.
/// (Wire version 2 is REAL Apache Arrow IPC: `foundation_arrow::ArrowIpcEncoder`.)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColumnarEncoder;

impl ProtocolEncoder<Vec<DomOp>> for ColumnarEncoder {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_ARROW
    }

    fn version(&self) -> u8 {
        PROTOCOL_VERSION
    }

    fn encode(&self, data: Vec<DomOp>) -> Vec<u8> {
        let mut batch = ColumnarBatch::new();
        for op in &data {
            batch.push(op);
        }
        batch.serialize()
    }

    fn decode(&self, payload: &[u8]) -> DecodeResult<Vec<DomOp>> {
        let mut cur = Cursor::new(payload);

        // Alignment shim.
        let pad_len = cur.u8()?;
        if pad_len >= 8 {
            return Err(DecodeError::SchemaMismatch {
                detail: alloc::format!("alignment shim {pad_len} out of range (0..=7)"),
            });
        }
        let _ = cur.bytes(pad_len as usize)?;
        let header_at = 1 + pad_len as usize;

        let count = cur.u32()? as usize;
        let _flags = cur.u32()?; // reserved (future cached-string demux)

        // op_id column (sequential, redundant on decode but part of the
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
        cur.skip_pad4(header_at)?;

        let attribute = read_string_column(&mut cur, count, STRING_COLUMNS[0], header_at)?;
        let value = read_string_column(&mut cur, count, STRING_COLUMNS[1], header_at)?;
        let text_val = read_string_column(&mut cur, count, STRING_COLUMNS[2], header_at)?;

        let mut ops = Vec::with_capacity(count);
        for (i, ((operation, node_id), ((attr, val), text))) in operations
            .into_iter()
            .zip(node_ids)
            .zip(attribute.into_iter().zip(value).zip(text_val))
            .enumerate()
        {
            ops.push(
                Row {
                    operation,
                    node_id,
                    attribute: Some(attr),
                    value: Some(val),
                    text_val: Some(text),
                }
                .into_op(i)?,
            );
        }
        Ok(ops)
    }
}

/// Read one string column (offsets + `data_len` + data + pad).
fn read_string_column(
    cur: &mut Cursor<'_>,
    count: usize,
    column_name: &'static str,
    header_at: usize,
) -> DecodeResult<Vec<String>> {
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
            return Err(DecodeError::SchemaMismatch {
                detail: alloc::format!("column `{column_name}` offsets out of bounds at row {i}"),
            });
        }
        let s = core::str::from_utf8(&data[start..end]).map_err(|_| DecodeError::InvalidUtf8 {
            column_name,
            row_index: i,
        })?;
        out.push(s.to_string());
    }
    cur.skip_pad4(header_at)?;
    Ok(out)
}
