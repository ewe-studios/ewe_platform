---
feature: linked-task-state-propagation
description: Fix DualSequenceChildAndParentLinkedTask silently discarding parent State signals
status: pending
priority: high
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
dependencies:
  - 01-sleeper-lifecycle-safety
---

# Feature 02: Linked Task State Propagation

## Problem

In `DualSequenceChildAndParentLinkedTask` (`dependent_lift.rs:38-71`), when both child and
parent execute, the parent's returned `State` is **completely ignored**:

```rust
if let Some(mut parent) = self.0.parent.take() {
    // Only checks Some vs None — the actual State value is discarded
    if parent.next(parent_id, engine).is_some() {
        self.0.parent = Some(parent);
    }
}
```

This means:

- **`State::Pending(Some(duration))`** → the sleep signal is lost. The parent should be
  put to sleep but the executor never knows.
- **`State::SpawnFinished(info)`** → the spawn is silently dropped. The child task created
  by the parent never gets processed by the executor.
- **`State::Panicked`** → the panic is swallowed. No recovery, no logging, no notification.
- **`State::Reschedule`** → the reschedule request is lost.

Any task that is the parent in a `DualSequence` relationship loses its ability to sleep,
spawn, or report panics. The parent runs in a degraded mode where only "I produced a value"
and "I'm done" are observable.

## Root Cause

`dependent_lift.rs:46-49` treats the parent as a side-effect — call `next()` to advance it
but ignore what it says. The design intent was "child drives progress, parent tags along,"
but the parent may have legitimate state signals that need executor attention.

## Approach

The fix must balance two constraints:
1. The linked task returns **one** `State` per `next()` call (the child's state takes priority)
2. Parent state signals must not be silently lost

**Strategy:** When the parent returns a state that requires executor action
(`Pending(Some(d))`, `SpawnFinished`, `Panicked`), store it and return it on the next call
instead of the child's state. This creates a "pending parent state" queue that drains
before the child is polled again.

For `FinishChildBeforeParentTask`, this issue doesn't apply because the parent only runs
after the child finishes, so parent state flows through normally.

## Tasks

- [ ] TASK-02-01: Add `pending_parent_state: Option<State>` field to `LinkedParentChildTaskInner`
- [ ] TASK-02-02: In `DualSequenceChildAndParentLinkedTask::next()`, capture parent's returned State; if it's `Pending(Some(_))`, `SpawnFinished(_)`, `Panicked`, or `Reschedule`, store it and return it on the next poll before polling child again
- [ ] TASK-02-03: Handle `State::Done` from parent correctly — parent is finished, continue with child only
- [ ] TASK-02-04: Add tests: parent returns `Pending(Some(d))` while child is active — verify executor receives the sleep signal
- [ ] TASK-02-05: Add tests: parent returns `SpawnFinished` while child is active — verify spawn info reaches executor
