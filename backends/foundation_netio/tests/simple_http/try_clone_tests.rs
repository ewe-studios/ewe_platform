//! `TryClone` for `SendSafeBody`.
//!
//! WHY: `SendSafeBody` cannot implement `Clone` — its iterator variants wrap a
//! source that is consumed as it is read, so a copy would be a second handle to
//! the same exhausted stream. Before `TryClone` existed, each caller that needed
//! a copy hand-rolled a `match` and they were free to disagree about which
//! variants were safe. One such call site (the SSE reconnect path) dropped the
//! body entirely, which sent a bodyless POST and earned a 400 from the server.
//!
//! WHAT: that owned payloads clone, that single-shot streams are refused rather
//! than silently emptied, and that the refusal names the offending variant.
//!
//! HOW: pure value construction — no sockets, no server.

use foundation_netio::shared::http::{SendSafeBody, TryClone, TryCloneError};

// ---------------------------------------------------------------------------
// Replayable variants
// ---------------------------------------------------------------------------

#[test]
fn none_clones_to_none() {
    let cloned = SendSafeBody::None.try_clone().expect("None is replayable");
    assert_eq!(cloned, SendSafeBody::None);
}

#[test]
fn text_clones_with_its_contents() {
    let body = SendSafeBody::Text(r#"{"model":"x"}"#.to_string());
    let cloned = body.try_clone().expect("Text is replayable");
    match cloned {
        SendSafeBody::Text(t) => assert_eq!(t, r#"{"model":"x"}"#),
        other => panic!("expected Text, got {other:?}"),
    }
}

#[test]
fn bytes_clone_with_their_contents() {
    let body = SendSafeBody::Bytes(vec![1, 2, 3, 255]);
    let cloned = body.try_clone().expect("Bytes are replayable");
    match cloned {
        SendSafeBody::Bytes(b) => assert_eq!(b, vec![1, 2, 3, 255]),
        other => panic!("expected Bytes, got {other:?}"),
    }
}

#[test]
fn cloning_a_body_leaves_the_original_usable() {
    // The reconnect path clones on every attempt, so the source must survive
    // being cloned repeatedly — a `take`-style copy would work once and then
    // start sending nothing.
    let body = SendSafeBody::Text("payload".to_string());
    let first = body.try_clone().expect("first clone");
    let second = body.try_clone().expect("second clone");
    assert_eq!(first, second);
    assert_eq!(
        body,
        SendSafeBody::Text("payload".to_string()),
        "the original must be untouched"
    );
}

// ---------------------------------------------------------------------------
// Single-shot variants
// ---------------------------------------------------------------------------

#[test]
fn a_stream_body_refuses_to_clone() {
    // `None` here is the empty iterator slot; the variant is what matters.
    let err = SendSafeBody::Stream(None)
        .try_clone()
        .expect_err("a single-shot stream must not be cloned");
    assert_eq!(err, TryCloneError::NotReplayable("SendSafeBody::Stream"));
}

#[test]
fn a_chunked_stream_body_refuses_to_clone() {
    let err = SendSafeBody::ChunkedStream(None)
        .try_clone()
        .expect_err("a single-shot stream must not be cloned");
    assert_eq!(
        err,
        TryCloneError::NotReplayable("SendSafeBody::ChunkedStream")
    );
}

#[test]
fn a_line_feed_stream_body_refuses_to_clone() {
    let err = SendSafeBody::LineFeedStream(None)
        .try_clone()
        .expect_err("a single-shot stream must not be cloned");
    assert_eq!(
        err,
        TryCloneError::NotReplayable("SendSafeBody::LineFeedStream")
    );
}

#[test]
fn an_sse_stream_body_refuses_to_clone() {
    let err = SendSafeBody::SseStream(None)
        .try_clone()
        .expect_err("a single-shot stream must not be cloned");
    assert_eq!(err, TryCloneError::NotReplayable("SendSafeBody::SseStream"));
}

// ---------------------------------------------------------------------------
// The error itself
// ---------------------------------------------------------------------------

#[test]
fn the_error_names_the_variant_that_refused() {
    // The message is what a caller sees in a log when a reconnect quietly stops
    // carrying its body; "cannot clone" alone would not say which body.
    let err = SendSafeBody::SseStream(None).try_clone().unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("SendSafeBody::SseStream"),
        "the error must name the variant, got: {text}"
    );
    assert!(
        text.contains("single-shot"),
        "the error must explain why, got: {text}"
    );
}

#[test]
fn the_error_implements_std_error() {
    fn assert_is_error<E: std::error::Error>(_: &E) {}
    let err = SendSafeBody::Stream(None).try_clone().unwrap_err();
    assert_is_error(&err);
}
