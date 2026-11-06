---
feature: "A pipe reports a close-only readiness, so a parked producer is not woken by every pop"
status: "done"
lane: "priority follow-up (found by feature 125)"
model: "sonnet"
branch: "spec70/f126-pipe-closed-readiness"
crates: ["foundation_core", "foundation_connectrpc"]
catalogue_rows: ["1"]
---

# Feature 126: a pipe reports a close-only readiness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **pipe**: a bounded one-way queue with a [`PipeSender`] half and a
  [`PipeReceiver`] half. The producer pushes; the consumer pops.
- **readiness**: a signal that a condition is true. A valtron task parks on it with
  `TaskStatus::Depends`.
- **park**: to stop polling a task until a readiness signal wakes it.
- **notifier**: the object that stores a parked task's waker and fires it when a
  condition changes ([`ReadinessNotifier`]).

## Problem (found by feature 125 on 2026-11-06)

A producer that must wait for "the consumer dropped the pipe" has no exact signal. The only
producer-side readiness is `PipeSender::vacancy()`, and
`QueueVacancyReadiness::is_ready` is true when the queue is closed **or not full**. A parked
producer must therefore treat every pop as a wake, because a pop is what the vacancy
notifier reports.

Feature 125 hit this in `H2Pump` (`foundation_connectrpc/src/native/transport/h2.rs`). A
cancelled call drops its response receivers, and the pump must see that and send
`RST_STREAM`. The pump cannot park on `vacancy()` directly: an empty pipe is "not full", so
the task would spin. It uses a local `ReceiverGone` adapter that registers the vacancy
notifier but reports readiness only for the close. That is correct, but each consumer pop
still wakes the pump for nothing.

Correction, measured in this feature: the spurious wake does **not** cost a pump turn.
The executor gates a woken sleeper on `is_ready`, so an adapter that reports "not ready"
makes the task sleep again without a poll. The cost is one executor-thread interrupt per
pop. The value of this feature is an exact shared signal in place of a private adapter,
not a lower poll count. See the Result for the measurement.

`docs/bug-backlog.md` §1 holds this entry.

## Design

### 1. Give the pipe a third notifier

`PipeInner` (`backends/foundation_core/src/valtron/pipe.rs`) already holds three wake
channels: `consumer_waker`, `producer_waker` and `close_wakers`. The first two are plain
`Waker` stashes for the future API; the readiness API uses the queue's own notifiers.

Add one field:

```rust
/// Fired only by [`PipeInner::close_queue`], never by a pop or a push.
closed: Arc<ReadinessNotifier>,
```

Create it in `Pipe::with_depth`. `close_queue` notifies it after the queue closes, next to
the existing `close_wakers` loop. No other path closes a pipe: every send and receive half
calls `close_queue`, never `queue.close()` directly.

### 2. Add the readiness type

`QueueClosedReadiness<T>` in `task.rs`, built like `QueueVacancyReadiness` with
`pub(crate) fn with_notifier(queue, notifier)`:

```rust
impl<T: Send> EventReadiness for QueueClosedReadiness<T> {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.queue.is_closed()
    }
    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}
```

Clone by hand, as `QueueVacancyReadiness` does, so it needs no `T: Clone` bound. Write the
same two `#[cfg(feature = "multi")]` / `#[cfg(not(feature = "multi"))]` impls the sibling
types use, with the same `Send` bound, so the crate builds in both feature sets.

### 3. Expose it where the sibling accessors live

- `EventQueue::closed_readiness()` in `readiness_wake.rs`, beside `readiness()` and
  `vacancy()`, using the queue's new notifier.
- `PipeSender::closed_readiness()` in `pipe.rs`, which forwards to the `EventQueue`.
- `MappedSender::closed_readiness()` in `pipe_mapped.rs`, which forwards to its inner
  sender.
- `ConcurrentQueue`-derived local queue: `LocalQueue::closed_readiness()` in
  `executors/local.rs`, beside `vacancy_readiness()`, only if that type owns a close
  notifier. If it does not, leave it out and say so in the Result.

### 4. Use it in the H2 pump

Replace `ReceiverGone` in `h2.rs` with `self.head_tx.closed_readiness()` and
`self.body_tx.closed_readiness()`. The park list keeps the same shape; only the signal
becomes exact. Delete the adapter and its doc comment.

## Version

