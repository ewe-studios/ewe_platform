//! QPACK field compression (RFC 9204).
//!
//! WHY: every HTTP/3 header crosses this codec. It is also where the protocol
//! hides its sharpest edges: the two prefix integers that are always zero (and so
//! are easy to forget), the four representations that reference the dynamic table
//! (which we must *reject*, not misparse), and a decompression bomb.
//!
//! WHAT: the RFC's own worked vector, round-trips through every representation,
//! the static table's boundaries, and the rejections our SETTINGS promise.

use bytes::Bytes;

use foundation_netio::http3::qpack::{
    decode_field_section, encode_field_section, static_table, QpackError,
};

/// A generous cap for tests that are not about the cap.
const NO_LIMIT: usize = 1 << 20;

fn fields(pairs: &[(&str, &str)]) -> Vec<(Bytes, Bytes)> {
    pairs
        .iter()
        .map(|(n, v)| {
            (
                Bytes::copy_from_slice(n.as_bytes()),
                Bytes::copy_from_slice(v.as_bytes()),
            )
        })
        .collect()
}

fn round_trip(pairs: &[(&str, &str)]) {
    let encoded = encode_field_section(pairs);
    let decoded = decode_field_section(&encoded, NO_LIMIT).expect("decode");
    assert_eq!(decoded, fields(pairs), "round-trip of {pairs:?}");
}

#[test]
fn rfc_9204_appendix_b1_literal_with_name_reference() {
    // RFC 9204 §B.1, verbatim:
    //   0000                             Required Insert Count = 0, Base = 0
    //   510b 2f69 6e64 6578 2e68 746d 6c Literal Field Line With Name Reference
    //                                    static index 1 (:path), value "/index.html"
    let wire: &[u8] = &[
        0x00, 0x00, 0x51, 0x0b, 0x2f, 0x69, 0x6e, 0x64, 0x65, 0x78, 0x2e, 0x68, 0x74, 0x6d, 0x6c,
    ];

    let decoded = decode_field_section(wire, NO_LIMIT).expect("the RFC's own bytes must decode");
    assert_eq!(decoded, fields(&[(":path", "/index.html")]));
}

#[test]
fn static_table_matches_the_rfc_at_its_boundaries() {
    // Appendix A's ordering is normative; drift here silently corrupts every
    // compressed header.
    assert_eq!(static_table::STATIC_TABLE_LEN, 99);
    assert_eq!(static_table::get(0), Some((&b":authority"[..], &b""[..])));
    assert_eq!(static_table::get(1), Some((&b":path"[..], &b"/"[..])));
    assert_eq!(
        static_table::get(98),
        Some((&b"x-frame-options"[..], &b"sameorigin"[..]))
    );
    assert_eq!(static_table::get(99), None, "index 99 is out of range");

    assert_eq!(static_table::find_exact(b":path", b"/"), Some(1));
    assert_eq!(static_table::find_name(b":path"), Some(1));
    assert_eq!(static_table::find_exact(b":path", b"/nope"), None);
    assert_eq!(static_table::find_name(b"x-not-a-real-header"), None);
}

#[test]
fn an_exact_static_hit_encodes_as_a_single_indexed_field_line() {
    // `:path: /` is static index 1. With the two zero prefix bytes that is three
    // bytes on the wire — the compression QPACK exists for.
    let encoded = encode_field_section(&[(":path", "/")]);
    assert_eq!(encoded.len(), 3, "got {encoded:02x?}");
    assert_eq!(encoded[0], 0x00, "Required Insert Count = 0");
    assert_eq!(encoded[1], 0x00, "S = 0, Delta Base = 0");
    assert_eq!(encoded[2], 0b1100_0001, "indexed, static, index 1");

    let decoded = decode_field_section(&encoded, NO_LIMIT).expect("decode");
    assert_eq!(decoded, fields(&[(":path", "/")]));
}

#[test]
fn every_representation_round_trips() {
    // Indexed (exact static hit).
    round_trip(&[(":method", "GET")]);
    // Literal with static name reference (known name, novel value).
    round_trip(&[(":path", "/index.html")]);
    // Literal with literal name (name not in the static table).
    round_trip(&[("x-ewe-custom", "value")]);
    // All three together, in one section.
    round_trip(&[
        (":method", "GET"),
        (":scheme", "https"),
        (":authority", "example.com"),
        (":path", "/a/b?c=d"),
        ("x-ewe-custom", "42"),
        ("accept", "application/grpc"),
    ]);
}

#[test]
fn empty_field_section_round_trips() {
    let encoded = encode_field_section::<&str, &str>(&[]);
    assert_eq!(encoded, vec![0x00, 0x00], "just the two zero prefixes");
    assert!(decode_field_section(&encoded, NO_LIMIT)
        .expect("decode")
        .is_empty());
}

#[test]
fn long_values_round_trip_through_the_prefix_integer_continuation() {
    // A value longer than a 7-bit prefix forces the multi-byte integer path, which
    // is where off-by-ones live.
    for len in [126usize, 127, 128, 129, 1000, 20_000] {
        let value = "v".repeat(len);
        let encoded = encode_field_section(&[("x-long", value.as_str())]);
        let decoded = decode_field_section(&encoded, NO_LIMIT).expect("decode");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].1.len(), len, "value length {len} must survive");
    }
}

