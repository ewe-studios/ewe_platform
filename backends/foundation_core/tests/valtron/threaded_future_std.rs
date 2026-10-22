//! Tests for `ThreadedIterFuture` executor (single-threaded std mode).

#![cfg(all(feature = "std", not(feature = "multi")))]

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use foundation_core::valtron::{ThreadedIterFuture, ThreadedValue};

/// A future that returns `Pending` exactly `remaining` times before `Ready`.
///
/// Regression fixture for the single-threaded `FutureIterator`: it must retain
/// the future across `Pending` polls (yielding [`ThreadedValue::Waiting`] each
/// time) rather than dropping it after the first `Pending`.
struct PendingNTimes {
    remaining: Cell<usize>,
}

impl Future for PendingNTimes {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        if this.remaining.get() > 0 {
            this.remaining.set(this.remaining.get() - 1);
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

#[test]
fn test_threaded_future_basic() {
    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(_)) | ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3]);
}

#[test]
fn test_threaded_future_error() {
    let threaded = ThreadedIterFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed")
    });

    let iter = threaded.execute();
    let results: Vec<Result<i32, &'static str>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err("future failed"));
}

#[test]
fn test_threaded_future_empty_iterator() {
    let threaded = ThreadedIterFuture::new(|| async { Ok::<_, ()>(vec![].into_iter()) });

    let iter = threaded.execute();
    let results: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            ThreadedValue::Waiting => None,
        })
        .collect();

    assert!(results.is_empty());
}

#[test]
fn test_threaded_future_large_iterator() {
    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>((0..1000).map(Ok).collect::<Vec<_>>().into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(_)) | ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results.len(), 1000);
    assert_eq!(results[0], 0);
    assert_eq!(results[999], 999);
}

#[test]
fn test_threaded_future_survives_pending() {
    // The factory future is `Pending` twice before resolving the inner iterator.
    // The old single-threaded `FutureIterator` dropped the future on the first
    // `Pending`, so `next()` yielded one `Waiting` then `None` — the values were
    // lost. It must instead retain the future and re-poll it on each `next()`.
    let threaded = ThreadedIterFuture::new(|| async {
        PendingNTimes {
            remaining: Cell::new(2),
        }
        .await;
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2)].into_iter())
    });

    let mut iter = threaded.execute();

    // Two `Pending` polls surface as two `Waiting` items, then the values flow.
    assert!(matches!(iter.next(), Some(ThreadedValue::Waiting)));
    assert!(matches!(iter.next(), Some(ThreadedValue::Waiting)));

    let rest: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            ThreadedValue::Waiting => None,
        })
        .collect();
    assert_eq!(rest, vec![Ok(1), Ok(2)]);
}
