---
feature: "State task iterators"
description: "Two native TaskIterator types — StateTask (sync) and AsyncStateTask (async) — that own a Ctx and a state T and drive a transition function turn by turn, plus the TransitionPending wrapper."
status: "pending"
priority: "high"
depends_on: ["00-ctx"]
estimated_effort: "medium"
created: 2026-10-18
last_updated: 2026-10-18
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
---

# State task iterators

## Overview

`StateTask` (sync) and `AsyncStateTask` (async) are the native core primitives.
Both own a `Ctx` and an `Option<T>` state, and advance the state by calling a
transition function once per `next_status`. They are the direct analogue of
`FutureTask`/`StreamTask`: first-class `TaskIterator` types in `executors/`,
composable with the existing builders and combinators, and usable with a plain
closure or fn pointer — no macro required.

## Common contract

The transition's shape is identical for both, and is the source of truth the
macro (feature 02) reads:

```
fn transition(ctx: Ctx, state: T) -> Option<(T, TaskStatus<R, P, S>)>
```

- `ctx: Ctx` — a cheap clone of the task's handle; the transition owns it.
- `state: T` — the current state, moved in.
- `Some((next, status))` — store `next` for the next turn, emit `status` this turn.
- `None` — task finished; `next_status` returns `None` from then on.

`R` is `TaskIterator::Ready` (the terminal value), `P` is `Pending` (progress),
`S` is `Spawner` (the `ExecutionAction` for `TaskStatus::Spawn`). The internal
`state: Option<T>` doubles as the termination flag: `None` ⇒ finished.

## StateTask (sync)

```rust
// executors/state_task.rs

pub struct StateTask<T, R, P, S, F> {
    ctx: Ctx,
    state: Option<T>,
    transition: F,                 // FnMut(Ctx, T) -> Option<(T, TaskStatus<R, P, S>)>
    _marker: PhantomData<fn(R, P, S)>,
}

impl<T, R, P, S, F> StateTask<T, R, P, S, F> {
    pub fn new(state: T, transition: F) -> Self;
    pub fn with_ctx(state: T, ctx: Ctx, transition: F) -> Self;
}

impl<T, R, P, S, F> TaskIterator for StateTask<T, R, P, S, F>
where
    F: FnMut(Ctx, T) -> Option<(T, TaskStatus<R, P, S>)>,   // + Send under multi
{
    type Ready = R; type Pending = P; type Spawner = S;
    fn next_status(&mut self) -> Option<TaskStatus<R, P, S>> {
        let state = self.state.take()?;
        let (next, status) = (self.transition)(self.ctx.clone(), state)?;
        self.state = Some(next);
        Some(status)
    }
}
```

`F` stays generic (no erasure): the macro passes the user's function as a nameable
`fn` pointer, and direct users can pass a closure. `Pending = P` — no wrapper,
because a sync transition cannot be "mid-await".

## AsyncStateTask (async)

```rust
pub struct AsyncStateTask<T, R, P, S> {
    ctx: Ctx,
    state: Option<T>,
    transition: Box<dyn Fn(Ctx, T) -> Pin<Box<dyn Future<Output = Option<(T, TaskStatus<R, P, S>)> + Send>> + Send>,  // Send variant
    fut: Option<Pin<Box<dyn Future<Output = Option<(T, TaskStatus<R, P, S>)> + Send>>>,
    wake_queue: Arc<ConcurrentQueue<WakeToken>>,
    _marker: PhantomData<fn(R, P, S)>,
}

pub enum TransitionPending<P> {
    Awaiting,   // the current transition's future is mid-await; no domain value yet
    Domain(P),  // the author returned TaskStatus::Pending(p)
}
```

`AsyncStateTask` **erases** both the transition and the future (`Box<dyn>`), so
its type is `AsyncStateTask<T, R, P, S>` — four generics, nameable by the macro.
This is a documented trade-off (see below); it avoids the `impl Future` opaque
return type that an `async fn`'s nameable future would otherwise force.

`next_status` is the `FutureTask` poll loop, made stateful:

```rust
type Ready = R; type Pending = TransitionPending<P>; type Spawner = S;
fn next_status(&mut self) -> Option<TaskStatus<R, TransitionPending<P>, S>> {
    loop {
        if self.fut.is_none() {
            let state = self.state.take()?;                       // None => finished
            let fut = (self.transition)(self.ctx.clone(), state);
            self.fut = Some(fut);
        }
        while self.wake_queue.pop().is_ok() {}                    // drain stale tokens
        let waker = queue_waker(self.wake_queue.clone(), 0);
        let mut cx = Context::from_waker(&waker);
        match self.fut.as_mut().unwrap().as_mut().poll(&mut cx) {
            Poll::Ready(None) => return None,                     // finished
            Poll::Ready(Some((next, status))) => {
                self.state = Some(next);
                self.fut = None;
                return Some(map_pending(status));                 // Pending(p) -> Pending(Domain(p))
            }
            Poll::Pending => {
                return if self.wake_queue.is_empty() {
                    Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(self.wake_queue.clone()))))
                } else {
                    Some(TaskStatus::Pending(TransitionPending::Awaiting))
                };
            }
        }
    }
}
```

`map_pending` is the only new helper: it maps a `TaskStatus<R, P, S>` to
`TaskStatus<R, TransitionPending<P>, S>`, wrapping only the `Pending(p)` variant
into `Pending(Domain(p))` and passing every other variant through unchanged. The
intermediate `Awaiting` is **synthesized by the core**, never authored.

```mermaid
stateDiagram-v2
    [*] --> Idle: state = Some(T)
    Idle --> Polling: next_status (start fut = transition(ctx, T))
    Polling --> Depends: Poll::Pending, wake_queue empty
    Polling --> Awaiting: Poll::Pending, self-wake token
    Depends --> Polling: wake (executor requeues)
    Awaiting --> Polling: next_status
    Polling --> Idle: Poll::Ready(Some((next, status))) — store next
    Polling --> Done: Poll::Ready(None)
    Idle --> Done: state = None (next_status -> None)
    Done --> [*]
```

## Send / !Send split

Mirror `FutureTask`'s cfg-split exactly:

- `#[cfg(feature = "multi")]`: `T: Send + 'static`, `R/P/S: Send + 'static`,
  `F: FnMut(Ctx, T) -> … + Send` (sync) / `Box<dyn Fn + Send>` + `Box<dyn Future
  + Send>` (async).
- `#[cfg(all(not(feature = "multi"), any(feature = "std", feature = "alloc")))]`:
  `T: 'static` (no `Send`), same shape with `dyn` objects that carry no `Send`.

Gating on the whole module is `#[cfg(any(feature = "std", feature = "alloc"))]`
(Boxed futures/`dyn` require `alloc`), matching `FutureTask`.

## Error handling

- **Panic isolation** matches `DoNext`/`task_iters.rs`: the engine polls through
  `catch_unwind(AssertUnwindSafe(..))`, so a panicking transition is caught at the
  boundary and reported as a task panic, not process abort.
- **No blocking.** A sync transition that blocks violates valtron's "block only at
  boundaries" rule; the async core never blocks — it parks on `Depends` and is
  woken by the future's waker.

## Trade-offs

- **Async erasure (`Box<dyn Fn>` + `Box<dyn Future>`)** costs two allocations and
  two vtable dispatches per transition, and loses monomorphization. Accepted:
  per-transition overhead is negligible next to the async work it fronts, and it
  is what makes `AsyncStateTask<T, R, P, S>` nameable by the macro. Direct users
  who want zero erasure still have `FutureTask<F>` and hand-written iterators.
- **Two-level `Pending`** (`TransitionPending<P>`) is the honest price of "the
  future is mid-await" being a different fact from "the author advanced state".
  The sync core avoids it because a sync transition is never mid-await.

## Tasks

1. Add `executors/state_task.rs` with `StateTask<T,R,P,S,F>` + `new`/`with_ctx` +
   both `TaskIterator` impls.
2. Add `TransitionPending<P>` and the `map_pending` helper.
3. Add `AsyncStateTask<T,R,P,S>` with the stateful `FutureTask` poll loop, the
   `wake_queue`, and both `TaskIterator` impls.
4. Re-export `StateTask`, `AsyncStateTask`, `TransitionPending` from
   `executors/mod.rs`.
5. Tests: a sync multi-state machine reaches `Ready` and then `None`; an async
   machine `await`s (parks on `Depends`, resumes on wake) and reaches `Ready`;
   a `TaskStatus::Spawn` from a transition spawns a child that shares the `Ctx`;
   the `!Send` impl compiles under single/wasm.

## Success criteria

- Sync and async machines complete to `Ready`, then return `None`.
- Async parking/resume matches `FutureTask` (no busy-poll).
- `Spawn` from a transition composes with the engine and shares `Ctx`.
- Both cfg impls compile (`multi` and single/wasm).