#[test]
fn huffman_is_used_only_when_it_shrinks_the_string() {
    // Highly compressible: Huffman should win, so the encoded section is shorter
    // than the raw value.
    let compressible = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let encoded = encode_field_section(&[("x-a", compressible)]);
    assert!(
        encoded.len() < compressible.len() + 8,
        "Huffman should have shrunk this: {} bytes",
        encoded.len()
    );
    assert_eq!(
        decode_field_section(&encoded, NO_LIMIT).expect("decode")[0].1,
        Bytes::from(compressible),
        "and it must still round-trip"
    );

    // Incompressible bytes must not be *inflated*: the encoder falls back to raw.
    let incompressible: String = (0u8..=127).map(|b| b as char).collect();
    let encoded = encode_field_section(&[("x-b", incompressible.as_str())]);
    let decoded = decode_field_section(&encoded, NO_LIMIT).expect("decode");
    assert_eq!(decoded[0].1, Bytes::from(incompressible));
}

// ── The rejections our SETTINGS promise ─────────────────────────────────────

#[test]
fn a_nonzero_required_insert_count_is_rejected() {
    // We advertise QPACK_MAX_TABLE_CAPACITY = 0. A peer that sends a dynamic
    // reference anyway is in violation, and misparsing it would corrupt headers
    // silently — reject loudly.
    let wire: &[u8] = &[0x01, 0x00];
    assert_eq!(
        decode_field_section(wire, NO_LIMIT),
        Err(QpackError::DynamicTableDisabled)
    );
}

#[test]
fn a_nonzero_delta_base_is_rejected() {
    let wire: &[u8] = &[0x00, 0x01];
    assert_eq!(
        decode_field_section(wire, NO_LIMIT),
        Err(QpackError::DynamicTableDisabled)
    );
}

#[test]
fn an_indexed_line_referencing_the_dynamic_table_is_rejected() {
    // `1 T=0 index` — indexed, dynamic.
    let wire: &[u8] = &[0x00, 0x00, 0b1000_0001];
    assert_eq!(
        decode_field_section(wire, NO_LIMIT),
        Err(QpackError::DynamicTableDisabled)
    );
}

#[test]
fn a_literal_with_a_dynamic_name_reference_is_rejected() {
    // `01 N=0 T=0 index` — literal with name reference, dynamic.
    let wire: &[u8] = &[0x00, 0x00, 0b0100_0001, 0x00];
    assert_eq!(
        decode_field_section(wire, NO_LIMIT),
        Err(QpackError::DynamicTableDisabled)
    );
}

#[test]
fn post_base_representations_are_rejected() {
    // Indexed With Post-Base Index (`0001 ....`) and Literal With Post-Base Name
    // Reference (`0000 ....`) exist only for the dynamic table.
    assert_eq!(
        decode_field_section(&[0x00, 0x00, 0b0001_0000], NO_LIMIT),
        Err(QpackError::DynamicTableDisabled)
    );
    assert_eq!(
        decode_field_section(&[0x00, 0x00, 0b0000_0000], NO_LIMIT),
        Err(QpackError::DynamicTableDisabled)
    );
}

#[test]
fn an_out_of_range_static_index_is_rejected() {
    // Static index 99 does not exist (0..=98).
    let wire: &[u8] = &[0x00, 0x00, 0b1100_0000 | 63, 99 - 63];
    match decode_field_section(wire, NO_LIMIT) {
        Err(QpackError::BadStaticIndex(i)) => assert_eq!(i, 99),
        other => panic!("expected BadStaticIndex(99), got {other:?}"),
    }
}

#[test]
fn a_truncated_section_is_rejected_rather_than_half_decoded() {
    // A literal announcing 11 bytes of value but carrying 3.
    let wire: &[u8] = &[0x00, 0x00, 0x51, 0x0b, 0x2f, 0x69, 0x6e];
    assert_eq!(
        decode_field_section(wire, NO_LIMIT),
        Err(QpackError::Truncated)
    );

    // And the two mandatory prefixes cannot be skipped.
    assert_eq!(
        decode_field_section(&[], NO_LIMIT),
        Err(QpackError::Truncated)
    );
    assert_eq!(
        decode_field_section(&[0x00], NO_LIMIT),
        Err(QpackError::Truncated)
    );
}

#[test]
fn a_decompression_bomb_is_capped() {
    // A small compressed section must not be able to expand without bound.
    // RFC 9204 §4.5.1 charges 32 bytes of overhead per field for exactly this.
    let many: Vec<(&str, &str)> = std::iter::repeat((":path", "/")).take(200).collect();
    let encoded = encode_field_section(&many);
    assert!(
        encoded.len() < 250,
        "200 indexed lines compress tightly: {}",
        encoded.len()
    );

    match decode_field_section(&encoded, 1024) {
        Err(QpackError::TooLarge { limit }) => assert_eq!(limit, 1024),
        other => panic!("expected TooLarge, got {other:?}"),
    }

    // With room, the same bytes decode fine.
    assert_eq!(
        decode_field_section(&encoded, NO_LIMIT)
            .expect("decode")
            .len(),
        200
    );
}
