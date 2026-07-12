# Decision 00: Valtron Async Readiness (foundation for the async handler model)

> **Status:** foundational prerequisite for this spec. Migrated/absorbed from
> `specifications/50-valtron-async-readiness` so spec 41 owns it end-to-end and generates
> its features from here. Everything in Decisions 04/07/10/11 (async handlers/clients +
> the queue seam) **depends on this**.

## Context — why this is Decision 00

The independent review found a load-bearing hole (C1): spec 41 says async handlers "park,
no spin-loop," but valtron's future bridge **busy-polls**. Confirmed in
`backends/foundation_core/src/valtron/executors/future_task.rs`: `FutureTask<F>` polls the
inner future with a **no-op waker** and, on `Poll::Pending`, returns
`TaskStatus::Pending(..)`, which the executor re-polls every turn:

```rust
// current behaviour
let waker = get_noop_waker();                  // wake() does nothing
let mut cx = Context::from_waker(&waker);
match self.future.as_mut().poll(&mut cx) {
    Poll::Ready(out) => Some(TaskStatus::Ready(out)),
    Poll::Pending    => Some(TaskStatus::Pending(FuturePollState::Pending)), // re-polled each turn
}
```

So `from_future`/`from_stream`-bridged handlers can emit only `Pending` (never
`Depends(QueueReadiness)`), and N idle bidi/client-stream handlers re-poll every scheduler
turn. The async handler model in Decisions 04/11 is unsound until this is fixed — hence it
is Decision 00.

### What valtron ALREADY has (do not reinvent)

The parking machinery exists and is the backbone:
- **`EventReadiness`** (`task.rs`): `is_ready(&self, Option<Duration>) -> bool`; the executor
  parks a task and re-runs it only when ready — zero spin.
- **`TaskStatus::Depends(Arc<dyn EventReadiness>)`** (`task.rs`): "park me until this signal
  is ready." Already threaded through every executor (`local.rs` `Sleepable::Readiness`,
  `on_next.rs`, `do_next.rs`, `collect_next.rs`, `task_iters.rs`, `wrappers.rs`).
- **`QueueReadiness<T>(Arc<ConcurrentQueue<T>>)`** (`task.rs`): an `EventReadiness` that is
  ready when the queue is non-empty; carries a payload, multi-producer safe.

The missing piece is only a bridge **Rust `Waker` → `QueueReadiness`**, plus a seam for an
external reactor to drive that waker. No new executor machinery.

### Who calls `wake()`? (why the queue exists, and who fires it)

The `wake_queue` is an **adapter between two wake models**. Rust futures speak
`Waker::wake()` — a *callback* ("ping me when I can progress"). Valtron speaks
`EventReadiness` — a *poll-the-readiness* model (a parked task carries
`Depends(QueueReadiness(q))` and is re-run only when `q.is_ready()`). `queue_waker` builds a
`Waker` whose `wake()` simply pushes a `WakeToken` onto the queue, so a `wake()` call becomes
"queue non-empty," which the executor already knows how to park/unpark on:

```
future calls waker.wake()  →  token pushed onto wake_queue  →  QueueReadiness ready
                           →  executor re-runs FutureTask  →  re-polls the future
```

Without the queue, the bridge can only return bare `Pending` (re-polled every turn) — that
is C1. **`FutureTask` itself never calls `wake()`**; it only *owns* the queue and hands the
waker down through `Context`. The thing that calls `wake()` is **whatever the leaf future is
ultimately waiting on**, and there are three distinct cases:

1. **In-process producer (a channel / our request queue).** A bidi handler awaits the next
   request frame; another valtron task produces it. The producing side fires the stashed
   waker — or, in our S1 design, pushes onto the **same** queue `QueueReadiness` already
   watches, so readiness flips with no separate `wake()` hop. **No reactor needed.**
2. **wasm.** A `JsFuture` registers a callback with the browser; when the promise resolves
   the **browser event loop** calls the waker. The browser *is* the reactor — which is why
   **Level 1 alone suffices on wasm**.
