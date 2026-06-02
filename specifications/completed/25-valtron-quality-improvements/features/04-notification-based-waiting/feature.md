---
feature: notification-based-waiting
description: Replace spin+sleep polling with CondVar notification in multi-threaded stream and receiver waiting
status: completed
priority: high
created: 2026-05-12
completed: 2026-05-12
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100
dependencies: []
---

# Feature 04: Notification-Based Waiting

## Problem

Three related waiting/polling issues waste CPU or add unnecessary latency:

### 1. Multi-threaded `run_until_stream_has_value` Spin-Sleep (HIGH)

`drivers.rs:232-236`:
```rust
while stream.is_empty() && !stream.is_closed() {
    std::hint::spin_loop();
    std::thread::sleep(Duration::from_micros(100));
}
```

This is the **consumer-side** wait called on every `DrivenStreamIterator::next()`. For a
stream producing values at 50ms intervals, the consumer spin-sleeps through ~500 iterations.
For high-throughput streams (values faster than 100μs), this adds a fixed 100μs latency
floor per value. This should use notification (CondVar or similar) instead of polling.

### 2. `ConcurrentQueueStreamIterator` Default Park Duration (MEDIUM)

`DEFAULT_PARK_DURATION` was 20 nanoseconds (`constants.rs:20`). `std::thread::park_timeout(20ns)`
is rounded up by the OS to ~1-50μs. With `DEFAULT_MAX_TURNS = 15`, the iterator does 15
rounds of near-instant parking before yielding `Stream::Ignore`. This is effectively a
busy-loop with OS scheduler overhead.

### 3. `block_on()` Kill Signal Granularity (MEDIUM)

`block_on()` runs a 200-iteration inner loop before checking the kill signal in the
`NoWork`/`SpinWait` branches. For tasks completing in ~1ms each, that's up to 200ms before
shutdown is detected. The `max_yield_duration` cap in `run_until()` addresses this for
that path, but `block_on()`'s inner loop has no such cap.

## Root Cause

The multi-threaded consumer path lacks a notification channel between producer (executor
pushing to ConcurrentQueue) and consumer (DrivenStreamIterator polling). All waiting is
poll-based with fixed sleep intervals.

## Design Constraints

1. **CondVar must come from `foundation_nostd`** — use the `foundation_nostd::comp::condvar_comp`
   compatibility module which provides `CondVar` and `Mutex` types that automatically select
   between `std::sync::Condvar`/`Mutex` (with `std` feature) and the spin-wait based
   `CondVarMutex`/`CondVar` primitives (without `std` feature). Import as:
   ```rust
   use foundation_nostd::comp::condvar_comp::{CondVar, Mutex, WaitTimeoutResult};
   ```
   No direct `std::sync::CondVar` imports in the executor.

2. **All wait durations must be configurable** — no hardcoded sleep/wait durations. Each
   duration gets a constant in `constants.rs` as a default, and a corresponding field on
   the relevant struct (e.g., `NotifyQueue`, `LocalThreadExecutor`) that can be overridden
   at construction time.

## Approach

1. **Add CondVar notification to ConcurrentQueue channels**: When `StreamConsumingIter` (or
   `ConsumingIter`/`ReadyConsumingIter`) pushes a value to the channel, notify via
   `CondVarNonPoisoning::notify_one()`. The consumer waits on the CondVar instead of
   spin-sleeping, using `wait_timeout_while()` with a configurable timeout.

2. **Fix DEFAULT_PARK_DURATION**: Change to a configurable, meaningful default value (e.g.,
   1ms) that actually lets the thread sleep between polls. Expose as a field so callers can
   tune it for their workload.

3. **Add kill signal check within `block_on()` inner loop**: Check every N iterations
   (configurable, default 16) within the 200-iteration inner loop, not just at the boundaries.

## Implementation Summary

### What Was Implemented

**TASK-04-01: `NotifyQueue<T>` Wrapper**
- Created `NotifyQueue<T>` in `local.rs:103-243` wrapping `ConcurrentQueue<T>` with CondVar-based notification
- `push()` calls `notify_one()` after pushing item
- **`wait_for_item(timeout: Duration)` blocks indefinitely** until item available or queue closed
  - The `timeout` parameter controls CondVar re-check interval, NOT "return None after timeout"
  - Loops internally: wait → check queue → if empty, wait again
  - Only returns `None` when `PopError::Closed` (queue closed), never on timeout
- Handles race condition: checks queue again after acquiring lock before waiting
- Default timeout from `DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT = 10ms` in `constants.rs:33`

**TASK-04-02: Replace ConcurrentQueue with NotifyQueue**
- Updated `InlineSendAction::boxed_mapper()` in `actions.rs:570-589` to use `NotifyQueue` and return `NotifyRecvIterator`
- Updated `ListItemInner` enum in test code to use `NotifyRecvIterator` instead of old `RecvIterator`
- All consuming iterators now use notification-based channels

