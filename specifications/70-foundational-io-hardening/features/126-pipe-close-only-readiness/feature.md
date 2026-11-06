---
feature: "A pipe reports a close-only readiness, so a parked producer is not woken by every pop"
status: "pending"
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
still wakes the pump for nothing, which costs one extra poll per response chunk.

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

- [ ] `PipeSender::closed_readiness()` exists and is ready only when the receiver dropped.
- [ ] A pop does not wake a task parked on it; a close does, exactly once.
- [ ] `H2Pump` uses it and the `ReceiverGone` adapter is gone.
- [ ] `foundation_core` is bumped one patch step and all 38 requirements follow;
      `Cargo.lock` is updated.
- [ ] The backlog §1 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started. Feature 125 found the gap and recorded it in the
backlog while it built the pump park.
