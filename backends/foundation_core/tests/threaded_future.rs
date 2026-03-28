//! Tests for ThreadedFuture executor.

use foundation_core::valtron::{ThreadedFuture, ThreadedValue};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_threaded_future_basic() {
    let threaded = ThreadedFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let (_handle, receiver) = threaded.execute();

    let recv_iter = receiver.into_recv_iter();
    let results: Vec<i32> = recv_iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3]);
}

#[test]
#[traced_test]
fn test_threaded_future_future_error() {
    let threaded = ThreadedFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed")
    });

    let (_handle, receiver) = threaded.execute();

    let recv_iter = receiver.into_recv_iter();
    let results: Vec<Result<i32, &'static str>> = recv_iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err("future failed"));
}

#[test]
#[traced_test]
fn test_threaded_future_empty_iterator() {
    let threaded = ThreadedFuture::new(|| async { Ok::<_, ()>(vec![].into_iter()) });

    let (_handle, receiver) = threaded.execute();

    let recv_iter = receiver.into_recv_iter();
    let results: Vec<Result<i32, ()>> = recv_iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
        })
        .collect();

    assert!(results.is_empty());
}

#[test]
#[traced_test]
fn test_threaded_future_custom_queue_size() {
    let threaded = ThreadedFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        4,
    );

    let (_handle, receiver) = threaded.execute();

    let recv_iter = receiver.into_recv_iter();
    let results: Vec<i32> = recv_iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 100);
}
