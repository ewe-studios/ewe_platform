---
description: "Make valtron drive real async without busy-polling: bridge Rust Waker → QueueReadiness so FutureTask parks via TaskStatus::Depends, and expose a reactor plug-in point for real I/O without pulling a reactor into foundation_core. Then let #[valtron]/#[valtron_test] accept async fn."
status: "proposed"
priority: "medium"
created: 2026-06-17
updated: 2026-06-17
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "medium"
  tags:
    - valtron
    - async
    - futures
    - waker
    - event-readiness
    - reactor
    - macros
has_features: true
has_fundamentals: true
builds_on:
  - "specifications/33-valtron-singleton"
related_specs:
  - "specifications/22-remote-valtron-tasks"
  - "specifications/33-valtron-singleton"
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# Valtron Async Readiness — Waker → QueueReadiness, Reactor Plug-in, async `#[valtron]`

## Problem

Valtron can already **run** futures: `FutureTask<F>` wraps any `Future` as a
`TaskIterator`, and `from_future` / `drive_future` / `block_on` drive them to
completion. Spec 38's async→sync bridge depends on exactly this.

But `FutureTask` polls with a **no-op waker** (`get_noop_waker()` in
`backends/foundation_core/src/valtron/executors/future_task.rs`). When the inner
future returns `Poll::Pending`, valtron just re-polls it on the next turn —
**busy-polling**. Concretely:

```rust
// future_task.rs — current behaviour
let waker = get_noop_waker();
let mut cx = Context::from_waker(&waker);
match self.future.as_mut().poll(&mut cx) {
    Poll::Ready(out) => Some(TaskStatus::Ready(out)),
    Poll::Pending    => Some(TaskStatus::Pending(FuturePollState::Pending)), // ← re-polled every turn
}
```

Consequences:
1. **No waker-driven wakeups.** A future blocked on real I/O is re-polled in a
   spin instead of being woken when the resource is ready. CPU is burned on
   tasks that cannot make progress.
2. **No reactor.** Nothing translates "the OS says this fd/timer is ready" into
   "re-schedule task N." A true async runtime (tokio/smol) provides this; valtron
   does not.

## What valtron ALREADY has (do not reinvent)

The parking machinery exists and is the backbone of this spec:

- **`EventReadiness` trait** (`task.rs`): `fn is_ready(&self, dur: Option<Duration>) -> bool`.
  The executor parks a task and only re-runs it when its readiness says ready —
  **zero CPU spinning**.
- **`TaskStatus::Depends(Arc<dyn EventReadiness>)`** (`task.rs`): a task returns
  this to say "park me until this signal is ready." Already threaded through
  every executor (`local.rs` `Sleepable::Readiness`, `on_next.rs`, `do_next.rs`,
  `collect_next.rs`, `task_iters.rs`, `wrappers.rs`).
- **`QueueReadiness<T>(Arc<ConcurrentQueue<T>>)`** (`task.rs`): an `EventReadiness`
  that is ready when the queue is **non-empty**. The executor parks the task and
  wakes it only when a message arrives. This is the preferred signal for
  producer→consumer wakeups and **it can carry a payload**, not just a flag.
- **`Sleepable::Readiness(Arc<dyn EventReadiness>, Entry)`** (`local.rs`): the
  executor's sleeper variant that holds a parked task on an `EventReadiness`.

So the missing piece is **not** new executor machinery — it is a bridge from a
Rust `Waker` to a `QueueReadiness`, plus a documented seam for an external
reactor to drive that waker.

## Design

### Core idea: a Waker that pushes into a shared queue

Build a `Waker` whose `wake()` / `wake_by_ref()` pushes a small token into a
shared `Arc<ConcurrentQueue<WakeToken>>`. `FutureTask` watches that same queue
via `QueueReadiness` and, on `Poll::Pending`, returns
`TaskStatus::Depends(QueueReadiness::new(queue))` instead of busy `Pending`.

