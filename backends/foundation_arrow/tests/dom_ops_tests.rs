//! WHY: `ArrowIpcEncoder` is the interop face of the DOM-op protocol — its
//! bytes must be REAL Arrow IPC (any ecosystem reader can consume them) while
//! decoding back to exactly the same `DomOp`s as every other wire format.
//!
//! WHAT: Feature 05 spec tests 1-12 — encoding shapes (column placement per
//! the decision-010 table), all-variant + large + unicode round-trips, and
//! the decode error paths.

use std::borrow::Cow;
use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray, UInt32Array, UInt8Array};
use arrow_schema::{DataType, Field, Schema};
use foundation_arrow::{encode_ipc, ArrowIpcEncoder, ARROW_IPC_VERSION};
use foundation_ui_traits::{
    AttrName, DecodeError, DomOp, HtmlTag, MorphAction, ProtocolEncoder, TargetSelector,
};

fn batch_of(ops: Vec<DomOp>) -> RecordBatch {
    ArrowIpcEncoder::to_record_batch(&ops)
}

fn string_cell(batch: &RecordBatch, col: usize, row: usize) -> Option<String> {
    let array = batch
        .column(col)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    if array.is_null(row) {
        None
    } else {
        Some(array.value(row).to_string())
    }
}

/// Test 1 — CreateElement row shape: op=0, attribute=tag, value=class,
/// text_val=null. Known tags ride as `id:` strings (F01 wire convention).
#[test]
fn create_element_row_shape() {
    let batch = batch_of(vec![DomOp::CreateElement {
        node_id: 5,
        tag: HtmlTag::from_static("div"),
        class: Cow::Borrowed("main"),
    }]);
    let operations = batch
        .column(2)
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    assert_eq!(operations.value(0), 0);
    assert_eq!(string_cell(&batch, 3, 0).as_deref(), Some("id:1")); // div = tag id 1
    assert_eq!(string_cell(&batch, 4, 0).as_deref(), Some("main"));
    assert_eq!(string_cell(&batch, 5, 0), None, "text_val is a real null");
}

/// Test 2 — CreateTextNode: op=1, text_val set, attribute null.
#[test]
fn create_text_node_row_shape() {
    let batch = batch_of(vec![DomOp::CreateTextNode {
        node_id: 9,
        content: Cow::Borrowed("hello"),
    }]);
    assert_eq!(string_cell(&batch, 5, 0).as_deref(), Some("hello"));
    assert_eq!(string_cell(&batch, 3, 0), None);
}

/// Tests 3 + 4 — secondary ids ride as decimal strings.
#[test]
fn secondary_ids_are_decimal_strings() {
    let batch = batch_of(vec![
        DomOp::AppendChild {
            parent_id: 1,
            child_id: 42,
        },
        DomOp::InsertBefore {
            parent_id: 1,
            child_id: 42,
            ref_id: 77,
        },
    ]);
    assert_eq!(string_cell(&batch, 3, 0).as_deref(), Some("42"));
    assert_eq!(string_cell(&batch, 3, 1).as_deref(), Some("42"));
    assert_eq!(string_cell(&batch, 5, 1).as_deref(), Some("77"));
}

