use alloc::{boxed::Box, string::String, vec::Vec};

use super::ColumnarError;
use crate::{
    vocab::{prop_kind, PropKind},
    Alignment, AlignmentValueT, AppendChildT, AttachStateT, Axis, AxisValueT, BindActionT,
    BoolValueT, ColorTokenT, ColumnarBatchT, CreateNodeT, EventId, FontTokenT, InsertBeforeT,
    LengthT, LengthUnit, LengthValueT, NodeKind, PropId, PropValueT, RemoveT, SetPropT, SetTextT,
    TextValueT, UiOperation, UiOperationT, UnbindActionT,
};

fn column<'a, T>(
    data: Option<&'a [T]>,
    name: &'static str,
    size: usize,
) -> Result<&'a [T], ColumnarError> {
    let values = data.unwrap_or_default();
    if values.len() != size {
        return Err(ColumnarError::LengthMismatch {
            column: name,
            expected: size,
            actual: values.len(),
        });
    }
    Ok(values)
}

pub(super) fn rows(batch: &ColumnarBatchT) -> Result<Vec<UiOperationT>, ColumnarError> {
    let codes = batch.op_codes.as_deref().unwrap_or_default();
    let count = codes.len();
    let nodes = column(batch.node_ids.as_deref(), "node_ids", count)?;
    let args = column(batch.arg_a.as_deref(), "arg_a", count)?;
    let values = column(batch.arg_b.as_deref(), "arg_b", count)?;
    let first = column(batch.blob_a_off.as_deref(), "blob_a_off", count + 1)?;
    let second = column(batch.blob_b_off.as_deref(), "blob_b_off", count + 1)?;
    let bytes = batch.blob_bytes.as_deref().unwrap_or_default();

    // Every eligible operation uses at most one blob. The first fixture
    // represents the unused second column with zero offsets.
    let end = first[count] as usize;
    if first[0] != 0 || end != bytes.len() || (second[0] != 0 && second[0] as usize != end) {
        return Err(ColumnarError::BadOffsets { row: 0 });
    }

    (0..count)
        .map(|row| {
            if second[row] != second[row + 1] {
                return Err(ColumnarError::BadOffsets { row });
            }
            let blob = bytes
                .get(first[row] as usize..first[row + 1] as usize)
                .ok_or(ColumnarError::BadOffsets { row })?;
            decode_row(
                row,
                UiOperation(codes[row]),
                nodes[row],
                args[row],
                values[row],
                blob,
            )
        })
        .collect()
}

fn text(bytes: &[u8], row: usize) -> Result<Option<String>, ColumnarError> {
    if bytes.is_empty() {
        return Ok(None);
    }
    core::str::from_utf8(bytes)
        .map(|value| Some(String::from(value)))
        .map_err(|_| ColumnarError::InvalidUtf8 { row })
}

fn decode_row(
    row: usize,
    op: UiOperation,
    node_id: u32,
    a: u32,
    b: u32,
    blob: &[u8],
) -> Result<UiOperationT, ColumnarError> {
    if !unused_fields_are_empty(op, a, b, blob) {
        return Err(ColumnarError::InvalidValue { row });
    }
    let narrow = |value| u16::try_from(value).map_err(|_| ColumnarError::InvalidValue { row });
    let operation = match op {
        UiOperation::CreateNode => UiOperationT::CreateNode(Box::new(CreateNodeT {
            node_id,
            kind: NodeKind(narrow(a)?),
        })),
        UiOperation::SetText => UiOperationT::SetText(Box::new(SetTextT {
            node_id,
            text: text(blob, row)?,
        })),
        UiOperation::SetProp => {
            let prop = PropId(narrow(a & 0xffff)?);
            let value = property(row, prop, a >> 16, b, blob)?;
            UiOperationT::SetProp(Box::new(SetPropT {
                node_id,
                prop,
                value,
            }))
        }
        UiOperation::AppendChild => UiOperationT::AppendChild(Box::new(AppendChildT {
            parent_id: node_id,
            child_id: a,
        })),
        UiOperation::InsertBefore => UiOperationT::InsertBefore(Box::new(InsertBeforeT {
            parent_id: node_id,
            child_id: a,
            before_id: b,
        })),
        UiOperation::Remove => UiOperationT::Remove(Box::new(RemoveT { node_id })),
        UiOperation::BindAction => UiOperationT::BindAction(Box::new(BindActionT {
            node_id,
            event: EventId(narrow(a)?),
            action: text(blob, row)?,
        })),
        UiOperation::UnbindAction => UiOperationT::UnbindAction(Box::new(UnbindActionT {
            node_id,
            event: EventId(narrow(a)?),
        })),
        UiOperation::AttachState => UiOperationT::AttachState(Box::new(AttachStateT {
            root_node: node_id,
            state: Some(blob.to_vec()),
        })),
        UiOperation::ReplaceSubtree | UiOperation::LiveMount | UiOperation::UiBehaviour => {
            return Err(ColumnarError::UnsupportedOp { row, code: op.0 });
        }
        _ => return Err(ColumnarError::UnknownOp { row, code: op.0 }),
    };
    Ok(operation)
}

fn unused_fields_are_empty(operation: UiOperation, a: u32, b: u32, blob: &[u8]) -> bool {
    match operation {
        UiOperation::CreateNode | UiOperation::AppendChild | UiOperation::UnbindAction => {
            b == 0 && blob.is_empty()
        }
        UiOperation::SetText | UiOperation::AttachState => a == 0 && b == 0,
        UiOperation::InsertBefore => blob.is_empty(),
        UiOperation::Remove => a == 0 && b == 0 && blob.is_empty(),
        UiOperation::BindAction => b == 0,
        _ => true,
    }
}

fn property(
    row: usize,
    prop: PropId,
    unit: u32,
    bits: u32,
    blob: &[u8],
) -> Result<PropValueT, ColumnarError> {
    let kind = prop_kind(prop).ok_or(ColumnarError::InvalidValue { row })?;
    if (kind != PropKind::Length && unit != 0) || (kind != PropKind::Text && !blob.is_empty()) {
        return Err(ColumnarError::InvalidValue { row });
    }
    Ok(match kind {
        PropKind::Length if unit <= u32::from(LengthUnit::ENUM_MAX) => {
            PropValueT::Length(Box::new(LengthValueT {
                v: Some(LengthT {
                    value: f32::from_bits(bits),
                    unit: LengthUnit(
                        u8::try_from(unit).map_err(|_| ColumnarError::InvalidValue { row })?,
                    ),
                }),
            }))
        }
        PropKind::ColorToken => PropValueT::ColorToken(Box::new(ColorTokenT { id: bits })),
        PropKind::FontToken => PropValueT::FontToken(Box::new(FontTokenT { id: bits })),
        PropKind::Alignment if bits <= u32::from(Alignment::ENUM_MAX) => {
            PropValueT::Alignment(Box::new(AlignmentValueT {
                v: Alignment(u8::try_from(bits).map_err(|_| ColumnarError::InvalidValue { row })?),
            }))
        }
        PropKind::Axis if bits <= u32::from(Axis::ENUM_MAX) => {
            PropValueT::Axis(Box::new(AxisValueT {
                v: Axis(u8::try_from(bits).map_err(|_| ColumnarError::InvalidValue { row })?),
            }))
        }
        PropKind::Bool if bits <= 1 => PropValueT::Bool(Box::new(BoolValueT { v: bits == 1 })),
        PropKind::Text if bits == 0 => PropValueT::Text(Box::new(TextValueT {
            v: text(blob, row)?,
        })),
        _ => return Err(ColumnarError::InvalidValue { row }),
    })
}