```
        ┌────────────────────────── FutureTask ──────────────────────────┐
        │  queue: Arc<ConcurrentQueue<WakeToken>>                          │
        │  waker: Waker  (wake() => queue.push(WakeToken))                 │
        │                                                                  │
        │  next_status():                                                  │
        │    drain queue (clear stale tokens)                              │
        │    poll(future, cx = Context::from(&waker))                      │
        │      Ready(out)  -> TaskStatus::Ready(out)                       │
        │      Pending     -> TaskStatus::Depends(QueueReadiness(queue))   │
        └──────────────────────────────────────────────────────────────────┘
                                   ▲
                                   │ queue.push(token)   (wakes the parked task)
                                   │
         ┌─────────────────────────┴──────────────────────────┐
         │  Whoever holds a clone of the Waker:                │
         │   - the inner future's leaf (timer/socket/JsFuture) │
         │   - OR an external reactor (Level 2)                │
         └─────────────────────────────────────────────────────┘
```

Why a queue, not a `BoolSignal`:
- The queue **carries information**. A `WakeToken` can encode which sub-future
  woke, a readiness reason, or a sequence number — useful for select/merge
  combinators and for debugging "who woke me."
- It is naturally **multi-producer**: several leaves (or a reactor + a timer)
  can wake the same task; each push is independent, no lost-update on a bool.
- It reuses `QueueReadiness` verbatim — no new `EventReadiness` impl, no new
  executor code path.

### `WakeToken`

Keep it tiny and `Copy` to avoid allocation churn on hot wake paths:

```rust
/// Token pushed onto the wake queue when a future's waker fires.
/// `id` lets multi-future combinators tell wakers apart; default is 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeToken {
    pub id: u64,
}
```

(Start with `{ id }`; reserve room to grow without churning the queue type.)

### The Waker bridge

```rust
/// A Waker backed by a shared ConcurrentQueue. wake() pushes a WakeToken.
/// Built with RawWaker/RawWakerVTable over an Arc<ConcurrentQueue<WakeToken>>
/// so it is std/alloc-only (no new deps), matching the existing noop waker.
fn queue_waker(queue: Arc<ConcurrentQueue<WakeToken>>, id: u64) -> Waker;
```

Implementation notes:
- The vtable's data pointer is an `Arc<ConcurrentQueue<WakeToken>>` raw pointer;
  `clone` bumps the Arc, `drop` releases it, `wake`/`wake_by_ref` push a token.
  This mirrors the existing `create_noop_waker` structure — same module, same
  `#[cfg(any(std, alloc))]` gating, **no new dependency**.
- A full-queue push (bounded queue) is treated as "already signalled" — a wake
  is idempotent, so dropping a duplicate token is harmless. Prefer an unbounded
  queue for the wake channel to avoid this entirely.

### `FutureTask` change

`FutureTask<F>` gains the shared queue + cached waker and swaps the no-op waker:

