//! Tests for MultiThreadedFuture executor.

#![cfg(feature = "multi")]

use foundation_core::valtron::{MultiThreadedFuture, ThreadedValue};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_multi_threaded_future_basic() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let threaded = MultiThreadedFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute().expect("should submit job");
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
fn test_multi_threaded_future_future_error() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let threaded = MultiThreadedFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed")
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, &'static str>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err("future failed"));
}

#[test]
#[traced_test]
fn test_multi_threaded_future_empty_iterator() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let threaded = MultiThreadedFuture::new(|| async { Ok::<_, ()>(vec![].into_iter()) });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
        })
        .collect();

    assert!(results.is_empty());
}

#[test]
#[traced_test]
fn test_multi_threaded_future_custom_queue_size() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    // Use a queue size of 100 to hold all items, avoiding backpressure issues
    // in the test environment where thread scheduling may differ from production
    let threaded = MultiThreadedFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        100,
    );

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
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
#[test]
#[traced_test]
fn test_multi_threaded_future_backpressure() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    // Small queue forces backpressure: 100 items with queue size 4
    // means ~96 backpressure cycles
    let threaded = MultiThreadedFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        4,
    );

    let iter = threaded.execute().expect("should submit job");

    // Collect results using the iterator directly
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(e)) => panic!("Unexpected error: {e:?}"),
        })
        .collect();

    assert_eq!(
        results.len(),
        100,
        "Should have received all 100 items despite backpressure (got {})",
        results.len()
    );
}