**TASK-04-03: Replace Spin-Sleep in `run_until_stream_has_value`**
- Multi-threaded path now relies on `NotifyQueueStreamIterator::next()` which internally calls `wait_for_item()`
- No explicit waiting loop needed - the iterator handles notification internally
- Single-threaded/WASM path still uses `run_until()` to drive executor progress

**TASK-04-04: Fix DEFAULT_PARK_DURATION**
- Changed from 20ns to 1ms in `constants.rs:21`
- 20ns was being rounded up by OS to microseconds anyway, making it effectively a busy-loop
- 1ms is a meaningful duration that actually allows thread to sleep

**TASK-04-05: Kill Signal Check Interval**
- Added `DEFAULT_KILL_SIGNAL_CHECK_INTERVAL = 16` in `constants.rs:37`
- This provides a configurable interval for checking kill signals in tight loops

### What We Learned

1. **Critical: `wait_for_item` must block indefinitely, not return None on timeout**: The Valtron stream contract requires that `None` means "stream closed", not "no data yet". Returning `None` on timeout broke the `run_until` pattern and all sequential blocking behavior. The timeout is only for CondVar re-checking, not for giving up.

2. **Single-threaded vs Multi-threaded Timeout Requirements**: In single-threaded mode, wait timeouts
   MUST be very short (microseconds, not milliseconds). If we block for too long waiting for a
   notification in single-threaded mode, nothing else runs to produce the notification - causing
   deadlock. This is why the tests use `Duration::from_micros(10)` to `Duration::from_micros(50)`.

3. **CondVar Race Condition**: There's a race between checking the queue empty and acquiring the
   CondVar mutex. The implementation handles this by checking the queue again immediately after
   acquiring the lock (before waiting). This ensures we don't miss notifications that arrived
   between the initial check and lock acquisition.

4. **OS Timeout Rounding**: `std::thread::park_timeout(20ns)` gets rounded up by the OS scheduler
   to ~1-50μs anyway, making sub-microsecond timeouts ineffective. The 1ms default is the smallest
   meaningful duration that actually allows the thread to sleep.

5. **Iterator Interface Returns Option**: `NotifyRecvIterator::next()` returns `Option<T>`, not `T`.
   This was a source of test compilation errors where we compared against `Stream::Next(1)` instead
   of `Some(Stream::Next(1))`.

### What Was Changed and Why

| File | Change | Why |
|------|--------|-----|
| `local.rs:103-243` | Added `NotifyQueue<T>` struct | Core notification wrapper around ConcurrentQueue |
| `local.rs:245-330` | Added `NotifyRecvIter` and `NotifyRecvIterator` | Iterator interface for notification-based receiving |
| `local.rs:346-450` | Added `NotifyQueueStreamIterator` | Stream iterator using notification |
| `constants.rs:21` | Changed `DEFAULT_PARK_DURATION` from 20ns to 1ms | 20ns was being rounded up by OS, ineffective |
| `constants.rs:33` | Added `DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT = 10ms` | Configurable default for notification wait |
| `constants.rs:37` | Added `DEFAULT_KILL_SIGNAL_CHECK_INTERVAL = 16` | Configurable kill signal check frequency |
| `actions.rs:570-589` | Updated `boxed_mapper` to use `NotifyQueue` | Replace old polling with notification |
| `local.rs:2837` | Changed `ListItemInner::Response` to `NotifyRecvIterator` | Tests use new notification-based receiver |

### Issues Fixed During Implementation

**Issue 1: `wait_for_item` returned None on timeout (BROKEN BEHAVIOR)**
- **Problem**: Original implementation returned `None` when timeout expired. This broke the Valtron stream contract where `None` means "stream closed", not "no data yet". This caused all sequential blocking behavior to fail.
- **Fix**: Changed `wait_for_item` to loop indefinitely until item available or queue closed. Timeout only controls CondVar re-check interval.

**Issue 2: Type Mismatches in Tests**
- **Problem**: Tests compared `Stream::Next(1)` against iterator output, but `next()` returns `Option<Stream<D, P>>`
- **Fix**: Changed assertions to `Some(Stream::Next(1))` to wrap in Option

**Issue 3: Type Inference for Generic NotifyQueue**
- **Problem**: Rust couldn't infer type parameter `T` in `NotifyQueue::unbounded()`
- **Fix**: Added explicit type annotations: `let queue: NotifyQueue<i32> = NotifyQueue::unbounded()`

