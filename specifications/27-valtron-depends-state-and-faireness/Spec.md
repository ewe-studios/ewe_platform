# Spec: Valtron — TaskStatus::Depends State & Worker Fairness Tracker

## Overview

This specification adds two major capabilities to Valtron, the platform's task execution engine.

### Feature A: `TaskStatus::Depends` — Signal-based Task Waiting

Tasks can yield with a `TaskStatus::Depends(Arc<AtomicBool>)`, allowing them to wait for an external signal rather than a timed duration. This solves the problem where `TaskStatus::Delayed(duration)` may undershoot or overshoot the condition a task needs to wait for, since timing may not be the right communication mechanism.

**Problem statement (user):**
> "A task may not always and evidently correctly perform well if it uses TaskStatus::Delayed when it may undershoot or overshoot the condition it needs to wait for, timing may not be the issue or the best way to communicate this to the execution engine."
>
> "Worst if we depend on timing, we may miss important signals that require low latency executions. But we do not want to bring in more heavy burden logic for these even channels or mutex guarded condvars."

### Feature B: Worker Fairness Tracker

A CAS-based fairness mechanism that controls which workers can take tasks from the global queue, preventing starvation when some workers' tasks sleep for extended periods while others become overloaded.

**Problem statement (user):**
> "Every task a thread takes on goes to sleep for an extended period of time till it ends up taking a lot more tasks than most other threads, starving others of work."

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        ThreadRegistry                             │
│  shared_tasks: SharedTaskQueue                                    │
│  latch: Arc<LockSignal>                                           │
│  kill_signal: Arc<OnSignal>                                       │
│  waitgroup: WaitGroup                                             │
│  trackers: Arc<Trackers>          ← NEW: fairness tracker          │
│  thread_handles: HashMap<ThreadId, JoinHandle>                    │
├──────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────┐  ┌─────────────────────────┐        │
│  │  Worker 1 (ThreadId)     │  │  Worker 2 (ThreadId)     │        │
│  │                          │  │                          │        │
│  │  ExecutorState           │  │  ExecutorState           │        │
│  │  ├─ local_tasks          │  │  ├─ local_tasks          │        │
│  │  ├─ processing           │  │  ├─ processing           │        │
│  │  ├─ sleepers             │  │  ├─ sleepers             │        │
│  │  │   ├─ Timable          │  │  │   ├─ Timable          │        │
│  │  │   └─ Atomic ← EXISTS  │  │  │   └─ Atomic ← EXISTS  │        │
│  │  ├─ depends_cycle_counts │  │  ├─ depends_cycle_counts │  NEW   │
│  │  ├─ task_timer           │  │  ├─ task_timer           │  NEW   │
│  │  ├─ worker_id: ThreadId  │  │  ├─ worker_id: ThreadId  │  NEW   │
│  │  └─ trackers: Arc<T>     │  │  └─ trackers: Arc<T>     │  NEW   │
│  │                          │  │                          │        │
│  │  schedule_and_do_work()  │  │  schedule_and_do_work()  │        │
│  │    1. wakeup_ready_sleepers()                           │        │
│  │    2. increment depends_cycle_counts                    │        │
│  │    3. force-repoll >10k cycles                          │        │
│  │    4. stats.update()                                    │        │
│  │    5. request_global_task() → trackers.can_take()       │        │
│  └──────────────────────────┘  └──────────────────────────┘        │
└──────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│  foundation_nostd (standalone utilities)                          │
│  ├─ PackedAtomic<T: AtomicPackable>  — CAS wrapper over AtomicU64│
│  └─ TimeTracker<K>                   — elapsed time tracking     │
├──────────────────────────────────────────────────────────────────┤
│  foundation_core (platform utilities)                             │
│  └─ SignalWaiters<K>                   — generic signal manager   │
│    (NOT used by valtron executor; standalone utility)              │
└──────────────────────────────────────────────────────────────────┘
```

### Existing Architecture (Key Finding)

**Critical discovery:** `Sleepable::Atomic(Arc<AtomicBool>, Entry)` already exists in `local.rs:50-58` with a proper `Waiter` impl that loads the atomic bool with `SeqCst` ordering. `Sleepers<Sleepable>` already handles it. What is missing is the wire from a `TaskStatus` variant to registration. This spec reuses the existing infrastructure — no new sleeper construct needed.

### Task Flow: `TaskStatus::Depends`

```
Task returns TaskStatus::Depends(signal)
  │
  ├── signal is true?  → PANIC tracking (3 consecutive → PANIC)
  │                     → Treat as Pending(None), push to BACK of queue
  │
  └── signal is false?
        │
        ├── Task already has sleeper record? → PANIC (should be awake)
        │
        └── Register as Sleepable::Atomic(signal, entry)
              → Remove from processing queue
              → Sleepers manages wake timing
              → Increment cycle counter (Atomic sleepers ONLY)
              → After 10,000 cycles → forced re-poll
