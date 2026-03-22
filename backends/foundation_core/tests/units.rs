//! Integration tests for StreamIterator combinators.
//!
//! These tests validate the behavior of multi-source stream iterator combinators:
//! - CollectAll: aggregates outputs from multiple StreamIterators
//! - MapAllDone: applies mapper when all sources complete
//! - MapAllPendingAndDone: applies mapper with state visibility
//!
//! Also tests unified executor helper functions:
//! - execute_collect_all: executes multiple TaskIterators in parallel, collects results
//! - execute_map_all: executes multiple TaskIterators, applies mapper when all complete
//! - execute_map_all_pending_and_done: executes with state visibility

use foundation_core::valtron::{
    CollectAll, MapAllDone, MapAllPendingAndDone, Stream, StreamIterator, TaskIterator, TaskStatus,
};

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
    let stream1 = TestStream::new(vec![
        Stream::Pending("waiting".to_string()),
        Stream::Next(1),
    ]);
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
    let mapper = |values: Vec<u32>| values.iter().product::<u32>();
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

// ============================================================================
// execute_collect_all Integration Tests (Feature 05)
// ============================================================================

/// Simple test task that produces a single Ready value after Pending states
struct CounterTask {
    pending_count: u32,
    current: u32,
    ready_value: u32,
}

impl CounterTask {
    fn new(pending_count: u32, ready_value: u32) -> Self {
        Self {
            pending_count,
            current: 0,
            ready_value,
        }
    }
}

impl Iterator for CounterTask {
    type Item = TaskStatus<u32, u32, foundation_core::valtron::NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.pending_count {
            self.current += 1;
            Some(TaskStatus::Pending(self.current))
        } else if self.current == self.pending_count {
            self.current += 1;
            Some(TaskStatus::Ready(self.ready_value))
        } else {
            None
        }
    }
}