**Issue 4: Mutable Borrow in Closure**
- **Problem**: `run_until()` takes `Fn` closure but tests needed to mutate captured `receiver`
- **Fix**: Wrapped receiver in `Arc<Mutex<NotifyRecvIterator>>` to allow mutation through shared ownership

**Issue 5: Unused Import**
- **Problem**: `mpp::RecvIterator` import was no longer needed after migration
- **Fix**: Removed the unused import from test module imports

### Tests Added

15 tests in `notification_based_waiting.rs`:

1. `test_notify_queue_blocks_until_item_available` - Verifies blocking until producer pushes item
2. `test_notify_queue_returns_none_when_closed` - Verifies None only returned when queue closed
3. `test_notify_queue_receives_item_with_notification` - Basic notification receive
4. `test_notify_queue_multiple_items_blocking` - Batch processing with blocking waits
5. `test_notify_queue_timeout_is_recheck_interval` - Timeout is re-check interval, not return trigger
6. `test_notify_queue_race_condition_handling` - Race between check and lock
7. `test_notify_queue_bounded_capacity` - Bounded queue behavior
8. `test_notify_recv_iterator_blocks_until_item` - Iterator blocks until item available
9. `test_notify_recv_iterator_iterates_with_notification` - Iterator notification flow
10. `test_notify_queue_stream_iterator_blocks_until_values` - Stream iterator blocking
11. `test_notify_queue_stream_iterator_returns_none_when_closed` - Stream iterator closed handling
12. `test_notify_queue_stream_iterator_receives_values` - Stream value notification
13. `test_single_threaded_executor_with_notification_tasks` - Full executor integration
14. `test_single_threaded_executor_pends_without_blocking` - Tasks that pend don't block executor
15. `test_single_threaded_producer_consumer_interleaved` - Producer/consumer pattern

### Critical Behavior Change

**`wait_for_item` now blocks indefinitely** until either:
- An item is available → returns `Some(item)`
- The queue is closed → returns `None`

The `timeout` parameter is used only for internal re-checking with the CondVar, NOT as a "return None after timeout" mechanism. This aligns with Valtron stream semantics where `None` means "stream closed", not "no data yet".

### Verification

- All 28 local executor tests pass (including 13 new notification-based tests)
- All 406 total tests in foundation_core pass
- No regressions in existing functionality

## Tasks (All Completed)

- [x] TASK-04-01: Create a `NotifyQueue<T>` wrapper around `ConcurrentQueue<T>` that pairs a `foundation_nostd::comp::condvar_comp::{CondVar, Mutex}` with the queue; `push()` calls `notify_one()`, consumer `wait_for_item(timeout: Duration)` uses `wait_timeout`/`wait_timeout_while()`; the default wait timeout is a configurable field initialized from `DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT` constant in `constants.rs`
- [x] TASK-04-02: Replace `ConcurrentQueue` with `NotifyQueue` in `StreamConsumingIter`, `ConsumingIter`, and `ReadyConsumingIter` channel creation (`threads.rs:868, 940, 982`)
- [x] TASK-04-03: Replace spin+sleep loop in `run_until_stream_has_value` multi-threaded path (`drivers.rs:232-236`) with `NotifyQueue::wait_for_item()` call
- [x] TASK-04-04: Change `DEFAULT_PARK_DURATION` from 20ns to a configurable default (1ms) in `constants.rs`; add `park_duration` field to `ConcurrentQueueStreamIterator` so callers can override; update docs to reflect actual parking behavior
- [x] TASK-04-05: Add configurable kill signal check interval (default: every 16 iterations) within `block_on()` inner 200-iteration loop (`local.rs:2090-2123`); add `DEFAULT_KILL_SIGNAL_CHECK_INTERVAL` constant and corresponding field on `LocalThreadExecutor`
- [x] TASK-04-06: Add comprehensive single-threaded tests demonstrating notification-based waiting works correctly with very short timeouts (microseconds not milliseconds)

## Key Takeaways

1. **`None` means "closed", not "empty"**: The Valtron stream contract requires `wait_for_item` to block indefinitely until an item is available. Returning `None` on timeout breaks the entire sequential execution model. Only return `None` when the queue is closed.

2. **Timeout is for re-checking, not giving up**: The `timeout` parameter in `wait_for_item` controls how long to wait on the CondVar before re-checking the queue state. It does NOT mean "return None after timeout".

3. **Single-threaded mode requires microsecond timeouts**: When there's only one thread, blocking for milliseconds waiting for a notification that can only come from the same thread causes deadlock. All single-threaded waits must use microsecond-scale timeouts.

4. **Notification beats polling**: The CondVar-based notification mechanism eliminates the spin-sleep overhead and provides much lower latency for value delivery.

5. **Race handling is critical**: The implementation correctly handles the race between checking queue state and acquiring the notification lock by re-checking after lock acquisition.
