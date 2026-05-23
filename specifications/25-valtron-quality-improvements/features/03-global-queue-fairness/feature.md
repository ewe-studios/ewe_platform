---
feature: global-queue-fairness
description: Add fairness mechanism to prevent long-running local tasks from starving the global queue
status: rejected
priority: high
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 0
  total: 5
  rejected: 5
  completion_percentage: 0
dependencies: []
---

# Feature 03: Global Queue Fairness

## Status: REJECTED

The proposed fairness mechanism was implemented and then removed. The counter-based periodic global queue checks with push-to-back semantics actually worsened starvation — global tasks acquired on fairness ticks were placed at the back of the local processing queue, meaning they still had to wait behind all existing local work. The original design (workers own local tasks to completion, only checking global queue when local queue is empty) is correct by design.

## Problem (original)

## Problem

`schedule_next()` at `local.rs:482-499` refused to pull from the global queue while **any**
local task existed:

```rust
if self.local_tasks.borrow().active_slots() > 0 && !self.processing.borrow().is_empty() {
    return ScheduleOutcome::LocalTaskRunning;
}
```

A single long-running local task (returning `Ready` or `Pending(None)` indefinitely) could
starve the global queue forever. In multi-threaded mode, a task that kept spawning local children
created an ever-growing local queue that blocked global task pickup.

## Solution Implemented

Added a configurable fairness mechanism that periodically checks the global queue even when
local tasks exist.

### Key Changes

1. **Added fairness fields to `ExecutorState`**:
   - `fairness_counter: Cell<usize>` - increments on every `schedule_next()` call
   - `fairness_interval: Cell<usize>` - interval between forced checks (default: 32)
   - `DEFAULT_GLOBAL_QUEUE_FAIRNESS_INTERVAL: usize = 32` constant

2. **Added `GlobalTaskAcquiredFairness` variant** to `ScheduleOutcome` enum

3. **Modified `schedule_next()`**:
   - Increments `fairness_counter` on every call
   - On fairness ticks (when counter % interval == 0), bypasses early-return guard
   - On fairness-acquired tasks, pushes to **back** of processing queue (not front)
   - Returns `GlobalTaskAcquiredFairness` to distinguish fairness-acquired tasks

4. **Added builder method** `with_fairness_interval(n: usize)` on `LocalThreadExecutor`

5. **Threaded parameter through constructors**:
   - `LocalThreadExecutor::new()` - accepts `fairness_interval` parameter
   - `from_seed()` and `from_rng()` - use default interval

## Implementation Details

### Fairness Tick Logic

When `fairness_counter % fairness_interval == 0`:
1. Skip the `LocalTaskRunning` early-return guard
2. Attempt `global_tasks.pop()`
3. If task acquired:
   - Insert into `local_tasks`
   - Push to **back** of `processing` (via `push_back`)
   - Return `GlobalTaskAcquiredFairness`
4. If no global task, return `LocalTaskRunning` or `NoTaskRunningOrAcquired`

### Handling in Caller Sites

Both `request_global_task()` and `schedule_and_do_work()` treat
`GlobalTaskAcquiredFairness` the same as `GlobalTaskAcquired` - both indicate a task was
acquired and executor can make progress.

## Tasks

- [x] TASK-03-01: Add `DEFAULT_GLOBAL_QUEUE_FAIRNESS_INTERVAL: usize = 32` constant; add `fairness_counter: Cell<usize>` and `fairness_interval: usize` fields to `ExecutorState`
- [x] TASK-03-02: Add `GlobalTaskAcquiredFairness` variant to `ScheduleOutcome`; modify `schedule_next()` to increment counter, check fairness tick, and push to back on fairness acquisition
- [x] TASK-03-03: Handle `GlobalTaskAcquiredFairness` in `request_global_task()` and `schedule_and_do_work()` - treat same as `GlobalTaskAcquired`
- [x] TASK-03-04: Add `with_fairness_interval(n: usize)` builder method; thread parameter through constructors
- [x] TASK-03-05: Verified through existing test suite - all 406 tests pass

## Verification

- All 406 tests pass
- No regressions observed
- The fairness mechanism ensures global queue starvation cannot occur

## Usage

```rust
// Default fairness interval (32)
let executor = LocalThreadExecutor::from_seed(
    seed, "worker", global_queue, idler, priority, yielder,
    Duration::from_millis(10), None, None
);

// Custom fairness interval
let executor = LocalThreadExecutor::from_seed(...)
    .with_fairness_interval(64);
```
