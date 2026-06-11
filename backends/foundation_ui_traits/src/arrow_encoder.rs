//! WHY: A columnar layout lets the JS runtime build zero-copy `TypedArray` views
//! over each column (decision 010), which is far cheaper than parsing row-by-row.
//! Real Apache Arrow IPC (flatbuffer schema messages, validity bitmaps) needs
//! `std` and the `arrow` crate; this `no_std` layout keeps the same column shape
//! while staying dependency-free. Feature 05 (arrow-encoding) owns the swap to
//! real Arrow IPC behind the same [`ProtocolEncoder`] face.
//!
//! WHAT: [`ArrowEncoder`] — encodes a `DomOp` batch into the six decision-010
//! columns: `op_id`, `node_id`, `operation`, `attribute`, `value`, `text_val`.
//!
//! HOW: `[row_count:4]` then the fixed-width columns (`op_id` u32×N, `node_id`
//! u32×N, `operation` u8×N) followed by three string columns. Each string column
//! is `[(N+1) offsets: u32 LE][data_len: u32][utf8 data]` — the Arrow varlen
//! layout. `None` cells encode as zero-length strings (the op code determines
//! which cells are meaningful, so absent and empty coincide; see `Row::into_op`).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::encoder::{
    to_u32, Cursor, DecodeError, DecodeResult, ProtocolEncoder, Row, PROTOCOL_ARROW,
    PROTOCOL_VERSION,
};
use crate::DomOp;

/// The Arrow-columnar [`ProtocolEncoder`] (protocol byte 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArrowEncoder;

/// Column names reported in [`DecodeError::InvalidUtf8`].
const STRING_COLUMNS: [&str; 3] = ["attribute", "value", "text_val"];

impl ArrowEncoder {
    /// Append an Arrow-style variable-length string column: an `N+1` offset
    /// buffer followed by the concatenated UTF-8 data.
    fn write_string_column(buf: &mut Vec<u8>, values: &[Option<&str>]) {
        let mut offset: u32 = 0;
        // Offsets buffer: N+1 cumulative byte offsets.
        buf.extend_from_slice(&offset.to_le_bytes());
        for v in values {
            offset += to_u32(v.map_or(0, str::len));
            buf.extend_from_slice(&offset.to_le_bytes());
        }
        // Data buffer: total length then concatenated bytes.
        buf.extend_from_slice(&offset.to_le_bytes());
        for v in values.iter().flatten() {
            buf.extend_from_slice(v.as_bytes());
        }
    }

    /// Read a string column written by [`Self::write_string_column`].
    fn read_string_column(
        cur: &mut Cursor<'_>,
        count: usize,
        column_name: &'static str,
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
                    detail: alloc::format!(
                        "column `{column_name}` offsets out of bounds at row {i}"
                    ),
                });
            }
            let s = core::str::from_utf8(&data[start..end]).map_err(|_| {
                DecodeError::InvalidUtf8 {
                    column_name,
                    row_index: i,
                }
            })?;
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
        let attribute: Vec<Option<&str>> = rows.iter().map(|r| r.attribute.as_deref()).collect();
        let value: Vec<Option<&str>> = rows.iter().map(|r| r.value.as_deref()).collect();
        let text_val: Vec<Option<&str>> = rows.iter().map(|r| r.text_val.as_deref()).collect();
        Self::write_string_column(&mut buf, &attribute);
        Self::write_string_column(&mut buf, &value);
        Self::write_string_column(&mut buf, &text_val);
        buf
    }

    fn decode(&self, payload: &[u8]) -> DecodeResult<Vec<DomOp>> {
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
        let attribute = Self::read_string_column(&mut cur, count, STRING_COLUMNS[0])?;
        let value = Self::read_string_column(&mut cur, count, STRING_COLUMNS[1])?;
        let text_val = Self::read_string_column(&mut cur, count, STRING_COLUMNS[2])?;

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
