//! Tests for `ThreadedIterFuture` executor.

#![cfg(feature = "multi")]

use foundation_core::valtron::{valtron_test, ThreadedIterFuture, ThreadedValue};
use tracing_test::traced_test;

#[traced_test]
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_basic() {

    let threaded = ThreadedIterFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(())) | ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3]);
}

#[traced_test]
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_future_error() {

    let threaded = ThreadedIterFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed")
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, &'static str>> = iter
        .map(|v| match v {
            ThreadedValue::Value(result) => result,
            ThreadedValue::Waiting => unreachable!("sync iteration should not yield Waiting"),
        })
        .collect();

    dbg!(&results);
}

#[traced_test]
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_empty_iterator() {

    let threaded = ThreadedIterFuture::new(|| async { Ok::<_, ()>(vec![].into_iter()) });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, ()>> = iter
        .map(|v| match v {
            ThreadedValue::Value(result) => result,
            ThreadedValue::Waiting => unreachable!("sync iteration should not yield Waiting"),
        })
        .collect();

    assert!(results.is_empty());
}

#[traced_test]
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_custom_queue_size() {

    // Use a queue size of 100 to hold all items, avoiding backpressure issues
    // in the test environment where thread scheduling may differ from production
    let threaded = ThreadedIterFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        100,
    );

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(())) | ThreadedValue::Waiting => None,
        })
        .collect();

    assert_eq!(results.len(), 100);
}

/// Test that backpressure handling works correctly with a small queue size.
///
/// WHY: Verify the backpressure mechanism allows the producer to complete
/// even when the queue is much smaller than the total output
///
/// WHAT: Produces 100 items with a queue size of only 4, forcing many
/// backpressure cycles
#[traced_test]
#[valtron_test(seed = 42, threads = 4)]
fn test_threaded_future_backpressure() {

    // Small queue forces backpressure: 100 items with queue size 4
    // means ~96 backpressure cycles
    let threaded = ThreadedIterFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        4,
    );

    let iter = threaded.execute().expect("should submit job");

    // Collect results using the iterator directly
    let results: Vec<i32> = iter
        .map(|v| match v {
            ThreadedValue::Value(Ok(val)) => val,
            ThreadedValue::Value(Err(e)) => panic!("Unexpected error: {e:?}"),
            ThreadedValue::Waiting => unreachable!("sync iteration should not yield Waiting"),
        })
        .collect();

    assert_eq!(
        results.len(),
        100,
        "Should have received all 100 items despite backpressure (got {})",
        results.len()
    );
}