This adds public API to `foundation_core`, so bump it by one patch step from whatever
version it has when this feature starts and every consumer's requirement in the same
commit. 38 manifests name the version. Update them with one mechanical edit, then run
`cargo metadata` to refresh `Cargo.lock`. `foundation_connectrpc` does not change its
public API here, so it keeps its version.

## Tests

Write these in `backends/foundation_core/tests/valtron/pipe_closed_readiness.rs` (register
the file in `tests/valtron/mod.rs`):

- `closed_readiness_is_ready_only_after_the_receiver_drops`: with a live receiver and a
  full pipe, `is_ready` is false. Drop the receiver; `is_ready` becomes true. This proves
  the signal is not the vacancy signal.
- `a_pop_does_not_wake_the_closed_reader`: park a counting waker with `register`, pop one
  item, and assert it was not woken. Then drop the receiver and assert it was woken
  exactly once. This is the regression: on the old code the pop wakes the parked reader.
- `a_closed_pipe_wakes_the_closed_reader_only_once`: two parks, one close, one wake.

On the old code the first two tests do not compile, because the accessor does not exist.
Record the compile error text in the Result, as earlier features do.

In `backends/foundation_connectrpc/tests/h2_pump_parking_tests.rs`, add:

- `a_response_body_pop_does_not_wake_the_pump`: a peer sends the head and three body
  chunks, 20 ms apart. Count the pump turns. With the close-only signal the count is the
  same as with no wake from the pops; with the shared vacancy notifier each pop adds one
  turn. Compare the two counts inside one run: park the pump with a receiver-drop signal
  and assert the count does not grow by three. Keep the bound generous (for example, at
  most 2 turns more than the number of socket edges), because thread scheduling moves the
  count by one.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_core
cargo test --profile uat -p foundation_connectrpc
cargo test --profile uat -p foundation_proxy
cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc \
  --no-default-features --features rpc
```

## Done when

- [x] `PipeSender::closed_readiness()` exists and is ready only when the receiver dropped.
- [x] A pop does not wake a task parked on it; a close does, exactly once.
- [x] `H2Pump` uses it and the `ReceiverGone` adapter is gone.
- [x] `foundation_core` is bumped one patch step and all 38 requirements follow;
      `Cargo.lock` is updated.
- [x] The backlog §1 entry is deleted.
- [x] CI green.

## Result

2026-11-06. Implemented in-tree on branch `spec70/priority-followups`.

### What changed

`foundation_core`:

- New public type `QueueClosedReadiness<T>` in
  `src/valtron/task.rs`. `is_ready` returns `queue.is_closed()`. `register`
  forwards to its `ReadinessNotifier`. It has a hand-written `Clone` with no
  `T: Clone` bound. It has the same `#[cfg(feature = "multi")]` (with
  `T: Send`) and `#[cfg(not(feature = "multi"))]` impls as its siblings.
- `EventQueueInner` in `src/valtron/readiness_wake.rs` gains a `closed`
  notifier. `EventQueue::close` notifies it only when this call changes the
  queue from open to closed. `EventQueue::closed_readiness()` hands it out.
- `PipeSender::closed_readiness()` in `src/valtron/pipe.rs` forwards to the
  pipe's `EventQueue`. `MappedSender::closed_readiness()` in
  `src/valtron/pipe_mapped.rs` forwards to its inner sender.

One correction to the design: §1 puts the notifier in `PipeInner`, and §3 says
`PipeSender::closed_readiness()` forwards to the `EventQueue`. Both cannot hold
without a second, unused notifier. The pipe's queue is an `EventQueue`, and
`PipeInner::close_queue` calls `EventQueue::close`. The one notifier therefore
lives in `EventQueueInner`. It fires only on the first close, whatever the
number of `close_queue` calls (the sender drop and the receiver drop can both
call it). It never fires on a push or a pop.

`foundation_connectrpc`:

- `H2Pump::park` uses `self.body_tx.closed_readiness()` and
  `self.head_tx.closed_readiness()` in place of the `ReceiverGone` adapter.
  The park list keeps its shape. The adapter and its doc comment are deleted.
  `client_cancel_resets_the_stream_on_the_server` passes: a cancelled call
  still makes the pump notice and send `RST_STREAM`.