```rust
pub struct FutureTask<F: Future> {
    future: Pin<Box<F>>,
    completed: bool,
    wake_queue: Arc<ConcurrentQueue<WakeToken>>,   // NEW
}

fn next_status(&mut self) -> Option<TaskStatus<F::Output, FuturePollState, NoAction>> {
    if self.completed { return None; }
    // Drain stale wake tokens so the next Pending genuinely re-parks.
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

The executor already parks on `Depends` and only re-runs the task when
`QueueReadiness::is_ready` (queue non-empty) is true. **No executor change.**

### Correctness: the "wake before park" race

A future may call `wake()` between our `poll` returning `Pending` and the
executor parking the task. Because the waker **pushes into the same queue** the
`QueueReadiness` watches, a wake that lands before parking leaves the queue
non-empty, so `is_ready` returns `true` immediately and the executor does not
park (or unparks at once). This is the same safe pattern as a condvar predicate
under the lock — the shared queue is the single source of truth, so no wakeup is
lost. (Draining stale tokens at the *top* of `next_status`, before polling,
ensures we don't treat a previous turn's token as a fresh wake.)

## Level 2 — Reactor plug-in WITHOUT pulling a reactor into foundation_core

Level 1 makes valtron park futures correctly. But a leaf future blocked on a
real OS resource (socket, timer) only makes progress if *something* calls its
waker when the resource is ready. On wasm the browser event loop is that
something (a `JsFuture`'s waker is driven by JS), so **Level 1 alone is enough
on wasm**. Native needs a reactor — and we must NOT bring `mio`/`polling`/tokio
into `foundation_core`.

### Seam: `foundation_core` defines the trait, owns no reactor

`foundation_core` exposes a minimal trait and a registration hook; the actual
reactor lives in a platform crate (e.g. `foundation_netio` / `foundation_nativeapis`)
and registers itself. The wake path is the SAME queue mechanism from Level 1.

```rust
// foundation_core::valtron — trait only, no implementation, no deps.
/// A source that can arrange to push a WakeToken onto a task's wake queue when
/// some external resource (fd, timer, JS promise) becomes ready.
pub trait ReadinessSource: Send + Sync {
    /// Register interest: when `interest` becomes ready, push a token onto `wake`.
    /// Returns a handle the task drops to deregister.
    fn register(
        &self,
        interest: Interest,
        wake: Arc<ConcurrentQueue<WakeToken>>,
        token: WakeToken,
    ) -> Box<dyn ReadinessRegistration>;
}

pub trait ReadinessRegistration: Send + Sync {} // RAII deregister on drop

/// What a task is waiting for. Kept abstract so foundation_core needs no OS types.
pub enum Interest { Readable, Writable, Timer(Duration), /* opaque/custom */ Custom(u64) }
```

- **foundation_core owns:** `ReadinessSource` + `ReadinessRegistration` traits,
  `Interest`, and an optional process-global `OnceLock<Arc<dyn ReadinessSource>>`
  registration slot (same pattern as the existing pool singletons). Nothing
  platform-specific, no new dependency.
- **A platform crate owns the reactor:** e.g. `foundation_netio` implements
  `ReadinessSource` over `polling`/`mio` (native) and registers it once at
  startup. When an fd is ready it pushes the `WakeToken` — driving the exact
  same Level-1 queue. On wasm, the `JsFuture` waker already does this; no native
  reactor is registered.
- **foundation_core has no idea what a socket is.** It only knows "a task
  depends on a queue, and some registered `ReadinessSource` promises to push to
  that queue." This is the whole point: the reactor plugs in, it is not pulled in.

### Who calls register?

The leaf future that knows its resource (e.g. `foundation_netio`'s socket
future) calls `ReadinessSource::register` in its own `poll` when it returns
`Pending`, handing it the `FutureTask`'s wake queue (obtained via the `Waker`'s
data, or via a context extension — see Open Questions). It drops the
registration handle when it completes. `foundation_core` never touches the fd.

## Level 3 — `#[valtron]` / `#[valtron_test]` accept `async fn`

Once Level 1 lands, the macro change is mechanical. Today the macro REJECTS
`async fn` (`valtron_entry.rs`: "valtron functions are synchronous"). Change it
to wrap an async body as a future, schedule it, and drive to completion:

```rust
// async fn body  →  (conceptually)
let __out = {
    let __fut = async move { #block };
    // single-threaded: run_until_complete + collect; multi: block_on/execute
    collect_one(execute(from_future(__fut), None)).expect("future produced no value")
};
__out
```

- Sync bodies keep today's behaviour exactly.
- Async bodies are wrapped so `return`/`?` still mean "return from the test/fn".
- `seed` / `threads` args are unchanged.
- The "reject async" error is replaced by the wrapping path.

This finally delivers the user-facing payoff: `#[valtron_test] async fn …` and
`#[valtron] async fn main()` Just Work, with futures that park instead of spin.

## Scope boundaries

- **In scope:** the Waker→QueueReadiness bridge (Level 1), the `ReadinessSource`
  seam in foundation_core (Level 2 trait only), and the async macro acceptance
  (Level 3).
