# Valtron

Valtron is `foundation_core`'s task execution engine. Instead of futures-first
async, its core abstraction is the **task iterator**: a plain Rust iterator that
yields *status* values (`Ready`, `Pending`, `Delayed`, `Spawn`) so the executor
can interleave, pause, prioritize, and resume many tasks cooperatively — on one
thread, across a thread pool, or inside a WASM host, all behind one unified API.

```rust
use foundation_core::valtron::{valtron, sync_one, NoSpawner, TaskIterator, TaskStatus};

struct Counting { max: usize, current: usize }

impl TaskIterator for Counting {
    type Ready = usize;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<usize, (), NoSpawner>> {
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

The unit of work. Three associated types and one method:

```rust
pub trait TaskIterator {
    type Ready;                    // actual produced values
    type Pending;                  // progress signal while not ready
    type Spawner: ExecutionAction; // sub-task spawning capability (or NoSpawner)

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;
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
| `Spawn(S)` | Hand this `ExecutionAction` to the executor to start related sub-tasks. |

### Task relationships (`ExecutionAction`)

A spawned action can relate to its parent in three ways (full design discussion
in [`executors/README.md`](./executors/README.md)):

- **Lift** — "I depend on this; run it *above* me and pause me until it
  finishes." A task may only ever lift one task at a time.
- **Schedule** — "Deferrable work; queue it after me on my executor."
- **Distribute** — "Send it to the global queue; another thread may own it"
  (requires `Send`, only meaningful under `multi`).

Use `NoSpawner` as `type Spawner` for tasks that never spawn anything.

---

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
| `run_future(fut)` | Block until an async block / future completes, return its outputs. |
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
- wasm32 builds of this workspace need `--profile uat` or
  `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm` (the dev profile's Cranelift backend
  also cannot initiate panic unwinding — panic-in-task tests abort under dev
  but pass under uat).

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
- [`docs/smart_notification_and_streams.md`](./docs/smart_notification_and_streams.md) —
  why `TaskStatus::Depends` can park efficiently but `Stream::Wait` cannot, the
  producer/consumer split, the WASM/JS exception, the `readiness()` bridge, and why
  this is a deliberate architectural trade-off rather than a missing feature.
- [`docs/`](./docs/) — focused design notes (thread locals, panic unwinding,
  static init, yield strategies, non-`Send` handling, …).
- `foundation_macros::valtron_entry` — the macro implementation, written as a
  documented walkthrough of the expansion.
