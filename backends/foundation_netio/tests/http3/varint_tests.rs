//! QUIC varint codec (RFC 9000 §16).
//!
//! WHY: every HTTP/3 frame type, length, setting and error code is a varint. A
//! subtle bug here corrupts everything above it, silently.
//!
//! WHAT: the RFC's own worked examples, the length-class boundaries, the
//! non-canonical encodings a receiver must accept, and the partial-input contract
//! the frame decoder relies on.

use foundation_netio::http3::varint::{VarInt, MAX};

/// Encode and decode must round-trip, and consume exactly the bytes produced.
fn round_trip(value: u64, expected_len: usize) {
    let v = VarInt::new(value).expect("in range");
    assert_eq!(v.encoded_len(), expected_len, "encoded_len for {value}");

    let mut out = Vec::new();
    v.encode(&mut out);
    assert_eq!(out.len(), expected_len, "wire length for {value}");

    let (decoded, used) = VarInt::decode(&out).expect("decode ok").expect("complete");
    assert_eq!(decoded.value(), value, "round-trip value for {value}");
    assert_eq!(used, expected_len, "bytes consumed for {value}");
}

#[test]
fn rfc_9000_worked_examples() {
    // RFC 9000 §A.1 gives these exact encodings.
    let cases: &[(&[u8], u64)] = &[
        (
            &[0xc2, 0x19, 0x7c, 0x5e, 0xff, 0x14, 0xe8, 0x8c],
            151_288_809_941_952_652,
        ),
        (&[0x9d, 0x7f, 0x3e, 0x7d], 494_878_333),
        (&[0x7b, 0xbd], 15_293),
        (&[0x25], 37),
        // The RFC's non-canonical example: 37 encoded in two bytes.
        (&[0x40, 0x25], 37),
    ];

    for (bytes, expected) in cases {
        let (v, used) = VarInt::decode(bytes).expect("decode ok").expect("complete");
        assert_eq!(v.value(), *expected, "decoding {bytes:02x?}");
        assert_eq!(used, bytes.len(), "consumed all of {bytes:02x?}");
    }
}

#[test]
fn length_class_boundaries_round_trip() {
    round_trip(0, 1);
    round_trip(63, 1);
    round_trip(64, 2);
    round_trip(16_383, 2);
    round_trip(16_384, 4);
    round_trip(1_073_741_823, 4);
    round_trip(1_073_741_824, 8);
    round_trip(MAX, 8);
}

#[test]
fn encoding_is_minimal() {
    // A minimal encoder must not waste bytes: 63 fits in one, 64 needs two.
    let mut out = Vec::new();
    VarInt::new(63).unwrap().encode(&mut out);
    assert_eq!(out, vec![63], "63 must use the one-byte form");

    out.clear();
    VarInt::new(64).unwrap().encode(&mut out);
    assert_eq!(out, vec![0x40, 0x40], "64 must use the two-byte form");
}

#[test]
fn non_canonical_encodings_are_accepted() {
    // RFC 9000 §16: "the encoding is not required to use the minimum number of
    // bytes". A decoder that rejects these breaks against conforming senders.
    let zero_forms: &[&[u8]] = &[
        &[0x00],
        &[0x40, 0x00],
        &[0x80, 0x00, 0x00, 0x00],
        &[0xc0, 0, 0, 0, 0, 0, 0, 0],
    ];
    for form in zero_forms {
        let (v, used) = VarInt::decode(form).expect("decode ok").expect("complete");
        assert_eq!(
            v.value(),
            0,
            "all encodings of zero decode to zero: {form:02x?}"
        );
        assert_eq!(used, form.len());
    }
}

#[test]
fn short_input_is_pending_not_an_error() {
    // The frame decoder depends on this: a truncated varint means "come back with
    // more bytes", never "the wire is malformed".
    assert!(
        VarInt::decode(&[]).expect("no error").is_none(),
        "empty input"
    );

    // A two-byte tag with only one byte present.
    assert!(
        VarInt::decode(&[0x40]).expect("no error").is_none(),
        "truncated 2-byte"
    );
    // A four-byte tag with three bytes present.
    assert!(
        VarInt::decode(&[0x80, 0, 0]).expect("no error").is_none(),
        "truncated 4-byte"
    );
    // An eight-byte tag with seven bytes present.
    assert!(
        VarInt::decode(&[0xc0, 0, 0, 0, 0, 0, 0])
            .expect("no error")
            .is_none(),
        "truncated 8-byte"
    );
}

#[test]
fn decode_stops_at_its_own_length_and_ignores_trailing_bytes() {
    // The frame decoder feeds the whole buffer and relies on `used` to advance.
    let mut buf = vec![0x25]; // 37, one byte
    buf.extend_from_slice(b"trailing");

    let (v, used) = VarInt::decode(&buf).expect("decode ok").expect("complete");
    assert_eq!(v.value(), 37);
    assert_eq!(used, 1, "must not consume the trailing bytes");
}

#[test]
fn values_at_or_above_2_pow_62_are_rejected() {
    assert!(VarInt::new(MAX).is_ok(), "2^62 - 1 is the maximum");
    assert!(VarInt::new(MAX + 1).is_err(), "2^62 has no varint encoding");
    assert!(VarInt::new(u64::MAX).is_err());

    let err = VarInt::new(MAX + 1).unwrap_err();
    assert!(
        err.to_string().contains("exceeds"),
        "the error must say what went wrong: {err}"
    );
}
