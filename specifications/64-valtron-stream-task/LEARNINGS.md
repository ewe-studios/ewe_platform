# LEARNINGS — 64-valtron-stream-task

Consolidated from `64-valtron-state-task`, `65-valtron-stream-cleanup`, and
`66-valtron-stream-wake`.

## Design decisions and rationale

### Phase 1 — Stream cleanup (was 65)

- **Audit before deletion.** The cleanup scope was determined by a full cross-crate
  sweep of every `ConcurrentQueue<Stream<D, P>>` communicator, not by eyeballing the
  obvious dead types. This surfaced that the combinator layer also uses
  `ConcurrentQueue<Stream<D, P>>` but is alive and cross-crate, which kept it out of
  scope.
- **Two dead types, plus one to migrate.** `RecvIterator` (`synca/mpp`) looked
  suspicious but is alive — its sole non-test caller is `ThreadedIterFuture`
  (`future_task.rs`, `multi`), the `!Send`-iterator sync bridge. Only its
  `StreamRecvIterator` wrapper is dead. Feature 01 migrates that caller to
  `NotifyQueue`, so `RecvIterator` loses its last user while the `mpp` module itself
  stays (`Broadcaster`/`Receiver`/`recv_timeout` are used cross-crate).
- **`Pipe` is waker-based, not thread-blocking.** `Pipe<T>` parks by stashing a
  `Waker` and exposes no blocking receive / `Iterator`, so it cannot block the raw
  `std::thread` that consumes `ThreadedIterFuture`'s iterator. `NotifyQueue` is the
  condvar (thread-blocking) member of the family and is the correct replacement.
- **The stale "default iterator" claim.** `ConcurrentQueueStreamIterator`'s docs
  still called it "the default iterator used by valtron executors", but the
  execution path had already moved to `NotifyQueueStreamIterator`.
- **`Waiting` becomes a ~1 s heartbeat on multi.** `NotifyRecvIterator` maps
  `NotificationItem::None → ThreadedValue::Waiting`, and native
  `DEFAULT_NOTIFY_QUEUE_MAX_SPINS = 100` × the 10 ms wait timeout means that yield
  fires only after ~1 s of empty queue. `close()` doesn't `notify_one`, so a parked
  consumer observes close on the next timeout expiry (≤ 10 ms).
- **The migration breaks three tests by contract, not by determinism.** Three
  `multi` tests assert `Waiting => unreachable!`; the assertion is now invalid and
  flaky under scheduling pressure — convert to tolerant and add a slow-iterator case.

### Phase 2 — Stream wake (was 66)

- **The wake signal is a level-triggered "queue became non-empty" edge, decided by
  the consumer.** `wake(Fn() -> ())` carries no payload, so the producer cannot —
  and must not — filter by variant. Fire on every `push` and on `close`; the woken
  bridge re-polls and re-classifies. This is the `Waker::wake()` contract.
- **`WakeableStream` must be opt-in (no blanket), and the `StreamIterator` blanket
  stays.** A blanket `WakeableStream` (default `NotSupported`) would overlap the
  waking terminal's `Successful` impl — E0119. And dropping the `StreamIterator`
  blanket instead is ~10× worse (~56 impls plus cross-crate `dyn StreamIterator`
  breakage). Decision: keep the blanket; opt-in `WakeableStream`; enforce the bound
  on bridges.
- **Autoref-specialization detection is a dead end here.** The bridges are generic;
  autoref specialization only dispatches at concrete-type or macro call sites, never
  over a generic `SI`. The runtime decision belongs in `wake()`'s return value
  (`Successful` = park, `NotSupported` = self-wake), with the compile-time
  `SI: WakeableStream` bound guaranteeing the method exists.
- **The parking machinery already exists.** `FutureTask::next_status` polls with a
  `queue_waker` and already splits self-wake (requeue) from external-wake
  (`Depends(QueueReadiness)` = park). `WakeableStream` just lets the bridges land in
  the park branch — no new executor primitives.
