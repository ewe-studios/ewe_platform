use flatbuffers::{EndianScalar, FlatBufferBuilder, Follow, Vector, Verifiable};
use foundation_ui_wire::create_aligned_vector;

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
    use foundation_ui_wire::{
        finish_frame, read_frame, ColumnarBatchT, FrameKindT, FrameMessageT, FrameT, MessageT,
        OperationT, WireLimits, SCHEMA_VERSION,
    };

    let batch = table!(ColumnarBatchT {
        op_codes: Some(vec![2; 3]),
        node_ids: Some(vec![1, 2, 3]),
        arg_a: Some(vec![0; 3]),
        arg_b: Some(vec![0; 3]),
        blob_a_off: Some(vec![0, 1, 3, 6]),
        blob_b_off: Some(vec![0; 4]),
        blob_bytes: Some(b"abbccc".to_vec()),
    });
    let operation = table!(OperationT {
        columnar: Some(Box::new(batch)),
    });
    let message = table!(MessageT {
        operation: Some(Box::new(operation)),
    });
    let body = table!(FrameMessageT {
        messages: Some(vec![message]),
    });
    let frame = table!(FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: 7,
        sequence_id: 20,
        kind: FrameKindT::FrameMessage(Box::new(body)),
    });
    let bytes = finish_frame(&frame).unwrap();
    let batch = read_frame(&bytes, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap()
        .get(0)
        .operation()
        .unwrap()
        .columnar()
        .unwrap();
    let columns = [
        batch.node_ids(),
        batch.arg_a(),
        batch.arg_b(),
        batch.blob_a_off(),
        batch.blob_b_off(),
    ];
    let offsets =
        columns.map(|column| column.unwrap().bytes().as_ptr().addr() - bytes.as_ptr().addr());

    // The pinned compiler needs the alignment helper for columnar writes.
    assert!(offsets.iter().any(|offset| offset % 8 != 0), "{offsets:?}");
}
