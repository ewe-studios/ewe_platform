use foundation_ui_wire::{
    finish_frame, finish_signed_state, read_frame,
    writer::{
        columnar_batch, columnar_rows, write_messages, ClientCaps, ColumnarError, FrameHeader,
        WriteError, WriterConfig,
    },
    Alignment, AlignmentValueT, AppendChildT, AttachStateT, Axis, AxisValueT, BindActionT,
    BoolValueT, ColorTokenT, ColumnarBatch, ColumnarBatchT, CreateNodeT, EventId, FontTokenT,
    FrameKindT, FrameMessageT, FrameT, InsertBeforeT, LengthT, LengthUnit, LengthValueT,
    LiveMountT, MessageT, NodeKind, OperationT, PropId, PropValueT, RemoveT, ReplaceSubtreeT,
    SetPropT, SetTextT, SignedStateT, TextValueT, UiBehaviourT, UiOperation, UiOperationT,
    UnbindActionT, WireLimits, SCHEMA_VERSION,
};

fn text(node_id: u32, value: Option<&str>) -> UiOperationT {
    UiOperationT::SetText(Box::new(table!(SetTextT {
        node_id: node_id,
        text: value.map(str::to_owned),
    })))
}

fn prop(id: PropId, value: PropValueT) -> UiOperationT {
    UiOperationT::SetProp(Box::new(table!(SetPropT {
        node_id: 4,
        prop: id,
        value: value
    })))
}

fn eligible_rows() -> Vec<UiOperationT> {
    vec![
        UiOperationT::CreateNode(Box::new(table!(CreateNodeT {
            node_id: 1,
            kind: NodeKind::TEXT
        }))),
        text(1, Some("αβ")),
        prop(
            PropId::WIDTH,
            PropValueT::Length(Box::new(table!(LengthValueT {
                v: Some(table!(LengthT {
                    value: 12.5,
                    unit: LengthUnit::Percent
                })),
            }))),
        ),
        prop(
            PropId::COLOR,
            PropValueT::ColorToken(Box::new(table!(ColorTokenT { id: 10 }))),
        ),
        prop(
            PropId::FONT,
            PropValueT::FontToken(Box::new(table!(FontTokenT { id: 20 }))),
        ),
        prop(
            PropId::ALIGN,
            PropValueT::Alignment(Box::new(table!(AlignmentValueT {
                v: Alignment::Center
            }))),
        ),
        prop(
            PropId::AXIS,
            PropValueT::Axis(Box::new(table!(AxisValueT {
                v: Axis::Horizontal
            }))),
        ),
        prop(
            PropId::ENABLED,
            PropValueT::Bool(Box::new(table!(BoolValueT { v: true }))),
        ),
        prop(
            PropId::HIDDEN,
            PropValueT::Bool(Box::new(table!(BoolValueT { v: false }))),
        ),
        prop(
            PropId::A11Y_LABEL,
            PropValueT::Text(Box::new(table!(TextValueT {
                v: Some("label".into())
            }))),
        ),
        UiOperationT::AppendChild(Box::new(table!(AppendChildT {
            parent_id: 1,
            child_id: 2
        }))),
        UiOperationT::InsertBefore(Box::new(table!(InsertBeforeT {
            parent_id: 1,
            child_id: 3,
            before_id: 2
        }))),
        UiOperationT::Remove(Box::new(table!(RemoveT { node_id: 3 }))),
        UiOperationT::BindAction(Box::new(table!(BindActionT {
            node_id: 2,
            event: EventId::TAP,
            action: Some("server:increment".into()),
        }))),
        UiOperationT::UnbindAction(Box::new(table!(UnbindActionT {
            node_id: 2,
            event: EventId::TAP
        }))),
        UiOperationT::AttachState(Box::new(table!(AttachStateT {
            root_node: 1,
            state: Some(finish_signed_state(&table!(SignedStateT {
                tag: Some(vec![0x22; 32])
            }))),
        }))),
    ]
}

fn batch_message(batch: ColumnarBatchT) -> MessageT {
    table!(MessageT {
        operation: Some(Box::new(table!(OperationT {
            columnar: Some(Box::new(batch))
        }))),
    })
}

fn write_batch(batch: ColumnarBatchT) -> Result<Vec<u8>, WriteError> {
    write_messages(
        vec![batch_message(batch)],
        &ClientCaps::web_current(),
        FrameHeader {
            stream_id: 7,
            sequence_id: 20,
            ack: 5,
        },
        &WriterConfig::default(),
    )
    .map(|(bytes, _)| bytes)
}

