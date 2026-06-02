---
feature: "WaitGroup and PoolGuard"
description: "Implement WaitGroup for thread completion tracking and PoolGuard as drop-based lifecycle handle"
status: "completed"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-03-23
completed: 2026-03-26
author: "Main Agent"
tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
---

# WaitGroup and PoolGuard

## WHY: Problem Statement

The current `ThreadPool` has a complex kill/wait lifecycle:
- `kill()` sends signal then blocks on a `kill_latch` CondVar
- `run_until()` runs the activity loop, joins threads, then signals the `kill_latch`
- You MUST NOT call `kill()` and `run_until()` on the same thread (deadlock)
- Tests need to spawn a separate thread just to call `kill()` after a timeout

We need:
1. **WaitGroup** — A counting barrier that blocks until N threads report completion. Uses existing `LockSignal` for blocking (no new primitives).
2. **PoolGuard** — A drop handle returned by `initialize_pool`. On drop: signals kill via `OnSignal::turn_on()`, wakes all threads via `LockSignal::signal_all()`, blocks via `WaitGroup::wait()` until all threads are dead, then joins handles. Tests can just drop the guard or call `shutdown()`.

---

## WHAT: Solution

### WaitGroup

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use crate::synca::LockSignal;

/// Tracks N outstanding work items. `wait()` blocks until count reaches 0.
///
/// Uses the existing `LockSignal` (CondVar-based) for the blocking mechanism.
/// Each worker thread gets a `WaitGroupGuard` that calls `done()` on drop,
/// ensuring cleanup even if the thread panics.
#[derive(Clone)]
pub struct WaitGroup {
    count: Arc<AtomicUsize>,
    signal: Arc<LockSignal>,
}

impl WaitGroup {
    pub fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
            signal: Arc::new(LockSignal::new()),
        }
    }

    /// Increment the counter by n.
    pub fn add(&self, n: usize) {
        self.count.fetch_add(n, Ordering::SeqCst);
    }

    /// Decrement the counter. If it reaches 0, signal all waiters.
    pub fn done(&self) {
        let prev = self.count.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            // count just reached 0
            self.signal.signal_all();
        }
    }

    /// Block until count reaches 0.
    pub fn wait(&self) {
        loop {
            if self.count.load(Ordering::SeqCst) == 0 {
                return;
            }
            self.signal.lock_and_wait();
        }
    }

    /// Create a RAII guard that calls `done()` on drop.
    pub fn guard(&self) -> WaitGroupGuard {
        WaitGroupGuard(self.clone())
    }
}

/// Calls `WaitGroup::done()` on drop — ensures threads that panic still decrement.
pub struct WaitGroupGuard(WaitGroup);

impl Drop for WaitGroupGuard {
    fn drop(&mut self) {
        self.0.done();
    }
}
```

### PoolGuard

```rust
use std::sync::atomic::AtomicBool;
use crate::synca::OnSignal;

/// Returned by `initialize_pool`. Provides deterministic cleanup.
///
/// On drop (or explicit `shutdown()` call):
/// 1. `OnSignal::turn_on()` — broadcast kill to all threads
/// 2. `LockSignal::signal_all()` — wake any blocked/parked threads
/// 3. `WaitGroup::wait()` — block until all threads report death
/// 4. Join all `JoinHandle`s
///
/// This replaces the old pattern of spawning a thread to call `get_pool().kill()`.
pub struct PoolGuard {
    registry: Arc<ThreadRegistry>,
    shut_down: AtomicBool,
}

impl PoolGuard {
    pub fn new(registry: Arc<ThreadRegistry>) -> Self {
        Self {
            registry,
            shut_down: AtomicBool::new(false),
        }
    }

    /// Explicit shutdown. Idempotent — safe to call multiple times.
    pub fn shutdown(&self) {
        if self.shut_down.compare_exchange(
            false, true, Ordering::SeqCst, Ordering::Relaxed
        ).is_ok() {
            self.registry.shutdown();
        }
    }
}

impl Drop for PoolGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}
```

---

## Tasks

- [ ] Implement `WaitGroup` struct using `LockSignal` for wait/signal
- [ ] Implement `WaitGroupGuard` RAII wrapper that calls `done()` on drop
- [ ] Implement `PoolGuard` with `shutdown()` and `Drop`
- [ ] Test: `WaitGroup` add(3)/done()/done()/done() unblocks `wait()`
- [ ] Test: `WaitGroupGuard` drop calls `done()` even during panic (`catch_unwind`)
- [ ] Test: `PoolGuard::shutdown()` is idempotent (call twice, no panic)
- [ ] Test: `PoolGuard` drop triggers shutdown sequence

## Files Changed

- `backends/foundation_core/src/valtron/executors/threads.rs` (new types)

## Verification

```bash
cargo test --package foundation_core -- waitgroup
cargo test --package foundation_core -- pool_guard
cargo clippy --package foundation_core -- -D warnings
```
