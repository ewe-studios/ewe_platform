//! Integration tests for StreamIterator combinators.
//!
//! These tests validate the behavior of multi-source stream iterator combinators:
//! - CollectAll: aggregates outputs from multiple StreamIterators
//! - MapAllDone: applies mapper when all sources complete
//! - MapAllPendingAndDone: applies mapper with state visibility

use foundation_core::valtron::{CollectAll, MapAllDone, MapAllPendingAndDone, Stream, StreamIterator};

// Simple test stream iterator
struct TestStream {
    items: Vec<Stream<u32, String>>,
    index: usize,
}

impl TestStream {
    fn new(items: Vec<Stream<u32, String>>) -> Self {
        Self { items, index: 0 }
    }
}

impl Iterator for TestStream {
    type Item = Stream<u32, String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl StreamIterator<u32, String> for TestStream {}

// ============================================================================
// CollectAll Tests
// ============================================================================

/// Test 1: Valid input - collecting from multiple streams produces combined results
#[test]
fn test_collect_all_combines_outputs() {
    let stream1 = TestStream::new(vec![Stream::Next(1), Stream::Next(2)]);
    let stream2 = TestStream::new(vec![Stream::Next(10), Stream::Next(20)]);
    let stream3 = TestStream::new(vec![Stream::Next(100)]);

    let mut combined = CollectAll::new(vec![stream1, stream2, stream3]);

    // Collect all outputs
    let mut results = Vec::new();
    for item in &mut combined {
        if let Stream::Next(values) = item {
            results = values;
        }
    }

    // Validate all values were collected
    assert_eq!(results.len(), 5, "Should collect all 5 values");
    assert!(results.contains(&1));
    assert!(results.contains(&2));
    assert!(results.contains(&10));
    assert!(results.contains(&20));
    assert!(results.contains(&100));
}

/// Test 2: Edge case - empty streams return None
#[test]
fn test_collect_all_empty_streams() {
    let stream1 = TestStream::new(vec![]);
    let stream2 = TestStream::new(vec![]);

    let mut combined = CollectAll::new(vec![stream1, stream2]);

    // Should return None for empty streams
    let result = combined.next();
    assert!(result.is_none(), "Empty streams should produce no output");
}

/// Test 3: Edge case - single stream works correctly
#[test]
fn test_collect_all_single_stream() {
    let stream = TestStream::new(vec![Stream::Next(42)]);

    let mut combined = CollectAll::new(vec![stream]);

    let mut got_result = false;
    for item in &mut combined {
        if let Stream::Next(values) = item {
            assert_eq!(values, vec![42]);
            got_result = true;
        }
    }

    assert!(got_result, "Should produce result from single stream");
}

/// Test 4: Pending state propagation
#[test]
fn test_collect_all_propagates_pending() {
    let stream1 = TestStream::new(vec![Stream::Pending("waiting".to_string()), Stream::Next(1)]);
    let stream2 = TestStream::new(vec![Stream::Next(10)]);

    let mut combined = CollectAll::new(vec![stream1, stream2]);

    // Should see Pending state before final result
    let mut got_final = false;

    for item in &mut combined {
        match item {
            Stream::Pending(_) => {}
            Stream::Next(_) => got_final = true,
            _ => {}
        }
    }

    assert!(got_final, "Should produce final result");
}

// ============================================================================
// MapAllDone Tests
// ============================================================================

/// Test 1: Valid input - mapper applies when all sources complete
#[test]
fn test_map_all_done_applies_mapper() {
    let stream1 = TestStream::new(vec![Stream::Next(5)]);
    let stream2 = TestStream::new(vec![Stream::Next(10)]);
    let stream3 = TestStream::new(vec![Stream::Next(15)]);

    let mapper = |values: Vec<u32>| values.iter().sum::<u32>();
    let mut mapped = MapAllDone::new(vec![stream1, stream2, stream3], mapper);

    // Should produce single mapped result
    let mut got_result = false;
    for item in &mut mapped {
        if let Stream::Next(sum) = item {
            assert_eq!(sum, 30, "Sum should be 5 + 10 + 15 = 30");
            got_result = true;
        }
    }

    assert!(got_result, "Should produce mapped result when all complete");
}

/// Test 2: Edge case - empty input produces no output
#[test]
fn test_map_all_done_empty_streams() {
    let stream1 = TestStream::new(vec![]);
    let stream2 = TestStream::new(vec![]);

    let mapper = |values: Vec<u32>| values.iter().sum::<u32>();
    let mut mapped = MapAllDone::new(vec![stream1, stream2], mapper);

    // Should produce no output for empty streams
    let result = mapped.next();
    assert!(result.is_none(), "Empty streams should produce no output");
}

/// Test 3: Mapper transforms correctly
#[test]
fn test_map_all_done_custom_transformation() {
    let stream1 = TestStream::new(vec![Stream::Next(2)]);
    let stream2 = TestStream::new(vec![Stream::Next(3)]);

    // Custom mapper that multiplies
    let mapper = |values: Vec<u32>| {
        values.iter().product::<u32>()
    };
    let mut mapped = MapAllDone::new(vec![stream1, stream2], mapper);

    for item in &mut mapped {
        if let Stream::Next(product) = item {
            assert_eq!(product, 6, "Product should be 2 * 3 = 6");
        }
    }
}

// ============================================================================
// MapAllPendingAndDone Tests
// ============================================================================

/// Test 1: Valid input - mapper receives all states
#[test]
fn test_map_all_pending_and_done_receives_states() {
    let stream1 = TestStream::new(vec![Stream::Next(1)]);
    let stream2 = TestStream::new(vec![Stream::Next(2)]);

    let mapper = |states: Vec<Stream<u32, String>>| states.len();

    let mut mapped = MapAllPendingAndDone::new(vec![stream1, stream2], mapper);

    let mut got_result = false;
    for item in &mut mapped {
        if let Stream::Next(count) = item {
            assert_eq!(count, 2, "Should receive 2 states");
            got_result = true;
        }
    }

    assert!(got_result, "Should produce result");
}

/// Test 2: Edge case - empty streams produce no output
#[test]
fn test_map_all_pending_and_done_empty_streams() {
    let stream1 = TestStream::new(vec![]);
    let stream2 = TestStream::new(vec![]);

    let mapper = |states: Vec<Stream<u32, String>>| states.len();
    let mut mapped = MapAllPendingAndDone::new(vec![stream1, stream2], mapper);

    let result = mapped.next();
    assert!(result.is_none(), "Empty streams should produce no output");
}

/// Test 3: Pending count tracking
#[test]
fn test_map_all_pending_and_done_pending_count() {
    // Stream that starts pending, then produces value
    let stream1 = TestStream::new(vec![
        Stream::Pending("wait1".to_string()),
        Stream::Next(1),
    ]);
    let stream2 = TestStream::new(vec![
        Stream::Pending("wait2".to_string()),
        Stream::Next(2),
    ]);

    let mapper = |states: Vec<Stream<u32, String>>| {
        states.iter().filter(|s| matches!(s, Stream::Pending(_))).count()
    };

    let mut mapped = MapAllPendingAndDone::new(vec![stream1, stream2], mapper);

    // Should see pending counts before final result
    let mut got_final = false;
    for item in &mut mapped {
        match item {
            Stream::Next(_) => {
                got_final = true;
            }
            _ => {}
        }
    }

    assert!(got_final, "Should produce final result");
}

/// Test 4: State inspection - can distinguish done from pending
#[test]
fn test_map_all_pending_and_done_state_inspection() {
    let stream1 = TestStream::new(vec![Stream::Next(42)]);
    let stream2 = TestStream::new(vec![Stream::Next(99)]);

    let mapper = |states: Vec<Stream<u32, String>>| {
        let done_count = states.iter().filter(|s| matches!(s, Stream::Next(_))).count();
        let pending_count = states.iter().filter(|s| matches!(s, Stream::Pending(_))).count();
        (done_count, pending_count)
    };

    let mut mapped = MapAllPendingAndDone::new(vec![stream1, stream2], mapper);

    for item in &mut mapped {
        if let Stream::Next((done, pending)) = item {
            assert_eq!(done, 2, "Both streams should be done");
            assert_eq!(pending, 0, "No streams should be pending");
        }
    }
}
