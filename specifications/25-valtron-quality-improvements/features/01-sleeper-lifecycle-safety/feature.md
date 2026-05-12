---
feature: sleeper-lifecycle-safety
description: Fix stale sleeper entries that can panic the executor when tasks are combined via lifts
status: pending
priority: critical
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0
dependencies: []
---

# Feature 01: Sleeper Lifecycle Safety

## Problem

Three related issues stem from sleeper/task lifecycle mismatch:

### 1. Stale Sleeper Panic (CRITICAL)

When a task returns `Pending(Some(duration))`, the executor:
1. Packs the task via `pack_task_and_dependents(top_entry)` (`local.rs:1038`)
2. Inserts a `Sleepable::Timable` referencing `top_entry` into sleepers (`local.rs:1041-1045`)

If that task later gets combined into a `FinishChildBeforeParentTask` or
`DualSequenceChildAndParentLinkedTask` (via `SpawnFinished` in `do_work` at lines 820-841),
the **old entry** is removed from `local_tasks` via `take()`, and a **new entry** is
created for the combined task. But the sleeper still references the old entry.

When the sleeper matures:
1. `wake_up()` pushes the stale entry into the processing queue
2. `do_work()` calls `local_tasks.park(&top_entry)` → returns `None`
3. Assert fires: `assert!(iter_container.is_some(), "An entry must always have a task attached")`
4. **Executor panics**

**Sequence:** task sleeps → task spawns child during subsequent poll → old entry removed →
sleeper still live → sleeper fires → executor panics.

### 2. `total_active_tasks()` Arithmetic Underflow (HIGH)

`local.rs:460-473`:
```rust
let active_task_count = local_task_count - sleeping_task_count;
```

If `sleeping_task_count > local_task_count` (possible due to stale sleepers), this panics
in debug mode or wraps to `usize::MAX` in release mode — making `has_active_tasks()` return
true forever, preventing the executor from reaching "no work" state.

### 3. Packed Tasks Not Cleaned on Removal (MEDIUM)

When a task finishes (`None`/`Done`/`Panicked`/`SpawnFailed`), `do_work()` removes it from
`local_tasks` via `take()` but never calls `unpack_task_and_dependents()`. Packed entries
remain in `packed_tasks` HashMap forever — a slow memory leak.

## Root Cause

Sleeper lifecycle is not coupled to task lifecycle. When tasks are combined or removed,
their sleeper entries and packed entries are not cleaned up.

## Approach

1. **Clear sleepers for old entries when tasks are combined**: In the `SpawnFinished`
   handling paths of `do_work()`, before `take()`-ing old entries, remove any sleepers
   that reference those entries.

2. **Add `remove_sleeper_for_entry(entry)` method to ExecutorState**: Allows explicit
   cleanup of sleepers by entry.

3. **Guard `total_active_tasks()` with `saturating_sub`**: Prevents underflow panic/wrap.

4. **Clean packed_tasks on task removal**: In all `do_work()` branches that call `take()`,
   also call `unpack_task_and_dependents()`.

5. **Add defensive check in `wake_up()`**: Before pushing to processing queue, verify the
   entry still exists in `local_tasks`.

## Tasks

- [ ] TASK-01-01: Add `remove_sleepers_for_entry(entry: Entry)` method to `ExecutorState` that removes any `Sleepable` referencing the given entry
- [ ] TASK-01-02: Call `remove_sleepers_for_entry()` in all `SpawnFinished` paths of `do_work()` before `take()`-ing old entries (lines 823-841, 881-894, 925-935)
- [ ] TASK-01-03: Replace `local_task_count - sleeping_task_count` with `local_task_count.saturating_sub(sleeping_task_count)` in `total_active_tasks()` (`local.rs:464`)
- [ ] TASK-01-04: Add `unpack_task_and_dependents()` calls in all `do_work()` branches that call `take()` (Done, Panicked, SpawnFailed, None paths)
- [ ] TASK-01-05: Add defensive check in `wake_up()`: verify entry exists in `local_tasks` before pushing to processing queue; log warning and skip if stale
- [ ] TASK-01-06: Add tests: task returns Pending(Some(d)) then spawns child via lift — verify no panic and sleeper is cleaned up
