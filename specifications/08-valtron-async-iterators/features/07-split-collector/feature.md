---
feature: "Split Collector Combinator"
description: "split_collector() and split_collect_one() combinators that fork an iterator into observer + continuation branches"
status: "pending"
priority: "high"
depends_on: ["01-task-iterators", "02-stream-iterators"]
estimated_effort: "large"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Split Collector Combinator Feature

## WHY: Problem Statement

Some use cases need to **observe intermediate values** while the computation continues:

```rust
// ClientRequest needs to:
// 1. Get intro/headers first (for status check)
// 2. Then get body (from same execution)
// But both come from the same SendRequestTask!

// Current workaround: Manual state machine storing partial results
enum ClientRequestState {
    NotStarted,
    Executing(Box<DrivenStreamIterator<SendRequestTask>>),
    IntroReady(Option<Box<RequestIntro>>), // Stores intro for later
    Completed,
}
```

**Problems**:
1. Manual state tracking boilerplate
2. Can't easily compose with other combinators
3. No clean separation between "observe" and "continue"

**Desired pattern**:

```rust
// Split the iterator: one branch for observing, one for continuing
let (observer, continuation) = send_request_task
    .split_collector(|item| matches!(item, RequestIntro::Success { .. }), 1);

// Observer: Gets intro/headers immediately
for status in observer {
    match status {
        Stream::Next(intro) => {
            // Got intro! Can return to user now
            return Ok((intro, continuation));
        }
        Stream::Pending(_) => continue,
    }
}

// Continuation: Continues the chain, can add more combinators
let body = continuation
    .map_ready(|response| response.body())
    .stream_collect();
```

## WHAT: Solution Overview

### split_collector() Method

```rust
/// Split the iterator into an observer branch and a continuation branch.
///
/// The observer receives a copy of items matching the predicate,
/// while the continuation continues the chain for further combinators.
///
/// # Type Requirements
///
/// - `Ready` must be `Clone` (observer gets a copy)
/// - `Pending` must be `Clone` (observer gets a copy)
///
/// # Arguments
///
/// * `predicate` - Function determining which items to send to observer
/// * `queue_size` - Size of the ConcurrentQueue between branches (1 = immediate delivery)
///
/// # Returns
///
/// Tuple of:
/// - `CollectorStreamIterator` - Observer that receives matched items
/// - `Self` (wrapped) - Continuation that continues the chain
///
/// # Example
///
/// ```rust
/// let (observer, continuation) = send_request_task
///     .split_collector(
///         |item| matches!(item, RequestIntro::Success { .. }),
///         1  // Queue size 1 for immediate delivery
///     );
///
/// // Observer: Gets intro/headers
/// rayon::spawn(move || {
///     for status in observer {
///         match status {
///             Stream::Next(intro) => {
///                 // Got intro! Send to user
///                 intro_tx.send(intro).unwrap();
///             }
///             Stream::Pending(_) => continue,
///             Stream::Done => break,
///         }
///     }
/// });
///
/// // Continuation: Can chain more combinators
/// let body = continuation
///     .map_ready(|response| response.body())
///     .stream_collect();
/// ```
fn split_collector<P, F>(
    self,
    predicate: F,
    queue_size: usize,
) -> (CollectorStreamIterator<Self::Ready, Self::Pending>, SplitCollectorContinuation<Self, P>)
where
    Self: Sized,
    Self::Ready: Clone,
    Self::Pending: Clone,
    P: Fn(&Self::Ready) -> bool,
    F: Fn(&Self::Ready) -> bool + Send + 'static;
```

### split_collect_one() Convenience Method

```rust
/// Convenience method: split_collector with queue_size = 1 and first-match predicate.
///
/// Sends the FIRST matching item to the observer, then continues.
/// Perfect for "get intro first, then body" patterns.
///
/// # Example
///
/// ```rust
/// let (observer, continuation) = send_request_task
///     .split_collect_one(|item| matches!(item, RequestIntro::Success { .. }));
///
/// // Observer gets the first Success intro
/// let intro = observer.into_future();
///
/// // Continuation can chain more combinators
/// let body = continuation
///     .map_ready(|response| response.body())
///     .stream_collect();
/// ```
fn split_collect_one<P>(
    self,
    predicate: P,
) -> (CollectorStreamIterator<Self::Ready, Self::Pending>, SplitCollectorContinuation<Self, P>)
where
    Self: Sized,
    Self::Ready: Clone,
    Self::Pending: Clone,
    P: Fn(&Self::Ready) -> bool + Send + 'static,
{
    self.split_collector(predicate, 1)
}
```

### CollectorStreamIterator Type

```rust
/// Observer branch from split_collector().
///
/// Receives copies of items matching the predicate via a ConcurrentQueue.
/// Yields Stream::Next for matched items, forwards Pending/Delayed from source.
pub struct CollectorStreamIterator<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
    /// Track if source is done
    source_done: Arc<AtomicBool>,
}