fn unchecked_bytes(batch: ColumnarBatchT) -> Vec<u8> {
    let frame = table!(FrameT {
        schema_version: SCHEMA_VERSION,
        stream_id: 7,
        sequence_id: 20,
        kind: FrameKindT::FrameMessage(Box::new(table!(FrameMessageT {
            subscription_id: 0,
            ack: 5,
            messages: Some(vec![batch_message(batch)]),
        }))),
    });
    finish_frame(&frame).unwrap()
}

fn read_batch(bytes: &[u8]) -> ColumnarBatch<'_> {
    let messages = read_frame(bytes, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap();
    assert_eq!(messages.len(), 1);
    messages.get(0).operation().unwrap().columnar().unwrap()
}

#[test]
fn columnar_rows_round_trips() {
    let expected = eligible_rows();
    let bytes = write_batch(columnar_batch(&expected).unwrap()).unwrap();
    assert_eq!(columnar_rows(read_batch(&bytes)).unwrap(), expected);
}

#[test]
fn columnar_holds_shared_rows() {
    let rows = eligible_rows();
    let bytes = write_batch(columnar_batch(&rows).unwrap()).unwrap();
    let batch = read_batch(&bytes);
    assert!(batch
        .op_codes()
        .unwrap()
        .iter()
        .eq(rows.iter().map(|row| row.ui_operation_type().0)));
    let first = batch.blob_a_off().unwrap();
    let second = batch.blob_b_off().unwrap();
    assert_eq!(first.get(rows.len()), second.get(0));
    assert!(second.iter().all(|value| value == second.get(0)));
}

#[test]
fn large_list_render_stays_columnar() {
    for count in [1, 3, 65, 1000] {
        let rows: Vec<_> = (1..=count).map(|node| text(node, Some("x"))).collect();
        let bytes = write_batch(columnar_batch(&rows).unwrap()).unwrap();
        let batch = read_batch(&bytes);
        assert_eq!(columnar_rows(batch).unwrap(), rows);
        for col in [
            batch.node_ids(),
            batch.arg_a(),
            batch.arg_b(),
            batch.blob_a_off(),
            batch.blob_b_off(),
        ] {
            let offset = col.unwrap().bytes().as_ptr().addr() - bytes.as_ptr().addr();
            assert_eq!(offset % 8, 0, "{count} rows, offset {offset}");
        }
    }
}

#[test]
fn columnar_fixture_matches() {
    let rows = [
        text(1, Some("a")),
        text(2, Some("bb")),
        text(3, Some("ccc")),
    ];
    let bytes = write_batch(columnar_batch(&rows).unwrap()).unwrap();
    assert_eq!(
        bytes,
        include_bytes!("../fixtures/frame_columnar_3rows.bin")
    );
}

#[test]
fn legacy_zero_offsets_still_decode_and_writes_canonicalize_them() {
    let legacy = super::fixtures_tests::columnar_bytes(0);
    let batch = read_batch(&legacy);
    assert_eq!(
        columnar_rows(batch).unwrap(),
        [
            text(1, Some("a")),
            text(2, Some("bb")),
            text(3, Some("ccc"))
        ]
    );
    assert_eq!(
        write_batch(batch.unpack()).unwrap(),
        include_bytes!("../fixtures/frame_columnar_3rows.bin")
    );
}

#[test]
fn columnar_empty_string_decodes_as_absent() {
    let rows = [
        text(1, Some("")),
        UiOperationT::BindAction(Box::new(table!(BindActionT {
            node_id: 1,
            event: EventId::TAP,
            action: Some(String::new())
        }))),
        prop(
            PropId::VALUE,
            PropValueT::Text(Box::new(table!(TextValueT {
                v: Some(String::new())
            }))),
        ),
    ];
    let bytes = write_batch(columnar_batch(&rows).unwrap()).unwrap();
    let decoded = columnar_rows(read_batch(&bytes)).unwrap();
    assert_eq!(
        decoded,
        [
            text(1, None),
            UiOperationT::BindAction(Box::new(table!(BindActionT {
                node_id: 1,
                event: EventId::TAP
            }))),
            prop(PropId::VALUE, PropValueT::Text(Box::default())),
        ]
    );
}

#[test]
fn columnar_rejects_normal_only_forms() {
    for row in [
        UiOperationT::ReplaceSubtree(Box::<ReplaceSubtreeT>::default()),
        UiOperationT::LiveMount(Box::<LiveMountT>::default()),
        UiOperationT::UiBehaviour(Box::<UiBehaviourT>::default()),
    ] {
        let code = row.ui_operation_type().0;
        assert_eq!(
            columnar_batch(&[row]),
            Err(ColumnarError::UnsupportedOp { row: 0, code })
        );
    }
}

