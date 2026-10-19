---
description: "Test-coverage plan for 64-valtron-stream-task: valtron-as-runtime behaviour, cross-runtime (tokio/smol) parity, use-case stories with edge cases and invariants, and the Ctx engine-context tests (forward-scoped to features 04-06)."
status: "in-progress"
created: 2026-10-19
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-10-19
---

# TEST PLAN — 64-valtron-stream-task

This document scopes the **test coverage** for the whole `64-valtron-stream-task`
arc, organised around four coverage areas the user asked to nail down:

1. **Area A — Valtron as the async runtime.** How the future/stream variants
   behave when valtron's own executor (not tokio/smol) drives async functions.
2. **Area B — Cross-runtime parity.** The same futures/streams running under
   tokio, smol, and (where applicable) wasm; enhancement of the existing
   `futures_in_valtron.rs` / `stream_future.rs` setup tests.
3. **Area C — Ctx from the engine.** Tests showing how to use the engine-seeded
   `Ctx` from different tasks (forward-scoped: depends on features 04–06).
4. **Area D — Gap analysis.** What is left to implement in this spec, and which
   test suites are currently missing or under-covered.

> **SCOPE NOTE (read first).** Features `02-wakeable-stream`, `03-bridge-park` and
> `04-ctx` are **landed** in `foundation_core` (plus the follow-up `Spread`-drain
> fix, commit `8d5d53adb`). Features `00-cleanup` and `01-migrate-recv-iterator`
> are **NOT landed** — `StreamRecvIterator`/`ConcurrentQueueStreamIterator` remain
> in `streams.rs` and `ThreadedIterFuture` still drives `mpp::RecvIterator`
> (`future_task.rs:677,786`); their commits (`ee0e42ab0`, `10af143b3`) only
> authored the spec `.md` files. Features `05-thread-ctx` and `06-state-task` are
> **spec-only** (no `Ctx` threading or state-task macro in source). Area A and
> Area B are testable **now**; Area C (Ctx-from-the-engine) is testable for the
> `Ctx` unit surface after feature 04, and fully after features 05–06. Do not
> write Area C threading/macro tests against the current tree — they will not
> compile.

---

## Reference: exact symbol surface

The tests in this plan target the public `foundation_core::valtron` surface.
Exact names (verified against source) are listed here once so the test files
stay consistent.

**Iterator traits and items**

- `StreamIterator: Iterator<Item = Stream<Self::D, Self::P>>` with associated
  types `D` (data) and `P` (progress/context). Blanket impl:
  `impl<M: Iterator<Item = Stream<D, P>>> StreamIterator for M`.
- `Stream<D, P>` — variants:
  `Init`, `Ignore`, `Delayed(std::time::Duration)`, `Pending(P)`, `Next(D)`,
  `Wait`, `Spread(Vec<StreamSpread<D, P>>)`.
- `StreamSpread<D, P>` — `Pending(P) | Done(D)`.
- `StreamIter<S: StreamIterator>(pub S)` — the plain-iterator newtype entry
  point; its `WakeableStream` impl defaults to `NotSupported`.
- `TaskIterator` (NOT `Iterator`) — `type Ready`, `type Pending`,
  `type Spawner: ExecutionAction`, `fn next_status(&mut self) -> Option<TaskStatus<…>>`.
  (Features 04–06 will change this signature to `next_status(&mut self, ctx: Ctx)`.)
- `TaskStatus<D, P, S>` — `Delayed(time::Duration)`, `Spawn(S)`, `Pending(P)`,
  `Init`, `Ready(D)`, `Ignore`, `Wait`, `Spread(Vec<TaskSpread<D, P>>)`,
  `Depends(EventReadinessPtr)`.
- `TaskSpread<D, P>` — `Pending(P) | Ready(D)` (note: `Ready`, not `Done`).
- `ExecutionIterator` — `fn next(&mut self, parent_id: Entry, engine: BoxedExecutionEngine) -> Option<State>`.
- `FuturePollState` — the `Pending`/self-wake classifier payload.

**Wake capability (feature 02/03)**

- `WakeableStream: StreamIterator` — `fn wake(&mut self, cb: WakeCallback) -> WakeResult`
  (default `NotSupported`; NO blanket impl — E0119).
- `WakeResult` — `Successful | NotSupported`.
- `WakeCallback` — `Box<dyn Fn() + Send + 'static>` under `multi`,
  `Box<dyn Fn() + 'static>` under `!multi`.