3. **Native real I/O (socket fd / OS timer).** Nothing fires `wake()` unless some component
   watches that fd (`epoll`/`kqueue`/`io_uring`). That watcher is the **reactor**, and
   `foundation_core` ships none (no such dep). So a raw socket future would never wake on
   native — this is what Level 2 addresses. **The reactor already exists in
   `foundation_nativeapis`** (`native::poll` + `RegisteredFd: EventReadiness`), so rather than
   a `wake()`-into-queue hop, a native task parks directly on `Depends(Arc<RegisteredFd>)`.
   Full native chain:

   ```
   leaf socket task, on Pending, holds Arc<RegisteredFd<fd>>  (RegisteredFd: EventReadiness)
           ↓  returns TaskStatus::Depends(Arc<RegisteredFd>)
   foundation_nativeapis reactor (epoll/kqueue/io_uring) tracks the fd
           ↓  fd becomes readable → RegisteredFd::is_ready() == true
   executor re-runs the task → re-polls → now Ready
   ```
   (The Level-1 `wake_queue` path still covers **in-process** wakers, case 1; the native
   reactor is the case-3 mechanism and needs no separate `ReadinessSource` abstraction.)

## Decision

Adopt the three-level design below as a spec-41 prerequisite. **Level 1 is mandatory**
before any streaming feature; Levels 2–3 land alongside.

### Level 1 — Waker → `QueueReadiness` bridge (resolves C1)

A `Waker` whose `wake()` pushes a token into a shared queue; `FutureTask` watches that queue
via `QueueReadiness` and returns `Depends` on `Pending`.

```rust
/// Token pushed onto a future's wake queue when its waker fires.
/// `id` distinguishes wakers in multi-future combinators (default 0). Copy, no alloc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeToken { pub id: u64 }

/// Waker backed by Arc<ConcurrentQueue<WakeToken>>. Built with RawWaker/RawWakerVTable —
/// std/alloc only, NO new dependency, mirroring the existing create_noop_waker structure.
/// vtable: data = Arc::into_raw(queue); clone = inc strong count; wake/wake_by_ref = push; drop = from_raw.
fn queue_waker(queue: Arc<ConcurrentQueue<WakeToken>>, id: u64) -> Waker;

pub struct FutureTask<F: Future> {
    future: Pin<Box<F>>,
    completed: bool,
    wake_queue: Arc<ConcurrentQueue<WakeToken>>,   // NEW (unbounded)
}

fn next_status(&mut self) -> Option<TaskStatus<F::Output, FuturePollState, NoAction>> {
    if self.completed { return None; }
    while self.wake_queue.pop().is_ok() {}                       // drain stale tokens BEFORE poll
    let waker = queue_waker(self.wake_queue.clone(), 0);
    let mut cx = Context::from_waker(&waker);
    match self.future.as_mut().poll(&mut cx) {
        Poll::Ready(out) => { self.completed = true; Some(TaskStatus::Ready(out)) }
        Poll::Pending    => {
            // Two distinct `Pending` semantics (00-F1 self-wake detection):
            //
            // 1. **Self-wake**: the future called `cx.waker().wake_by_ref()` during poll,
            //    pushing a token onto this queue. This is the stream-future pattern
            //    (`StreamReadyFuture`, `StreamCollectFuture`, `StreamPendingFuture`) —
            //    the underlying `StreamIterator` advanced one step but isn't ready yet;
            //    the future wants another poll after other tasks run. Return `Pending`
            //    (requeue) — NOT `Depends` (park), because the token was produced by
            //    polling itself, not by an external event.
            //
            // 2. **External-wake**: no token was pushed during poll. The future stashed
            //    the waker (e.g. `RecvFuture` waiting on a pipe) and an *external* entity
            //    will fire it when data arrives. Return `Depends(queue)` to park until
            //    that wake.
            //
            // Draining at the top guarantees the only token NOW on the queue is a
            // self-wake token from *this* poll call.
            if self.wake_queue.is_empty() {
                Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(self.wake_queue.clone()))))
            } else {
                Some(TaskStatus::Pending(FuturePollState::Pending))
            }
        }
    }
}
```

- Apply to **both** `future_task.rs` impls — the `!Send` (single/wasm) and `multi` (native)
  ones; `WakeToken`/`queue_waker` are shared (not behind `multi`).
- **Wake-before-park race:** a `wake()` between our `poll` returning `Pending` and the
  executor parking lands in the same queue the `QueueReadiness` watches → `is_ready` true →
  no park / immediate unpark. Draining stale tokens at the *top* of `next_status` (before
  poll) prevents treating a prior turn's token as a fresh wake. No lost wakeups.
- **No-waker futures (never wake):** **no timed fallback** — we match tokio/smol, which both
  rely strictly on the wake contract + a reactor and do **not** add a "just in case" re-poll.
  A future that returns `Pending` must arrange a `wake()`; every real leaf wakeup comes from
  the `foundation_nativeapis` reactor (`RegisteredFd: EventReadiness`) or an in-process
  producer. A genuinely non-waking future is a **bug to fix**, not something the executor
  masks. (Zero idle cost; ecosystem-standard behaviour.)

