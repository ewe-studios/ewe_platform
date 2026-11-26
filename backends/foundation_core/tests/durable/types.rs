//! Tests for the core protocol types.

use core::time::Duration;
use foundation_core::durable::{
    BadRequestKind, Chunk, ContentType, Expiry, ProducerId, ReadFrom, ReadLimit, ReadPage,
    ReadRequest, ResumeCursor, StoreError, StreamName, StreamOffset, StreamSeq,
};

#[test]
fn offsets_sort_by_bytes() {
    let a = StreamOffset::from_bytes([1_u8, 2]);
    let b = StreamOffset::from_bytes([1_u8, 3]);
    let c = StreamOffset::from_bytes([2_u8, 0]);

    assert!(a < b);
    assert!(b < c);
    assert_eq!(a, StreamOffset::from_bytes([1_u8, 2]));
}

#[test]
fn sequences_sort_by_bytes() {
    let two = StreamSeq::from_bytes(b"2".to_vec());
    let ten = StreamSeq::from_bytes(b"10".to_vec());

    assert!(ten < two, "byte order puts \"10\" before \"2\"");
    assert_eq!(two.as_bytes(), b"2");
}

#[test]
fn content_type_normalizes_and_ignores_parameters() {
    let json = ContentType::parse("Application/JSON; charset=utf-8").expect("valid type");

    assert_eq!(json.as_str(), "application/json");
    assert!(json.is_json());
}

#[test]
fn content_type_rejects_malformed_values() {
    for raw in ["", "json", "/plain", "text/", "text/\u{0}plain"] {
        let error = ContentType::parse(raw).expect_err("must reject");
        assert!(matches!(
            error,
            StoreError::BadRequest(BadRequestKind::Header)
        ));
    }
}

#[test]
fn content_type_defaults_to_octet_stream() {
    assert_eq!(ContentType::default().as_str(), "application/octet-stream");
}

#[test]
fn stream_name_accepts_paths() {
    for raw in ["events/abc", "a", "v1/streams/orders/42"] {
        let name = StreamName::parse(raw).expect("valid name");
        assert_eq!(name.as_str(), raw);
    }
}

#[test]
fn stream_name_rejects_traversal_and_bad_paths() {
    for raw in [
        "", "/a", "a/", "a//b", "a/../b", "a/./b", "a b", "a?b", "a#b", "a\\b",
    ] {
        let error = StreamName::parse(raw).expect_err("must reject");
        assert!(matches!(
            error,
            StoreError::BadRequest(BadRequestKind::Header)
        ));
    }
}

#[test]
fn producer_id_rejects_empty_values() {
    let error = ProducerId::parse("").expect_err("must reject");
    assert!(matches!(
        error,
        StoreError::BadRequest(BadRequestKind::Producer)
    ));
    assert_eq!(
        ProducerId::parse("order-service-1")
            .expect("valid")
            .as_str(),
        "order-service-1"
    );
}

#[test]
fn expiry_accepts_one_form() {
    let ttl = Expiry::from_headers(Some(Duration::from_secs(30)), None).expect("valid");
    assert!(matches!(ttl, Some(Expiry::Ttl(duration)) if duration == Duration::from_secs(30)));

    let absolute = Expiry::from_headers(None, Some(1_800_000_000_000)).expect("valid");
    assert!(matches!(
        absolute,
        Some(Expiry::Absolute(value)) if value == 1_800_000_000_000
    ));

    assert!(Expiry::from_headers(None, None).expect("valid").is_none());
}

#[test]
fn expiry_rejects_both_forms() {
    let error = Expiry::from_headers(Some(Duration::from_secs(30)), Some(1_800_000_000_000))
        .expect_err("must reject");

    assert!(matches!(
        error,
        StoreError::BadRequest(BadRequestKind::Header)
    ));
}

#[test]
fn error_carries_detail() {
    match (StoreError::Fenced { current_epoch: 7 }) {
        StoreError::Fenced { current_epoch } => assert_eq!(current_epoch, 7),
        other => panic!("unexpected error: {other:?}"),
    }

    match (StoreError::SequenceGap {
        expected: 1,
        received: 3,
    }) {
        StoreError::SequenceGap { expected, received } => {
            assert_eq!(expected, 1);
            assert_eq!(received, 3);
        }
        other => panic!("unexpected error: {other:?}"),
    }

    match (StoreError::Closed {
        tail: StreamOffset::from_bytes([9_u8]),
    }) {
        StoreError::Closed { tail } => assert_eq!(tail.as_bytes(), [9_u8]),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn read_page_carries_cursor() {
    let request = ReadRequest {
        from: ReadFrom::Offset(StreamOffset::from_bytes([1_u8])),
        limit: ReadLimit::new(Some(16), Some(1)),
    };
    assert_eq!(request.limit.max_bytes, Some(16));

    let page = ReadPage {
        chunks: vec![Chunk::new([1_u8, 2])],
        cursor: ResumeCursor {
            next: StreamOffset::from_bytes([2_u8]),
            up_to_date: true,
            closed: false,
        },
    };

    assert_eq!(page.chunks[0].as_bytes(), [1_u8, 2]);
    assert_eq!(page.cursor.next.as_bytes(), [2_u8]);
    assert!(page.cursor.up_to_date);
    assert!(!page.cursor.closed);
}
