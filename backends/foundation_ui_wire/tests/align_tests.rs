use flatbuffers::{EndianScalar, FlatBufferBuilder, Follow, Vector, Verifiable};
use foundation_ui_wire::{
    create_aligned_vector, finish_frame, read_frame, ArrowIpcBlobT, ColumnDataT, ColumnT,
    ColumnarBatch, ColumnarBatchT, DataT, DataTableT, F64sT, FrameKindT, FrameMessageT, FrameT,
    I32sT, I64sT, MessageT, OperationT, ReplaceSubtreeT, UiOperationT, WireLimits,
    FRAME_IDENTIFIER, SCHEMA_VERSION,
};

#[test]
fn aligned_vector_starts_on_8_bytes() {
    for length in [0, 1, 3, 7, 64] {
        assert_alignment(&[7_u32; 64][..length]);
        assert_alignment(&[-7_i64; 64][..length]);
        assert_alignment(&[7.5_f64; 64][..length]);
    }
}

fn assert_alignment<T>(items: &[T])
where
    T: EndianScalar + core::fmt::Debug + for<'a> Follow<'a, Inner = T>,
    for<'a> Vector<'a, T>: Verifiable,
{
    let mut builder = FlatBufferBuilder::new();
    let vector = create_aligned_vector(&mut builder, items);
    builder.finish(vector, None);
    let bytes = builder.finished_data();
    let vector = flatbuffers::root::<Vector<'_, T>>(bytes).unwrap();
    let start = vector.bytes().as_ptr().addr() - bytes.as_ptr().addr();
    assert_eq!(start % 8, 0);
    assert!(vector.iter().eq(items.iter().copied()));
}

