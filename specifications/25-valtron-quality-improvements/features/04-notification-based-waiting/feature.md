---
feature: notification-based-waiting
description: Replace spin+sleep polling with CondVar notification in multi-threaded stream and receiver waiting
status: pending
priority: high
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0
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

`DEFAULT_PARK_DURATION` is 20 nanoseconds (`constants.rs:20`). `std::thread::park_timeout(20ns)`
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

## Approach

1. **Add CondVar notification to ConcurrentQueue channels**: When `StreamConsumingIter` (or
   `ConsumingIter`/`ReadyConsumingIter`) pushes a value to the channel, notify a CondVar.
   The consumer waits on the CondVar instead of spin-sleeping.

2. **Fix DEFAULT_PARK_DURATION**: Change to a meaningful value (e.g., 1ms) that actually
   lets the thread sleep between polls.

3. **Add kill signal check within `block_on()` inner loop**: Check every N iterations
   (e.g., every 16) within the 200-iteration inner loop, not just at the boundaries.

## Tasks

- [ ] TASK-04-01: Create a `NotifyQueue<T>` wrapper around `ConcurrentQueue<T>` that pairs a CondVar with the queue; `push()` calls `notify_one()`, consumer `wait_for_item()` uses `wait_timeout_while()`
- [ ] TASK-04-02: Replace `ConcurrentQueue` with `NotifyQueue` in `StreamConsumingIter`, `ConsumingIter`, and `ReadyConsumingIter` channel creation (`threads.rs:868, 940, 982`)
- [ ] TASK-04-03: Replace spin+sleep loop in `run_until_stream_has_value` multi-threaded path (`drivers.rs:232-236`) with `NotifyQueue::wait_for_item()` call
- [ ] TASK-04-04: Change `DEFAULT_PARK_DURATION` from 20ns to 1ms in `constants.rs`; update `ConcurrentQueueStreamIterator` docs to reflect actual parking behavior
- [ ] TASK-04-05: Add kill signal check every 16 iterations within `block_on()` inner 200-iteration loop (`local.rs:2090-2123`)
- [ ] TASK-04-06: Add benchmark test comparing latency of old spin-sleep vs new CondVar notification for stream value delivery
