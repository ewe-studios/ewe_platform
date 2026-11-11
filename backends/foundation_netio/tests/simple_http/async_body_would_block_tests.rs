//! A non-blocking body read must park, never fail.
//!
//! WHY: `AsyncSendSafeBody` classifies a `WouldBlock` item as "no bytes yet"
//! and registers the read waker. The body-state conversion
//! (`body_reader::map_err`) used to collapse every iterator error into
//! `io::Error::other`, which erased the `WouldBlock` kind. A streaming relay
//! then saw a terminal error instead of a wake — and, on a blocking socket it
//! blocked the valtron worker outright (spec 70 feature 121).
//!
//! WHAT: a chunked body iterator that yields `HttpReaderError::WouldBlock`
//! forever. One poll must return `Pending` and register the read wake source.
//!
//! On the old code the first poll returned `Ready(Some(Err(..)))`.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use foundation_core::extensions::result_ext::BoxedError;
use foundation_netio::shared::client::body_reader::AsyncSendSafeBody;
use foundation_netio::shared::http::{ChunkedData, HttpReaderError, SendSafeBody};
use futures_core::Stream as FuturesStream;

struct WouldBlockForever;

impl Iterator for WouldBlockForever {
    type Item = Result<ChunkedData, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Err(Box::new(HttpReaderError::WouldBlock)))
    }
}

#[derive(Default)]
struct RecordingWake(AtomicBool);

impl Wake for RecordingWake {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn a_would_block_body_read_parks_instead_of_failing() {
    let registered = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&registered);
    let source = Arc::new(move |_waker: &Waker| {
        flag.store(true, Ordering::SeqCst);
        true
    });

    let mut body = AsyncSendSafeBody::from(SendSafeBody::ChunkedStream(Some(Box::new(
        WouldBlockForever,
    ))))
    .with_read_wake(source);

    let waker = Waker::from(Arc::new(RecordingWake::default()));
    let mut cx = Context::from_waker(&waker);

    let poll = Pin::new(&mut body).poll_next(&mut cx);
    assert!(
        matches!(poll, Poll::Pending),
        "a WouldBlock body read must park, not fail: {poll:?}"
    );
    assert!(
        registered.load(Ordering::SeqCst),
        "the read wake source must be registered"
    );
}