fn all_nineteen() -> Vec<DomOp> {
    vec![
        DomOp::CreateElement {
            node_id: 1,
            tag: HtmlTag::from_static("span"),
            class: Cow::Borrowed(""),
        },
        DomOp::CreateTextNode {
            node_id: 2,
            content: Cow::Borrowed("t"),
        },
        DomOp::SetText {
            node_id: 3,
            text: Cow::Borrowed("x"),
        },
        DomOp::SetAttribute {
            node_id: 4,
            name: AttrName::from_static("class"),
            value: Cow::Borrowed("c"),
        },
        DomOp::RemoveAttribute {
            node_id: 5,
            name: AttrName::from_static("style"),
        },
        DomOp::SetProperty {
            node_id: 6,
            name: AttrName::from_static("value"),
            value: Cow::Borrowed("\"v\""),
        },
        DomOp::AddEventListener {
            node_id: 7,
            event_name: AttrName::from_static("click"),
        },
        DomOp::RemoveEventListener {
            node_id: 8,
            event_name: AttrName::from_static("click"),
        },
        DomOp::AppendChild {
            parent_id: 9,
            child_id: 10,
        },
        DomOp::RemoveChild {
            parent_id: 11,
            child_id: 12,
        },
        DomOp::RemoveNode { node_id: 13 },
        DomOp::InsertBefore {
            parent_id: 14,
            child_id: 15,
            ref_id: 16,
        },
        DomOp::ReplaceNode {
            old_id: 17,
            new_id: 18,
        },
        DomOp::SetStyle {
            node_id: 19,
            prop: AttrName::from_static("color"),
            value: Cow::Borrowed("red"),
        },
        DomOp::AddClass {
            node_id: 20,
            class: Cow::Borrowed("on"),
        },
        DomOp::RemoveClass {
            node_id: 21,
            class: Cow::Borrowed("off"),
        },
        DomOp::MorphNode {
            target: TargetSelector::Query(Cow::Borrowed("main > p:first-child")),
            action: MorphAction::ReplaceChildren,
            content: Cow::Borrowed("<b>m</b>"),
        },
        DomOp::RegisterNode { node_id: 22 },
        DomOp::UnregisterNode { node_id: 23 },
    ]
}

/// Test 5 — all 19 variants round-trip; operation column matches discriminants.
#[test]
fn all_variants_round_trip() {
    let ops = all_nineteen();
    let bytes = ArrowIpcEncoder.encode(ops.clone());
    let decoded = ArrowIpcEncoder.decode(&bytes).unwrap();
    assert_eq!(decoded, ops);

    let batch = batch_of(all_nineteen());
    let operations = batch
        .column(2)
        .as_any()
        .downcast_ref::<UInt8Array>()
        .unwrap();
    for i in 0..19 {
        assert_eq!(u32::from(operations.value(i)), u32::try_from(i).unwrap());
    }
}

/// Test 6 — 1000 SetText ops, byte-exact texts.
#[test]
fn thousand_set_texts_round_trip() {
    let ops: Vec<DomOp> = (0..1000u32)
        .map(|i| DomOp::SetText {
            node_id: i,
            text: Cow::Owned(format!("text {i}")),
        })
        .collect();
    let bytes = ArrowIpcEncoder.encode(ops.clone());
    assert_eq!(ArrowIpcEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 7 — unicode survives byte-exact.
#[test]
fn unicode_round_trips() {
    let ops = vec![
        DomOp::SetText {
            node_id: 1,
            text: Cow::Borrowed("漢字テスト中文"),
        },
        DomOp::SetText {
            node_id: 2,
            text: Cow::Borrowed("🦀🚀✨"),
        },
        DomOp::SetText {
            node_id: 3,
            text: Cow::Borrowed("مرحبا بالعالم"),
        },
    ];
    let bytes = ArrowIpcEncoder.encode(ops.clone());
    assert_eq!(ArrowIpcEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 8 — empty batch: a valid zero-row IPC stream.
#[test]
fn empty_batch_round_trips() {
    let bytes = ArrowIpcEncoder.encode(vec![]);
    assert!(!bytes.is_empty(), "schema + EOS still present");
    assert_eq!(ArrowIpcEncoder.decode(&bytes).unwrap(), vec![]);
}

/// The bytes are REAL Arrow IPC — the generic foundation_arrow reader (and by
/// extension any ecosystem implementation) consumes them.
#[test]
fn bytes_are_plain_arrow_ipc() {
    let bytes = ArrowIpcEncoder.encode(all_nineteen());
    let batch = foundation_arrow::decode_ipc(&bytes).expect("generic IPC reader");
    assert_eq!(batch.num_rows(), 19);
    assert_eq!(batch.schema_ref().field(2).name(), "operation");
}

/// Protocol identity: byte 1 (Arrow), version 2 (IPC wire form).
#[test]
fn protocol_identity() {
    assert_eq!(ArrowIpcEncoder.protocol_byte(), 1);
    assert_eq!(ArrowIpcEncoder.version(), ARROW_IPC_VERSION);
    assert_eq!(ARROW_IPC_VERSION, 2);
}

// ─── Decode errors (tests 9-12) ────────────────────────────────────────────────

/// Test 9 — op=99 reports UnknownOperation with the row index.
#[test]
fn unknown_operation_reports_row() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("op_id", DataType::UInt32, false),
        Field::new("node_id", DataType::UInt32, false),
        Field::new("operation", DataType::UInt8, false),
        Field::new("attribute", DataType::Utf8, true),
        Field::new("value", DataType::Utf8, true),
        Field::new("text_val", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt32Array::from(vec![0])),
            Arc::new(UInt32Array::from(vec![1])),
            Arc::new(UInt8Array::from(vec![99u8])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![None::<&str>])),
        ],
    )
    .unwrap();
    let bytes = encode_ipc(&batch).unwrap();
    assert_eq!(
        ArrowIpcEncoder.decode(&bytes),
        Err(DecodeError::UnknownOperation {
            op_id: 99,
            row_index: 0
        })
    );
}

