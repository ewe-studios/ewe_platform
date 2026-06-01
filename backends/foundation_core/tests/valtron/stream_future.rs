//! Tests for the Stream-to-Future async bridge.
//!
//! Sync tests use a noop waker to poll directly.
//! Async tests use tokio and smol runtimes to validate `.await` works.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use foundation_core::valtron::{
    Stream, StreamCollectFuture, StreamPendingFuture,
    StreamReadyFuture, StreamAsFutureStream, StreamIteratorExt,
};
use futures_core::Stream as FuturesStream;

// ============================================================================
// No-Op Waker for sync polling
// ============================================================================

fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RAW_WAKER,
        |_| {},
        |_| {},
        |_| {},
    );
    const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(RAW_WAKER) }
}

fn poll_once<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(f).poll(&mut cx)
}

fn poll_stream_next<S: FuturesStream + Unpin>(s: &mut S) -> Poll<Option<S::Item>> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(s).poll_next(&mut cx)
}

// ============================================================================
// Sync tests — StreamCollectFuture (native only — uses tracing-test)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod sync_tests {
    use super::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_collect_future_all_next_completes_in_one_poll() {
        tracing::info!("creating StreamCollectFuture with 3 Next items");
        let iter = vec![
            Stream::<i32, &str>::Next(1),
            Stream::Next(2),
            Stream::Next(3),
        ].into_iter();
        let mut future = StreamCollectFuture::new(iter);

        match poll_once(&mut future) {
            Poll::Ready(v) => {
                tracing::info!("collected: {:?}", v);
                assert_eq!(v, vec![1, 2, 3]);
            }
            _ => panic!("expected Ready"),
        }
    }

    #[test]
    #[traced_test]
    fn test_collect_future_with_pending_returns_pending_then_completes() {
        tracing::info!("creating StreamCollectFuture with Next + Pending + Next");
        let iter = vec![
            Stream::<i32, &str>::Next(1),
            Stream::Pending("wait"),
            Stream::Next(2),
        ].into_iter();
        let mut future = StreamCollectFuture::new(iter);

        assert!(matches!(poll_once(&mut future), Poll::Pending));
        tracing::info!("first poll returned Pending as expected");

        match poll_once(&mut future) {
            Poll::Ready(v) => {
                tracing::info!("second poll collected: {:?}", v);
                assert_eq!(v, vec![1, 2]);
            }
            _ => panic!("expected Ready"),
        }
    }

    #[test]
    #[traced_test]
    fn test_collect_future_skips_ignore() {
        tracing::info!("creating StreamCollectFuture with Ignore items");
        let iter = vec![
            Stream::<i32, &str>::Ignore,
            Stream::Next(42),
            Stream::Ignore,
        ].into_iter();
        let mut future = StreamCollectFuture::new(iter);

        match poll_once(&mut future) {
            Poll::Ready(v) => {
                tracing::info!("collected: {:?}", v);
                assert_eq!(v, vec![42]);
            }
            _ => panic!("expected Ready"),
        }
    }

    #[test]
    #[traced_test]
    fn test_collect_future_empty_returns_empty_vec() {
        let iter: Vec<Stream<i32, &str>> = vec![];
        let mut future = StreamCollectFuture::new(iter.into_iter());

        match poll_once(&mut future) {
            Poll::Ready(v) => {
                tracing::info!("empty iterator produced empty vec: {:?}", v);
                assert!(v.is_empty());
            }
            _ => panic!("expected Ready"),
        }
    }

    // ============================================================================
    // Sync tests — StreamReadyFuture
    // ============================================================================

    #[test]
    #[traced_test]
    fn test_ready_future_returns_first_next_with_remaining() {
        tracing::info!("creating StreamReadyFuture with two Next items");
        let iter = vec![
            Stream::<i32, &str>::Next(42),
            Stream::Next(99),
        ].into_iter();
        let mut future = StreamReadyFuture::new(iter);

        match poll_once(&mut future) {
            Poll::Ready(Some((value, mut remaining))) => {
                tracing::info!("got value={} with remaining iterator", value);
                assert_eq!(value, 42);
                assert!(matches!(remaining.next(), Some(Stream::Next(99))));
                tracing::info!("remaining iterator yields Next(99) as expected");
            }
            other => panic!("expected Some, got {:?}", other),
        }
    }

    #[test]
    #[traced_test]
    fn test_ready_future_skips_init_and_pending() {
        tracing::info!("creating StreamReadyFuture with Init + Pending + Next");
        let iter = vec![
            Stream::<i32, &str>::Init,
            Stream::Pending("loading"),
            Stream::Next(1),
        ].into_iter();
        let mut future = StreamReadyFuture::new(iter);

        assert!(matches!(poll_once(&mut future), Poll::Pending));
        assert!(matches!(poll_once(&mut future), Poll::Pending));
        tracing::info!("two Pending polls for Init and Pending states");

        match poll_once(&mut future) {
            Poll::Ready(Some((value, _))) => {
                tracing::info!("got value={}", value);
                assert_eq!(value, 1);
            }
            other => panic!("expected Some, got {:?}", other),
        }
    }

    #[test]
    #[traced_test]
    fn test_ready_future_none_on_no_next() {
        tracing::info!("creating StreamReadyFuture with no Next values");
        let iter = vec![
            Stream::<i32, &str>::Ignore,
            Stream::Init,
        ].into_iter();
        let mut future = StreamReadyFuture::new(iter);

        assert!(matches!(poll_once(&mut future), Poll::Pending));

        match poll_once(&mut future) {
            Poll::Ready(None) => {
                tracing::info!("exhausted without Next, returned None");
            }
            other => panic!("expected None, got {:?}", other),
        }
    }

    // ============================================================================
    // Sync tests — StreamPendingFuture
    // ============================================================================

    #[test]
    #[traced_test]
    fn test_pending_future_returns_first_pending() {
        tracing::info!("creating StreamPendingFuture with Next + Pending + Next");
        let iter = vec![
            Stream::<i32, &str>::Next(1),
            Stream::Pending("waiting"),
            Stream::Next(2),
        ].into_iter();
        let mut future = StreamPendingFuture::new(iter);

        assert!(matches!(poll_once(&mut future), Poll::Pending));
        tracing::info!("first poll returned Pending for Next value");

        match poll_once(&mut future) {
            Poll::Ready(Some((ctx, _))) => {
                tracing::info!("got pending context: {}", ctx);
                assert_eq!(ctx, "waiting");
            }
            other => panic!("expected Some, got {:?}", other),
        }
    }

    #[test]
    #[traced_test]
    fn test_pending_future_none_on_no_pending() {
        tracing::info!("creating StreamPendingFuture with no Pending values");
        let iter = vec![
            Stream::<i32, &str>::Next(1),
            Stream::Next(2),
        ].into_iter();
        let mut future = StreamPendingFuture::new(iter);

        assert!(matches!(poll_once(&mut future), Poll::Pending));
        assert!(matches!(poll_once(&mut future), Poll::Pending));
        match poll_once(&mut future) {
            Poll::Ready(None) => {
                tracing::info!("exhausted without Pending, returned None");
            }
            other => panic!("expected None, got {:?}", other),
        }
    }

    // ============================================================================
    // Sync tests — StreamAsFutureStream
    // ============================================================================

    #[test]
    #[traced_test]
    fn test_future_stream_yields_all_items_one_to_one() {
        tracing::info!("creating StreamAsFutureStream");
        let mut stream = StreamAsFutureStream::new(vec![
            Stream::<i32, &str>::Next(1),
            Stream::Pending("p"),
            Stream::Next(2),
        ].into_iter());

        assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(Stream::Next(1))));
        assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(Stream::Pending("p"))));
        assert_eq!(poll_stream_next(&mut stream), Poll::Ready(Some(Stream::Next(2))));
        assert_eq!(poll_stream_next(&mut stream), Poll::Ready(None));
        tracing::info!("all 3 items yielded in order, then None");
    }

    #[test]
    #[traced_test]
    fn test_future_stream_empty() {
        tracing::info!("creating StreamAsFutureStream with empty iterator");
        let mut stream = StreamAsFutureStream::new(Vec::<Stream<i32, &str>>::new().into_iter());
        let mut items = Vec::new();
        while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
            items.push(item);
        }
        tracing::info!("collected {} items", items.len());
        assert!(items.is_empty());
    }
}

