//! WHY: The encoders are the wire contract between Rust producers (WASM, HTTP,
//! SSE, WS) and the JS applicator — every byte-level regression must be caught
//! here, without a JS host.
//!
//! WHAT: Feature 01 spec section 10: Arrow round-trips (tests 11-17), JSON
//! round-trips + shape (18-21), Envelope (25-29), error paths (30-33), and
//! cross-encoder consistency (34-35). Tests 22-24 (the byte-0 Custom Binary
//! round-trips) live with `BatchInstructionsV1` in
//! `foundation_wasm_ui/tests/protocol_impl_tests.rs` per decision 022.

use std::borrow::Cow;

use foundation_ui_traits::{
    encode_with_envelope, ArrowEncoder, AttrName, DecodeError, DomOp, Envelope, HtmlTag,
    JsonEncoder, MorphAction, ProtocolEncoder, TargetSelector, ATTR_CLASS, ENVELOPE_SIZE,
    PROTOCOL_ARROW, PROTOCOL_JSON, PROTOCOL_VERSION, TAG_DIV, TAG_SPAN,
};

/// Five mixed ops exercising ids, names, and every string column.
fn five_mixed_ops() -> Vec<DomOp> {
    vec![
        DomOp::CreateElement {
            node_id: 10,
            tag: HtmlTag::Id(TAG_DIV),
            class: Cow::Borrowed("main"),
        },
        DomOp::RegisterNode { node_id: 10 },
        DomOp::SetText {
            node_id: 10,
            text: Cow::Borrowed("hello"),
        },
        DomOp::SetAttribute {
            node_id: 10,
            name: AttrName::Id(ATTR_CLASS),
            value: Cow::Borrowed("active"),
        },
        DomOp::AppendChild {
            parent_id: 1,
            child_id: 10,
        },
    ]
}

/// One op per variant — all 19, in discriminant order.
fn all_nineteen_ops() -> Vec<DomOp> {
    vec![
        DomOp::CreateElement {
            node_id: 1,
            tag: HtmlTag::Id(TAG_SPAN),
            class: Cow::Borrowed(""),
        },
        DomOp::CreateTextNode {
            node_id: 2,
            content: Cow::Borrowed("text"),
        },
        DomOp::SetText {
            node_id: 3,
            text: Cow::Borrowed("new text"),
        },
        DomOp::SetAttribute {
            node_id: 4,
            name: AttrName::from_name("data-x"),
            value: Cow::Borrowed("1"),
        },
        DomOp::RemoveAttribute {
            node_id: 5,
            name: AttrName::Id(ATTR_CLASS),
        },
        DomOp::SetProperty {
            node_id: 6,
            name: AttrName::from_name("value"),
            value: Cow::Borrowed("\"typed\""),
        },
        DomOp::AddEventListener {
            node_id: 7,
            event_name: AttrName::from_name("click"),
        },
        DomOp::RemoveEventListener {
            node_id: 8,
            event_name: AttrName::from_name("click"),
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
            prop: AttrName::from_name("color"),
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
            target: TargetSelector::Query(Cow::Borrowed("main > div:first-child")),
            action: MorphAction::ReplaceChildren,
            content: Cow::Borrowed("<p>morphed</p>"),
        },
        DomOp::RegisterNode { node_id: 22 },
        DomOp::UnregisterNode { node_id: 23 },
    ]
}

// ─── Tests 11-17: ArrowEncoder round-trips ─────────────────────────────────────