```

### Fairness Flow: `Trackers::can_take()`

```
Worker calls request_global_task()
  │
  ├── Has active tasks? → CanProgress(None), skip fairness check
  │
  └── No active tasks
        │
        └── trackers.can_take(&worker_id)
              │
              ├── Read all workers' StatsSnapshots
              │
              ├── Identify candidates (active_tasks == 0)
              │
              ├── ALL candidates idle? (total_waiters==0 AND total_tasks==0)
              │     → Return true for ALL (deadlock guarantee)
              │
              └── Pick winner:
                    1. lowest total_waiters wins
                    2. tiebreak: lowest total_tasks wins
                    3. tiebreak: lowest elapsed_ms wins
                    │
                    └── Is asking worker the winner? → true : false
```

## Design Discussion

### Reuse of Existing `Sleepable::Atomic`

**Key decision:** No new `SignalWaiters` construct needed inside the executor. The existing `Sleepers<Sleepable>` already supports `Sleepable::Atomic(Arc<AtomicBool>, Entry)` with proper `Waiter` impl. We simply wire `TaskStatus::Depends` to register tasks as `Sleepable::Atomic` sleepers.

User agreed: "Solid i like this, sounds good"

### Inverted `update()` API

Original spec said `update()` should "ensuring the key does exists before, else returns an Err() if key already is taken" — which was contradictory. Clarified:

- `add()` → Err if key already exists
- `update()` → Err if key does NOT exist

User agreed: "Ok make sense"

### Re-poll Invariant

User clarified: "a task would not be polled until its atomic is true, so if it later returns another TaskStatus::Depends then it must be: 1. After its previous one has become true and removed 2. It truly after getting polled indicate it still wants to wait for another signal and provides a new AtomicBool, if it is the same as last then we treat it as pending and just not register it and ensure its previous sleeper record was removed."

Decision: PANIC if a task that should be asleep sends another `State::Depends` — loud fail.

### SignalWaiters as Standalone Utility

User: "i like those suggestions for the first idea, but lets also add this construct but not use it, just implement it, add tests so it can be used for other usecases, we can add it to foundation_nostd"

### Stream Conversion

User: "yes, for this we will just swap it with Stream::Ignore, streams should not care about this, its internal to the worker and communicating with the LocalTaskExecutor."

### Fairness Complexity Correction

I initially suggested workers hold at most 2 tasks. User corrected: "No you are wrong, a worker can theoretically keep taking on more tasks if every tasks keeps going to sleep for extended period of time, that why we have this new faireness mechanism."

### AtomicValue vs PackedAtomic

I proposed `AtomicValue<AtomicU64>`. User asked: "can AtomicValue take a generic struct?" Explained Rust only supports CAS on fixed-width atomics. User: "if not, we might as well jsut use AtomicU64 directly." Then: "I want this in a struct construct that owns and has a nice API with methods to make this easy and encapsulated away."

User wanted: "the construct in foundation_nostd should not care and just require input to have a to_u64 method from a trait it defines, this way anyone can use this construct whenever." Renamed from `AtomicValue` to `PackedAtomic<T>` to avoid conflict with std.

### TaskTimeWatcher Discussion

I suggested dropping it. User corrected: "Correction, we dont care for tasks to know each others duration, the worker tracks how long a tasks really takes to finish, both for metrics but also for better enhancement for us long term."

### Worker Death Detection

User: "Worker and worker thread is used synonymously, you should be able to see that."

### Trackers Location

User: "Dont be stupid, every LocalExecutor has a ThreadId, see line 1624 in threads.rs. Yes, we pass the trackers to each LocalThreadExecutor, ThreadReggistry should own the tracker."

### Cycle Nudging

I proposed 1000 cycles. User: "I fear 1000 is too short and we are being overly nosy, maybe make it 10k cycles then nudge." Also: "we dont do this for duration sleepers, so we need to ensure its only for signal watchers this ever happens for."

### Signal Already True = Panic

User: "a task should not be sending us AtomicBool that is now true, and it must be false, else its just considered another TaskStatus::Pending, I think we can track how many times a task is doing this and just panic. I will prefer a loud fail that gets reported and forces people to go fix their stupidity than try to mitigate this."

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEPENDS_ZOMBIE_CYCLE_THRESHOLD` | 10,000 | Max cycles a signal watcher sleeps before forced re-poll |
| `DEPENDS_TRUE_PANIC_THRESHOLD` | 3 | Consecutive `Depends(true)` violations before panic |

