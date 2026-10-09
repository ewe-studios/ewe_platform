---
feature: "Waker → QueueReadiness Bridge"
description: "FutureTask parks via TaskStatus::Depends(QueueReadiness) instead of busy-polling with a no-op waker"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "medium"
created: 2026-06-17
---

# Feature 01: Waker → QueueReadiness Bridge

## Description

Replace `FutureTask`'s no-op-waker busy-poll with a real waker that pushes a
token into a shared `ConcurrentQueue`, and return
`TaskStatus::Depends(QueueReadiness::new(queue))` on `Pending` so the executor
parks the task until a wake token arrives. Reuses the existing
`EventReadiness` / `QueueReadiness` / `Depends` machinery — **no executor
change**.

## Current state (`future_task.rs`)

```rust
let waker = get_noop_waker();              // does nothing on wake()
let mut cx = Context::from_waker(&waker);
match self.future.as_mut().poll(&mut cx) {
    Poll::Ready(out) => Some(TaskStatus::Ready(out)),
    Poll::Pending    => Some(TaskStatus::Pending(FuturePollState::Pending)), // re-polled each turn
}
```

## New types

```rust
/// Token pushed onto a future's wake queue when its waker fires.
/// `id` distinguishes wakers in multi-future combinators (default 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeToken {
    pub id: u64,
}
```

## Waker bridge

```rust
/// A Waker backed by Arc<ConcurrentQueue<WakeToken>>. wake()/wake_by_ref()
/// push a WakeToken; clone bumps the Arc; drop releases it.
/// Built with RawWaker/RawWakerVTable — std/alloc only, NO new dependency,
/// same structure/cfg-gating as the existing create_noop_waker().
fn queue_waker(queue: Arc<ConcurrentQueue<WakeToken>>, id: u64) -> Waker;
```

VTable mapping:
- data pointer = `Arc::into_raw(queue)` (an `Arc<ConcurrentQueue<WakeToken>>`)
- `clone` = `Arc::increment_strong_count` then new `RawWaker`
- `wake` = reconstruct Arc, `queue.push(WakeToken { id })`, consume the Arc
- `wake_by_ref` = borrow Arc, push, `forget` (don't drop the borrow's count)
- `drop` = `Arc::from_raw` and drop

(Mirror the exact null-safe pattern already used by `create_noop_waker`.)

## FutureTask change

```rust
pub struct FutureTask<F: Future> {
    future: Pin<Box<F>>,
    completed: bool,
    wake_queue: Arc<ConcurrentQueue<WakeToken>>,   // NEW (unbounded)
}

impl FutureTask<F> {
    pub fn new(future: F) -> Self {
        Self {
            future: Box::pin(future),
            completed: false,
            wake_queue: Arc::new(ConcurrentQueue::unbounded()),
        }
    }
}

fn next_status(&mut self) -> Option<TaskStatus<F::Output, FuturePollState, NoAction>> {
    if self.completed { return None; }
    // Drain stale tokens BEFORE polling so a token from a prior turn doesn't
    // masquerade as a fresh wake. A wake landing DURING poll arrives after this
    // drain → queue non-empty → re-scheduled (no lost wakeup).
    while self.wake_queue.pop().is_ok() {}
    let waker = queue_waker(self.wake_queue.clone(), 0);
    let mut cx = Context::from_waker(&waker);
    match self.future.as_mut().poll(&mut cx) {
        Poll::Ready(out) => { self.completed = true; Some(TaskStatus::Ready(out)) }
        Poll::Pending => Some(TaskStatus::Depends(
            Arc::new(QueueReadiness::new(self.wake_queue.clone())),
        )),
    }
}
```

Apply to BOTH impls in `future_task.rs`:
- the `#[cfg(all(not(feature = "multi"), ...))]` !Send impl (wasm/single)
- the `#[cfg(feature = "multi")]` Send impl (native)

Both already exist; only the waker + return arm change. `WakeToken` and
`queue_waker` are shared (not behind `multi`).

## Why QueueReadiness over BoolSignal

- Carries information (which waker, a reason, a seq) — useful for select/merge.
- Multi-producer safe: several leaves can wake the same task; each push is
  independent (no lost-update like a bool flip).
- Reuses `QueueReadiness` verbatim — no new `EventReadiness` impl.

## No-waker future fallback (Open Question 3)

Some hand-rolled futures return `Pending` without ever waking. With a pure
park-on-`Depends` they'd hang. Mitigation options (pick during impl):
- (a) `Depends` with a readiness that ALSO carries a timeout, reusing the
  executor's `Sleepable::Timable`, so a never-waking future is re-polled after a
  bounded delay (cooperative, not hot-spin).
- (b) Document that futures driven by valtron must wake via the context waker.

Prefer (a) for robustness: park primarily on the queue, fall back to a bounded
timed re-poll. Confirm the executor supports a combined readiness+timer sleeper;
if not, keep the timer as a secondary `Depends` cycle.

## Module changes

- `backends/foundation_core/src/valtron/executors/future_task.rs` — add
  `WakeToken`, `queue_waker`, `wake_queue` field, swap waker + Pending→Depends
- Re-export `WakeToken` from `valtron` module root if useful to combinators

## Testing (use `#[valtron_test]` — never `#[test]`/`#[serial]`)

- A future that is `Pending` then woken via `cx.waker().wake()` re-runs and
  completes (token lands → `QueueReadiness::is_ready` true → re-scheduled).
- A blocked future does NOT re-poll while no token is present — assert the
  inner future's `poll` call-count stays flat over many executor turns.
- Wake-before-park stress: spawn many futures that wake from another thread the
  instant they return Pending; all complete, none lost.
- An already-ready future (`Poll::Ready` on first poll) still returns
  `TaskStatus::Ready` unchanged.
- Parity: run the suite on both single (wasm cfg) and multi executors.
