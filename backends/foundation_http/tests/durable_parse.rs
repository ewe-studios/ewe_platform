//! Tests for the Durable Streams HTTP parsers.

use core::time::Duration;
use foundation_core::durable::{BadRequestKind, ReadFrom, StoreError, StreamOffset};
use foundation_http::shared::durable::{
    decode_offset, encode_offset, parse_offset, parse_producer, parse_stream_closed, parse_ttl,
};

#[test]
fn given_stream_closed_values_when_parsed_then_only_true_counts() {
    assert!(parse_stream_closed(Some("true")));
    assert!(parse_stream_closed(Some("TRUE")));
    assert!(!parse_stream_closed(Some("false")));
    assert!(!parse_stream_closed(Some("1")));
    assert!(!parse_stream_closed(None));
}

#[test]
fn given_ttl_values_when_parsed_then_only_decimal_integers_are_valid() {
    assert_eq!(parse_ttl("3600").expect("valid"), Duration::from_secs(3600));
    assert_eq!(parse_ttl("0").expect("valid"), Duration::ZERO);
    for invalid in ["", "03600", "+3600", "3600.0", "3.6e3", "-1", "x"] {
        assert!(parse_ttl(invalid).is_err(), "{invalid} must fail");
    }
}

#[test]
fn given_producer_headers_when_parsed_then_all_or_none_is_enforced() {
    assert!(parse_producer(None, None, None).expect("none").is_none());
    assert!(parse_producer(Some("p"), Some("0"), Some("0"))
        .expect("valid")
        .is_some());

    for invalid in [
        (Some("p"), None, None),
        (Some("p"), Some("0"), None),
        (Some(""), Some("0"), Some("0")),
        (Some("p"), Some("-1"), Some("0")),
        (Some("p"), Some("0"), Some("1e3")),
        (Some("p"), Some("0"), Some("9007199254740992")),
    ] {
        let error = parse_producer(invalid.0, invalid.1, invalid.2).expect_err("must fail");
        assert!(matches!(
            error,
            StoreError::BadRequest(BadRequestKind::Producer)
        ));
    }
}

#[test]
fn given_offset_values_when_parsed_then_sentinels_are_recognised() {
    assert_eq!(parse_offset("-1").expect("start"), ReadFrom::Start);
    assert_eq!(parse_offset("now").expect("tail"), ReadFrom::Tail);
    assert!(matches!(
        parse_offset("00ff").expect("offset"),
        ReadFrom::Offset(_)
    ));
    assert!(parse_offset("").is_err());
    assert!(parse_offset("a,b").is_err());
    assert!(parse_offset("abc").is_err());
    assert!(parse_offset("00FF").is_err());
}

#[test]
fn given_an_offset_when_encoded_then_the_text_decodes_back_to_the_same_bytes() {
    let offset = StreamOffset::from_bytes(vec![0x00, 0x01, 0xfe, 0xff]);
    let text = encode_offset(&offset);
    assert_eq!(text, "0001feff");
    assert_eq!(
        decode_offset(&text).expect("decode").as_bytes(),
        offset.as_bytes()
    );
}

#[test]
fn given_offsets_when_encoded_then_the_text_sorts_with_the_bytes() {
    let low = encode_offset(&StreamOffset::from_bytes(vec![0x00, 0x00, 0x00, 0x01]));
    let high = encode_offset(&StreamOffset::from_bytes(vec![0x00, 0x00, 0x00, 0x02]));
    assert!(low < high);

    let short = encode_offset(&StreamOffset::from_bytes(vec![0x01]));
    let long = encode_offset(&StreamOffset::from_bytes(vec![0x01, 0x00]));
    assert!(short < long);
}
