//! Feature 07 (Decision 12 §7): pushable client request body over `Pipe<Bytes>`.
//!
//! The producer pushes body chunks after the request has started; the renderer
//! drains them as `Data` batches. Backed by the waker-hooked 00-F4 pipe, so a full
//! pipe parks the async producer (no whole-request buffering, no polling handoff).

use bytes::Bytes;
use std::sync::Arc;

use core::future::Future;
use core::task::{Context, Poll};

use concurrent_queue::ConcurrentQueue;
use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::readers::Data;
use foundation_core::valtron::{queue_waker, BoxedSendableDataIterator, TrySendError, WakeToken};
use foundation_netio::shared::http::{pushable_request_body_with_depth, SendSafeBody};

/// Extract the backing stream iterator from a `SendSafeBody::Stream`.
fn into_stream(body: SendSafeBody) -> BoxedSendableDataIterator<BoxedError> {
    match body {
        SendSafeBody::Stream(Some(iter)) => iter,
        _ => panic!("pushable body must be a SendSafeBody::Stream"),
    }
}

/// WHY: Pushed chunks must reach the renderer in FIFO order as `Data::Bytes`; a
///      transient empty body is `Data::Retry`; close ends the body (`None`).
#[test]
fn pushed_chunks_drain_as_data_in_order() {
    let (producer, body) = pushable_request_body_with_depth(4);
    let mut stream = into_stream(body);

    producer.try_push(Bytes::from_static(b"one")).unwrap();
    producer.try_push(Bytes::from_static(b"two")).unwrap();

    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"one"));
    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"two"));
    // Empty but still open -> transient retry, never an error, never end.
    assert!(matches!(stream.next(), Some(Ok(Data::Retry))));

    producer.close();
    // Closed and drained -> end of body.
    assert!(stream.next().is_none());
}

/// Acceptance: bytes pushed AFTER rendering has begun still reach the wire.
#[test]
fn push_after_send_start_reaches_the_renderer() {
    let (producer, body) = pushable_request_body_with_depth(4);
    let mut stream = into_stream(body);

    // "Send start": push + drain the first chunk.
    producer.try_push(Bytes::from_static(b"head")).unwrap();
    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"head"));
    // Renderer sees no data yet.
    assert!(matches!(stream.next(), Some(Ok(Data::Retry))));

    // Push MORE after send has started — it must reach the renderer.
    producer.try_push(Bytes::from_static(b"tail")).unwrap();
    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"tail"));

    producer.close();
    assert!(stream.next().is_none());
}

/// WHY: The bounded pipe provides backpressure — no whole-request buffering.
#[test]
fn bounded_pipe_applies_backpressure() {
    let (producer, body) = pushable_request_body_with_depth(2);
    let mut stream = into_stream(body);

    producer.try_push(Bytes::from_static(b"a")).unwrap();
    producer.try_push(Bytes::from_static(b"b")).unwrap();
    // Third push exceeds the depth-2 bound.
    match producer.try_push(Bytes::from_static(b"c")) {
        Err(TrySendError::Full(_)) => {}
        other => panic!("expected Full backpressure, got {other:?}"),
    }

    // Draining one frees a slot.
    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"a"));
    producer.try_push(Bytes::from_static(b"c")).unwrap();
}

/// WHY: The async producer parks on a full pipe and is woken when the renderer
///      drains a chunk (waker-hooked pipe, no polling handoff).
#[test]
fn async_push_parks_on_full_and_wakes_on_drain() {
    let (producer, body) = pushable_request_body_with_depth(1);
    let mut stream = into_stream(body);

    producer.try_push(Bytes::from_static(b"x")).unwrap(); // fill depth-1 pipe

    let obs = Arc::new(ConcurrentQueue::<WakeToken>::unbounded());
    let waker = queue_waker(obs.clone(), 0);
    let mut cx = Context::from_waker(&waker);

    // Async push onto the full pipe parks.
    let mut push = Box::pin(producer.push(Bytes::from_static(b"y")));
    assert!(matches!(push.as_mut().poll(&mut cx), Poll::Pending));
    assert!(obs.is_empty(), "no wake while the pipe stays full");

    // The renderer drains a chunk -> wakes the parked producer.
    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"x"));
    assert!(!obs.is_empty(), "drain woke the parked producer");

    // The push now completes; the second chunk reaches the renderer.
    assert!(matches!(push.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"y"));
}

/// WHY: Dropping the producer (not just `close`) also ends the body.
#[test]
fn dropping_producer_ends_the_body() {
    let (producer, body) = pushable_request_body_with_depth(4);
    let mut stream = into_stream(body);

    producer.try_push(Bytes::from_static(b"last")).unwrap();
    drop(producer);

    // Backlog drains first, then end-of-body.
    assert!(matches!(stream.next(), Some(Ok(Data::Bytes(b))) if b == b"last"));
    assert!(stream.next().is_none());
}
