//! WHY: Servers (HTTP `application/primal-arrow`, SSE `text/event-stream-arrow`)
//! and analytics tooling want DOM-op batches in REAL Apache Arrow IPC — readable
//! by every Arrow ecosystem implementation (feature 05, decision 010). The WASM
//! loop keeps `foundation_ui_traits::ArrowEncoder`'s compact owned layout
//! (binary-size; see the feature-05 status.md deviation) — THIS encoder is the
//! interop face, behind the very same `ProtocolEncoder` contract.
//!
//! WHAT: [`ArrowIpcEncoder`] — `Vec<DomOp>` ⇄ Arrow IPC stream bytes, using the
//! canonical decision-010 six-column schema:
//!
//! | column      | type   | nullable |
//! |-------------|--------|----------|
//! | `op_id`     | UInt32 | no       |
//! | `node_id`   | UInt32 | no       |
//! | `operation` | UInt8  | no       |
//! | `attribute` | Utf8   | yes      |
//! | `value`     | Utf8   | yes      |
//! | `text_val`  | Utf8   | yes      |
//!
//! HOW: Encoding maps each op through `foundation_ui_traits::Row` (THE one
//! `DomOp` ⇄ row mapping every wire format shares) into typed Arrow arrays and
//! writes one `RecordBatch` as an IPC stream. Decoding validates the schema,
//! rebuilds `Row`s (nulls become `None` cells), and runs the same
//! `Row::into_op`. The two Arrow wire forms are told apart by the envelope
//! VERSION byte: `1` = owned columnar layout, [`ARROW_IPC_VERSION`] = real IPC.

use std::sync::Arc;

use arrow_array::builder::{StringBuilder, UInt32Builder, UInt8Builder};
use arrow_array::{Array, RecordBatch, StringArray, UInt32Array, UInt8Array};
use arrow_schema::{DataType, Field, Schema};
use foundation_ui_traits::{DecodeError, DecodeResult, DomOp, ProtocolEncoder, Row, PROTOCOL_ARROW};

use crate::ipc::{decode_ipc, encode_ipc};

/// Envelope version byte for the REAL Arrow IPC wire form (the owned columnar
/// layout ships as version 1 / `foundation_ui_traits::PROTOCOL_VERSION`).
pub const ARROW_IPC_VERSION: u8 = 2;

/// The decision-010 six-column schema.
fn dom_ops_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("op_id", DataType::UInt32, false),
        Field::new("node_id", DataType::UInt32, false),
        Field::new("operation", DataType::UInt8, false),
        Field::new("attribute", DataType::Utf8, true),
        Field::new("value", DataType::Utf8, true),
        Field::new("text_val", DataType::Utf8, true),
    ]))
}

/// `Vec<DomOp>` ⇄ Apache Arrow IPC stream bytes (protocol byte 1, version 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArrowIpcEncoder;

impl ArrowIpcEncoder {
    /// Build the six-column `RecordBatch` for `ops` (also useful directly for
    /// analytics pipelines that want the batch, not the bytes).
    ///
    /// # Panics
    /// Never in practice — the builders cannot fail for in-memory data and the
    /// schema is fixed; a failure would indicate an arrow-rs bug.
    #[must_use]
    pub fn to_record_batch(ops: &[DomOp]) -> RecordBatch {
        let mut op_ids = UInt32Builder::with_capacity(ops.len());
        let mut node_ids = UInt32Builder::with_capacity(ops.len());
        let mut operations = UInt8Builder::with_capacity(ops.len());
        let mut attributes = StringBuilder::new();
        let mut values = StringBuilder::new();
        let mut text_vals = StringBuilder::new();

        for (i, op) in ops.iter().enumerate() {
            let row = Row::from_op(op);
            op_ids.append_value(u32::try_from(i).expect("more than u32::MAX ops in one batch"));
            node_ids.append_value(row.node_id);
            operations.append_value(row.operation);
            attributes.append_option(row.attribute);
            values.append_option(row.value);
            text_vals.append_option(row.text_val);
        }

        RecordBatch::try_new(
            dom_ops_schema(),
            vec![
                Arc::new(op_ids.finish()),
                Arc::new(node_ids.finish()),
                Arc::new(operations.finish()),
                Arc::new(attributes.finish()),
                Arc::new(values.finish()),
                Arc::new(text_vals.finish()),
            ],
        )
        .expect("fixed schema and equal-length columns cannot mismatch")
    }

