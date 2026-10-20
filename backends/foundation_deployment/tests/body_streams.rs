//! The streaming response bodies the generated API clients hand back.
//!
//! WHY: A body of unbounded size must be read only as far as it is asked for,
//! a JSON-lines body must be parsed line by line without holding more than
//! the line it is assembling, and an error body must be read only up to its
//! cap.
//!
//! WHAT: [`JsonLinesStream`], [`ByteStream`], [`read_bounded`] and
//! [`read_error_body`] over in-memory and counted streamed bodies, and the
//! [`BodyLimits`] that cap them.

use core::task::{Context, Poll};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_core::io::readers::Data;
use foundation_deployment::providers::common::{
    read_bounded, read_error_body, ApiClient, ApiError, BodyLimits, ByteStream, JsonLinesStream,
};
use foundation_netio::shared::http::SendSafeBody;
use serde::de::DeserializeOwned;

/// The default line limit, which the stream tests below run under.
const MAX_LINE: usize = 1024 * 1024;

#[derive(Debug, serde::Deserialize, PartialEq)]
struct Item {
    n: u32,
}

/// Drive a future to completion. The streams here never wait on I/O
/// (their bodies are in memory), so a no-op waker suffices.
fn block_on<F: core::future::Future>(fut: F) -> F::Output {
    let mut fut = core::pin::pin!(fut);
    let mut cx = Context::from_waker(core::task::Waker::noop());
    loop {
        if let Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// A streamed body of `chunks`, counting how many have been read.
fn counted_stream(chunks: Vec<&'static [u8]>, reads: Arc<AtomicUsize>) -> SendSafeBody {
    let iter = chunks.into_iter().map(move |c| {
        reads.fetch_add(1, Ordering::SeqCst);
        Ok(Data::Bytes(c.to_vec()))
    });
    SendSafeBody::Stream(Some(Box::new(iter)))
}

fn all<T: DeserializeOwned>(mut s: JsonLinesStream<T>) -> Vec<Result<T, ApiError>> {
    block_on(async move {
        let mut out = Vec::new();
        while let Some(item) = s.next().await {
            out.push(item);
        }
        out
    })
}

#[test]
fn json_lines_split_across_chunks_parse_one_item_per_line() {
    let reads = Arc::new(AtomicUsize::new(0));
    let body = counted_stream(vec![b"{\"n\":1}\n{\"n\"", b":2}\n\n{\"n\":3}"], reads);
    let got: Vec<u32> = all::<Item>(JsonLinesStream::new(body, MAX_LINE))
        .into_iter()
        .map(|r| r.expect("item").n)
        .collect();
    assert_eq!(got, vec![1, 2, 3]);
}

#[test]
fn json_lines_are_read_only_as_far_as_they_are_asked_for() {
    let reads = Arc::new(AtomicUsize::new(0));
    let body = counted_stream(
        vec![
            b"{\"n\":1}\n",
            b"{\"n\":2}\n",
            b"{\"n\":3}\n",
            b"{\"n\":4}\n",
        ],
        Arc::clone(&reads),
    );
    let mut stream = JsonLinesStream::<Item>::new(body, MAX_LINE);
    let first = block_on(stream.next()).expect("an item").expect("valid");
    assert_eq!(first, Item { n: 1 });
    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "only the first chunk is read for the first line"
    );
}

#[test]
fn a_malformed_line_is_a_parse_error_and_ends_the_stream() {
    let body = SendSafeBody::Bytes(b"{\"n\":1}\nnot json\n{\"n\":3}\n".to_vec());
    let got = all::<Item>(JsonLinesStream::new(body, MAX_LINE));
    assert_eq!(got.len(), 2, "nothing after the bad line: {got:?}");
    assert!(
        matches!(got[1], Err(ApiError::ParseFailed(_))),
        "{:?}",
        got[1]
    );
}

#[test]
fn a_line_that_never_ends_is_an_error_not_an_unbounded_buffer() {
    static BIG: [u8; 64 * 1024] = [b'x'; 64 * 1024];
    let reads = Arc::new(AtomicUsize::new(0));
    let chunks = vec![&BIG[..]; MAX_LINE / BIG.len() + 2];
    let body = counted_stream(chunks, Arc::clone(&reads));
    let got = all::<Item>(JsonLinesStream::new(body, MAX_LINE));
    assert!(
        matches!(got.as_slice(), [Err(ApiError::ParseFailed(m))] if m.contains("longer than")),
        "{got:?}"
    );
}

#[test]
fn a_failed_read_ends_the_stream_with_a_transport_error() {
    // Built lazily: the boxed error is not `Send`, the closure is.
    let iter = (0..2).map(|i| {
        if i == 0 {
            Ok(Data::Bytes(b"{\"n\":1}\n".to_vec()))
        } else {
            Err(Box::new(std::io::Error::other("connection reset"))
                as foundation_core::extensions::result_ext::BoxedError)
        }
    });
    let body = SendSafeBody::Stream(Some(Box::new(iter)));
    let got = all::<Item>(JsonLinesStream::new(body, MAX_LINE));
    assert!(got[0].is_ok());
    assert!(
        matches!(&got[1], Err(ApiError::RequestSendFailed(m)) if m.contains("connection reset")),
        "{:?}",
        got[1]
    );
}

#[test]
fn bytes_are_streamed_chunk_by_chunk_and_drain_counts_them() {
    let reads = Arc::new(AtomicUsize::new(0));
    let body = counted_stream(vec![b"abc", b"de"], Arc::clone(&reads));
    let mut stream = ByteStream::new(body);
    let first = block_on(stream.next()).expect("a chunk").expect("ok");
    assert_eq!(&first[..], b"abc");
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    assert_eq!(block_on(stream.drain()).expect("drain"), 2);
}

#[test]
fn read_bounded_stops_at_the_limit_without_reading_further() {
    let reads = Arc::new(AtomicUsize::new(0));
    let body = counted_stream(vec![b"0123", b"4567", b"89"], Arc::clone(&reads));
    assert_eq!(block_on(read_bounded(body, 6)), "012345");
    assert_eq!(
        reads.load(Ordering::SeqCst),
        2,
        "the third chunk is never read"
    );
    assert_eq!(block_on(read_error_body(SendSafeBody::None, 1024)), None);
    assert_eq!(
        block_on(read_error_body(
            SendSafeBody::Text("{\"message\":\"no\"}".into()),
            1024
        ))
        .as_deref(),
        Some("{\"message\":\"no\"}")
    );
}

#[test]
fn the_default_limits_are_the_documented_ones() {
    let limits = BodyLimits::default();
    assert_eq!(limits.max_error_body, 64 * 1024);
    assert_eq!(limits.max_json_line, MAX_LINE);
    let tuned = limits.with_max_error_body(10).with_max_json_line(20);
    assert_eq!((tuned.max_error_body, tuned.max_json_line), (10, 20));
    // Zero is a valid (if useless) limit, not a sentinel.
    assert_eq!(BodyLimits::default().with_max_error_body(0).max_error_body, 0);
}

#[test]
fn a_configured_line_limit_is_the_one_enforced() {
    // A 16-byte limit: a short line passes, the 40-byte one after it is an
    // error, and nothing after that is read.
    let body = SendSafeBody::Bytes(
        b"{\"n\":1}\n{\"n\":2,\"pad\":\"xxxxxxxxxxxxxxxxxxxxxx\"}\n{\"n\":3}\n".to_vec(),
    );
    let got = all::<Item>(JsonLinesStream::new(body, 16));
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(got[0].as_ref().expect("first line fits").n, 1);
    assert!(
        matches!(&got[1], Err(ApiError::ParseFailed(m)) if m.contains("longer than 16 bytes")),
        "{:?}",
        got[1]
    );
}

#[test]
fn a_line_exactly_at_the_limit_is_accepted() {
    // `{"n":1}` is 7 bytes; the newline is not part of the line.
    let body = SendSafeBody::Bytes(b"{\"n\":1}\n{\"n\":2}".to_vec());
    let got: Vec<u32> = all::<Item>(JsonLinesStream::new(body, 7))
        .into_iter()
        .map(|r| r.expect("item").n)
        .collect();
    assert_eq!(got, vec![1, 2]);
}

#[test]
fn a_bare_transport_reads_under_the_default_limits() {
    let http = foundation_netio::HttpClientBuilder::new().build();
    assert_eq!(ApiClient::from(http.clone()).limits(), BodyLimits::default());
    let limits = BodyLimits::default().with_max_error_body(1);
    assert_eq!(ApiClient::new(http).with_limits(limits).limits(), limits);
}

#[test]
fn an_error_body_is_cut_at_the_configured_limit() {
    let reads = Arc::new(AtomicUsize::new(0));
    let body = counted_stream(vec![b"0123", b"4567", b"89"], Arc::clone(&reads));
    assert_eq!(block_on(read_error_body(body, 5)).as_deref(), Some("01234"));
    assert_eq!(reads.load(Ordering::SeqCst), 2, "the third chunk is never read");
    // A zero limit reads nothing, which reads as no body at all.
    let body = counted_stream(vec![b"0123"], Arc::new(AtomicUsize::new(0)));
    assert_eq!(block_on(read_error_body(body, 0)), None);
}