// ============================================================================
// Extension trait tests — smol runtime (native only — smol doesn't support wasm32)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod smol_ext_tests {
    use super::*;
    use smol;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_into_collect_future_via_ext() {
        let result = smol::block_on(
            vec![Stream::<&str, &str>::Next("a"), Stream::Next("b")]
                .into_iter()
                .into_collect_future()
        );
        tracing::info!("collect via ext: {:?}", result);
        assert_eq!(result, vec!["a", "b"]);
    }

    #[test]
    #[traced_test]
    fn test_into_ready_future_via_ext() {
        let (value, mut remaining) = smol::block_on(
            vec![Stream::<i32, &str>::Next(42), Stream::Next(1)]
                .into_iter()
                .into_ready_future()
        ).unwrap();
        tracing::info!("ready via ext: value={}, remaining exhausted={}", value, remaining.next().is_none());
        assert_eq!(value, 42);
        assert!(remaining.next().is_none());
    }

    #[test]
    #[traced_test]
    fn test_into_pending_future_via_ext() {
        let (ctx, _) = smol::block_on(
            vec![Stream::<i32, &str>::Next(1), Stream::Pending("x")]
                .into_iter()
                .into_pending_future()
        ).unwrap();
        tracing::info!("pending via ext: ctx={}", ctx);
        assert_eq!(ctx, "x");
    }

    #[test]
    #[traced_test]
    fn test_into_future_stream_via_ext() {
        let mut stream = vec![
            Stream::<i32, &str>::Next(1),
            Stream::Pending("p"),
            Stream::Next(2),
        ].into_iter().into_future_stream();
        let mut items = Vec::new();
        while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
            items.push(item);
        }
        tracing::info!("future stream via ext: {} items", items.len());
        assert_eq!(items.len(), 3);
    }
}