- **Out of scope:** shipping a native reactor implementation. That is a separate
  spec in a platform crate (`foundation_netio`). This spec only proves the seam
  with an in-tree test reactor (a thread that pushes a token after a delay) and
  the wasm `JsFuture` path.
- **Out of scope:** changing the busy-poll fallback's semantics for callers who
  pass a future that never registers a waker — those still make progress via
  re-poll on each turn (we keep that as the safe default when no token ever
  arrives, guarded so it does not spin hot — see Open Questions).

## Open Questions (resolve during feature design)

1. **Waker → wake-queue handle plumbing.** A leaf future gets a `&Waker` in its
   `Context`, not our queue. Options: (a) the leaf clones the standard waker and
   our `queue_waker` is what it wakes (works automatically — the leaf calls
   `cx.waker().wake()`, which pushes our token; **preferred, zero plumbing**);
   (b) a `Context` extension to pass the queue explicitly (more invasive). Lean
   on (a): any well-behaved future already wakes via the `Context` waker we
   supply, so Level 1 needs NO cooperation from leaf futures.
2. **Stale-token draining vs. lost wake.** Drain at the top of `next_status`
   before polling (a token from a prior turn is stale once we re-poll). A wake
   that arrives *during* our poll lands after the drain → queue non-empty →
   re-scheduled. Confirm with a stress test.
3. **No-waker futures (pure busy-poll).** If a future returns `Pending` and never
   wakes (some hand-rolled futures), the queue stays empty and the task parks
   forever. Mitigation: keep a bounded re-poll fallback (e.g. `Depends` with a
   readiness that also times out, reusing `Sleepable::Timable`) OR document that
   futures must wake via the context waker. Decide per the executor's existing
   timed-sleeper support.
4. **Single vs multi executor parity.** Wire and test the bridge on both the
   `single` (wasm) and `multi` (native) executors; both already handle `Depends`.

## Feature Index

| Feature | Description | Depends on |
|---|---|---|
| [01-waker-queue-bridge](features/01-waker-queue-bridge/) | `WakeToken`, `queue_waker`, `FutureTask` returns `Depends(QueueReadiness)` instead of busy `Pending`; single + multi parity tests | — |
| [02-readiness-source-seam](features/02-readiness-source-seam/) | `ReadinessSource`/`ReadinessRegistration`/`Interest` traits + optional global registration slot in foundation_core; in-tree test reactor proving the seam (no real reactor) | F01 |
| [03-async-valtron-macros](features/03-async-valtron-macros/) | `#[valtron]`/`#[valtron_test]` accept `async fn` by wrapping the body as a driven future; sync path unchanged | F01 |

## Success Criteria

- [ ] A `Pending` future parks via `TaskStatus::Depends(QueueReadiness)` — no
      re-poll until a token lands on its wake queue (verified: CPU/turn count
      does not grow while a future is blocked).
- [ ] A future woken via the context waker is re-scheduled promptly (token lands,
      `is_ready` true, executor re-runs it).
- [ ] No lost wakeup under a wake-before-park stress test.
- [ ] `foundation_core` gains the `ReadinessSource` seam with **no new
      dependency**; an in-tree test reactor drives a future to completion through it.
- [ ] `#[valtron_test] async fn` and `#[valtron] async fn` compile and run;
      `?`/`return` behave; sync forms unchanged.
- [ ] Works on both `single` (wasm) and `multi` (native) executors.

## Module References

- `backends/foundation_core/src/valtron/executors/future_task.rs` — `FutureTask`, noop waker → queue waker
- `backends/foundation_core/src/valtron/task.rs` — `EventReadiness`, `QueueReadiness`, `TaskStatus::Depends`
- `backends/foundation_core/src/valtron/executors/local.rs` — `Sleepable::Readiness` (executor park/wake)
- `backends/foundation_core/src/valtron/executors/sendables.rs` / `non_sendables.rs` — `from_future`, `drive_future`
- `backends/foundation_macros/src/valtron_entry.rs` — `#[valtron]` / `#[valtron_test]` async acceptance

## Language Stack

- **Rust** — all implementation

---

_Created: 2026-06-17_