/// Test 10 — wrong column count is a SchemaMismatch.
#[test]
fn four_column_buffer_is_schema_mismatch() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("op_id", DataType::UInt32, false),
        Field::new("node_id", DataType::UInt32, false),
        Field::new("operation", DataType::UInt8, false),
        Field::new("attribute", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt32Array::from(vec![0])),
            Arc::new(UInt32Array::from(vec![1])),
            Arc::new(UInt8Array::from(vec![2u8])),
            Arc::new(StringArray::from(vec![Some("x")])),
        ],
    )
    .unwrap();
    let bytes = encode_ipc(&batch).unwrap();
    assert!(matches!(
        ArrowIpcEncoder.decode(&bytes),
        Err(DecodeError::SchemaMismatch { .. })
    ));
}

/// Test 10b — right arity, wrong type/name also mismatch.
#[test]
fn wrong_column_type_is_schema_mismatch() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("op_id", DataType::UInt32, false),
        Field::new("node_id", DataType::Utf8, true), // wrong type
        Field::new("operation", DataType::UInt8, false),
        Field::new("attribute", DataType::Utf8, true),
        Field::new("value", DataType::Utf8, true),
        Field::new("text_val", DataType::Utf8, true),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt32Array::from(vec![0])),
            Arc::new(StringArray::from(vec![Some("not a number")])),
            Arc::new(UInt8Array::from(vec![2u8])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![None::<&str>])),
            Arc::new(StringArray::from(vec![Some("x")])),
        ],
    )
    .unwrap();
    let bytes = encode_ipc(&batch).unwrap();
    assert!(matches!(
        ArrowIpcEncoder.decode(&bytes),
        Err(DecodeError::SchemaMismatch { .. })
    ));
}

/// Test 12 — empty/garbage byte slices fail cleanly.
/// (Test 11 — invalid UTF-8 in a string column — is UNREACHABLE through this
/// decoder: the Arrow IPC layer validates UTF-8 before our code runs, surfacing
/// as a SchemaMismatch-wrapped read error instead.)
#[test]
fn empty_and_garbage_payloads_fail_cleanly() {
    assert_eq!(
        ArrowIpcEncoder.decode(&[]),
        Err(DecodeError::TruncatedBuffer {
            expected_min: 8,
            actual: 0
        })
    );
    let err = ArrowIpcEncoder.decode(b"definitely not arrow ipc").unwrap_err();
    assert!(matches!(err, DecodeError::SchemaMismatch { .. }), "{err:?}");
}

/// Cross-encoder consistency: the IPC form and the owned columnar form decode
/// to identical ops (the F01 spec's test-35 spirit, now spanning crates).
#[test]
fn ipc_and_owned_columnar_agree() {
    let ops = all_nineteen();
    let from_ipc = ArrowIpcEncoder.decode(&ArrowIpcEncoder.encode(ops.clone())).unwrap();
    let owned = foundation_ui_traits::ColumnarEncoder;
    let from_owned = owned.decode(&owned.encode(ops.clone())).unwrap();
    assert_eq!(from_ipc, ops);
    assert_eq!(from_owned, ops);
}
