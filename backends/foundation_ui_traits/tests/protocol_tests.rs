//! WHY: Feature 00 (Layer 1) requires every encoder to round-trip a mixed `DomOp`
//! batch and the `Envelope` header to write/parse exactly. These integration tests
//! exercise the public encoding API the same way HTTP/SSE/WASM consumers will.
//!
//! WHAT: Round-trip coverage for Arrow, JSON, and Custom Binary encoders, envelope
//! framing, protocol-byte identity, and decoder error handling.
//!
//! HOW: Build a representative `Vec<DomOp>`, encode, decode, assert equality.

use foundation_ui_traits::{
    ArrowEncoder, DecodeError, DomOp, Envelope, JsonEncoder, ProtocolEncoder,
    PROTOCOL_ARROW, PROTOCOL_CUSTOM_BINARY, PROTOCOL_JSON,
};

/// A representative batch covering every `DomOp` variant, including text that needs
/// JSON escaping and child/ref/new id references.
fn sample_ops() -> Vec<DomOp> {
    vec![
        DomOp::CreateEl {
            node_id: 1000,
            tag: "div".into(),
            class: "card primary".into(),
        },
        DomOp::SetText {
            node_id: 1001,
            text: "Hello \"world\"\n\tline2 — café".into(),
        },
        DomOp::SetAttr {
            node_id: 1002,
            name: "data-id".into(),
            value: "x=1&y=2".into(),
        },
        DomOp::SetClass {
            node_id: 1002,
            class: "active".into(),
        },
        DomOp::SetStyle {
            node_id: 1002,
            prop: "color".into(),
            value: "red".into(),
        },
        DomOp::AppendChild {
            parent_id: 1000,
            child_id: 1001,
        },
        DomOp::InsertBefore {
            parent_id: 1000,
            child_id: 1002,
            ref_id: 1001,
        },
        DomOp::Replace {
            old_id: 1001,
            new_id: 2000,
        },
        DomOp::Remove { node_id: 2000 },
        DomOp::Morph {
            node_id: 1000,
            html: "<span>patched</span>".into(),
        },
    ]
}

#[test]
fn arrow_round_trip_is_lossless() {
    let ops = sample_ops();
    let encoder = ArrowEncoder;
    let bytes = encoder.encode(ops.clone());
    let decoded = encoder.decode(&bytes).expect("arrow decode");
    assert_eq!(decoded, ops);
}

#[test]
fn json_round_trip_is_lossless_and_valid_utf8() {
    let ops = sample_ops();
    let encoder = JsonEncoder;
    let bytes = encoder.encode(ops.clone());
    // Must be valid UTF-8 JSON text.
    let text = core::str::from_utf8(&bytes).expect("json is utf-8");
    assert!(text.starts_with('['));
    assert!(text.ends_with(']'));
    let decoded = encoder.decode(&bytes).expect("json decode");
    assert_eq!(decoded, ops);
}

#[test]
fn empty_batch_round_trips_for_all_encoders() {
    let ops: Vec<DomOp> = Vec::new();
    assert_eq!(ArrowEncoder.decode(&ArrowEncoder.encode(ops.clone())).unwrap(), ops);
    assert_eq!(JsonEncoder.decode(&JsonEncoder.encode(ops.clone())).unwrap(), ops);
}

#[test]
fn protocol_bytes_match_spec() {
    assert_eq!(ArrowEncoder.protocol_byte(), PROTOCOL_ARROW);
    assert_eq!(ArrowEncoder.protocol_byte(), 1);
    assert_eq!(JsonEncoder.protocol_byte(), PROTOCOL_JSON);
    assert_eq!(JsonEncoder.protocol_byte(), 2);
    // Protocol byte 0 (Custom Binary) is the foundation_wasm Instructions format
    // (decision 022) — an instruction stream, not a Layer-1 value encoder; its
    // transport impl (BatchInstructionsV1) lives in foundation_wasm_ui.
    assert_eq!(PROTOCOL_CUSTOM_BINARY, 0);
}

#[test]
fn envelope_write_parse_round_trips() {
    let payload = b"some-arbitrary-payload-bytes";
    let framed = Envelope::write(1, 0, payload);
    assert_eq!(framed.len(), Envelope::HEADER_LEN + payload.len());
    let (env, body) = Envelope::parse(&framed).expect("parse");
    assert_eq!(env.protocol, 1);
    assert_eq!(env.version, 0);
    assert_eq!(env.length as usize, payload.len());
    assert_eq!(body, payload);
}

#[test]
fn envelope_parse_rejects_short_header() {
    assert_eq!(Envelope::parse(&[0u8; 3]), Err(DecodeError::UnexpectedEnd));
}

#[test]
fn envelope_parse_rejects_truncated_payload() {
    // header claims 100 bytes but only 2 follow
    let mut framed = Envelope::write(2, 0, b"hi");
    // bump declared length to overrun
    framed[2] = 100;
    assert_eq!(Envelope::parse(&framed), Err(DecodeError::UnexpectedEnd));
}

#[test]
fn encoders_can_be_enveloped_and_recovered() {
    let ops = sample_ops();
    for (proto, bytes) in [
        (PROTOCOL_ARROW, ArrowEncoder.encode(ops.clone())),
        (PROTOCOL_JSON, JsonEncoder.encode(ops.clone())),
    ] {
        let framed = Envelope::write(proto, 0, &bytes);
        let (env, payload) = Envelope::parse(&framed).unwrap();
        assert_eq!(env.protocol, proto);
        let decoded = match proto {
            PROTOCOL_ARROW => ArrowEncoder.decode(payload).unwrap(),
            _ => JsonEncoder.decode(payload).unwrap(),
        };
        assert_eq!(decoded, ops);
    }
}

#[test]
fn json_decode_rejects_malformed_input() {
    assert!(matches!(
        JsonEncoder.decode(b"[{\"op\":}]"),
        Err(DecodeError::MalformedJson(_))
    ));
}