#[test]
fn columnar_rejects_bad_lengths_offsets_and_utf8() {
    let good = columnar_batch(&[text(1, Some("a")), text(2, Some("b"))]).unwrap();

    let mut short = good.clone();
    short.node_ids = Some(vec![1]);
    let mismatch = ColumnarError::LengthMismatch {
        column: "node_ids",
        expected: 2,
        actual: 1,
    };
    assert_eq!(
        columnar_rows(read_batch(&unchecked_bytes(short.clone()))),
        Err(mismatch.clone())
    );
    assert!(matches!(
        write_batch(short),
        Err(WriteError::Columnar { message_index: 0, error }) if error == mismatch
    ));

    let mut overflow = good.clone();
    overflow.blob_a_off = Some(vec![0, 3, 2]);
    assert_eq!(
        columnar_rows(read_batch(&unchecked_bytes(overflow))),
        Err(ColumnarError::BadOffsets { row: 0 })
    );

    let mut second_range = good.clone();
    second_range.blob_b_off = Some(vec![2, 3, 3]);
    assert_eq!(
        columnar_rows(read_batch(&unchecked_bytes(second_range))),
        Err(ColumnarError::BadOffsets { row: 0 })
    );

    let mut bad_text = good;
    bad_text.blob_bytes = Some(vec![b'a', 0xff]);
    assert_eq!(
        columnar_rows(read_batch(&unchecked_bytes(bad_text))),
        Err(ColumnarError::InvalidUtf8 { row: 1 })
    );
}

#[test]
fn columnar_rejects_bad_codes_and_scalar_values() {
    let mut batch = columnar_batch(&[text(1, None)]).unwrap();
    batch.op_codes = Some(vec![250]);
    assert_eq!(
        columnar_rows(read_batch(&unchecked_bytes(batch.clone()))),
        Err(ColumnarError::UnknownOp { row: 0, code: 250 })
    );
    batch.op_codes = Some(vec![UiOperation::LiveMount.0]);
    assert_eq!(
        columnar_rows(read_batch(&unchecked_bytes(batch))),
        Err(ColumnarError::UnsupportedOp {
            row: 0,
            code: UiOperation::LiveMount.0
        }),
    );

    let row = prop(PropId::ENABLED, PropValueT::Bool(Box::default()));
    let mut batch = columnar_batch(&[row]).unwrap();
    batch.arg_b = Some(vec![2]);
    assert_eq!(
        columnar_rows(read_batch(&unchecked_bytes(batch))),
        Err(ColumnarError::InvalidValue { row: 0 })
    );
    let wrong_kind = prop(PropId::ENABLED, PropValueT::Text(Box::default()));
    assert_eq!(
        columnar_batch(&[wrong_kind]),
        Err(ColumnarError::InvalidValue { row: 0 })
    );
}

#[test]
fn columnar_gating_preserves_fallback_and_removal_reports() {
    use foundation_ui_wire::writer::{FilterReason, FilteredPart};

    let source = [
        UiOperationT::CreateNode(Box::new(table!(CreateNodeT {
            node_id: 1,
            kind: NodeKind(NodeKind::ENUM_MAX + 1),
        }))),
        UiOperationT::UnbindAction(Box::new(table!(UnbindActionT {
            node_id: 1,
            event: EventId(99),
        }))),
        text(1, Some("kept")),
    ];
    let batch = columnar_batch(&source).unwrap();
    let (bytes, report) = write_messages(
        vec![batch_message(batch)],
        &ClientCaps::web_current(),
        FrameHeader {
            stream_id: 7,
            sequence_id: 20,
            ack: 5,
        },
        &WriterConfig::default(),
    )
    .unwrap();
    let filtered = &report.filtered;
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].message_index, 0);
    assert_eq!(filtered[0].part, FilteredPart::NestedOperation);
    assert_eq!(filtered[0].reason, FilterReason::UnknownVocabulary);
    assert_eq!(report.replaced_kinds, [0]);
    assert_eq!(
        columnar_rows(read_batch(&bytes)).unwrap(),
        [
            UiOperationT::CreateNode(Box::new(table!(CreateNodeT {
                node_id: 1,
                kind: NodeKind::UNKNOWN,
            }))),
            text(1, Some("kept")),
        ]
    );
}