- **The `loop` in the bridges has two jobs; only one dies.** The `Ignore` skip
  (consumed-but-dropped items — `FilterDone`, `TaskStatus::Spawn/Ignore/Depends`)
  must stay and cannot be a park (the item is gone, no wake will come). The
  self-wake on `Pending`/`Wait`/`Init`/`Delayed` is what becomes register-then-park.
- **Lost-wakeup needs register-then-drain.** A `push` between `next()` (empty) and
  `wake(…)` fires the still-empty registry and will not re-fire. The bridge must
  stash the callback, re-check `next()` once, then park — the Pipe discipline.

### Phase 2 implementation notes (feature 02, landed)

- **Associated-type bounds do not propagate through a supertrait.** `I: WakeableStream`
  only implies `I: StreamIterator`; the output `I::D`/`I::P` `Send + 'static`
  (or `'static`) bounds must be re-spelled on *every* generic impl. Skipping them on
  `impl<D, P> WakeableStream for NotifyQueueStreamIterator<D, P>` produced
  E0310/E0277/E0271; the fix is a cfg-split impl with explicit bounds. This is the
  single most common mistake when adding a trait that wraps `StreamIterator`.
- **`WakeRegistry` must live behind `Arc<WakeRegistry>` in `NotifyQueue`.** The
  producer and consumer hold different clones of the same queue, and `Clone` must
  share the registry, not copy it — a plain `Mutex<Vec<…>>` field would break the
  producer/consumer handoff.
- **The forward/default split is mechanical to derive.** Every per-item combinator
  has a literal `inner: I` field; the buffering/fan-out/flatten ones don't (they hold
  a `Vec` of inner iterators or a continuation). Grep for `inner: I` to get the
  exact 23-forward list; the remaining 19 stream structs get empty `NotSupported`
  impls. Same split (42 each) in `sendable.rs` and `non_sendable.rs`.
- **Valtron tests abort under the default (cranelift) codegen backend.** The pinned
  toolchain ships `rustc-codegen-cranelift` and `.cargo/config.toml` enables
  `unstable.codegen-backend`; `can_finish_even_when_task_panics` panics, and cranelift
  can't unwind → "fatal runtime error: failed to initiate panic, error 5" (SIGABRT).
  Run the suite with `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm` (the mise test tasks
  already do).
- **The `mod` integration-test target always has `multi` on.** `--test mod` pulls
  `foundation_testing` (a dev-dependency) → `foundation_netio` → `foundation_core/multi`
  via feature unification, so integration tests only exercise the `sendable` (multi)
  path regardless of `--no-default-features`. The `!multi`/`non_sendable` path is
  covered by `cargo test --lib` (no dev-dependencies in the lib build).

### Phase 2 implementation notes (feature 03, landed)

- **Four bridges park, one is a raw lens.** The feature was written as "five
  bridges", but `StreamAsFutureStream`'s `poll_next` is `Poll::Ready(iter.next())`
  always — it can never return `Pending`, so it never self-wakes and has nothing to
  park. Only `StreamCollectFuture` / `StreamReadyFuture` / `StreamPendingFuture` /
  `StreamNextStream` take the `SI: WakeableStream` bound; `into_future_stream` (the
  raw lens) keeps `StreamIterator + Unpin`. The spec's "five" was corrected to four
  before landing.
- **`collect_one` / `collect_result` are not bridge callers.** Task 3 assumed they
  build on the bridges, but they are plain sync `Iterator::filter_map`/`find_map`
  helpers in `executors/sync.rs` — no `where` change, no rewrite. Audit callers by
  reading the actual call site, not by the summary.
- **The one real cross-crate rewrite: `d1_wasm.rs`.** `set_valtron_async`/
  `get_valtron_async` wrap a type-erased `Box<dyn Iterator<Item = Stream<…>>>` in
  `StreamReadyFuture::new(stream)`; a `dyn` iterator is not `WakeableStream`. Fix:
  `StreamReadyFuture::new(stream.into_stream_iter())` — `StreamIter` is the
  default-`NotSupported` plain-iterator entry point, so the bridge self-wakes.