#[test]
fn generated_pack_can_misalign_a_columnar_vector() {
    let frame = message_frame(operation_message(columnar_operation(3)));
    let mut builder = FlatBufferBuilder::new();
    let root = frame.pack(&mut builder);
    builder.finish(root, Some(FRAME_IDENTIFIER));
    let bytes = builder.finished_data();
    let operation = read_frame(bytes, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap()
        .get(0)
        .operation()
        .unwrap();
    let offsets = columnar_offsets(bytes, operation.columnar().unwrap());
    assert!(offsets.iter().any(|offset| offset % 8 != 0), "{offsets:?}");
}

#[test]
fn finish_frame_aligns_top_level_and_nested_columnar_vectors() {
    for rows in [0, 1, 3, 65, 1000] {
        let nested = table!(OperationT {
            ui: UiOperationT::ReplaceSubtree(Box::new(table!(ReplaceSubtreeT {
                node_id: 2,
                operations: Some(vec![columnar_operation(rows)]),
            }))),
            columnar: Some(Box::new(columnar_batch(rows))),
        });
        let frame = message_frame(operation_message(nested));
        let bytes = finish_frame(&frame).unwrap();
        let decoded = read_frame(&bytes, &WireLimits::default()).unwrap();
        assert_eq!(decoded.unpack(), frame);

        let operation = decoded
            .kind_as_frame_message()
            .unwrap()
            .messages()
            .unwrap()
            .get(0)
            .operation()
            .unwrap();
        let nested_operation = operation
            .ui_as_replace_subtree()
            .unwrap()
            .operations()
            .unwrap()
            .get(0);
        for batch in [operation.columnar(), nested_operation.columnar()] {
            let offsets = columnar_offsets(&bytes, batch.unwrap());
            assert!(offsets.iter().all(|offset| offset % 8 == 0), "{offsets:?}");
        }
    }
}

#[test]
fn finish_frame_aligns_table_columns_and_arrow_bytes() {
    for length in [0, 1, 5, 65] {
        let columns = vec![
            table!(ColumnT {
                name: Some("i32".into()),
                data: ColumnDataT::I32(Box::new(table!(I32sT {
                    values: Some(vec![1; length])
                }))),
            }),
            table!(ColumnT {
                name: Some("i64".into()),
                data: ColumnDataT::I64(Box::new(table!(I64sT {
                    values: Some(vec![2; length])
                }))),
            }),
            table!(ColumnT {
                name: Some("f64".into()),
                data: ColumnDataT::F64(Box::new(table!(F64sT {
                    values: Some(vec![3.5; length])
                }))),
            }),
        ];
        let data = table!(DataT {
            table: Some(Box::new(table!(DataTableT {
                target_node: 9,
                rows: length as u64,
                columns: Some(columns),
            }))),
            arrow: Some(Box::new(table!(ArrowIpcBlobT {
                target_node: 9,
                bytes: Some(vec![5; length]),
            }))),
        });
        let frame = message_frame(table!(MessageT {
            data: Some(Box::new(data))
        }));
        let bytes = finish_frame(&frame).unwrap();
        let decoded = read_frame(&bytes, &WireLimits::default()).unwrap();
        assert_eq!(decoded.unpack(), frame);

        let data = decoded
            .kind_as_frame_message()
            .unwrap()
            .messages()
            .unwrap()
            .get(0)
            .data()
            .unwrap();
        let columns = data.table().unwrap().columns().unwrap();
        let payloads = [
            columns
                .get(0)
                .data_as_i32()
                .unwrap()
                .values()
                .unwrap()
                .bytes(),
            columns
                .get(1)
                .data_as_i64()
                .unwrap()
                .values()
                .unwrap()
                .bytes(),
            columns
                .get(2)
                .data_as_f64()
                .unwrap()
                .values()
                .unwrap()
                .bytes(),
            data.arrow().unwrap().bytes().unwrap().bytes(),
        ];
        let offsets = payloads.map(|payload| payload_offset(&bytes, payload));
        assert!(offsets.iter().all(|offset| offset % 8 == 0), "{offsets:?}");
    }
}

#[test]
fn finish_frame_preserves_published_fixtures() {
    let fixtures: [&[u8]; 4] = [
        include_bytes!("../fixtures/frame_columnar_3rows.bin"),
        include_bytes!("../fixtures/frame_data_table.bin"),
        include_bytes!("../fixtures/frame_ops_all_forms.bin"),
        include_bytes!("../fixtures/action_increment.bin"),
    ];
    for fixture in fixtures {
        let frame = read_frame(fixture, &WireLimits::default())
            .unwrap()
            .unpack();
        assert_eq!(finish_frame(&frame).unwrap(), fixture);
    }
}

fn payload_offset(bytes: &[u8], payload: &[u8]) -> usize {
    payload.as_ptr().addr() - bytes.as_ptr().addr()
}

fn columnar_offsets(bytes: &[u8], batch: ColumnarBatch<'_>) -> [usize; 5] {
    [
        batch.node_ids(),
        batch.arg_a(),
        batch.arg_b(),
        batch.blob_a_off(),
        batch.blob_b_off(),
    ]
    .map(|column| payload_offset(bytes, column.unwrap().bytes()))
}

fn message_frame(message: MessageT) -> FrameT {
    table!(FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: 7,
        sequence_id: 20,
        kind: FrameKindT::FrameMessage(Box::new(table!(FrameMessageT {
            subscription_id: 0,
            messages: Some(vec![message]),
        }))),
    })
}

fn operation_message(operation: OperationT) -> MessageT {
    table!(MessageT {
        operation: Some(Box::new(operation)),
    })
}

fn columnar_operation(rows: usize) -> OperationT {
    table!(OperationT {
        columnar: Some(Box::new(columnar_batch(rows))),
    })
}

fn columnar_batch(rows: usize) -> ColumnarBatchT {
    table!(ColumnarBatchT {
        op_codes: Some(vec![2; rows]),
        node_ids: Some(vec![1; rows]),
        arg_a: Some(vec![0; rows]),
        arg_b: Some(vec![0; rows]),
        blob_a_off: Some(vec![0; rows + 1]),
        blob_b_off: Some(vec![0; rows + 1]),
        blob_bytes: Some(vec![]),
    })
}
