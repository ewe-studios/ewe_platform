---
feature: sleeper-lifecycle-safety
description: Fix stale sleeper entries that can panic the executor when tasks are combined via lifts
status: completed
priority: critical
created: 2026-05-12
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100
dependencies: []
---

# Feature 01: Sleeper Lifecycle Safety

## Problem

Three related issues stem from sleeper/task lifecycle mismatch:

### 1. Stale Sleeper Panic (CRITICAL) - FIXED

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

**Root Cause:** The `Sleepers` data structure used `EntryList` which generated new entry IDs
for each inserted sleeper. This caused entry ID collisions where sleeper entries (e.g., 3/0)
could conflict with task entries (e.g., 3/0). When task 3/0 completed, removing its sleeper
would accidentally remove task 5/0's sleeper.

**Solution:** Rewrote `Sleepers<T>` to use `HashMap<Entry, T>` keyed directly by the task entry,
eliminating entry ID collisions entirely.

### 2. `total_active_tasks()` Arithmetic Underflow (HIGH) - FIXED

`local.rs:460-473`:
```rust
let active_task_count = local_task_count - sleeping_task_count;
```

If `sleeping_task_count > local_task_count` (possible due to stale sleepers), this panics
in debug mode or wraps to `usize::MAX` in release mode — making `has_active_tasks()` return
true forever, preventing the executor from reaching "no work" state.

**Solution:** Changed to `saturating_sub` to prevent underflow.

### 3. Packed Tasks Not Cleaned on Removal (MEDIUM) - ADDRESSED

When a task finishes (`None`/`Done`/`Panicked`/`SpawnFailed`), `do_work()` removes it from
`local_tasks` via `take()` but never calls `unpack_task_and_dependents()`. Packed entries
remain in `packed_tasks` HashMap forever — a slow memory leak.

**Solution:** The pack/unpack mechanism was removed as part of simplifying the `wake_up()`
logic. The `packed_tasks` registry is now cleaned directly in `wake_up()`.

## Implementation Summary

### Changes to `sleepers.rs`

1. **Rewrote `Sleepers<T>` to use `HashMap<Entry, T>` instead of `EntryList<T>`**
   - Changed `insert(&self, wakeable: T) -> Entry` to `insert(&self, entry: Entry, wakeable: T)`
   - Sleepers are now keyed by task entry directly, preventing entry ID collisions
   - Updated all timing methods to use `HashMap::values()` instead of `EntryList` methods
   - Fixed `DurationStore::min_duration()` which incorrectly used `.max()` instead of `.min()`

2. **Added `Timeable` trait implementation for `MockSleeper`** to support timing tests

### Changes to `local.rs`

1. **Added `remove_sleepers_for_entry(&self, entry: &Entry)` method** (TASK-01-01)
   - Allows explicit cleanup of sleepers by entry

2. **Added sleeper cleanup in task removal paths** (TASK-01-02, 01-04)
   - Called in `None`, `SpawnFailed`, `Panicked`, and `Done` branches of `do_work()`
   - Prevents stale sleeper entries from accumulating

3. **Fixed `total_active_tasks()` underflow** (TASK-01-03)
   - Changed `local_task_count - sleeping_task_count` to `saturating_sub`

4. **Added defensive check in `wake_up()`** (TASK-01-05)
   - Verifies entry exists in `local_tasks` before pushing to processing queue
   - Logs warning and skips if entry is stale

5. **Simplified `wake_up()` logic**
   - Removed dependency on `packed_tasks` registry for dependents
   - Directly pushes the woken entry to processing queue

## Tasks

- [x] TASK-01-01: Add `remove_sleepers_for_entry(entry: Entry)` method to `ExecutorState`
- [x] TASK-01-02: Call `remove_sleepers_for_entry()` in all task removal paths of `do_work()`
- [x] TASK-01-03: Replace `local_task_count - sleeping_task_count` with `saturating_sub`
- [x] TASK-01-04: Add sleeper cleanup calls in all `do_work()` branches that call `take()`
- [x] TASK-01-05: Add defensive check in `wake_up()`: verify entry exists before pushing
- [x] TASK-01-06: Rewrite `Sleepers` to use `HashMap<Entry, T>` to prevent entry collisions

## Verification

All tests pass:
- 33 sleepers-specific tests
- 2 scenario_5 tests (the original failing tests)
- 406 total tests in foundation_core
