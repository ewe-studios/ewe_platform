//! Tests for ThreadedIterFuture executor (single-threaded std mode).

#![cfg(all(feature = "std", not(feature = "multi")))]

use foundation_core::valtron::{ThreadedIterFuture, ThreadedValue};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_threaded_future_basic() {
    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3]);
}

#[test]
#[traced_test]
fn test_threaded_future_error() {
    let threaded = ThreadedIterFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed")
    });

    let iter = threaded.execute();
    let results: Vec<Result<i32, &'static str>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err("future failed"));
}

#[test]
#[traced_test]
fn test_threaded_future_empty_iterator() {
    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>(vec![].into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            _ => None,
        })
        .collect();

    assert!(results.is_empty());
}

#[test]
#[traced_test]
fn test_threaded_future_large_iterator() {
    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>((0..1000).map(Ok).collect::<Vec<_>>().into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 1000);
    assert_eq!(results[0], 0);
    assert_eq!(results[999], 999);
}
