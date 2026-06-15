---
description: Add TaskStatus::Depends signal-based waiting and Worker Fairness Tracker to Valtron
status: in_progress
priority: high
created: 2026-05-15
author: "Main Agent"
context_optimization: true
metadata:
  version: '1.0'
  last_updated: 2026-05-15
  estimated_effort: large
  tags:
  - rust
  - valtron
  - executor
  - fairness
  - atomic
  - signals
  stack_files:
  - .agents/stacks/rust.md
  skills: []
  tools:
  - Rust
  - cargo
  - clippy
builds_on: []
related_specs:
- specifications/23-valtron-executor-deep-dive
- specifications/25-valtron-quality-improvements
- specifications/09-multi-threaded-executor-improvements
has_features: true
has_fundamentals: false
tasks:
  completed: 0
  uncompleted: 56
  total: 56
  completion_percentage: 0%
---

# Requirements: Valtron — TaskStatus::Depends & Worker Fairness Tracker

## Overview

Two major capabilities added to Valtron:
1. **`TaskStatus::Depends`** — signal-based task waiting via `Arc<AtomicBool>`
2. **Worker Fairness Tracker** — CAS-based global queue access control preventing starvation

See `Spec.md` for architecture, `analysis.md` for current state, `start.md` for workflow entry.

## Feature 01 — TaskStatus::Depends State

- [ ] Add `TaskStatus::Depends(Arc<AtomicBool>)` variant to enum in `task.rs`
- [ ] Update `From<TaskStatus>` for `Stream<D, P>` — map `Depends` to `Stream::Ignore`
- [ ] Update `PartialEq` impl — add `Depends` variant match `(Depends(_), Depends(_)) => true`
- [ ] Update `Display` and `Debug` impls — add `Depends` to internal `TStatus` enum
- [ ] Add `State::Depends` variant if needed (check how `State` relates to `TaskStatus` in local.rs)
- [ ] Add handler for `State::Depends` in `do_work()` in `local.rs`
- [ ] Add `depends_cycle_counts: Rc<RefCell<HashMap<Entry, u32>>>` field to `ExecutorState`
- [ ] Implement cycle counter increment logic in `schedule_and_do_work()` (Atomic sleepers only)
- [ ] Implement forced re-poll after 10,000 cycles threshold
- [ ] Add panic logic for 3 consecutive `Depends(true)` violations
- [ ] Add unit tests for `TaskStatus::Depends` lifecycle

## Feature 02 — PackedAtomic Utility

- [ ] Create `foundation_nostd/src/atomics/mod.rs` module
- [ ] Implement `AtomicPackable` trait (`to_u64`, `from_u64`)
- [ ] Implement `PackedAtomic<T: AtomicPackable>` struct with new/load/store/swap/update/compare_and_swap
- [ ] Implement `StatsSnapshot` struct with `AtomicPackable` impl (bitfield encode/decode)
- [ ] Add unit tests for `PackedAtomic<T>` — CAS retry loop, packing/unpacking
- [ ] Add unit tests for `StatsSnapshot` bitfield encode/decode

## Feature 03 — SignalWaiters Utility

- [ ] Create `foundation_core/src/synca/signal_waiters.rs`
- [ ] Implement `SignalWaiters<K>` with add/update/get_ready/count/contains
- [ ] Implement `get_ready()` — returns keys whose signal is true, atomically removes them
- [ ] Implement error handling using `foundation_errstack`
- [ ] Add unit tests for `SignalWaiters<K>` — add, update, get_ready, error cases

## Feature 04 — Worker Fairness Tracker

- [ ] Create `TimeTracker<K>` in `foundation_nostd/src/atomics/time_tracker.rs`
- [ ] Implement `Trackers` struct (in threads.rs or new file)
- [ ] Implement `Trackers::register()` returning `PackedAtomic<StatsSnapshot>`
- [ ] Implement `Trackers::unregister()`
- [ ] Implement `Trackers::can_take()` with priority rules and deadlock guarantee
- [ ] Add unit tests for `Trackers::can_take()` — all priority scenarios
- [ ] Add unit tests for `TimeTracker<K>`

## Feature 05 — Integration and Testing

- [ ] Add `Arc<Trackers>` to `ThreadRegistry`
- [ ] Pass `Arc<Trackers>` and `worker_id` to `LocalThreadExecutor` during spawn
- [ ] Add `task_timer: TimeTracker<Entry>` to `ExecutorState`
- [ ] Integrate stats update at start of `schedule_and_do_work()`
- [ ] Integrate `can_take()` gate in `request_global_task()`
- [ ] Integrate `task_timer.start/stop` around task execution
- [ ] Integrate `trackers.unregister()` on `ThreadActivity::Stopped`
- [ ] Add `worker_id` and `task_timer` to `ExecutorState::new()` and Clone
- [ ] Add integration tests for fairness mechanism under load
- [ ] Add integration tests for `TaskStatus::Depends` with real signal flipper

## Verification Commands

```bash
cargo build --package foundation_core
cargo build --package foundation_nostd
cargo clippy --package foundation_core -- -D warnings
cargo clippy --package foundation_nostd -- -D warnings
cargo fmt -- --check
cargo test --package foundation_core -- depends
cargo test --package foundation_core -- fairness
cargo test --package foundation_nostd
cargo test --package foundation_core
```

---

*Created: 2026-05-15*
*Last Updated: 2026-05-15*
*Status: In Progress*
