//! `SendSafeBodyBytesIterator` reports "nothing yet" the same way for every
//! body kind: `Stream::Wait`. A sized body's `Data::Retry` used to come out as
//! `Ignore` (also used for bookkeeping items) and a chunked body's `WouldBlock`
//! as a `StreamError`, so callers could not tell waiting from bookkeeping or
//! failure.
//!
//! The bodies are built lazily (`map` over a `Range`) on purpose: the boxed
//! error is not `Send`, so a `Vec` + `into_iter` would store it and fail the
//! `Send + Sync` bound on `SendSafeBody`'s iterator. A lazy iterator does not
//! store its output, so it is `Send + Sync` even though its item is not.

use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::readers::Data;
use foundation_core::valtron::Stream;
use foundation_netio::shared::client::body_reader::{
    SendSafeBodyBytesItem, SendSafeBodyBytesIterator,
};
use foundation_netio::shared::http::{ChunkedData, SendSafeBody};

fn would_block() -> BoxedError {
    Box::new(std::io::Error::from(std::io::ErrorKind::WouldBlock))
}

#[test]
fn a_sized_body_that_would_block_reports_wait() {
    let iter = (0..2).map(|i| match i {
        0 => Ok(Data::Retry),
        _ => Ok(Data::Bytes(b"ok".to_vec())),
    });
    let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Stream(Some(Box::new(iter))));
    assert!(matches!(iter.next(), Some(Stream::Wait)));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(_)))
    ));
}

#[test]
fn a_chunked_body_that_would_block_reports_wait_not_an_error() {
    let iter = (0..3).map(|i| match i {
        0 => Err(would_block()),
        1 => Ok(ChunkedData::Data(b"ok".to_vec(), None)),
        _ => Ok(ChunkedData::DataEnded),
    });
    let mut iter =
        SendSafeBodyBytesIterator::new(SendSafeBody::ChunkedStream(Some(Box::new(iter))));
    assert!(matches!(iter.next(), Some(Stream::Wait)));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::Chunk(_)))
    ));
    // The end-of-chunks marker is bookkeeping, not waiting.
    assert!(matches!(iter.next(), Some(Stream::Ignore)));
}

#[test]
fn a_real_chunked_failure_is_still_an_error() {
    let iter = (0..1).map(|_| {
        Err(Box::new(std::io::Error::from(std::io::ErrorKind::ConnectionReset)) as BoxedError)
    });
    let mut iter =
        SendSafeBodyBytesIterator::new(SendSafeBody::ChunkedStream(Some(Box::new(iter))));
    assert!(matches!(
        iter.next(),
        Some(Stream::Next(SendSafeBodyBytesItem::StreamError(_)))
    ));
}