`LocalQueue`: no type with that name exists in the workspace, and
`executors/local.rs` has no such queue. `NotifyQueue` has a `space_tasks`
notifier, but it fires on every pop and on close (it is the vacancy signal).
It has no close-only notifier, so it did not gain `closed_readiness()`.

Version: `foundation_core` 0.0.9 to 0.0.10. Every consumer requirement moved
with it: 42 requirement lines in 38 manifests, plus the crate's own version.
`Cargo.lock` is updated. `foundation_connectrpc` keeps 0.0.5.

### Tests and measurements

New `backends/foundation_core/tests/valtron/pipe_closed_readiness.rs`,
registered in `tests/valtron/mod.rs`:

- `closed_readiness_is_ready_only_after_the_receiver_drops`: false on an
  empty open pipe (where vacancy is true), false on a full open pipe, true
  after the receiver drops.
- `a_pop_does_not_wake_the_closed_reader`: a pop wakes the reader zero times;
  the receiver drop wakes it once.
- `a_closed_pipe_wakes_the_closed_reader_only_once`: two parks of one task
  and a second close still produce one wake.

On the old code these tests do not compile. The exact error is:

```
error[E0599]: no method named `closed_readiness` found for struct `PipeSender<T>` in the current scope
  --> backends/foundation_core/tests/valtron/pipe_closed_readiness.rs:56:21
   |
56 |     let closed = tx.closed_readiness();
   |                     ^^^^^^^^^^^^^^^^ method not found in `PipeSender<u32>`
```

The same error repeats at lines 80 and 99, then
`error: could not compile `foundation_core` (test "mod") due to 3 previous errors`.

New `a_response_body_pop_does_not_wake_the_pump` in
`tests/h2_pump_parking_tests.rs`. The peer sends the head and three body
chunks. The consumer waits `BODY_POP_DELAY` (220 ms) before each pop, so the
chunk is buffered and the pump has parked. The count starts after the head.

- Close-only signal (this change): 3 pump turns for three socket edges.
- `ReceiverGone` adapter (the old code): 3 pump turns. The test passes on the
  old adapter too.

That second result corrects the plan. The executor gates a woken sleeper on
`Sleepable::is_ready` (`executors/local.rs`): `(park.take_woken() || due) &&
signal.is_ready(dur)`. The old adapter's `is_ready` is false after a pop, so
the spurious wake never becomes a pump turn. It only interrupts the executor
thread. The plan's "each pop adds one turn" does not hold with this executor.

To prove the test still guards the dangerous regression, replace
`closed_readiness()` with a plain `vacancy()` in the pump's park. The pipe is
empty, so vacancy is ready, the executor re-runs the task at once, and the
test fails after about 2 000 `[DEPENDS] ... already-ready signal` warnings with
an executor panic (exit 101). The turn bound therefore rejects a park on the
shared vacancy signal.

The plan's 20 ms chunk spacing was also too short: the consumer pop raced the
pump's park and woke nobody even on the old signal. The test now uses 200 ms
between chunks and a 220 ms consumer delay. The unobservable spurious wake is
recorded in `docs/bug-backlog.md` §4.

### Commands

| command | result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings` | clean |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings` | clean |
| `cargo test --profile uat -p foundation_core` | 867 passed, 0 failed |
| `cargo test --profile uat -p foundation_connectrpc` | 242 passed, 0 failed, 1 ignored |
| `cargo test --profile uat -p foundation_connectrpc --test grpc_tests client_cancel_resets_the_stream_on_the_server` | 1 passed |
| `cargo test --profile uat -p foundation_proxy` | blocked: `ld: ... No space left on device` (link step) |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc` | not run: disk full |

Run again by the main session after it deleted the incremental caches:

| command | result |
|---|---|
| `RUSTFLAGS='-D warnings' cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc` | exit 0, no warning |
| `cargo test --profile uat -p foundation_proxy` | every binary passes except `proxy_features_tests::websocket_upgrade_relays_and_tunnels_bytes`, the known macOS failure in backlog §9 (feature 129) |

The default `foundation_core` feature set (no `multi`) compiled and ran its
tests, so the `not(multi)` readiness impl is proven. The `multi` impl compiled
in the `foundation_proxy` graph, which enables `foundation_core/multi`, before
that build ran out of disk. The dedicated wasm check could not run. The
`docs/bug-backlog.md` §1 close-only entry is deleted. The feature stays
`pending (CI proof)` until the pull-request CI is green.
