//! Tests for the Durable Streams status table.

use foundation_core::durable::{BadRequestKind, StoreError};
use foundation_http::shared::durable::{header, status_for};

#[test]
fn given_a_core_refusal_when_it_maps_then_the_status_matches_the_contract() {
    assert_eq!(status_for(&StoreError::NotFound), 404);
    assert_eq!(status_for(&StoreError::Gone), 410);
    assert_eq!(status_for(&StoreError::Trimmed), 410);
    assert_eq!(status_for(&StoreError::Fenced { current_epoch: 3 }), 403);
    assert_eq!(status_for(&StoreError::InvalidEpochSeq { seq: 1 }), 400);
    assert_eq!(
        status_for(&StoreError::SequenceGap {
            expected: 1,
            received: 2,
        }),
        409
    );
    assert_eq!(status_for(&StoreError::ConfigConflict), 409);
    assert_eq!(
        status_for(&StoreError::StreamSeqConflict { last: None }),
        409
    );
    assert_eq!(
        status_for(&StoreError::ContentTypeMismatch {
            expected: foundation_core::durable::ContentType::default(),
            got: foundation_core::durable::ContentType::default(),
        }),
        409
    );
    assert_eq!(
        status_for(&StoreError::Closed {
            tail: foundation_core::durable::StreamOffset::from_bytes(vec![0]),
        }),
        409
    );
    assert_eq!(
        status_for(&StoreError::Storage(Box::new(std::io::Error::other("x")))),
        500
    );
    assert_eq!(
        status_for(&StoreError::RateLimited { retry_after: None }),
        429
    );
    assert_eq!(status_for(&StoreError::Unsupported), 501);
    assert_eq!(
        status_for(&StoreError::BadRequest(BadRequestKind::Json)),
        400
    );
}

#[test]
fn given_the_wire_contract_when_headers_render_then_the_names_are_exact() {
    assert_eq!(header::STREAM_NEXT_OFFSET, "Stream-Next-Offset");
    assert_eq!(header::STREAM_CLOSED, "Stream-Closed");
    assert_eq!(header::PRODUCER_SEQ, "Producer-Seq");
    assert_eq!(header::STREAM_FORK_SUB_OFFSET, "Stream-Fork-Sub-Offset");
}