- **`StreamIter` needs `#[derive(Debug)]`.** Wrapping plain iterators in `StreamIter`
  made `Poll<Option<(D, StreamIter<…>)>>` in bridge `{other:?}` panic messages
  unprintable (E0277). The newtype wrapper is the common terminal for tests, so it
  should stay `Debug`.
- **`DrivenStreamIterator` forwards `wake`.** `execute()` output is a
  `DrivenStreamIterator` wrapping a waking `NotifyQueueStreamIterator`; without a
  forwarding `WakeableStream` impl, `sleep_async().into_ready_future()` fails the
  new bound (E0277). The impl forwards to the inner terminal, returning
  `NotSupported` once the inner is taken out (exhausted). Added to both
  `non_sendables.rs` (Send-less) and `sendables.rs` (Send) variants.
- **Split observers default to `NotSupported`.** `SplitUntilObserverMap` /
  `SplitCollectorMapObserver` have empty `WakeableStream` impls, so the connectrpc
  `into_next_stream()` callers self-wake — behavior-preserving, no rewrite needed.

### Spread-draining bug class (fixed after feature 03 landed)

- **`Stream::Spread` is a multi-value `Done`/`Pending` item; a Done-collector must
  *drain* it, never drop it or park on it.** `StreamCollectFuture` originally
  grouped `Spread(_)` into the parking arm (dropping every `Done` payload), and
  `collect_result`/`collect_one` used `filter_map`/`find_map` matching only
  `Stream::Next`, silently discarding every `Done` inside a `Spread`. The
  canonical collector is the `Collect` combinator (`sendable.rs:1554-1558`):
  drain every `StreamSpread::Done(d)` into the accumulator, skip `Pending`.
- **The lost-wakeup re-check arm has the same bug.** A `Spread` can land between
  the bridge's empty `next()` and its `wake()`; the re-check `next()` then
  consumed it and the `Some(_) => Poll::Pending` catch-all dropped its `Done`
  payloads forever. The fix adds a dedicated `Spread` arm to the re-check too.
- **`StreamReadyFuture` / `StreamPendingFuture` peel only the first matching
  payload from a `Spread`** (`Done`/`Pending` respectively) and discard the rest —
  correct by contract (they return a single value + the remaining iterator), not
  a bug. `StreamNextStream` buffers all `Done` (the lossless fan-out path).
- **Tests that asserted the buggy behavior had to be rewritten** (`spread.rs`:
  `test_collect_future_spread_done_returns_pending` and
  `test_map_done_then_collect_with_spread_done_in_chain` both asserted that
  `Spread(Done)` was dropped). Regression coverage now lives in
  `tests/valtron/spread_drain.rs`.

### Phase 3 — State task (was 64)

- **Engine-owned `Ctx`, forked per ExecutionIterator.** The engine owns a root `Ctx`
  (standard elements: clock, config, RNG). At each ExecutionIterator boundary
  (top-level schedule and every spawn) it forks the root — `fork()` copies the
  type-map into a fresh `Arc<CtxInner>`. Sibling ExecutionIterators get unique maps.
- **`clone()` shares, `fork()` copies.** Within one ExecutionIterator the wrapper
  forwards its ctx to its inner task by `clone()` (cheap `Arc` bump, shared map).
  `fork()` clones the `HashMap` structure only — the `Arc` values stay shared.
- **`Ctx` by value everywhere, no borrow.** `Ctx` wraps an `Arc`; passing it by
  value is a cheap clone. No `&Ctx`/`&mut Ctx` (a `&mut Arc` would be a mutable ref
  to the pointer, not the map). The async transition's future owns its clone.
- **Native core iterators over inline macro output.** `StateTask`/`AsyncStateTask`
  are first-class `TaskIterator` types (parallel to `FutureTask`/`StreamTask`); the
  macro is a thin layer over them.