**The five async bridges (stream_future.rs)**

- `StreamCollectFuture` / `into_collect_future()` — drains all `Next`/`Spread(Done)`.
- `StreamReadyFuture` / `into_ready_future()` — first `Next` + remaining iterator.
- `StreamPendingFuture` / `into_pending_future()` — first `Pending` + remaining iterator.
- `StreamAsFutureStream` / `into_future_stream()` — raw one-to-one lens, never parks.
- `StreamNextStream` / `into_next_stream()` — lossless fan-out of `Next`/`Spread(Done)`.
- Bounds: the four *parking* bridges require `Self: WakeableStream`;
  `into_future_stream` requires only `StreamIterator + Unpin`.

**Combinators (`StreamIteratorExt`)**

`map_done`, `map_pending`, `map_pending_and_done`, `filter_done`, `map_delayed`,
`collect`, `split_collector`, `split_collect_one`, `split_collect_until`,
`map_circuit`, `first_next`, `first_pending`, `split_collector_map`,
`split_collect_one_map`, `map_iter`, `map_iter_done`, `map_iter_pending`,
`flatten_next`, `flatten_pending`, `map_state`, `filter_state`,
`into_collect_future`, `into_ready_future`, `into_pending_future`,
`into_future_stream`, `into_next_stream`, `into_stream_iter`,
`collect_result`, `collect_one`. Free fns: `collect_all`, `map_all_done`,
`map_all_pending_and_done`, `collect_next_from_streams`.

**Executor entry points**

- `execute(task, wait_cycle) -> GenericResult<DrivenStreamIterator<T>>`.
- `execute_with_config`, `execute_as_task`, `execute_collect_all`, `execute_map_all`,
  `execute_map_all_pending_and_done`, `execute_collect_next_from_all`,
  `execute_cancellable_futures`.
- `DrivenStreamIterator<T>(Option<NotifyQueueStreamIterator<T::Ready, T::Pending>>)`
  — forwards `WakeableStream` (returns `NotSupported` once inner is taken).
- `from_future(future) -> FutureTask<F>`, `from_stream(stream) -> StreamTask<S>`.
- `FutureTask<F>` (`future_task.rs:214`), `StreamTask<S>` (`future_task.rs:374`).
- `sleep(duration) -> DrivenStreamIterator<…>`; `sleep_async(duration) =
  sleep(duration).into_ready_future().await`.
- `collect_result` / `collect_one` — sync boundary helpers (drain `Spread(Done)`).
- Macros: `#[valtron_test]` (emits its own `#[test]`, initialises the multi pool,
  serialises pool users, supports `async fn`), `#[valtron::task]` (feature 06),
  `timeout`.

**Build/test constraints (must be respected by every new test file)**

- `--test mod` always has `multi` on (dev-dep feature unification) — integration
  tests exercise the `sendable` (multi) path. `--lib` covers the `!multi` path.
- Panicking tests SIGABRT under the cranelift backend; run with
  `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm`.
- No `futures-util` dev-dep — use `core::future::poll_fn` (or `futures_core::Stream`
  which IS available as a regular dep) to drive streams cross-runtime.

---

## Area A — Valtron as the async runtime

### A.1 What "valtron as the runtime" means here