/// Test 11 — 5 mixed ops match field-by-field after a round-trip.
#[test]
fn arrow_round_trips_five_mixed_ops() {
    let ops = five_mixed_ops();
    let bytes = ArrowEncoder.encode(ops.clone());
    assert_eq!(ArrowEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 12 — an empty batch is a valid zero-row buffer.
#[test]
fn arrow_empty_batch_round_trips() {
    let bytes = ArrowEncoder.encode(vec![]);
    assert_eq!(ArrowEncoder.decode(&bytes).unwrap(), vec![]);
}

/// Test 13 — tag and class survive a single `CreateElement` round-trip; known
/// tags ride the wire as `id:` strings.
#[test]
fn arrow_create_element_keeps_tag_and_class() {
    let op = DomOp::CreateElement {
        node_id: 7,
        tag: HtmlTag::Id(TAG_DIV),
        class: Cow::Borrowed("header dark"),
    };
    let bytes = ArrowEncoder.encode(vec![op.clone()]);
    // The wire carries the compact id form, not the tag name.
    let as_text = String::from_utf8_lossy(&bytes);
    assert!(as_text.contains("id:1"), "known tag should encode as id:1");
    assert!(!as_text.contains("div"), "tag NAME should not be on the wire");
    assert_eq!(ArrowEncoder.decode(&bytes).unwrap(), vec![op]);
}

/// Test 14 — all 19 variants round-trip in one batch.
#[test]
fn arrow_round_trips_all_nineteen_variants() {
    let ops = all_nineteen_ops();
    assert_eq!(ops.len(), 19);
    let bytes = ArrowEncoder.encode(ops.clone());
    assert_eq!(ArrowEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 15 — 1000 `SetText` ops with unique texts all match.
#[test]
fn arrow_round_trips_thousand_set_texts() {
    let ops: Vec<DomOp> = (0..1000u32)
        .map(|i| DomOp::SetText {
            node_id: i,
            text: Cow::Owned(format!("text number {i}")),
        })
        .collect();
    let bytes = ArrowEncoder.encode(ops.clone());
    assert_eq!(ArrowEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 16 — CJK, emoji, and RTL text are byte-exact after a round-trip.
#[test]
fn arrow_round_trips_unicode() {
    let ops = vec![
        DomOp::SetText {
            node_id: 1,
            text: Cow::Borrowed("漢字テスト中文"),
        },
        DomOp::SetText {
            node_id: 2,
            text: Cow::Borrowed("🦀🚀✨ emoji"),
        },
        DomOp::SetText {
            node_id: 3,
            text: Cow::Borrowed("مرحبا بالعالم"),
        },
    ];
    let bytes = ArrowEncoder.encode(ops.clone());
    assert_eq!(ArrowEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 17 — a hand-crafted op=99 reports `UnknownOperation` with the row index.
#[test]
fn arrow_unknown_operation_reports_op_and_row() {
    let mut bytes = ArrowEncoder.encode(vec![DomOp::RemoveNode { node_id: 1 }]);
    // Layout: [count:4][op_id:4][node_id:4][operation:1]... — patch the op byte.
    bytes[12] = 99;
    assert_eq!(
        ArrowEncoder.decode(&bytes),
        Err(DecodeError::UnknownOperation {
            op_id: 99,
            row_index: 0
        })
    );
}

// ─── Tests 18-21: JsonEncoder ──────────────────────────────────────────────────

/// Test 18 — 5 mixed ops round-trip through JSON.
#[test]
fn json_round_trips_five_mixed_ops() {
    let ops = five_mixed_ops();
    let bytes = JsonEncoder.encode(ops.clone());
    assert_eq!(JsonEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 19 — an empty batch decodes to an empty vec.
#[test]
fn json_empty_batch_round_trips() {
    let bytes = JsonEncoder.encode(vec![]);
    assert_eq!(&bytes, b"[]");
    assert_eq!(JsonEncoder.decode(&bytes).unwrap(), vec![]);
}

/// Test 20 — all 19 variants round-trip through JSON.
#[test]
fn json_round_trips_all_nineteen_variants() {
    let ops = all_nineteen_ops();
    let bytes = JsonEncoder.encode(ops.clone());
    assert_eq!(JsonEncoder.decode(&bytes).unwrap(), ops);
}

/// Test 21 — the output is an array of FLAT objects with the Arrow column
/// field names (spec section 4.2), nulls included.
#[test]
fn json_shape_is_flat_rows_with_column_names() {
    let bytes = JsonEncoder.encode(vec![
        DomOp::SetText {
            node_id: 5,
            text: Cow::Borrowed("hello"),
        },
        DomOp::CreateElement {
            node_id: 10,
            tag: HtmlTag::from_name("my-widget"),
            class: Cow::Borrowed("main"),
        },
    ]);
    let text = std::str::from_utf8(&bytes).unwrap();
    // Pin the exact flat shape — field order and nulls are part of the contract.
    assert_eq!(
        text,
        concat!(
            "[{\"op_id\":0,\"node_id\":5,\"operation\":2,",
            "\"attribute\":null,\"value\":null,\"text_val\":\"hello\"},",
            "{\"op_id\":1,\"node_id\":10,\"operation\":0,",
            "\"attribute\":\"my-widget\",\"value\":\"main\",\"text_val\":null}]"
        )
    );
}

// ─── Tests 25-29: Envelope ─────────────────────────────────────────────────────

/// Test 25 — write-then-parse preserves protocol, version, length, payload.
#[test]
fn envelope_round_trips() {
    let payload = b"some payload bytes";
    let message = Envelope::write(PROTOCOL_ARROW, PROTOCOL_VERSION, payload);
    let (envelope, parsed) = Envelope::parse(&message).unwrap();
    assert_eq!(envelope.protocol, PROTOCOL_ARROW);
    assert_eq!(envelope.version, PROTOCOL_VERSION);
    assert_eq!(envelope.length as usize, payload.len());
    assert_eq!(parsed, payload);
}

/// Test 26 — an empty payload makes a 6-byte message with length 0.
#[test]
fn envelope_empty_payload() {
    let message = Envelope::write(PROTOCOL_JSON, PROTOCOL_VERSION, &[]);
    assert_eq!(message.len(), ENVELOPE_SIZE);
    let (envelope, payload) = Envelope::parse(&message).unwrap();
    assert_eq!(envelope.length, 0);
    assert!(payload.is_empty());
}

/// Test 27 — buffers shorter than the header fail as truncated.
#[test]
fn envelope_truncated_header_fails() {
    assert_eq!(
        Envelope::parse(&[1, 0, 5]),
        Err(DecodeError::TruncatedBuffer {
            expected_min: ENVELOPE_SIZE,
            actual: 3
        })
    );
}

/// Test 28 — a declared length past the buffer end fails as truncated.
#[test]
fn envelope_overrunning_length_fails() {
    let mut message = Envelope::write(PROTOCOL_ARROW, PROTOCOL_VERSION, b"abc");
    message.truncate(ENVELOPE_SIZE + 2); // declared 3, only 2 present
    assert_eq!(
        Envelope::parse(&message),
        Err(DecodeError::TruncatedBuffer {
            expected_min: ENVELOPE_SIZE + 3,
            actual: ENVELOPE_SIZE + 2
        })
    );
}

/// Test 29 — `encode_with_envelope` produces a self-describing message whose
/// payload decodes back to the input.
#[test]
fn encode_with_envelope_is_self_describing() {
    let ops = five_mixed_ops();
    let message = encode_with_envelope(&ArrowEncoder, ops.clone());
    assert_eq!(message[0], PROTOCOL_ARROW);
    assert_eq!(message[1], PROTOCOL_VERSION);
    let (envelope, payload) = Envelope::parse(&message).unwrap();
    assert_eq!(envelope.protocol, PROTOCOL_ARROW);
    assert_eq!(ArrowEncoder.decode(payload).unwrap(), ops);
}

// ─── Tests 30-33: error paths ──────────────────────────────────────────────────

/// Test 30 — invalid UTF-8 in a string column reports the column name.
#[test]
fn arrow_invalid_utf8_reports_column() {
    let mut bytes = ArrowEncoder.encode(vec![DomOp::SetText {
        node_id: 1,
        text: Cow::Borrowed("ok"),
    }]);
    // The text data bytes are the trailing "ok" — corrupt the first byte.
    let data_start = bytes.len() - 2;
    bytes[data_start] = 0xFF;
    assert_eq!(
        ArrowEncoder.decode(&bytes),
        Err(DecodeError::InvalidUtf8 {
            column_name: "text_val",
            row_index: 0
        })
    );
}

/// Test 31 — a malformed columnar layout reports schema/truncation, not panic.
#[test]
fn arrow_malformed_layout_fails_cleanly() {
    // Claim 4 rows but provide nothing else.
    let bytes = 4u32.to_le_bytes();
    let err = ArrowEncoder.decode(&bytes).unwrap_err();
    assert!(
        matches!(err, DecodeError::TruncatedBuffer { .. }),
        "got: {err:?}"
    );
}

/// Test 32 — non-JSON input reports `JsonParseError`.
#[test]
fn json_garbage_fails_with_parse_error() {
    let err = JsonEncoder.decode(b"not json").unwrap_err();
    assert!(matches!(err, DecodeError::JsonParseError { .. }), "{err:?}");
}

/// Test 33 — decoding an empty Arrow payload reports truncation.
#[test]
fn arrow_empty_payload_is_truncated() {
    assert_eq!(
        ArrowEncoder.decode(&[]),
        Err(DecodeError::TruncatedBuffer {
            expected_min: 4,
            actual: 0
        })
    );
}

// ─── Tests 34-35: cross-encoder consistency ────────────────────────────────────

/// Test 34 — Arrow and JSON decode the same 10 ops identically.
#[test]
fn arrow_and_json_agree_on_ten_ops() {
    let mut ops = five_mixed_ops();
    ops.extend(vec![
        DomOp::SetStyle {
            node_id: 2,
            prop: AttrName::from_name("color"),
            value: Cow::Borrowed("blue"),
        },
        DomOp::AddClass {
            node_id: 2,
            class: Cow::Borrowed("x"),
        },
        DomOp::MorphNode {
            target: TargetSelector::Id(Cow::Borrowed("main")),
            action: MorphAction::ReplaceElement,
            content: Cow::Borrowed("<div/>"),
        },
        DomOp::RemoveChild {
            parent_id: 1,
            child_id: 2,
        },
        DomOp::UnregisterNode { node_id: 2 },
    ]);
    assert_eq!(ops.len(), 10);

    let from_arrow = ArrowEncoder.decode(&ArrowEncoder.encode(ops.clone())).unwrap();
    let from_json = JsonEncoder.decode(&JsonEncoder.encode(ops.clone())).unwrap();
    assert_eq!(from_arrow, ops);
    assert_eq!(from_json, ops);
    assert_eq!(from_arrow, from_json);
}

/// Test 35 — every encoder in this crate decodes the all-variant batch to the
/// same `Vec<DomOp>`. (The byte-0 encoder's leg of this test lives with
/// `BatchInstructionsV1` in `foundation_wasm_ui`.)
#[test]
fn all_encoders_agree_on_every_variant() {
    let ops = all_nineteen_ops();
    let from_arrow = ArrowEncoder.decode(&ArrowEncoder.encode(ops.clone())).unwrap();
    let from_json = JsonEncoder.decode(&JsonEncoder.encode(ops.clone())).unwrap();
    assert_eq!(from_arrow, ops);
    assert_eq!(from_json, ops);
}

// ─── Morph packing details ─────────────────────────────────────────────────────

/// Every selector kind and action round-trips; node-id targets ride the
/// `node_id` column.
#[test]
fn morph_selector_and_action_round_trip() {
    let cases = vec![
        (TargetSelector::NodeId(42), MorphAction::ReplaceChildren),
        (
            TargetSelector::Id(Cow::Borrowed("app")),
            MorphAction::ReplaceElement,
        ),
        (
            TargetSelector::Class(Cow::Borrowed("card")),
            MorphAction::InsertBefore,
        ),
        (
            // CSS queries may contain `:` — the packing must survive it.
            TargetSelector::Query(Cow::Borrowed("ul > li:nth-child(2)")),
            MorphAction::AppendSibling,
        ),
    ];
    for (target, action) in cases {
        let op = DomOp::MorphNode {
            target: target.clone(),
            action,
            content: Cow::Borrowed("<span>x</span>"),
        };
        let decoded = ArrowEncoder.decode(&ArrowEncoder.encode(vec![op.clone()])).unwrap();
        assert_eq!(decoded, vec![op], "target {target:?} action {action:?}");
    }
}
