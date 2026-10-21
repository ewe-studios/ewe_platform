# Valtron

Valtron is `foundation_core`'s task execution engine. Instead of futures-first
async, its core abstraction is the **task iterator**: a plain Rust iterator that
yields *status* values (`Ready`, `Pending`, `Delayed`, `Spawn`) so the executor
can interleave, pause, prioritize, and resume many tasks cooperatively — on one
thread, across a thread pool, or inside a WASM host, all behind one unified API.

```rust
use foundation_core::valtron::{valtron, sync_one, Ctx, TaskIterator, TaskStatus};

struct Counting { max: usize, current: usize }

impl TaskIterator for Counting {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<usize, ()>> {
        if self.current >= self.max { return None; }
        self.current += 1;
        Some(TaskStatus::Ready(self.current))
    }
}

#[valtron]
fn main() {
    let values = sync_one(Counting { max: 3, current: 0 }).unwrap();
    assert_eq!(values, vec![1, 2, 3]);
}
```

---

## Getting an engine: the `#[valtron]` / `#[valtron_test]` macros

Everything in valtron needs an initialized engine. The macros (defined in
`foundation_macros`, re-exported here) handle the full lifecycle the same way
`#[tokio::main]` / `#[tokio::test]` do:

```rust
use foundation_core::valtron::{valtron, valtron_test};

// Entry point: engine up before the body, torn down after it returns.
#[valtron]
fn main() {
    // spawn / execute / sync_one / run_future ...
}

// Test: also emits #[test]; runs under cargo test with the engine live.
#[valtron_test]
fn my_engine_test() {
    // ...
}

// Both accept optional arguments:
#[valtron_test(seed = 42, threads = 4)]
fn reproducible_scheduling() {
    // ...
}
```

What the expansion does (simplified):

```rust
fn main() {
    fn __valtron_body() { /* your original body, return type preserved */ }

    let __valtron_guard = foundation_core::valtron::initialize_pool(seed, threads);
    let out = __valtron_body();
    drop(__valtron_guard);   // engine shutdown AFTER the body fully returns
    out
}
```

| Argument  | `#[valtron]` default | `#[valtron_test]` default |
|-----------|----------------------|---------------------------|
| `seed`    | fresh entropy via `RandomState` (no `rand` dependency in your crate) | same |
| `threads` | `None` (engine decides) | `Some(3)`; explicit values are **clamped to a minimum of 3** |

Why the test clamp: scheduling-sensitive tests (lifts, broadcasts,
blocked-on-execution scenarios) need real thread interleaving to mean anything —
1–2 threads can mask ordering bugs. Pass `seed = N` when you need reproducible
scheduling.