// ============================================================================
// Async tests — tokio runtime (native only — not available on wasm32)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
mod tokio_tests {
    use super::*;
    use tokio;
    use smol;
    use tracing_test::traced_test;

#[tokio::test]
#[traced_test]
async fn test_tokio_await_collect() {
    tracing::info!("tokio: awaiting collect future");
    let result = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Next(2),
        Stream::Next(3),
    ].into_iter().into_collect_future().await;
    tracing::info!("tokio collect result: {:?}", result);
    assert_eq!(result, vec![1, 2, 3]);
}

#[tokio::test]
#[traced_test]
async fn test_tokio_await_ready_future() {
    tracing::info!("tokio: awaiting ready future");
    let (value, mut remaining) = vec![
        Stream::<i32, &str>::Next(42),
        Stream::Pending("wait"),
    ].into_iter().into_ready_future().await.unwrap();
    tracing::info!("tokio ready: value={}, remaining exhausted={}", value, remaining.next().is_none());
    assert_eq!(value, 42);
    assert!(remaining.next().is_none());
}

#[tokio::test]
#[traced_test]
async fn test_tokio_await_pending_future_first() {
    tracing::info!("tokio: awaiting pending future (target first)");
    let (ctx, _) = vec![
        Stream::<i32, &str>::Pending("done"),
        Stream::Next(1),
    ].into_iter().into_pending_future().await.unwrap();
    tracing::info!("tokio pending: ctx={}", ctx);
    assert_eq!(ctx, "done");
}