### Level 1b — producer-side (vacancy) readiness, composite signals & two-sided pipe wake

`QueueReadiness` covers only the **consumer** direction (ready when non-empty). The bounded
seam pipes (Decision 11) also park **producers** when a pipe is full; without a defined wake
source for that direction, a full pipe degrades to bare `Pending` re-polling — the exact
busy-poll this decision exists to remove. Three pieces close it. A parked task still returns
exactly **one** `Depends`; composition happens *inside* the readiness object:

```rust
/// EventReadiness for the producer side: ready when the bounded queue has capacity.
pub struct QueueVacancyReadiness<T>(Arc<ConcurrentQueue<T>>);
impl<T> EventReadiness for QueueVacancyReadiness<T> {
    fn is_ready(&self, _: Option<Duration>) -> bool { !self.0.is_full() }
}

/// Combinator: ready when ANY child signal is ready. A task parks on ONE object, but that
/// object may own arbitrary wake logic over several underlying signals
/// (e.g. "response pipe has vacancy OR request cancelled").
pub struct AnyReadiness(Vec<Arc<dyn EventReadiness>>);
```

- **Readiness impls are open logic.** The type handed to `Depends` may watch queues, flags,
  fds, or any combination of them — and because the wake queue is a real queue, tokens can
  carry a **payload** the woken task uses to decide *what* to do (which side woke it, which
  sub-signal fired). `QueueReadiness` / `QueueVacancyReadiness` are stock building blocks,
  not the ceiling.
- **Futures path — waker hooks on the pipe (both directions).** For async callers
  (`MessageSink::send(..).await`, client stream `send`), the bounded pipe itself stashes the
  blocked side's `Waker`: a `push` fires the stashed **consumer** waker; a `pop` on a
  previously-full pipe fires the stashed **producer** waker. Each `wake()` lands a token on
  that future's L1 wake queue → its `Depends(QueueReadiness)` unparks. This generalizes
  case 1 above ("in-process producer") to both directions — *consumer-drains-wakes-producer
  is defined wiring, not an implication.*
- **Task path.** A valtron task producing into a full pipe returns
  `Depends(Arc<QueueVacancyReadiness>)` (or a composite); a task consuming an empty pipe
  returns `Depends(Arc<QueueReadiness>)`. **Bare `Pending` is never the answer to
  pipe-full / pipe-empty.**

### Level 2 — reactor seam (⚠️ **superseded — the reactor already exists**)

> **Superseded by `foundation_nativeapis` (see Consequences → "Native real-I/O parking").**
> The `ReadinessSource` / `ReadinessRegistration` / `Interest` / `READINESS_SOURCE` `OnceLock`
> sketch below was written assuming we'd *build* the native reactor and its bridge. We don't:
> `foundation_nativeapis` already ships the epoll/kqueue reactor (`native::poll`) **and** the
> valtron bridge (`native::fd::RegisteredFd<T: AsRawFd>` / `FdRegistration`, which implement
> `EventReadiness`). So a task parks by holding an `Arc<RegisteredFd>` and returning
> `Depends` — **no new `ReadinessSource` trait, no `OnceLock` slot.** The remaining wiring is
> `RawStream: AsRawFd` (Decision 12 §12) + io_uring backend (Decision 14). The sketch below is
> retained only as the *conceptual* seam description; **do not build it as a parallel
> abstraction.**

Level 1 parks futures that wake via the context waker. A leaf future on a real OS resource
(socket/timer) only progresses if *something* fires its waker. On **wasm** the browser event
loop (`JsFuture`) does this — Level 1 suffices. **Native** needs a reactor, but
`foundation_core` must not depend on `mio`/`polling`/tokio — which is exactly why the reactor
lives in the sibling platform crate `foundation_nativeapis` and bridges through
`foundation_core`'s existing `EventReadiness` trait. The conceptual seam:

```rust
// foundation_core::valtron — trait only, no impl, no deps.
pub trait ReadinessSource: Send + Sync {
    /// When `interest` becomes ready, push `token` onto `wake` (the F01 wake queue).
    /// Returned handle deregisters on drop (RAII).
    fn register(&self, interest: Interest, wake: Arc<ConcurrentQueue<WakeToken>>, token: WakeToken)
        -> Box<dyn ReadinessRegistration>;
}
pub trait ReadinessRegistration: Send + Sync {}     // RAII deregister on drop

#[non_exhaustive]
pub enum Interest { Readable, Writable, Timer(core::time::Duration), Custom(u64) }

// optional process-global registration slot (same pattern as the pool singletons)
static READINESS_SOURCE: OnceLock<Arc<dyn ReadinessSource>> = OnceLock::new();
pub fn set_readiness_source(src: Arc<dyn ReadinessSource>) -> Result<(), ...>;
pub fn readiness_source() -> Option<Arc<dyn ReadinessSource>>;
```

- **foundation_core owns:** the two traits, `Interest`, the registration slot, `WakeToken`
  + the wake queue (from L1). **No reactor, no OS types, no new dep.**
- **The platform crate that owns the reactor is `foundation_nativeapis`** (epoll/kqueue,
  io_uring per Decision 14). Its `RegisteredFd`/`FdRegistration` already implement
  `EventReadiness`, so a leaf task registers its fd and returns `Depends(Arc<RegisteredFd>)`
  directly — `foundation_core` never touches the fd, and there is **no `ReadinessSource`
  trait or `OnceLock` slot to build** (the conceptual bridge above is realized by the existing
  `EventReadiness` impls).
- **In this spec:** prove the parking path with an in-tree **test `EventReadiness`** (a thread
  that flips ready after a delay). The **production reactor already exists** in
  `foundation_nativeapis`; the only new work is wiring (`RawStream: AsRawFd`, Decision 12 §12)
  and the io_uring backend (Decision 14) — not a new reactor.

### Level 3 — `#[valtron]` / `#[valtron_test]` accept `async fn`

Today the macro rejects `async fn` (`backends/foundation_macros/src/valtron_entry.rs`). With
L1 making futures park, wrap an async body as a driven future:

```rust
// when sig.asyncness.is_some():  (sync bodies keep today's expansion byte-for-byte)
#vis fn #name() #output {
    let __guard = fc::valtron::initialize_pool(#seed, #threads);
    let __out = fc::valtron::block_on_future(async move #block);   // ? / return still mean "exit the fn"
    drop(__guard);
    __out
}

/// One executor-agnostic driver added to foundation_core; uses from_future + the run-to-
/// completion path. With L1, Pending parks instead of spinning.
pub fn block_on_future<F>(fut: F) -> F::Output
where F: Future + Send + 'static, F::Output: Send + 'static;   // (single/wasm cfg variant w/o Send)
```

## Consequences

- **Resolves C1:** bridged async handlers park via `Depends(QueueReadiness)`; no busy-poll.
  The "no spin-loop" guarantee in Decision 11 becomes true *because of this decision*.
- **Underpins the async handler model** (Decisions 04/07/10/11): `from_future`/`from_stream`
  now park correctly, so `async fn` handlers and async `Stream`s are sound on the pool.
- **The request-frame → `Stream<Req>` adapter** (review S1) plugs into this: when it returns
  `Pending` on an empty request queue, L1 parks the handler future; the reader task's push
  (or the reactor) wakes it via the wake queue. The adapter is wired through L2/the wake
  queue rather than busy-polling.
- **`#[valtron(_test)] async fn`** lets us write/test handlers and the conformance harness in
  plain async.
