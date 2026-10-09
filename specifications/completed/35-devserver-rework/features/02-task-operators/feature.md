---
feature: "Task Operators"
description: "Replace async Operator trait with valtron TaskIterator — all devserver components become TaskIterators"
status: "complete"
priority: "high"
depends_on: ["01-crate-scaffolding"]
estimated_effort: "medium"
created: 2026-06-01
last_updated: "2026-06-15"
---

# Feature: Task Operators

## Problem

Current devserver uses an `Operator` trait:
```rust
pub trait Operator {
    fn run(&self, cancel_signal: broadcast::Receiver<()>) -> JoinHandle<()>;
}
```

Each `Operator` implementation uses `tokio::spawn` internally and returns a `JoinHandle`. `ParrellelOps` and `SequentialOps` combine operators using `futures::future::join_all` and `tokio::spawn`.

This entire model depends on tokio's async runtime.

## Solution

Replace the `Operator` model with valtron's `TaskIterator` trait:

```rust
pub trait TaskIterator {
    type Pending;
    type Ready;
    type Spawner: ExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
}
```

Each devserver component becomes a `TaskIterator` that:
1. Performs one unit of work per `next_status()` call
2. Returns `TaskStatus::Depends(EventReadiness)` to park until a signal fires (preferred — zero CPU spinning)
3. Returns `TaskStatus::Wait(duration)` for short-duration polls (e.g. reload timer, <10ms)
4. Returns `TaskStatus::Ready(result)` when work is complete
5. Returns `TaskStatus::Spawn(action)` to spawn child tasks
6. Returns `None` to terminate

### Coordination Model

Instead of `ParrellelOps` which `tokio::spawn`s all operators concurrently, the `DevService` spawns all component tasks into valtron, and they communicate through:
- **`concurrent_queue::ConcurrentQueue`** for producer→consumer event streams
- **`foundation_core::synca`** primitives for signals and barriers
- **`valtron::run_background_job`** for blocking operations (cargo build) — offloads to `BackgroundJobRegistry` pool, task returns `Depends(QueueReadiness)` to park while work runs

### Task Breakdown

1. [ ] Define the component TaskIterator types (no implementations yet)
2. [ ] Replace `ParrellelOps` with valtron spawn + queue coordination
3. [ ] Replace `SequentialOps` with valtron sequenced/lift spawns
4. [ ] Replace `cancel_signal: broadcast::Receiver<()>` with `synca` signals
5. [ ] Write `ServiceCoordinator` TaskIterator that manages component lifecycle

### Before → After Mapping

| Before (async Operator) | After (TaskIterator) |
|------------------------|---------------------|
| `Operator::run(signal) -> JoinHandle` | `TaskIterator::next_status() -> TaskStatus` |
| `tokio::spawn(async move { loop { ... } })` | `loop { yield Depends(signal) or Wait(ms); }` |
| `tokio::select! { event = rx.recv() => ..., _ = signal.recv() => ... }` | `Depends(QueueReadiness)` — executor parks, wakes on queue message |
| `tokio::process::Command.output().await` | `run_background_job` → push to queue → `Depends(QueueReadiness)` |
| `ParrellelOps::run(signal)` | valtron engine spawns each as separate task |
| `broadcast::channel(2)` for events | `ConcurrentQueue` + `QueueReadiness` |
| `signal.resubscribe()` for new listeners | `broadcaster.subscribe()` per subscriber |

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/operators.rs` | Create — TaskIterator coordination helpers |
| `backends/foundation_toolings/src/service/coordinator.rs` | Create — ServiceCoordinator TaskIterator |

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Coordination | valtron spawn + queues | No async runtime needed; valtron provides scheduling |
| Cancellation | synca signals | foundation_core already provides cancellation primitives |
| Parallelism | valtron engine | Each TaskIterator runs as a valtron task, scheduled by the engine |

---

_Created: 2026-06-01_
