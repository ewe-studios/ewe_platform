// The generated Rust packers ignore forced vector alignment. Keep their table
// construction order and use the alignment helper only for marked vectors.
use alloc::vec::Vec;

use flatbuffers::{EndianScalar, FlatBufferBuilder, UnionWIPOffset, Vector, WIPOffset};

use crate::{
    create_aligned_vector, ArrowIpcBlob, ArrowIpcBlobArgs, Column, ColumnArgs, ColumnDataT,
    ColumnT, ColumnarBatch, ColumnarBatchArgs, ColumnarBatchT, Data, DataArgs, DataT, DataTable,
    DataTableArgs, F64s, F64sArgs, FrameMessage, FrameMessageArgs, FrameMessageT, I32s, I32sArgs,
    I64s, I64sArgs, Message, MessageArgs, MessageT, Operation, OperationArgs, OperationT,
    ReplaceSubtree, ReplaceSubtreeArgs, UiOperationT,
};

pub(super) fn pack_message_frame<'a>(
    builder: &mut FlatBufferBuilder<'a>,
    frame: &FrameMessageT,
) -> WIPOffset<FrameMessage<'a>> {
    let messages = frame.messages.as_ref().map(|messages| {
        let offsets: Vec<_> = messages
            .iter()
            .map(|msg| pack_message(builder, msg))
            .collect();
        builder.create_vector(&offsets)
    });
    FrameMessage::create(
        builder,
        &FrameMessageArgs {
            ack: frame.ack,
            messages,
        },
    )
}

fn pack_message<'a>(builder: &mut FlatBufferBuilder<'a>, msg: &MessageT) -> WIPOffset<Message<'a>> {
    let operation = msg.operation.as_ref().map(|op| pack_operation(builder, op));
    let data = msg.data.as_ref().map(|data| pack_data(builder, data));
    let control = msg.control.as_ref().map(|control| control.pack(builder));
    Message::create(
        builder,
        &MessageArgs {
            operation,
            data,
            control,
        },
    )
}

fn pack_operation<'a>(
    builder: &mut FlatBufferBuilder<'a>,
    operation: &OperationT,
) -> WIPOffset<Operation<'a>> {
    let ui = match &operation.ui {
        UiOperationT::ReplaceSubtree(subtree) => {
            let operations = subtree.operations.as_ref().map(|operations| {
                let offsets: Vec<_> = operations
                    .iter()
                    .map(|op| pack_operation(builder, op))
                    .collect();
                builder.create_vector(&offsets)
            });
            let offset = ReplaceSubtree::create(
                builder,
                &ReplaceSubtreeArgs {
                    node_id: subtree.node_id,
                    operations,
                },
            );
            Some(offset.as_union_value())
        }
        ui => ui.pack(builder),
    };
    let web = operation.web.pack(builder);
    let mobile = operation.mobile.pack(builder);
    let columnar = operation
        .columnar
        .as_ref()
        .map(|batch| pack_columnar(builder, batch));
    Operation::create(
        builder,
        &OperationArgs {
            ui_type: operation.ui.ui_operation_type(),
            ui,
            web_type: operation.web.web_operation_type(),
            web,
            mobile_type: operation.mobile.mobile_operation_type(),
            mobile,
            columnar,
        },
    )
}

fn pack_columnar<'a>(
    builder: &mut FlatBufferBuilder<'a>,
    batch: &ColumnarBatchT,
) -> WIPOffset<ColumnarBatch<'a>> {
    let args = ColumnarBatchArgs {
        op_codes: batch
            .op_codes
            .as_ref()
            .map(|codes| builder.create_vector(codes)),
        node_ids: aligned_vector(builder, batch.node_ids.as_deref()),
        arg_a: aligned_vector(builder, batch.arg_a.as_deref()),
        arg_b: aligned_vector(builder, batch.arg_b.as_deref()),
        blob_a_off: aligned_vector(builder, batch.blob_a_off.as_deref()),
        blob_b_off: aligned_vector(builder, batch.blob_b_off.as_deref()),
        blob_bytes: batch
            .blob_bytes
            .as_ref()
            .map(|blob| builder.create_vector(blob)),
    };
    ColumnarBatch::create(builder, &args)
}

fn pack_data<'a>(builder: &mut FlatBufferBuilder<'a>, data: &DataT) -> WIPOffset<Data<'a>> {
    let arrow = data.arrow.as_ref().map(|arrow| {
        let bytes = aligned_vector(builder, arrow.bytes.as_deref());
        ArrowIpcBlob::create(
            builder,
            &ArrowIpcBlobArgs {
                target_node: arrow.target_node,
                bytes,
            },
        )
    });
    let table = data.table.as_ref().map(|table| {
        let columns = table.columns.as_ref().map(|columns| {
            let offsets: Vec<_> = columns
                .iter()
                .map(|column| pack_column(builder, column))
                .collect();
            builder.create_vector(&offsets)
        });
        DataTable::create(
            builder,
            &DataTableArgs {
                target_node: table.target_node,
                rows: table.rows,
                columns,
            },
        )
    });
    let msgpack = data.msgpack.as_ref().map(|msgpack| msgpack.pack(builder));
    Data::create(
        builder,
        &DataArgs {
            target_node: data.target_node,
            arrow,
            table,
            msgpack,
        },
    )
}

fn pack_column<'a>(builder: &mut FlatBufferBuilder<'a>, column: &ColumnT) -> WIPOffset<Column<'a>> {
    let name = column.name.as_ref().map(|name| builder.create_string(name));
    let data = pack_column_data(builder, &column.data);
    let validity = column
        .validity
        .as_ref()
        .map(|validity| builder.create_vector(validity));
    Column::create(
        builder,
        &ColumnArgs {
            name,
            data_type: column.data.column_data_type(),
            data,
            validity,
        },
    )
}

fn pack_column_data(
    builder: &mut FlatBufferBuilder<'_>,
    data: &ColumnDataT,
) -> Option<WIPOffset<UnionWIPOffset>> {
    match data {
        ColumnDataT::I32(column) => {
            let values = aligned_vector(builder, column.values.as_deref());
            Some(I32s::create(builder, &I32sArgs { values }).as_union_value())
        }
        ColumnDataT::I64(column) => {
            let values = aligned_vector(builder, column.values.as_deref());
            Some(I64s::create(builder, &I64sArgs { values }).as_union_value())
        }
        ColumnDataT::F64(column) => {
            let values = aligned_vector(builder, column.values.as_deref());
            Some(F64s::create(builder, &F64sArgs { values }).as_union_value())
        }
        other => other.pack(builder),
    }
}

fn aligned_vector<'a, T: EndianScalar>(
    builder: &mut FlatBufferBuilder<'a>,
    values: Option<&[T]>,
) -> Option<WIPOffset<Vector<'a, T>>> {
    values.map(|values| create_aligned_vector(builder, values))
}