impl<D, P> Iterator for CollectorStreamIterator<D, P>
where
    D: Clone,
    P: Clone,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // Try to get item from queue
        match self.queue.pop() {
            Ok(item) => Some(item),
            Err(PopError::Empty) => {
                // Check if source is done
                if self.source_done.load(Ordering::SeqCst) {
                    None  // No more items
                } else {
                    Some(Stream::Pending(P::default()))  // Still waiting
                }
            }
            Err(PopError::Closed) => None,
        }
    }
}
```

### SplitCollectorContinuation Type

```rust
/// Continuation branch from split_collector().
///
/// Wraps the original iterator, copying matched items to the observer queue
/// while continuing the chain for further combinators.
pub struct SplitCollectorContinuation<I, P>
where
    I: Iterator,
{
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<I::Ready, I::Pending>>>,
    /// Flag to signal when source is done
    source_done: Arc<AtomicBool>,
    /// Predicate to determine which items to copy
    predicate: P,
}

impl<I, P> Iterator for SplitCollectorContinuation<I, P>
where
    I: TaskStatusIterator,
    I::Ready: Clone,
    I::Pending: Clone,
    P: Fn(&I::Ready) -> bool,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next()?;

        // Copy matched items to observer queue
        if let TaskStatus::Ready(value) = &item {
            if (self.predicate)(value) {
                // Convert to Stream and copy to queue
                let stream_item = Stream::Next(value.clone());
                let _ = self.queue.force_push(stream_item);
            }
        }

        // Always forward to continuation
        Some(item)
    }
}
```

## HOW: Implementation Approach

1. Create `CollectorStreamIterator<D, P>` struct with ConcurrentQueue
2. Create `SplitCollectorContinuation<I, P>` wrapper struct
3. Implement `split_collector()` on TaskIteratorExt
4. Implement `split_collect_one()` as convenience wrapper
5. Ensure proper Clone bounds on Ready and Pending
6. Add tests for observer/continuation behavior
7. Demonstrate with ClientRequest intro/body pattern

## Requirements

1. **Clone bounds** - Ready and Pending must be Clone for split_collector
2. **ConcurrentQueue** - Size-configurable queue between branches
3. **AtomicBool** - Signal when source is done
4. **split_collector()** - Main method with predicate + queue_size
5. **split_collect_one()** - Convenience with queue_size=1
6. **Observer yields matched items** - CollectorStreamIterator gets copies
7. **Continuation forwards all items** - SplitCollectorContinuation continues chain

## Tasks

1. [ ] Create `CollectorStreamIterator<D, P>` struct
2. [ ] Create `SplitCollectorContinuation<I, P>` struct
3. [ ] Implement `split_collector()` method on TaskIteratorExt
4. [ ] Implement `split_collect_one()` convenience method
5. [ ] Add Clone bounds to trait where needed
6. [ ] Write unit tests for split_collector behavior
7. [ ] Test: Observer receives matched items
8. [ ] Test: Continuation can chain more combinators
9. [ ] Integration test: ClientRequest intro/body pattern
10. [ ] Run clippy and fmt checks

## Verification

```bash
cargo test -p foundation_core -- valtron::split_collector
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 10 tasks completed
- `split_collector()` compiles with Clone bounds
- `split_collect_one()` works as convenience wrapper
- Observer receives matched items while source continues
- Continuation can chain additional combinators
- Integration test demonstrates ClientRequest pattern
- Zero clippy warnings

## Use Cases

### ClientRequest: Get Intro First, Then Body

```rust
let task = SendRequestTask::new(request, pool, config);

// Split: observer gets intro, continuation gets body
let (intro_observer, body_continuation) = task
    .split_collect_one(|item| matches!(item, RequestIntro::Success { .. }));

// Observer: Get intro/headers immediately
let (intro, headers) = intro_observer
    .into_future()  // Wait for first matched item
    .unwrap();

// Continuation: Chain body extraction
let body = body_continuation
    .map_ready(|response| response.body())
    .stream_collect();
```

### Progress Reporting During Long Operations

```rust
let (progress_observer, result_continuation) = long_running_task
    .split_collector(
        |status| matches!(status, TaskStatus::Pending(Progress::PercentComplete(_))),
        5  // Queue size for buffering
    );

// Observer: Report progress to UI
rayon::spawn(move || {
    for status in progress_observer {
        if let Stream::Next(Progress::PercentComplete(pct)) = status {
            ui.update_progress(pct);
        }
    }
});

// Continuation: Get final result
let result = result_continuation
    .stream_collect();
```

## Design Notes

1. **Clone requirement** - Necessary because observer gets a copy; this is acceptable trade-off
2. **Queue size 1** - Immediate delivery, minimal buffering (default for split_collect_one)
3. **Configurable queue size** - Allows buffering for high-frequency updates
4. **AtomicBool for done signal** - Observer knows when to stop polling
5. **force_push()** - Queue never blocks; if full, items dropped (acceptable for progress updates)

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (Enables ClientRequest intro/body pattern with observer + continuation branches)_
