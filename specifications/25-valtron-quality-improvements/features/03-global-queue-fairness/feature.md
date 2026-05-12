---
feature: global-queue-fairness
description: Add fairness mechanism to prevent long-running local tasks from starving the global queue
status: pending
priority: high
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
dependencies: []
---

# Feature 03: Global Queue Fairness

## Problem

`schedule_next()` at `local.rs:481-498` refuses to pull from the global queue while **any**
local task exists:

```rust
if self.local_tasks.borrow().active_slots() > 0 && !self.processing.borrow().is_empty() {
    return ScheduleOutcome::LocalTaskRunning;
}
```

A single long-running local task (returning `Ready` or `Pending(None)` indefinitely) starves
the global queue forever. In multi-threaded mode, a task that keeps spawning local children
creates an ever-growing local queue that blocks global task pickup. Tasks submitted to the
global queue by other threads may never be processed by that worker.

**Impact:** In a multi-threaded deployment with N worker threads, if M workers are occupied
with long-running local tasks, only N-M workers serve the global queue. In the worst case,
all workers are stuck on local tasks and the global queue grows unboundedly.

## Root Cause

The design prioritizes "finish local tasks first" with no escape hatch. There is no fairness
mechanism, no "check global queue every N iterations," and no priority aging.

## Approach

Add a configurable fairness interval: every N calls to `schedule_next()`, check the global
queue regardless of local task state. When a global task is pulled in fairness mode, it
gets appended to the **back** of the processing queue (not front), preserving local task
priority while ensuring global tasks eventually get picked up.

This must be balanced — too aggressive and local task completion suffers; too conservative
and the starvation problem persists. A reasonable default is every 32 or 64 ticks.

## Tasks

- [ ] TASK-03-01: Add `fairness_counter: usize` and `fairness_interval: usize` fields to `ExecutorState`; add `GLOBAL_QUEUE_FAIRNESS_INTERVAL` constant (default: 32) to `constants.rs`
- [ ] TASK-03-02: Modify `schedule_next()` to check global queue every `fairness_interval` calls regardless of local task state; append acquired tasks to back of processing queue
- [ ] TASK-03-03: Add test: submit task to global queue while worker has a long-running local task; verify global task is eventually processed within fairness interval
- [ ] TASK-03-04: Add `with_fairness_interval(n: usize)` configuration to `LocalThreadExecutor` constructor chain