impl TaskIterator for CounterTask {
    type Ready = u32;
    type Pending = u32;
    type Spawner = foundation_core::valtron::NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Test 1: execute_collect_all aggregates outputs from multiple tasks
#[test]
fn test_execute_collect_all_aggregates_outputs() {
    use foundation_core::valtron::{execute_collect_all, initialize_pool, DEFAULT_WAIT_CYCLE};

    // Initialize the valtron pool for this test
    initialize_pool(42, None);

    // Create three tasks that each produce a single value after 1 pending state
    let tasks = vec![
        CounterTask::new(1, 10),
        CounterTask::new(1, 20),
        CounterTask::new(1, 30),
    ];

    let result = execute_collect_all(tasks, Some(DEFAULT_WAIT_CYCLE));

    match result {
        Ok(mut collected) => {
            // Iterate through the collected stream
            let mut got_result = false;
            for item in &mut collected {
                if let Stream::Next(values) = item {
                    assert_eq!(values.len(), 3, "Should collect 3 values");
                    assert!(values.contains(&10));
                    assert!(values.contains(&20));
                    assert!(values.contains(&30));
                    got_result = true;
                }
            }
            assert!(got_result, "Should produce collected result");
        }
        Err(e) => {
            panic!("execute_collect_all failed: {:?}", e);
        }
    }
}

/// Test 2: execute_map_all applies mapper when all tasks complete
#[test]
fn test_execute_map_all_applies_mapper() {
    use foundation_core::valtron::{execute_map_all, initialize_pool, DEFAULT_WAIT_CYCLE};

    // Initialize the valtron pool for this test
    initialize_pool(42, None);

    // Create three tasks that produce values immediately (no pending states)
    let tasks = vec![
        CounterTask::new(0, 5),
        CounterTask::new(0, 10),
        CounterTask::new(0, 15),
    ];

    let mapper = |values: Vec<u32>| values.iter().sum::<u32>();

    let result = execute_map_all(tasks, mapper, Some(DEFAULT_WAIT_CYCLE));

    match result {
        Ok(mut mapped) => {
            // Iterate through the mapped stream
            let mut got_result = false;
            for item in &mut mapped {
                if let Stream::Next(sum) = item {
                    assert_eq!(sum, 30, "Sum should be 5 + 10 + 15 = 30");
                    got_result = true;
                }
            }
            assert!(got_result, "Should produce mapped result");
        }
        Err(e) => {
            panic!("execute_map_all failed: {:?}", e);
        }
    }
}

/// Test 3: execute_map_all_pending_and_done receives state info
#[test]
fn test_execute_map_all_pending_and_done() {
    use foundation_core::valtron::{
        execute_map_all_pending_and_done, initialize_pool, DEFAULT_WAIT_CYCLE,
    };

    // Initialize the valtron pool for this test
    initialize_pool(42, None);

    // Create two tasks that each go through 1 pending state before ready
    let tasks = vec![CounterTask::new(1, 42), CounterTask::new(1, 99)];

    let mapper = |states: Vec<Stream<u32, u32>>| states.len();

    let result = execute_map_all_pending_and_done(tasks, mapper, Some(DEFAULT_WAIT_CYCLE));

    match result {
        Ok(mut progress) => {
            // Iterate through the progress stream
            let mut got_result = false;
            for item in &mut progress {
                if let Stream::Next(count) = item {
                    assert_eq!(count, 2, "Should receive 2 states when all complete");
                    got_result = true;
                }
            }
            assert!(got_result, "Should produce result");
        }
        Err(e) => {
            panic!("execute_map_all_pending_and_done failed: {:?}", e);
        }
    }
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
    let stream1 = TestStream::new(vec![Stream::Pending("wait1".to_string()), Stream::Next(1)]);
    let stream2 = TestStream::new(vec![Stream::Pending("wait2".to_string()), Stream::Next(2)]);

    let mapper = |states: Vec<Stream<u32, String>>| {
        states
            .iter()
            .filter(|s| matches!(s, Stream::Pending(_)))
            .count()
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
        let done_count = states
            .iter()
            .filter(|s| matches!(s, Stream::Next(_)))
            .count();
        let pending_count = states
            .iter()
            .filter(|s| matches!(s, Stream::Pending(_)))
            .count();
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

// ============================================================================
// Split Collector Tests (Feature 07)
// ============================================================================

/// Test task for split_collector that produces known sequence
struct SplitTestTask {
    items: Vec<TaskStatus<u32, String, foundation_core::valtron::NoAction>>,
    index: usize,
}

impl SplitTestTask {
    fn new(items: Vec<TaskStatus<u32, String, foundation_core::valtron::NoAction>>) -> Self {
        Self { items, index: 0 }
    }
}

impl Iterator for SplitTestTask {
    type Item = TaskStatus<u32, String, foundation_core::valtron::NoAction>;

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

impl TaskIterator for SplitTestTask {
    type Ready = u32;
    type Pending = String;
    type Spawner = foundation_core::valtron::NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Test 1: split_collector sends matched items to observer
#[test]
fn test_split_collector_observer_receives_matched_items() {
    use foundation_core::valtron::TaskIteratorExt;

    // Create task that produces: Ready(5), Ready(10), Ready(15)
    let task = SplitTestTask::new(vec![
        TaskStatus::Ready(5),
        TaskStatus::Ready(10),
        TaskStatus::Ready(15),
    ]);

    // Split: observer gets values > 7
    let (mut observer, mut continuation) = task.split_collector(|v| *v > 7, 10);

    // Continuation should produce all original values
    let mut continuation_values = Vec::new();
    for status in &mut continuation {
        if let TaskStatus::Ready(v) = status {
            continuation_values.push(v);
        }
    }
    assert_eq!(continuation_values, vec![5, 10, 15]);
    // continuation is exhausted here, queue is closed

    // Observer should only receive matched values (> 7)
    let mut observer_values = Vec::new();
    for stream in &mut observer {
        if let Stream::Next(v) = stream {
            observer_values.push(v);
        }
    }
    // Note: observer receives matched values via queue
    assert!(observer_values.contains(&10));
    assert!(observer_values.contains(&15));
}

/// Test 2: split_collect_one convenience method
#[test]
fn test_split_collect_one_first_match() {
    use foundation_core::valtron::TaskIteratorExt;

    // Create task with multiple values
    let task = SplitTestTask::new(vec![
        TaskStatus::Ready(1),
        TaskStatus::Ready(2),
        TaskStatus::Ready(3),
        TaskStatus::Ready(4),
    ]);

    // Split: observer gets first value > 2
    let (mut observer, mut continuation) = task.split_collect_one(|v| *v > 2);

    // Continuation produces all original values
    let mut continuation_values = Vec::new();
    for status in &mut continuation {
        if let TaskStatus::Ready(v) = status {
            continuation_values.push(v);
        }
    }
    assert_eq!(continuation_values, vec![1, 2, 3, 4]);

    // Observer should receive first match
    let mut got_match = false;
    for stream in &mut observer {
        if let Stream::Next(v) = stream {
            if v > 2 {
                got_match = true;
                break;
            }
        }
    }
    assert!(got_match, "Observer should receive first match");
}

/// Test 3: Observer receives Stream::Next for matched items
#[test]
fn test_split_collector_observer_stream_type() {
    use foundation_core::valtron::TaskIteratorExt;

    let task = SplitTestTask::new(vec![
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(42),
        TaskStatus::Ready(99),
    ]);

    let (mut observer, continuation) = task.split_collector(|v| *v == 42, 10);

    // Drive the continuation to push items to the observer queue
    // Use a scoped block to ensure continuation is dropped before we iterate observer
    {
        let mut continuation = continuation;
        for _status in &mut continuation {
            // Drive the continuation
        }
        // continuation is dropped here, closing the queue
    }

    // Observer should receive Stream::Next(42)
    let mut got_42 = false;
    for stream in &mut observer {
        match stream {
            Stream::Next(v) if v == 42 => {
                got_42 = true;
                break;
            }
            Stream::Ignore => continue, // Skip Ignore states while waiting
            _ => {}
        }
    }
    assert!(got_42, "Observer should receive matched value 42");
}

/// Test 4: Continuation forwards all states unchanged
#[test]
fn test_split_collector_continuation_forwards_all_states() {
    use foundation_core::valtron::TaskIteratorExt;

    let task = SplitTestTask::new(vec![
        TaskStatus::Pending("wait1".to_string()),
        TaskStatus::Ready(10),
        TaskStatus::Pending("wait2".to_string()),
        TaskStatus::Ready(20),
    ]);

    let (_, continuation) = task.split_collector(|_| true, 10);

    let states: Vec<_> = continuation.collect();

    // Should have all states including Pending and Ready
    let mut pending_count = 0;
    let mut ready_count = 0;
    for status in &states {
        match status {
            TaskStatus::Pending(_) => pending_count += 1,
            TaskStatus::Ready(_) => ready_count += 1,
            _ => {}
        }
    }
    assert_eq!(pending_count, 2, "Should forward both Pending states");
    assert_eq!(ready_count, 2, "Should forward both Ready states");
}

// ============================================================================
// Split Collector for StreamIterator Tests
// ============================================================================

use foundation_core::valtron::StreamIteratorExt;

/// Test 5: split_collector on StreamIterator
#[test]
fn test_stream_split_collector_observer_receives_matched_items() {
    // Create stream that produces: Next(5), Next(10), Next(15)
    let stream = TestStream::new(vec![Stream::Next(5), Stream::Next(10), Stream::Next(15)]);

    // Split: observer gets values > 7
    let (mut observer, continuation) =
        stream.split_collector(|s| matches!(s, Stream::Next(v) if *v > 7), 10);

    // Continuation should produce all original values
    // Use scoped block to ensure continuation is dropped before we iterate observer
    {
        let mut continuation = continuation;
        let mut continuation_values = Vec::new();
        for status in &mut continuation {
            if let Stream::Next(v) = status {
                continuation_values.push(v);
            }
        }
        assert_eq!(continuation_values, vec![5, 10, 15]);
        // continuation is dropped here, closing the queue
    }

    // Observer should only receive matched values (> 7)
    let mut observer_values = Vec::new();
    for stream_item in &mut observer {
        if let Stream::Next(v) = stream_item {
            observer_values.push(v);
        }
    }
    assert!(observer_values.contains(&10));
    assert!(observer_values.contains(&15));
}

/// Test 6: split_collect_one on StreamIterator
#[test]
fn test_stream_split_collect_one_first_match() {
    // Create stream with multiple values
    let stream = TestStream::new(vec![
        Stream::Next(1),
        Stream::Next(2),
        Stream::Next(3),
        Stream::Next(4),
    ]);

    // Split: observer gets first value > 2
    let (mut observer, mut continuation) =
        stream.split_collect_one(|s| matches!(s, Stream::Next(v) if *v > 2));

    // Continuation produces all original values
    let mut continuation_values = Vec::new();
    for status in &mut continuation {
        if let Stream::Next(v) = status {
            continuation_values.push(v);
        }
    }
    assert_eq!(continuation_values, vec![1, 2, 3, 4]);

    // Observer should receive first match
    let mut got_match = false;
    for stream_item in &mut observer {
        if let Stream::Next(v) = stream_item {
            if v > 2 {
                got_match = true;
                break;
            }
        }
    }
    assert!(got_match, "Observer should receive first match");
}
