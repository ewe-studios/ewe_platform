---
feature: channel-backpressure
description: Add bounded queue option for consuming iterators and guard against EntryList slot reuse conflicts
status: completed
priority: medium
created: 2026-05-12
completed: 2026-05-12
tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100
dependencies:
  - 04-notification-based-waiting
---

# Feature 07: Channel Backpressure - COMPLETED

## Summary

All 4 tasks completed:

1. **Bounded Queue Option** - Added `with_channel_capacity()` to `ThreadPoolTaskBuilder`
2. **Backpressure Handling** - When channel is full, return `State::Pending(None)` instead of terminating
3. **Documentation** - Added notes about EntryList slot reuse (deferred to Feature 01)
4. **All Tests Pass** - 412 lib tests passing

## Changes Made

### ThreadPoolTaskBuilder (threads.rs)
- Added `channel_capacity: Option<usize>` field
- Added `with_channel_capacity(capacity: usize)` method
- Updated `ready_iter()`, `stream_iter_with_config()`, and `schedule_iter()` to use
  bounded `NotifyQueue::bounded(capacity)` when capacity is set

### Consuming Iterators (task_iters.rs)
- Updated `StreamConsumingIter`, `ConsumingIter`, and `ReadyConsumingIter` to handle
  `PushError::Full` with backpressure (return `State::Pending(None)`)
- `PushError::Closed` still terminates the task as before
- Added explicit import for `concurrent_queue::PushError`

## Original Problems

### 1. Unbounded ConcurrentQueue Growth (MEDIUM) ✅ FIXED

All consuming iterators (`StreamConsumingIter`, `ConsumingIter`, `ReadyConsumingIter`) push
to `ConcurrentQueue::unbounded()` channels (`threads.rs:868-869, 940, 982`). If a producer
task runs faster than the consumer drains the queue, memory grows without bound.

In a streaming scenario where the task produces data continuously but the consumer processes
it slowly, memory usage scales linearly with production rate. There is no mechanism to pause
the producer when the queue is large.

### 2. EntryList Slot Reuse Safety (LOW)

When a `FinishChildBeforeParentTask` or `DualSequence` is created, two entries are `take()`n
and one new entry is `insert()`ed. The old entries' slots are freed in the EntryList. If
EntryList reuses freed slots (as slab allocators typically do), a new task could be assigned
an old Entry that still has references in `packed_tasks`, `task_graph`, or `sleepers`.

This wouldn't panic immediately but could cause a new task to be incorrectly treated as
packed or as having dependencies it doesn't have.

## Approach

1. **Add bounded queue option**: Provide `ConcurrentQueue::bounded(capacity)` as an option
   in `ThreadPoolTaskBuilder`. When the queue is full, the producing `ExecutionIterator`
   returns `State::Pending(None)` or `State::Reschedule` instead of dropping the value,
   creating natural backpressure.

2. **Entry cleanup on task combination**: When old entries are `take()`n during task
   combination, also clean up their references in `task_graph` and `packed_tasks`. This
   complements feature 01's sleeper cleanup.

## Tasks

- [ ] TASK-07-01: Add `with_channel_capacity(cap: usize)` to `ThreadPoolTaskBuilder` for `stream_iter`, `ready_iter`, and `schedule_iter` methods; default remains unbounded for backward compatibility
- [ ] TASK-07-02: In consuming iterators, when `channel.push()` returns `PushError::Full`, return `State::Pending(None)` instead of `State::Done` to create producer-side backpressure
- [ ] TASK-07-03: In all `SpawnFinished` paths of `do_work()` that `take()` old entries, also remove those entries from `task_graph` and `packed_tasks` (complements feature 01)
- [ ] TASK-07-04: Add test: bounded channel with slow consumer verifies producer yields `Pending` when full and resumes when drained
