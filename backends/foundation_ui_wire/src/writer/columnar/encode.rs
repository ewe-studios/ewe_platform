use alloc::{vec, vec::Vec};

use super::ColumnarError;
use crate::{
    vocab::check_prop, Alignment, Axis, ColumnarBatchT, LengthUnit, PropValueT, SetPropT,
    UiOperationT,
};

#[derive(Default)]
struct Fields<'a> {
    node: u32,
    a: u32,
    b: u32,
    blob: &'a [u8],
}

pub(super) fn batch(rows: &[UiOperationT]) -> Result<ColumnarBatchT, ColumnarError> {
    let mut codes = Vec::with_capacity(rows.len());
    let mut nodes = Vec::with_capacity(rows.len());
    let mut args = Vec::with_capacity(rows.len());
    let mut values = Vec::with_capacity(rows.len());
    let mut offsets = vec![0];
    let mut blob = Vec::new();
    for (row, operation) in rows.iter().enumerate() {
        let fields = fields(operation, row)?;
        let end = blob
            .len()
            .checked_add(fields.blob.len())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or(ColumnarError::BlobTooLarge { row })?;
        codes.push(operation.ui_operation_type().0);
        nodes.push(fields.node);
        args.push(fields.a);
        values.push(fields.b);
        offsets.push(end);
        blob.extend_from_slice(fields.blob);
    }
    let end = offsets[rows.len()];
    Ok(ColumnarBatchT {
        op_codes: Some(codes),
        node_ids: Some(nodes),
        arg_a: Some(args),
        arg_b: Some(values),
        blob_b_off: Some(vec![end; offsets.len()]),
        blob_a_off: Some(offsets),
        blob_bytes: Some(blob),
    })
}

fn fields(op: &UiOperationT, row: usize) -> Result<Fields<'_>, ColumnarError> {
    Ok(match op {
        UiOperationT::CreateNode(v) => Fields {
            node: v.node_id,
            a: u32::from(v.kind.0),
            ..Fields::default()
        },
        UiOperationT::SetText(v) => Fields {
            node: v.node_id,
            blob: v.text.as_deref().unwrap_or_default().as_bytes(),
            ..Fields::default()
        },
        UiOperationT::SetProp(v) => property(v, row)?,
        UiOperationT::AppendChild(v) => Fields {
            node: v.parent_id,
            a: v.child_id,
            ..Fields::default()
        },
        UiOperationT::InsertBefore(v) => Fields {
            node: v.parent_id,
            a: v.child_id,
            b: v.before_id,
            ..Fields::default()
        },
        UiOperationT::Remove(v) => Fields {
            node: v.node_id,
            ..Fields::default()
        },
        UiOperationT::BindAction(v) => Fields {
            node: v.node_id,
            a: u32::from(v.event.0),
            blob: v.action.as_deref().unwrap_or_default().as_bytes(),
            ..Fields::default()
        },
        UiOperationT::UnbindAction(v) => Fields {
            node: v.node_id,
            a: u32::from(v.event.0),
            ..Fields::default()
        },
        UiOperationT::AttachState(v) => Fields {
            node: v.root_node,
            blob: v.state.as_deref().unwrap_or_default(),
            ..Fields::default()
        },
        UiOperationT::ReplaceSubtree(_)
        | UiOperationT::LiveMount(_)
        | UiOperationT::UiBehaviour(_) => {
            return Err(ColumnarError::UnsupportedOp {
                row,
                code: op.ui_operation_type().0,
            })
        }
        UiOperationT::NONE => return Err(ColumnarError::UnknownOp { row, code: 0 }),
    })
}

fn property(prop: &SetPropT, row: usize) -> Result<Fields<'_>, ColumnarError> {
    check_prop(prop.prop, &prop.value).map_err(|_| ColumnarError::InvalidValue { row })?;
    let mut fields = Fields {
        node: prop.node_id,
        a: u32::from(prop.prop.0),
        ..Fields::default()
    };
    match &prop.value {
        PropValueT::Length(v) => {
            let length = v.v.as_ref().ok_or(ColumnarError::InvalidValue { row })?;
            if length.unit.0 > LengthUnit::ENUM_MAX {
                return Err(ColumnarError::InvalidValue { row });
            }
            fields.a |= u32::from(length.unit.0) << 16;
            fields.b = length.value.to_bits();
        }
        PropValueT::ColorToken(v) => fields.b = v.id,
        PropValueT::FontToken(v) => fields.b = v.id,
        PropValueT::Alignment(v) if v.v.0 <= Alignment::ENUM_MAX => fields.b = u32::from(v.v.0),
        PropValueT::Axis(v) if v.v.0 <= Axis::ENUM_MAX => fields.b = u32::from(v.v.0),
        PropValueT::Bool(v) => fields.b = u32::from(v.v),
        PropValueT::Text(v) => fields.blob = v.v.as_deref().unwrap_or_default().as_bytes(),
        _ => return Err(ColumnarError::InvalidValue { row }),
    }
    Ok(fields)
}