- **Async core erases the transition and future** (`Box<dyn Fn>` + `Box<dyn
  Future>`) so `AsyncStateTask<T, R, P, S>` is nameable.
- **Two-level `Pending` for async** (`TransitionPending::Awaiting | Domain(P)`).
- **Same-name struct** (`fn build_house` → `struct build_house`) relies on Rust's
  separate value/type namespaces.

### Phase 3 implementation notes (feature 04, landed)

- **`Ctx` is `std`-gated, not `alloc`-gated.** The spec's
  `#[cfg(any(feature = "std", feature = "alloc"))]` was tightened to
  `#[cfg(feature = "std")]` — the type map needs `std::collections::HashMap`, and
  the traits `Ctx` threads through (`TaskIterator`/`ExecutionIterator`) are
  themselves `std`-gated today. Documented deviation; the `alloc`-only relaxation
  is deferred until a `!std` build actually needs `Ctx`.
- **Concrete standard-element types.** `Rng` = `foundation_compact::rng::ChaCha8Rng`
  (the executor's own RNG, so they stay interchangeable), default-seeded
  `seed_from_u64(0)` and engine-replaceable at init. `Clock` = a minimal `Copy`
  struct over `foundation_compact::Instant` (`new`/`now`/`elapsed`). `Config` = a
  minimal `Copy` unit struct (`Default`) — a stable handle for a later feature to
  populate. Draws go through `ctx.rng().lock().next_u64()` with
  `use foundation_compact::rng::Rng;` in scope — `RngCore` is deprecated on
  rand_core 0.10.
- **`insert` returns `Option<Arc<T>>` but is not `#[must_use]`.** Matching
  `HashMap::insert`, replacing an entry is a normal operation whose return value a
  caller may legitimately ignore; `#[must_use]` here raised six "unused return
  value" warnings at bare `ctx.insert(…)` call sites.
- **Accessors return owned `Arc<T>`, never a guard.** `ctx.rng().lock()` on the
  temporary `Arc` borrows from a dropped temporary (E0716) — bind `let rng =
  ctx.rng();` first. This is why `clock`/`config`/`rng` return `Arc<T>` rather than
  `&T`: a task can hold the `Arc` across `.await` without borrowing from `self`, and
  the RNG guard never outlives a temporary.
- **`fork()` copies the map *snapshot* — insertion order decides `contains`.** A
  `fork()` taken before an insert does not see it (and vice versa). The doc example
  and `fork_isolates_the_map` test insert *before* forking for the shared case and
  *after* for the isolated case; swapping them yields a false "parent sees the
  fork's write" assertion.

## Challenges

- Distinguishing "no callers" from "only test callers" vs "only doc-comment
  references" requires grepping each reference class separately, not a single name
  search (cleanup).
- "Which blanket to drop" was the pivotal fork of the wake design: `WakeableStream`
  blanket (coherence) vs `StreamIterator` blanket (56 impls + cross-crate ripple).
- Forking must be per-ExecutionIterator, not per-poll — a per-poll fork resets a
  task's map every turn and loses entries inserted on earlier turns.
- The `impl Future`-in-type-position problem drove the async erasure decision.

## Patterns to reuse

- For a dead-code sweep: grep the type name three ways — definitions, non-comment
  non-test references, and cross-crate imports — then decide. Record the audit as
  the task list so the implementer can trust the "safe to delete" claim.
- For "optionally implement a trait method, with a graceful fallback" on stable
  Rust: opt-in trait + compile-time bound + a `Result`-style return that carries the
  runtime "can I actually do it" answer. Not a blanket default, not autoref.
- `FutureTask`'s poll loop (drain stale wake tokens → poll once → `Depends`/`Pending`).
- The `multi`/`!multi` cfg-split for `Send`/`!Send` impls.

---

_Version: 1.0 — Created 2026-10-18_