## File Changes

| File | Change |
|------|--------|
| `backends/foundation_core/src/valtron/task.rs` | Add `TaskStatus::Depends`, update `Stream`, `PartialEq`, `Display`, `Debug` |
| `backends/foundation_core/src/valtron/executors/local.rs` | Handle `State::Depends` in `do_work`, add `depends_cycle_counts`, integrate fairness gate in `request_global_task` |
| `backends/foundation_core/src/valtron/executors/threads.rs` | Add `Arc<Trackers>` to registry, pass to workers, unregister on death |
| `backends/foundation_core/src/synca/mod.rs` | Export `SignalWaiters` |
| `backends/foundation_core/src/synca/signal_waiters.rs` | New file — `SignalWaiters<K>` utility |
| `backends/foundation_nostd/src/lib.rs` | Export `PackedAtomic`, `AtomicPackable`, `TimeTracker` |
| `backends/foundation_nostd/src/atomics/packed.rs` | New file — `PackedAtomic<T>` + `AtomicPackable` trait |
| `backends/foundation_nostd/src/atomics/time_tracker.rs` | New file — `TimeTracker<K>` |
| `backends/foundation_nostd/src/atomics/mod.rs` | New file — module |

## Feature Dependency Graph

```
01-taskstatus-depends-state  (can be done independently)
02-packed-atomic-utility     (can be done independently, foundation_nostd)
03-signal-waiters-utility    (can be done independently, foundation_core)
04-worker-fairness-tracker   (depends on 02 — needs PackedAtomic<StatsSnapshot>)
05-integration-and-testing   (depends on 01, 04 — wires everything together)

Implementation order:

01-taskstatus-depends-state ──┐
02-packed-atomic-utility      │
03-signal-waiters-utility     │
                              │
04-worker-fairness-tracker ───┘ (depends on 02)
                              │
05-integration-and-testing ───┘ (depends on 01, 04)
```

## Features

| # | Feature | Depends On | Effort | Description |
|---|---------|-----------|--------|-------------|
| 01 | [TaskStatus::Depends State](features/01-taskstatus-depends-state/feature.md) | — | Large | Add Depends variant, wire to Sleepable::Atomic, zombie detection |
| 02 | [PackedAtomic Utility](features/02-packed-atomic-utility/feature.md) | — | Medium | Generic CAS wrapper over AtomicU64 with AtomicPackable trait |
| 03 | [SignalWaiters Utility](features/03-signal-waiters-utility/feature.md) | — | Small | Standalone signal waiter map (not used by valtron) |
| 04 | [Worker Fairness Tracker](features/04-worker-fairness-tracker/feature.md) | 02 | Large | Trackers with StatsSnapshot bitfield, can_take priority rules |
| 05 | [Integration and Testing](features/05-integration-and-testing/feature.md) | 01, 04 | Large | Wire trackers, task_timer, fairness gate, integration tests |

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

## Agent Rules Reference

### Mandatory Rules
- `.agents/rules/01-rule-naming-and-structure.md`
- `.agents/rules/02-rules-directory-policy.md`
- `.agents/rules/03-dangerous-operations-safety.md`
- `.agents/rules/04-work-commit-and-push-rules.md`

### Role-Specific
- `.agents/rules/13-implementation-agent-guide.md`
- `.agents/stacks/rust.md`

## File Organization

1. `Spec.md` — This file
2. `analysis.md` — Architecture analysis, gap analysis, existing infrastructure review
3. `start.md` — Agent workflow entry point
4. `requirements.md` — Task tracking with frontmatter
5. `features/` — Feature specifications (one directory per feature)
6. `idea.md` — Original idea document (pre-existing, do not delete)

---

*Created: 2026-05-15*
*Last Updated: 2026-05-15*
*Status: In Progress*