    /// Decode a `RecordBatch` (already IPC-parsed) back into ops.
    ///
    /// # Errors
    /// [`DecodeError::SchemaMismatch`] for wrong column count/types,
    /// [`DecodeError::UnknownOperation`] / [`DecodeError::Other`] from the
    /// shared row mapping.
    pub fn from_record_batch(batch: &RecordBatch) -> DecodeResult<Vec<DomOp>> {
        if batch.num_columns() != 6 {
            return Err(DecodeError::SchemaMismatch {
                detail: format!("expected 6 columns, found {}", batch.num_columns()),
            });
        }
        let column = |idx: usize, name: &str| -> DecodeResult<&Arc<dyn Array>> {
            let field = batch.schema_ref().field(idx).clone();
            if field.name() != name {
                return Err(DecodeError::SchemaMismatch {
                    detail: format!("column {idx}: expected `{name}`, found `{}`", field.name()),
                });
            }
            Ok(batch.column(idx))
        };
        let downcast_err = |name: &str, ty: &str| DecodeError::SchemaMismatch {
            detail: format!("column `{name}` is not {ty}"),
        };

        let node_ids = column(1, "node_id")?
            .as_any()
            .downcast_ref::<UInt32Array>()
            .ok_or_else(|| downcast_err("node_id", "UInt32"))?;
        let operations = column(2, "operation")?
            .as_any()
            .downcast_ref::<UInt8Array>()
            .ok_or_else(|| downcast_err("operation", "UInt8"))?;
        let strings = |idx: usize, name: &'static str| -> DecodeResult<&StringArray> {
            column(idx, name)?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| downcast_err(name, "Utf8"))
        };
        let attributes = strings(3, "attribute")?;
        let values = strings(4, "value")?;
        let text_vals = strings(5, "text_val")?;

        let cell = |col: &StringArray, i: usize| -> Option<String> {
            if col.is_null(i) {
                None
            } else {
                Some(col.value(i).to_string())
            }
        };

        let mut ops = Vec::with_capacity(batch.num_rows());
        for i in 0..batch.num_rows() {
            ops.push(
                Row {
                    operation: operations.value(i),
                    node_id: node_ids.value(i),
                    attribute: cell(attributes, i),
                    value: cell(values, i),
                    text_val: cell(text_vals, i),
                }
                .into_op(i)?,
            );
        }
        Ok(ops)
    }
}

impl ProtocolEncoder<Vec<DomOp>> for ArrowIpcEncoder {
    fn protocol_byte(&self) -> u8 {
        PROTOCOL_ARROW
    }

    fn version(&self) -> u8 {
        ARROW_IPC_VERSION
    }

    fn encode(&self, data: Vec<DomOp>) -> Vec<u8> {
        let batch = Self::to_record_batch(&data);
        encode_ipc(&batch).expect("in-memory IPC encoding cannot fail")
    }

    fn decode(&self, payload: &[u8]) -> DecodeResult<Vec<DomOp>> {
        if payload.is_empty() {
            return Err(DecodeError::TruncatedBuffer {
                expected_min: 8,
                actual: 0,
            });
        }
        let batch = decode_ipc(payload).map_err(|error| DecodeError::SchemaMismatch {
            detail: format!("arrow IPC read failed: {error}"),
        })?;
        Self::from_record_batch(&batch)
    }
}