- **Native real-I/O parking — the reactor already exists in `foundation_nativeapis`.** The L2
  "reactor seam" below was written assuming we'd build one; in fact `foundation_nativeapis`
  (spec 34) already ships an epoll/kqueue reactor (`native::poll` — `Poll`/`Registry`/
  `Interest`/`Token`) plus `native::fd::RegisteredFd<T: AsRawFd>` / `FdRegistration` that
  **implement `EventReadiness`**. So a leaf task can register its socket fd and return
  `TaskStatus::Depends(Arc<RegisteredFd>)` to park — *the L2 bridge is realized over the
  existing `EventReadiness` trait, not a new `ReadinessSource`/`OnceLock` abstraction.* The
  only remaining wiring is "how a task obtains the reactor `Registry`," and exposing
  `AsRawFd` on `netio`'s `RawStream` (Decision 12 §12). Our HTTP/1.1, `http2/`,
  `http3/`, and **WebSocket** (Decision 13 E2) transports park through this. The Linux backend
  is being upgraded to **io_uring** for efficient high-connection-count listening — see
  **[Decision 14](14-io-uring-reactor-backend.md)** — with a shared single reactor replacing
  the current per-fd epoll. Until a task is wired to the reactor, WebSocket uses a timeout-poll
  + `Delayed` fallback (Decision 13), so it works without it but does not truly park. On wasm,
  the browser drives wakers, so L1 alone suffices.

  > **Reconciliation note:** treat the L2 `ReadinessSource`/`ReadinessRegistration`/`Interest`
  > trait sketch below as *superseded* by `foundation_nativeapis`'s reactor + `RegisteredFd:
  > EventReadiness`. Keep L2 only as the conceptual seam description; do not build a parallel
  > abstraction. Layering was unblocked by removing the unused optional `foundation_netio` dep
  > from `foundation_nativeapis` (the crates are now siblings on `foundation_core`), so the
  > transports can reach the reactor either by `netio → nativeapis` or by wiring at the
  > ConnectRPC crate.

## Features (generated from this decision)

| # | Feature | Depends on |
|---|---|---|
| 00-F1 | Waker → `QueueReadiness` bridge: `WakeToken`, `queue_waker`, `FutureTask` returns `Depends`; single+multi parity | — |
| 00-F2 | **Reactor parking via the existing `foundation_nativeapis` reactor** — a task holds `Arc<RegisteredFd<…>>` (which is `EventReadiness`) and returns `Depends`; wire access to the reactor `Registry` + `RawStream: AsRawFd` (Decision 12 §12). Prove with an in-tree test `EventReadiness`. **No new `ReadinessSource`/global-slot abstraction** (superseded — see Reconciliation note). | 00-F1, Decision 12 §12, Decision 14 |
| 00-F3 | `#[valtron]`/`#[valtron_test]` accept `async fn` via `block_on_future`; sync path unchanged | 00-F1 |
| 00-F4 | **`FramePipe` — the bounded seam pipe** (L1b; named primitive, Decision 11): bounded `ConcurrentQueue` + two waker stashes (`push` wakes stashed consumer, `pop` wakes stashed producer) + `QueueReadiness`/`QueueVacancyReadiness` accessors + `AnyReadiness` combinator (awaits/parks compose the call's `CancelSignal`); replaces `ConcurrentQueueStreamIterator`'s polling handoff on the seam; no bare `Pending` for pipe-full/pipe-empty anywhere | 00-F1 |

## Success Criteria

- A `Pending` future parks via `Depends(QueueReadiness)` — no re-poll until a token lands
  (verified: turn-count flat while blocked).
- A **self-waking** future (stream-future pattern: calls `wake_by_ref()` during poll, returns
  `Pending`) produces `Pending(FuturePollState::Pending)` — requeued without parking. No
  `Depends-true` violation. Verified by `test_self_wake_loop_no_depends_violations` (20-cycle
  self-wake loop, every poll returns `Pending` not `Depends`).
- An **externally-woken** future (pipe-future pattern: stashes waker, returns `Pending`, no
  self-wake) produces `Depends(not-ready)` and parks. Verified by
  `test_external_wake_makes_depends_ready`.
- Mixed patterns (self-wake → external-wake) coexist in one `FutureTask`. Verified by
  `test_mixed_self_wake_then_external_park`.
- `StreamTask` (async `Stream`) implements the same self-wake detection as `FutureTask`.
- A future woken via the context waker is re-scheduled promptly; no lost wakeup under a
  wake-before-park stress test.
- A producer future awaiting a **full** bounded pipe parks and is woken by the consumer's
  `pop` (turn-count flat while full) — symmetric to the consumer/empty case; a task
  producer parks via `Depends(QueueVacancyReadiness)`.
- Native fd parking works through `foundation_nativeapis`'s reactor (`RegisteredFd:
  EventReadiness`) with **no new `foundation_core` dependency and no new `ReadinessSource`
  abstraction**; an in-tree test `EventReadiness` drives a future to completion via `Depends`.
- `#[valtron_test] async fn` / `#[valtron] async fn` compile and run; `?`/`return` behave;
  sync forms unchanged. Works on `single` and `multi`.

## Module References

- `backends/foundation_core/src/valtron/executors/future_task.rs` — `FutureTask`, noop → queue waker
- `backends/foundation_core/src/valtron/task.rs` — `EventReadiness`, `QueueReadiness`, `Depends`
- `backends/foundation_core/src/valtron/executors/local.rs` — `Sleepable::Readiness` (park/wake)
- `backends/foundation_core/src/valtron/executors/sendables.rs` / `non_sendables.rs` — `from_future`, `drive_future`
- `backends/foundation_macros/src/valtron_entry.rs` — `#[valtron]` / `#[valtron_test]` async acceptance