Valtron is not a general-purpose async runtime; it is a **task executor** that
polls `TaskIterator`s and surfaces their progress as a `StreamIterator`. An
`async fn` enters valtron via `from_future`/`from_stream` (or the macro), and is
driven to completion **by valtron's own poll loop**, not by tokio/smol. The
bridges (`into_*_future`, `into_*_stream`) are what let a *host* (`#[valtron_test]`,
a test's manual poll, or a foreign runtime) wait on that progress.

Area A proves three things for each future/stream variant:

1. **Progressing** — valtron polls the wrapped async fn to `Ready` without an
   external runtime.
2. **Parking** — when the wrapped future is pending and *event-driven*, valtron
   parks (does not busy-poll) and is woken by the producer's `push`/`close`.
3. **Self-wake vs external-wake classification** — a self-waking future requeues
   (`Pending`), a stashing future parks (`Depends`).

### A.2 Use-case stories (Area A)

**UCS-A1 — `from_future` immediate value.**
A valtron task wraps `core::future::ready(v)`; the first `next_status()` returns
`TaskStatus::Ready(v)` and the second returns `None`. *Already covered*
(`futures_in_valtron.rs::test_future_task_immediate_ready`).

**UCS-A2 — `from_future` over a self-waking async chain.**
A hand-rolled future that self-wakes `N` times then completes is polled by
`FutureTask` and must produce exactly `N` × `Pending(FuturePollState::Pending)`
followed by one `Ready`, never a `Depends`. *Partially covered*
(`test_self_wake_loop_no_depends_violations`, N=20).

**UCS-A3 — `from_future` over a stashing (event-driven) future.**
A future that stashes its waker and returns `Pending` yields `Depends(not-ready)`;
flipping the gate then re-polling yields `Ready`. *Partially covered*
(`test_pending_without_self_wake_returns_depends_not_ready`,
`test_external_wake_makes_depends_ready`).

**UCS-A4 — `execute` drives an async task end-to-end under `#[valtron_test]`.**
`execute(from_future(async_fn), None)` → `into_collect_future().await` (inside
`#[valtron_test]`) yields the async fn's value. This exercises the real
`DrivenStreamIterator → NotifyQueueStreamIterator` path under valtron's own pool.
*Under-covered: only the `Spread` task is exercised this way
(`spread_drain.rs::valtron_execute_spread_task_collects_all_done`).*

**UCS-A5 — `execute` + `into_next_stream` fans out every value.**
`execute(task, None).into_next_stream()` yields each `Next` in order then `None`,
driven by valtron's executor. *Under-covered.*

**UCS-A6 — `sleep_async` / `sleep` resolve on valtron's clock.**
`sleep_async(d)` completes only after `d`; `sleep(d)` produces a
`Delayed(d)`-carrying stream that resolves to `Ready` after the delay. *Existing
`sleep_tests.rs` covers sleep basics; add an `execute`-driven `sleep_async`
completion test under `#[valtron_test]`.*

**UCS-A7 — parking is producer-woken, not busy-looped.**
A task emits `Pending(p)` then (after a producer push) `Next(v)`; the
`into_collect_future()` bridge registers `wake(Successful)` and parks, and the
producer's push fires the callback so the `.await` resolves without re-poll spin.
This is the **key Area A invariant** and is only partially pinned by
`bridge_park.rs` (which polls manually with a noop waker, not under valtron's
pool).

### A.3 Edge cases (Area A)

- A `FutureTask` whose future is `Pending` forever → the task keeps yielding
  `Depends(not-ready)` and the executor does not spin (assert no `Ready` and
  no unbounded requeue).
- A future that returns `Poll::Pending` but self-wakes exactly once per poll
  must not be mistaken for an external wake (no `Depends`).
- `StreamTask` over an empty futures stream → immediately `None`/`Ready` of empty.
- `from_stream` over a stream that yields `None` then an item is impossible
  (futures streams are terminal) — assert the terminal `None` propagates.
- `execute` on a task whose `TaskIterator` returns `None` immediately →
  `into_collect_future()` resolves to `vec![]`.
- `Delayed(d)` and `Wait` and `Init` items are skipped by Done-collectors but
  must still advance the iterator (no park on them *unless* the backing queue
  returns `Successful` and has nothing yet).

### A.4 Invariants to prove (Area A)

- **I-A1 (classification).** `Pending(FuturePollState::Pending)` ⟺ the future
  self-woke this poll; `Depends(_)` ⟺ it stashed and did not self-wake. No other
  classification.
- **I-A2 (no busy-poll on park).** When `wake` returns `Successful`, the bridge
  must not `cx.waker().wake_by_ref()` and re-enter the loop; it must park and be
  woken by the queue `push`/`close`.
- **I-A3 (self-wake fallback).** When `wake` returns `NotSupported`
  (`StreamIter`, buffering combinators), the bridge self-wakes and still resolves.
- **I-A4 (lossless Done).** Every `Next(d)` and every `Spread(.. Done(d))` is
  collected exactly once, in order, by `into_collect_future`/`collect_result`/
  `collect_one`/`into_next_stream`.
- **I-A5 (determinism).** A deterministic task under the single-threaded engine
  produces the same `Stream` sequence every run for a fixed seed.

---

## Area B — Cross-runtime parity (tokio, smol, wasm)

### B.1 What exists and what is missing

`stream_future.rs` and `futures_in_valtron.rs` already drive the bridges under
tokio and smol — but almost always over **static `Vec<Stream<…>>`** wrapped in
`into_stream_iter()`, whose `WakeableStream` returns `NotSupported`. That path
never exercises a **real park**; it self-wakes. The genuine
`Successful`-parking terminal is `execute(…)`/`DrivenStreamIterator` →
`NotifyQueueStreamIterator`, and it is currently driven only by `#[valtron_test]`.

Enhancement: run the *same* task through `execute(…)` and await the bridge under
tokio, smol, and valtron's own pool, asserting identical results. This proves the
bridges are runtime-agnostic even on the parking path.

### B.2 Use-case stories (Area B)

**UCS-B1 — same `execute` stream, three runtimes, one result.**
One helper builds `execute(ProbeTask::default(), None)`; a `#[valtron_test]`
awaits `into_collect_future()`, a `#[tokio::test]` awaits the same, and
`smol::block_on` awaits the same; all assert `vec![…]`. *New.*

**UCS-B2 — parking under a foreign runtime.**
A task emits `Pending(p)`, then its backing producer `push`es `Next(v)`. A
`#[tokio::test]` and a `smol::block_on` each await `into_collect_future()` and
must receive `v` — proving the `Successful` park is woken across runtimes, not
just under valtron's pool. *New (currently only `bridge_park.rs`, noop-waker).*

**UCS-B3 — `Spread` fan-out under every runtime.**
`Spread([Done(1), Pending(x), Done(2)])` then `Next(3)` must yield `[1,2,3]`
via `into_collect_future` and `[1,2,3]` via `into_next_stream`, under tokio and
smol. *Already added for the `NotSupported` path (`spread_drain.rs`); add the
`execute` path for the `Successful` terminal.*

**UCS-B4 — combinator chain ends in a bridge, cross-runtime.**
`execute(task).map_done(f).into_future_stream()` must park (forward `wake`) and
still resolve under tokio and smol. *New — proves the `WakeableStream` forwarding
through combinators works on the real path, not just `StreamIter`.*

**UCS-B5 — wasm single-threaded (where applicable).**
For `#[cfg(target_family = "wasm")]` modules, the same bridge logic runs on a
single-threaded executor (no `Send` bound, no producer thread). Assert
`from_future`/`from_stream`/bridges complete on wasm. *Existing
`wasm_yielder_tests.rs` / `wasm_js_yield_integration.rs` cover yield; extend to
the bridges if not present.*

### B.3 Edge cases (Area B)

- `smol::block_on` on a future that never resolves would hang — guard such tests
  with a bounded poll loop, not an unconditional await.
- tokio multi-thread vs current-thread runtime: a `Send`-bound `execute` task
  must compile and run on both; a `!Send` task must be exercised under `--lib`
  only.
- An `execute` stream awaited twice (polled after `None`) must not panic —
  `DrivenStreamIterator` returns `NotSupported` once exhausted.

### B.4 Invariants to prove (Area B)

- **I-B1 (runtime-agnostic result).** For the same `TaskIterator`, the collected
  values are identical under valtron, tokio, and smol.
- **I-B2 (park works cross-runtime).** A producer-woken park resolves under
  tokio and smol with no busy-wait (assert a bounded number of polls, e.g. via a
  poll-counting waker, stays below a small threshold).
- **I-B3 (forwarding).** `execute(..).map_done(..).into_future_stream()` returns
  `Successful` (forwards) — a buffering/fan-out combinator in the chain still
  resolves, and a plain `StreamIter` returns `NotSupported` and self-wakes.
- **I-B4 (`Send` boundary).** Under `multi`, the whole `execute`→bridge chain is
  `Send + 'static`; under `!multi` the same code compiles with the relaxed
  `'static` bound.

---

## Area C — Ctx from the engine (forward-scoped: features 04–06)

> **BLOCKED.** `Ctx` does not exist in source. These stories are written against
> the feature-04 interface contract so they are ready to implement the moment
> `ctx.rs` lands; do not attempt them before features 04–06.

### C.1 The Ctx API under test (from `features/04-ctx/feature.md`)

```rust
pub struct Ctx(Arc<CtxInner>);                    // Send + Sync

impl Ctx {
    pub fn new() -> Self;                                   // empty map + default elements
    pub fn insert<T: Any + Send + Sync>(&self, value: Arc<T>) -> Option<Arc<T>>;
    pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
    pub fn get_or_insert<T: Any + Send + Sync>(&self, value: T) -> Arc<T>;
    pub fn remove<T: Any + Send + Sync>(&self) -> Option<Arc<T>>;
    pub fn contains<T: Any + Send + Sync>(&self) -> bool;
    pub fn clock(&self) -> Arc<Clock>;                      // read-only, shared
    pub fn config(&self) -> Arc<Config>;                    // read-only, shared
    pub fn rng(&self) -> Arc<Mutex<Rng>>;                   // lock() to draw
    pub fn fork(&self) -> Ctx;                              // copy map structure; Arc values shared
}
impl Clone for Ctx { /* shares the Arc<CtxInner> map */ }
impl Default for Ctx { /* Ctx::new() */ }
```

Key semantics (from the spec): `clone()` **shares** the map; `fork()` **copies**
the map *structure* (fresh `HashMap`, `Arc` values still shared); standard
elements (`clock`/`config`/`rng`) are `Arc`-shared across every fork; `Mutex` is
the codebase `compati::Mutex`; type erasure is `Arc<dyn Any + Send + Sync>`.

### C.2 Use-case stories (Area C)

**UCS-C1 — type-map round-trip.**
`insert` an `Arc<U>`, `get::<U>()` returns it, `contains::<U>()` is true,
`remove::<U>()` returns it and `get` is then `None`. `get::<V>()` for an
unrelated `V` returns `None` (wrong type ≠ panic). `get_or_insert` with a value
returns a live `Arc` and a second call returns the same entry (no duplicate).

**UCS-C2 — clone shares, fork isolates.**
`let a = Ctx::new(); let b = a.clone(); let c = a.fork();`
`a.insert(x)` is visible from `b.get::<X>()` (shared map) but **not** from
`c.get::<X>()` (forked map). Conversely `c.insert(y)` is invisible to `a` and
`b`. This is the sibling-ExecutionIterator-isolation invariant.

**UCS-C3 — values are Arc-shared across forks.**
Insert an `Arc<Counter>` (atomic). Bump it through `c` (the fork); the bump is
visible through `a`/`b` because the *entry* is a shared `Arc`, only the *map* is
copied. Distinguish "structural isolation" from "value sharing" explicitly.

**UCS-C4 — standard elements reachable from every clone.**
`clock()`, `config()`, `rng()` return `Arc`s from any clone/fork, and
`Arc::ptr_eq(a.clock(), c.clock())` holds (the same element across forks).
`rng().lock()` draws a value; the lock guard is scoped and never held across an
await (no method returns a guard).

**UCS-C5 — RNG determinism (single-threaded).**
With a fixed engine seed, two single-threaded runs of a task that draws from
`ctx.rng()` produce the same sequence. Document that under `multi` the
interleaving is schedule-dependent (only the single-threaded engine guarantees
one global sequence).

**UCS-C6 — sub-task shares the map.**
A task clones its `ctx` and passes it into a spawned sub-task (via
`next_status(ctx.clone())`); both observe the same `CtxInner` — an insert in the
sub-task is visible to the parent. This exercises the feature-04 mermaid
"sub-task sharing" pattern.

**UCS-C7 — `#[valtron::task]` transition receives the engine Ctx per poll.**
A `fn build_house(ctx: Ctx, state: T) -> Option<(T, TaskStatus<…>)>` reads
`ctx.clock()` on every transition; assert the transition sees the *same* clock
`Arc` each poll (threaded, not recreated).

**UCS-C8 — async transition owns its Ctx across `.await`.**
An `async fn` transition (`AsyncStateTask`) holds the `ctx` clone across an
await point; after resuming it still reads `ctx.get::<T>()` — proving the
future owns its clone (no borrow, no `&mut Arc`).

**UCS-C9 — compile-time `Send + Sync`.**
A `const`/`static` assertion that `Ctx: Send + Sync` (e.g.
`fn assert_send_sync<T: Send + Sync>() {}` instantiated with `Ctx`).

### C.3 Edge cases (Area C)

- `get::<T>()` with a matching `TypeId` but a `T` that is `Send + Sync + 'static`
  (downcast must not be refused for a covariant type).
- `insert` replaces a prior `T` and returns the previous `Arc<T>` (assert the
  returned value is the *old* one).
- `remove` of an absent type returns `None` without error.
- `fork` twice → the two forks are isolated from each other AND from the parent,
  but all share `clock`/`config`/`rng`.
- Empty `Ctx::new()` → all `get`/`remove`/`contains` return `None`/`false`;
  `clock()`/`config()`/`rng()` still return defaults.
- `rng().lock()` then re-lock on the same thread does not deadlock (`Mutex`, not
  re-entrant — assert no panic on a scoped single lock).

### C.4 Invariants to prove (Area C)

- **I-C1.** `Ctx: Send + Sync` (static assertion).
- **I-C2.** `clone()` shares the map (clone-observed writes are bidirectional).
- **I-C3.** `fork()` isolates the map *structure* (sibling inserts invisible).
- **I-C4.** `fork()` shares the *values* (Arc-shared mutation visible).
- **I-C5.** Standard elements are identical `Arc`s across forks
  (`Arc::ptr_eq`).
- **I-C6.** `get::<WrongType>()` returns `None`, never panics.
- **I-C7.** RNG sequence is reproducible on the single-threaded engine for a
  fixed seed.
- **I-C8.** No `Ctx` method returns a lock guard → no lock can be held across an
  `.await`.

---

## Area D — Gap analysis: what is left to do in this spec

### D.1 Remaining implementation (not test) work

The spec's remaining **features are unimplemented**:

| Feature | Status | Effort | What it entails |
|---|---|---|---|
| `04-ctx` | ⬜ Pending | small | `ctx.rs`: `Ctx`/`CtxInner`, type map, `Clock`/`Config`/`Rng` concrete types (resolve the RNG decision), re-export from `valtron/mod.rs`, unit tests. |
| `05-thread-ctx` | ⬜ Pending | large | Add `ctx: Ctx` (by value) to `TaskIterator::next_status` and `ExecutionIterator::next`; engine owns `root: Ctx` and forks per ExecutionIterator; update every impl, wrapper, poll site, and all backend task impls; compile `multi` + `!multi` (incl. wasm). |
| `06-state-task` | ⬜ Pending | large | `StateTask` (sync) + `AsyncStateTask` (async) + `TransitionPending<P>`; `#[valtron::task]` proc-macro (signature validation, expansion, `crate_paths::foundation_core_path()`). |

Area C tests become writable in this order: `04-ctx` unblocks UCS-C1–C4, C9;
`05-thread-ctx` unblocks UCS-C6, C7; `06-state-task` unblocks UCS-C7 (macro
form), C8.

### D.2 Spec bookkeeping to correct

The `requirements.md` Feature Index and frontmatter are stale relative to the
landed commits:

- Feature `02-wakeable-stream` is marked `⬜ Pending` but is **landed**
  (`wake.rs`; commit `7394ae813`). Feature `04-ctx` is **landed** (`ctx.rs`).
- Features `00-cleanup` and `01-migrate-recv-iterator` are marked `⬜ Pending`,
  which is **correct** — they are *not* landed: `StreamRecvIterator`/
  `ConcurrentQueueStreamIterator` remain in `streams.rs`, and `ThreadedIterFuture`
  still drives `mpp::RecvIterator` (`future_task.rs:677,786`). Commits
  `ee0e42ab0`/`10af143b3` only authored the spec `.md` files.
- Frontmatter reads `completed: 0 / uncompleted: 7`. It should read
  `completed: 3 / uncompleted: 4` (features 00, 01, 05, 06 remaining).

### D.3 Test coverage gaps to close (testable now)

1. **Area A4/A5** — `execute(async_fn)` → `into_collect_future` / `into_next_stream`
   driven by `#[valtron_test]` (only the `Spread` task is covered today).
2. **Area A7** — producer-woken parking under valtron's own pool (currently only
   `bridge_park.rs` noop-waker polling).
3. **Area B1/B2/B4** — the `Successful`-parking `execute` path awaited under
   tokio and smol (currently only `NotSupported` static streams).
4. **Area A6** — `sleep_async` completion under `#[valtron_test]`.

### D.4 Suggested implementation order

1. Land `04-ctx` (small) → immediately add Area C tests UCS-C1–C4, C9 and
   I-C1–C6, I-C8.
2. Land `05-thread-ctx` (large, mechanical) → add UCS-C6, C7 (non-macro), I-C7.
3. Land `06-state-task` (large) → add UCS-C7 (macro), C8.
4. Close the Area A/B gaps (D.3) in parallel — they are independent of 04–06.
5. Correct the `requirements.md` Feature Index (D.2).

---

_Created: 2026-10-19_
