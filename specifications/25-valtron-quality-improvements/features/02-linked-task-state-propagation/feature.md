---
feature: linked-task-state-propagation
description: Fix DualSequenceChildAndParentLinkedTask silently discarding parent State signals
status: completed
priority: high
created: 2026-05-12
tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100
dependencies:
  - 01-sleeper-lifecycle-safety
---

# Feature 02: Linked Task State Propagation

## Problem

In `DualSequenceChildAndParentLinkedTask` (`dependent_lift.rs`), when both child and
parent execute, the parent's returned `State` was **completely ignored**:

```rust
if let Some(mut parent) = self.0.parent.take() {
    // Only checks Some vs None — the actual State value is discarded
    if parent.next(parent_id, engine).is_some() {
        self.0.parent = Some(parent);
    }
}
```

This meant:

- **`State::Pending(Some(duration))`** → the sleep signal was lost
- **`State::SpawnFinished(info)`** → the spawn was silently dropped
- **`State::Panicked`** → the panic was swallowed
- **`State::Reschedule`** → the reschedule request was lost

## Solution Implemented

Added `pending_parent_state: Option<State>` field to `LinkedParentChildTaskInner` and
modified `DualSequenceChildAndParentLinkedTask::next()` to:

1. **Check for pending parent state first** - If a parent state was stored on the
   previous poll, return it before polling the child again

2. **Capture parent's actual State** when polling parent while child is active

3. **Store states requiring executor action** - When parent returns `Pending(Some(_))`,
   `SpawnFinished(_)`, `Panicked`, or `Reschedule`, store it in `pending_parent_state`
   and return it on the next call

4. **Handle `State::Done` correctly** - When parent returns Done, don't restore it

## Implementation Details

### Changes to `dependent_lift.rs`

1. Added `pending_parent_state: Option<State>` to `LinkedParentChildTaskInner`
2. Updated both `DualSequenceChildAndParentLinkedTask::new()` and
   `FinishChildBeforeParentTask::new()` to initialize the new field
3. Rewrote `DualSequenceChildAndParentLinkedTask::next()` to:
   - Check and return `pending_parent_state` first
   - Use `matches!()` to detect states requiring propagation
   - Store parent state before checking if parent should be restored

## Tasks

- [x] TASK-02-01: Add `pending_parent_state: Option<State>` field to `LinkedParentChildTaskInner`
- [x] TASK-02-02: In `DualSequenceChildAndParentLinkedTask::next()`, capture parent's returned State; if it's `Pending(Some(_))`, `SpawnFinished(_)`, `Panicked`, or `Reschedule`, store it and return it on the next poll before polling child again
- [x] TASK-02-03: Handle `State::Done` from parent correctly — parent is finished, continue with child only
- [x] TASK-02-04: Tests verified through scenario_5 tests
- [x] TASK-02-05: Implementation verified through code review and existing test suite

## Verification

- All 406 tests pass
- The `scenario_5_task_a_spawns_task_b` test exercises the combined task behavior
- No regressions observed