The wrapped function must be **synchronous and take no arguments** — valtron is
its own scheduler; there is no futures executor implied. (Futures are still
usable *inside* the engine; see [Futures and streams](#futures-and-streams).)
`Result`-returning bodies and `?` work as usual because the body is moved into
an inner `fn`, not a closure.

### Manual setup

If you can't use the macro (e.g. engine lifetime narrower than a function):

```rust
use foundation_core::valtron::initialize_pool;

let guard = initialize_pool(42, None);
// ... engine is live while `guard` is alive ...
drop(guard); // threads shut down here
```

**The classic footgun is `let _ = initialize_pool(..)`** — the guard drops on
that same line and the pool dies before any task runs. Always bind it to a
named local. This is exactly the mistake the macros make structurally
impossible.

---

## Core concepts

### `TaskIterator`

The unit of work. Two associated types and one method:

```rust
pub trait TaskIterator {
    type Ready;                    // actual produced values
    type Pending;                  // progress signal while not ready

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>>;
}
```

Returning `None` means the task is finished. Each `next_status` call is one
cooperative slice — never block inside it; report `Pending`/`Delayed` instead
and let the executor come back to you.

### `TaskStatus`

What a task can tell the executor on each turn:

| Variant | Meaning |
|---------|---------|
| `Ready(D)` | A real value was produced. |
| `Pending(P)` | Still working; `P` carries progress info (e.g. a `Duration` estimate). |
| `Delayed(Duration)` | Put me to sleep for at least this long. |
| `Spawn(BoxedExecutionAction)` | Hand this boxed `ExecutionAction` to the executor to start related sub-tasks. |

### Task relationships (`ExecutionAction`)

A spawned action can relate to its parent in three ways (full design discussion
in [`executors/README.md`](./executors/README.md)):

- **Lift** — "I depend on this; run it *above* me and pause me until it
  finishes." A task may only ever lift one task at a time.
- **Schedule** — "Deferrable work; queue it after me on my executor."
- **Distribute** — "Send it to the global queue; another thread may own it"
  (requires `Send`, only meaningful under `multi`).

To spawn sub-work, box an `ExecutionAction` and return it in the `Spawn` variant
(`TaskStatus::Spawn(Box::new(action))`). Tasks that never spawn simply never
produce that variant.

---

## State tasks: `#[task]` transition functions (sync & async)

The `Counting` example at the top of this file writes the whole
`impl TaskIterator` by hand. Most state machines don't need that ceremony — they
boil down to *"given my current state, what's my next state and what should I
report this turn?"*, which is exactly one function. `#[task]` turns that
function into a task for you.

The transition has this shape:

```rust
// synchronous:
fn transition(ctx: Ctx, state: T) -> Option<(T, TaskStatus<R, P>)>

// asynchronous (same shape, with `.await` inside the body):
async fn transition(ctx: Ctx, state: T) -> Option<(T, TaskStatus<R, P>)>
```

`#[task]` generates a **same-named struct** (`transition::new(state)`) that
implements `TaskIterator`, and leaves the function untouched, so you can keep
calling `transition(ctx, state)` directly. The `Ctx` is the same engine `Ctx`
every task receives on `next_status` — use it for spawning, deadlines, or
whatever the turn needs.

### Synchronous transition

```rust
use foundation_core::valtron::{execute, valtron, task, Ctx, StreamIteratorExt, TaskStatus};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage { Foundation, Walls, Roof, Done }

#[derive(Debug, PartialEq)]
struct House { walls: u32 }

// One building step per poll. `Ready` is *not* terminal: the final step emits
// `Ready(House)` and advances to `Stage::Done`, whose `None` ends the machine.
// Intermediate turns use `Ignore` — "nothing produced yet, keep driving me".
#[task]
fn build_house(ctx: Ctx, stage: Stage) -> Option<(Stage, TaskStatus<House, ()>)> {
    let _ = ctx;
    match stage {
        Stage::Foundation => Some((Stage::Walls, TaskStatus::Ignore)),
        Stage::Walls      => Some((Stage::Roof,  TaskStatus::Ignore)),
        Stage::Roof       => Some((Stage::Done,  TaskStatus::Ready(House { walls: 4 }))),
        Stage::Done       => None,
    }
}

#[valtron]
async fn main() {
    // `build_house::new(stage)` is the generated struct — drive it like any task,
    // and await its `Ready` through the ready-future bridge (no collect-all).
    let (house, _rest) = execute(build_house::new(Stage::Foundation), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("a house was built");
    assert_eq!(house, House { walls: 4 });

    // The transition itself stays directly callable:
    let (next, status) = build_house(Ctx::new(), Stage::Walls).unwrap();
    assert_eq!((next, status), (Stage::Roof, TaskStatus::Ignore));
}
```

### Asynchronous transition

An `async fn` transition can `.await` inside the body — the signature is the
same shape as the synchronous form. While the future is still in flight it
self-wakes into `TaskStatus::Ignore` ("still working, nothing to see"), and
each `.await` parks on the same wake-queue seam `FutureTask` uses — no
busy-polling.

```rust
use foundation_core::valtron::{execute, valtron, task, Ctx, StreamIteratorExt, TaskStatus};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// A tiny future that simulates a unit of async work: it yields `steps` times
// (each yield is one `Poll::Pending` with a self-wake) before completing.
struct Work(u32);

impl Future for Work {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0 == 0 {
            Poll::Ready(())
        } else {
            self.0 -= 1;
            cx.waker().wake_by_ref(); // "made progress, poll me again"
            Poll::Pending
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct House { walls: u32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Building { Raising(u32), Done }

#[task]
async fn build_house(ctx: Ctx, walls: Building) -> Option<(Building, TaskStatus<House, ()>)> {
    let _ = ctx;
    match walls {
        Building::Done => None,
        Building::Raising(n) => {
            if n < 4 {
                Work(2).await; // async work happens *inside* the transition
                Some((Building::Raising(n + 1), TaskStatus::Ignore))
            } else {
                Some((Building::Done, TaskStatus::Ready(House { walls: n })))
            }
        }
    }
}

#[valtron]
async fn main() {
    let (house, _rest) = execute(build_house::new(Building::Raising(0)), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("a house was built");
    assert_eq!(house, House { walls: 4 });
}
```

### From `#[task]` to a future or stream

`#[task]` lowers a transition function into a **`TaskIterator`** — the state
machine. `execute(..)` schedules that iterator on the engine and hands back a
**`StreamIterator`**, a stream that emits the machine's `Stream<D, P>` states one
by one. The `into_*` bridges then reshape that stream into whichever shape you
want to consume it as:

- **a future** — `into_ready_future()` (the first `Ready` value *plus* the
  remaining iterator) or `into_collect_future()` (all `Ready` values);
- **a `futures::Stream`** — `into_next_stream()` (`Item = D`, the `Ready`
  payloads) or `into_future_stream()` (the raw `Stream<D, P>` items).

Short form: `#[task]` → `TaskIterator` → `execute(..)` schedules it →
`StreamIterator` of states → `into_*` → future or `futures::Stream`.

### Semantics

- **`None` is terminal** — returning it finishes the task and yields neither a
  `next` state nor a `TaskStatus`. That is the *only* way a machine ends.
- **`Ready` is *not* terminal** in either form: the machine emits it, threads the
  returned `next` state back, and keeps running. A single machine may emit `Ready`
  many times (e.g. one value per step); it ends only when it returns `None`.
  Intermediate turns may use any status — `Ignore` is the idiomatic "not yet" —
  or `Pending` / `Delayed` / `Spawn` / `Depends` for real scheduling signals.
- **Reach that terminal `None` with an explicit `Done` variant on the state enum:**
  `SomeVariant(..)` = "still running", `Done` = done. The transition is a plain
  `match state { Done => None, .. }` — the terminal branch is spelled out, not
  implicit. After the final `Ready` you advance the state to `Done`, so the
  *next* turn's `Done => None` ends the machine. An `Option<T>` state is also
  valid (`let v = state?;` early-returns the terminal `None`), but the enum
  `Done` variant is the clearer, more explicit form.
- **The transition keeps its name.** The generated struct is a *braced* struct
  (type namespace only), so it never collides with the `fn` of the same name
  (value namespace).
- **The `Ctx` is threaded in on every turn** — neither `StateTask` nor
  `AsyncStateTask` stores its own; they forward the engine's.

Readable, in-module copies of these examples live in
[`examples/state_task_sync.rs`](./examples/state_task_sync.rs) and
[`examples/state_task_async.rs`](./examples/state_task_async.rs); the compiled,
runnable versions are in [`foundation_core/examples/`](../../examples/):

```sh
cargo run -p foundation_core --example valtron_state_task
cargo run -p foundation_core --example valtron_state_task_async
```

---

## Async first, sync at the boundary

Write APIs once, async: an `async fn`, a returned future, or an async
`Stream`. Waiting is then a real `.await` on every target — including wasm in
a Worker or browser, where blocking the one thread would starve the JS event
loop that delivers the I/O. Code that needs sync wraps the async API at its
boundary with valtron; libraries never carry a sync twin.

**Valtron stream → async** (methods on any `StreamIteratorExt` stream; waiting
parks, it never polls):

| Method | Gives | (old name, still works) |
|--------|-------|----------|
| `.values()` | `Stream<Item = D>`: each `Next`, and each `Done` inside a `Spread` | `into_next_stream` |
| `.items()` | `Stream<Item = Stream<D, P>>`: every item, progress included | `into_future_stream` |
| `.collect_async()` | future of `Vec<D>` | `into_collect_future` |
| `.first()` | future of `Option<(D, rest_of_stream)>` | `into_ready_future` |

**Async → sync** (native and the host-ABI wasm path only):

| Call | Gives |
|------|-------|
| `fut.block()` / `block_on_future(fut)` | the future's output |
| `stream.blocking()?` / `run_stream_iter(stream)?` | a plain `Iterator` to `for`-loop or `collect()` |
| `run_future_iter(..)` | a future that resolves to an iterator, as an iterator |
| `next_values(stream_iter)` | the values of a valtron stream, lazily, as an `Iterator` |

These do not exist on JS-hosted wasm (`js-wasmbindgen`, `js-foundation-wasm`):
blocking there would freeze the isolate, so using one is a compile error —
`.await` instead. The same goes for `sync_one`, `sync_all`,
`sync_collect_one` and `block_on_collect`.

**Inside a task** that pulls a stream, one call maps an item to the status to
return — `Next` → `Ready`, `Wait` → parked on the stream (`park_on`), end of
stream → `None`:

```rust
fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<u32, char>> {
    match TaskStatus::from_stream(&mut self.input)? {
        TaskStatus::Ready(v) => Some(TaskStatus::Ready(v * 2)),
        other => Some(other),
    }
}
```

Never drain a stream with a loop that `continue`s on `Wait`: on wasm that
spins forever.

## The unified API

These free functions live at `foundation_core::valtron::*` and resolve to the
right engine for your platform/features (single-threaded, `multi` thread pool,
or WASM). **Prefer them over the engine-specific `single::` / `multi::`
modules** — code written against the unified surface runs unchanged everywhere.

### Running task iterators

| Function | Use it for |
|----------|-----------|
| `execute(task, wait_cycle)` | Schedule a task, get back a stream iterator of its results. The composable default. |
| `execute_with_config(task, cfg)` | Same with explicit `StreamConfig`. |
| `execute_collect_all(tasks)` | Schedule many homogeneous tasks in parallel, stream one combined `Vec` when all finish. |
| `send(task)` | Fire-and-forget: schedule with no result channel. |

### Blocking collection (surface points)

| Function | Returns |
|----------|---------|
| `sync_one(task)` | `GenericResult<Vec<T::Ready>>` — all values of one task. |
| `sync_collect_one(task)` | `GenericResult<T::Ready>` — exactly one value. |
| `sync_all(tasks)` | `GenericResult<Vec<T::Ready>>` — all values of many tasks, run in parallel. |

Use these sparingly — at entry/exit boundaries, one-shot initializations, CLI
tools. Inside the system, prefer returning streams so callers can compose.

```rust
use foundation_core::valtron::{valtron_test, sync_one};

#[valtron_test]
fn collects_everything() {
    let values = sync_one(Counting { max: 3, current: 0 }).unwrap();
    assert_eq!(values, vec![1, 2, 3]);
}
```

### Futures and streams

Futures interop without bringing in a futures executor — valtron drives the
polling itself:

| Function | Use it for |
|----------|-----------|
| `block_on_future(fut)` / `fut.block()` | Block until a future completes and return its output (not on JS wasm). |
| `run_future_iter(future_fn, queue_size, backpressure_sleep)` | Run a future that resolves to an iterator of `Result`s; stream the items out (queue/backpressure options apply under `multi`). |
| `from_future(fut)` / `from_stream(s)` | Wrap a future / `futures_core::Stream` as a `TaskIterator` (then `execute` it like any task). |
| `drive_future(fut)` / `drive_future_stream(s)` | Same, pre-wrapped into a driven iterator. |
| `drive_iterator` / `drive_stream` / `drive_receiver` | Adapt existing iterators / streams / receivers into the engine. |

### Background jobs

```rust
use foundation_core::valtron::run_background_job;

run_background_job(|| {
    // blocking closure; runs on the background registry under `multi`,
    // inline on single-threaded / wasm builds
})?;
```

### Driving the engine (single-threaded builds)

On the single-threaded engine *you* are the event loop. After scheduling,
advance it with one of:

- `run_until_complete()` — drain everything.
- `run_until_next_state()` / `run_until_ready_state()` /
  `run_until_next_state_within(&[State...])` — step with fine control.

The blocking helpers (`sync_one` & friends) and the returned stream iterators
do this internally — you only reach for `run_until_*` when hand-driving.
Under `multi`, pool threads drive tasks for you.

---

## Platform & feature matrix

| Platform | Cargo features | Engine behind the unified API |
|----------|----------------|-------------------------------|
| native | default (no `multi`) | `single` — one thread-local executor; caller drives it |
| native | `multi` | `multi` — work-stealing thread pool; `initialize_pool`'s `threads` argument takes effect |
| wasm32 | `js-foundation-wasm` (owned runtime) | `single`, yielding to the JS event loop through the owned ABI |
| wasm32 | `js-wasmbindgen` (interop opt-in) | `single`, yielding via wasm-bindgen |

Notes:

- The `threads` argument to `initialize_pool` is **ignored** without `multi`
  (there is no pool to size). The macros still accept it so the same code
  compiles under both configurations.
- **Feature-unification gotcha:** even when `multi` isn't in your feature list,
  another crate in your build graph may enable it (in this workspace,
  `cargo test -p foundation_core` gets `multi` through the dev-dependency
  chain). This is exactly why tests and shared code should use the unified
  API: `single::spawn()` panics with "Thread pool not initialized" when the
  unified `initialize_pool` actually brought up the multi pool.
- Under mise, `dev` builds use Cranelift (mise's `[env]`), which can't
  target wasm32 and can't initiate panic unwinding (panic-in-task tests abort
  under it). Use `--profile uat`, or unset `CARGO_PROFILE_DEV_CODEGEN_BACKEND`;
  plain `cargo` outside mise builds `dev` with LLVM.

---

## Threading model

Valtron is **not** a futures/async runtime — it's a cooperative, iterator-based
task engine. Tasks yield control by returning `Pending` / `Delayed` / `Depends`
from `next_status()` rather than `.await`-ing. The executor interleaves tasks
between `next_status()` calls; tasks never preempt each other. This shapes how
valtron fits into different threading environments.

### One hardware thread, many OS threads (`multi` feature)

Enable the `multi` Cargo feature. The pool spawns N worker OS threads, each
running a `LocalThreadExecutor`. A global `ConcurrentQueue` acts as the
work-stealing channel — tasks sent via `broadcast()` land on another worker.

On a single-core machine the OS preemptively time-slices across all pool
threads, so work still makes progress (just serially, not in parallel). The
design is honest about this: `NotifyQueue<T>` uses `CondVar` so idle workers
park efficiently, and `Sleepable::Timable` lets the executor sleep until the
next task deadline rather than busy-spinning.

| Mechanism | Behaviour |
|---|---|
| `broadcast()` | Sends to the global queue; another worker thread picks it up |
| `run_background_job()` | Submits to the `BackgroundJobRegistry` thread pool |
| `broadcast_or_lift()` | Broadcasts to the pool |
| `broadcast_or_sequence()` | Broadcasts to the pool |
| `EventReadinessPtr` | `Arc<dyn EventReadiness + Send + Sync>` |
| Task types | Require `Send` — enforced at the type level by `sendables.rs` |

### One hardware thread, one OS thread (no `multi`)

This is the **default** configuration and the model for WASM targets.
Everything runs on the calling thread through a single `LocalThreadExecutor`.
**You** are the event loop: after scheduling tasks, drive progress with
`run_until_complete()`, `run_once()`, or `run_until_next_state()`. The
`Driven*Iterator` wrappers call `run_until_next_state()` internally on each
`next()`, so consuming a stream auto-drives the engine.

Because there is nowhere to offload blocking work, several operations degrade
gracefully:

| Mechanism | Under `multi` | Under `not(multi)` (1 OS thread) |
|---|---|---|
| `broadcast()` | Global queue → another worker | Degrades to `schedule()` — runs locally |
| `run_background_job()` | Background thread pool | Runs **inline** via `catch_unwind(job())` — blocks the event loop |
| `broadcast_or_lift()` | Broadcasts | Lifts (prioritises locally) |
| `broadcast_or_sequence()` | Broadcasts | Sequences (child→parent linked loop) |
| Blocking work | Offloaded via `BackgroundJobRegistry` | **Blocks inline** — no offload possible |
| `EventReadinessPtr` | `Arc<dyn … + Send + Sync>` | `Arc<dyn …>` (no `Send`/`Sync` bound) |
| Task types | Require `Send` | `!Send` types allowed (`Rc`, `RefCell`, raw pointers, etc.) |
| Internal state | `Arc<Mutex<…>>` / atomics | `Rc<RefCell<…>>` — single-threaded, no synchronisation overhead |

The `!Send` support is structural: under `not(multi)` the executor state uses
`Rc<RefCell<…>>` throughout (`ExecutorState`, `LocalThreadExecutor`), the
`SharedTaskQueue` element type drops the `Send` bound, and the `InlineAction`
broadcast arm degrades to a local `schedule()`. This means tasks can freely
hold `Rc`, `RefCell`, `Cell`, raw pointers, or any other `!Send` data — the
executor never crosses a thread boundary.

### Why this works where async runtimes struggle

Tokio's `current_thread` and smol's `LocalExecutor` still rely on futures with
their `Pin`, waker vtable, and allocation-per-task overhead. Their
`spawn_blocking` / background-thread offload requires **at least two OS
threads** — one for the event loop, one for blocking work.

Valtron sidesteps both constraints:

1. **Tasks are iterators, not futures** — no `Pin`, no waker allocation per
   poll, no `Box`-ing required (though boxing is available when dynamic
   dispatch is needed).
2. **Cooperative yielding** — tasks return `Pending`/`Delayed`/`Depends` to
   yield; the executor interleaves other work. No OS preemption needed.
3. **Futures interop is a bridge, not the foundation** — `from_future()` /
   `run_future()` wrap a `Future` as a `TaskIterator`; valtron polls it
   cooperatively. No separate async executor is required.
4. **Distribution is opt-in** — `broadcast()` only exists under `multi`;
   under `not(multi)` it degrades gracefully to local scheduling. No code
   changes required at the call site.

---

## Engine-specific surfaces (when you need them)

`single::spawn()` / `multi::spawn()` expose builder-style scheduling with
resolvers and mappers:

```rust
use foundation_core::valtron::{single, FnReady, TaskStatus};

single::spawn()
    .with_task(my_task)
    .with_resolver(Box::new(FnReady::new(|item, _engine| {
        if let TaskStatus::Ready(v) = item { /* handle v */ }
    })))
    .schedule()
    .expect("engine initialized");
single::run_until_complete();
```

Reach for these when you need per-item callbacks wired at schedule time or
engine-specific configuration; otherwise stay on the unified API.

---

## Testing valtron code

Use `#[valtron_test]` and rely **only** on the macro for engine setup — if your
test also calls `initialize_pool`, the second init is at best redundant and at
worst hides guard-lifetime bugs. See
[`tests/valtron/valtron_macro_tests.rs`](../../tests/valtron/valtron_macro_tests.rs)
for the canonical patterns: default init, explicit `seed`/`threads`,
`Result`-returning test bodies, and `#[valtron]` on helper entry points.

---

## Further reading

- [`executors/README.md`](./executors/README.md) — executor design: tasks,
  lift/schedule/distribute semantics, sleep/wake machinery.
- [`executors/state_task.rs`](./executors/state_task.rs) — the `StateTask` /
  `AsyncStateTask` adapters behind `#[task]`.
- [`examples/`](./examples/) — readable, in-module example code (sync + async
  `#[task]`), with compiled counterparts in `foundation_core/examples/`.
- [`docs/smart_notification_and_streams.md`](./docs/smart_notification_and_streams.md) —
  why `TaskStatus::Depends` can park efficiently but `Stream::Wait` cannot, the
  producer/consumer split, the WASM/JS exception, the `readiness()` bridge, and why
  this is a deliberate architectural trade-off rather than a missing feature.
- [`docs/`](./docs/) — focused design notes (thread locals, panic unwinding,
  static init, yield strategies, non-`Send` handling, …).
- `foundation_macros::valtron_entry` — the macro implementation, written as a
  documented walkthrough of the expansion.