#[tokio::test]
#[traced_test]
async fn test_tokio_await_pending_future_after_next() {
    tracing::info!("tokio: awaiting pending future (after Next values)");
    let (ctx, _) = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Next(2),
        Stream::Pending("done"),
    ].into_iter().into_pending_future().await.unwrap();
    tracing::info!("tokio pending: ctx={}", ctx);
    assert_eq!(ctx, "done");
}

#[tokio::test]
#[traced_test]
async fn test_tokio_await_future_stream() {
    tracing::info!("tokio: awaiting future stream");
    let mut stream = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Pending("p"),
        Stream::Next(2),
    ].into_iter().into_future_stream();
    let mut items = Vec::new();
    while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
        items.push(item);
    }
    tracing::info!("tokio stream: {} items", items.len());
    assert_eq!(items.len(), 3);
}

// ============================================================================
// Async tests — smol runtime
// ============================================================================

#[test]
#[traced_test]
fn test_smol_run_collect() {
    tracing::info!("smol: block_on collect");
    let result = smol::block_on(async {
        vec![
            Stream::<i32, &str>::Next(10),
            Stream::Next(20),
        ].into_iter().into_collect_future().await
    });
    tracing::info!("smol collect result: {:?}", result);
    assert_eq!(result, vec![10, 20]);
}

#[test]
#[traced_test]
fn test_smol_run_ready_future() {
    tracing::info!("smol: block_on ready future");
    let (value, mut remaining) = smol::block_on(async {
        vec![
            Stream::<i32, &str>::Init,
            Stream::Next(99),
        ].into_iter().into_ready_future().await.unwrap()
    });
    tracing::info!("smol ready: value={}, remaining exhausted={}", value, remaining.next().is_none());
    assert_eq!(value, 99);
    assert!(remaining.next().is_none());
}

#[test]
#[traced_test]
fn test_smol_run_future_stream() {
    tracing::info!("smol: block_on future stream");
    let mut stream = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Pending("p"),
        Stream::Next(2),
    ].into_iter().into_future_stream();
    let mut items = Vec::new();
    while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
        items.push(item);
    }
    tracing::info!("smol stream: {} items", items.len());
    assert_eq!(items.len(), 3);
}

// ============================================================================
// Combinator chaining tests
// ============================================================================

#[test]
#[traced_test]
fn test_combinator_then_collect_future() {
    let result = smol::block_on(async {
        vec![
            Stream::<i32, &str>::Next(1),
            Stream::Next(2),
            Stream::Next(3),
        ].into_iter()
            .map_done(|v| v * 10)
            .into_collect_future()
            .await
    });
    tracing::info!("combinator then collect: {:?}", result);
    assert_eq!(result, vec![10, 20, 30]);
}

#[test]
#[traced_test]
fn test_combinator_then_ready_future() {
    let (value, _) = smol::block_on(async {
        vec![
            Stream::<i32, &str>::Pending("a"),
            Stream::Next(42),
        ].into_iter()
            .map_pending(|p| p.len())
            .into_ready_future()
            .await
            .unwrap()
    });
    tracing::info!("combinator then ready: value={}", value);
    assert_eq!(value, 42);
}

#[test]
#[traced_test]
fn test_combinator_then_future_stream() {
    let mut stream = vec![
        Stream::<i32, &str>::Next(1),
        Stream::Next(2),
    ].into_iter()
        .map_done(|v| v * 2)
        .into_future_stream();
    let mut items = Vec::new();
    while let Poll::Ready(Some(item)) = poll_stream_next(&mut stream) {
        items.push(item);
    }
    tracing::info!("combinator then stream: {:?}", items);
    assert_eq!(items, vec![
        Stream::Next(2),
        Stream::Next(4),
    ]);
}

}
