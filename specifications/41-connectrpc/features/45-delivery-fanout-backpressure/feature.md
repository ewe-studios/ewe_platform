---
feature: "Delivery & fan-out backpressure — enforce Decision 00 §L1b in valtron's own queues; N-way split; shrink Pipe"
description: "Wire QueueVacancyReadiness into the executor's result-delivery iterators and the split combinators (no bare Pending / no force_push drop), add an N-way split, then migrate the transport seam off hand-rolled Pipes for the output/fan-out direction — Pipe survives only for caller→task input"
status: "completed"
priority: "high"
phase: 1
depends_on: ["00-valtron-async-readiness", "02-pipe-primitive", "17-transport-seam", "23-h1-transport-client"]
estimated_effort: "large"
created: 2026-07-07
---
# Feature 45-delivery-fanout-backpressure: enforce producer-side backpressure in valtron's own delivery/fan-out queues

## Description

Decision 00 §Level 1b already ratified the rule: **"Bare `Pending` is never the
answer to pipe-full / pipe-empty."** Feature 00-F4 applied it to the Decision 11
*seam* pipe (`FramePipe`). But valtron's **own** result-delivery and fan-out
queues — the ones every `execute()` / `stream_iter` / `split_collector` allocates
— were never wired to the same rule. They still either **busy-retain** (`bare
Pending(None)`) or **silently drop** (`force_push`) when their downstream queue
fills.

This feature closes that gap in three moves, and cashes in the payoff:

1. **Delivery layer** — the consuming iterators (`StreamConsumingIter`,
   `ConsumingIter`, `ReadyConsumingIter`, native + wasm) park on
   `QueueVacancyReadiness` instead of returning bare `Pending(None)` when their
   `iter_chan` is full.
2. **Fan-out layer** — the `split_collector` family parks the source on
   `QueueVacancyReadiness` instead of `force_push`-dropping matched items, and a
   new **N-way** split lands.
3. **Shrink Pipe** — with backpressured delivery + real fan-out, the transport
   seam stops hand-rolling `Pipe`s for the response head/body. `Pipe` survives
   only for the one thing splits can't do: **caller→task input** (`send_body`).

## Normative sources (single source of truth — read before writing code)

- `specifications/41-connectrpc/decisions/00-valtron-async-readiness.md` —
  §"Level 1b — producer-side (vacancy) readiness, composite signals & two-sided
  pipe wake"; §Features 00-F4; §Success Criteria (turn-count flat while full).
- `backends/foundation_core/src/valtron/task.rs` — `EventReadiness`,
  `QueueReadiness`, **`QueueVacancyReadiness`** (already implemented, lines
  148-175), `AnyReadiness` (line 203), `TaskStatus::Depends`.
- `backends/foundation_core/src/valtron/executors/local.rs` —
  `Sleepable::Readiness` (park/re-check via `is_ready`); `NotifyQueue`
  (delivery channel, lines 207-351); `NotifyQueueStreamIterator` receiver.

## Problem (with current-code evidence)

### 1. The delivery channel exists on every `execute()`, and only its consumer side is wired

Every scheduling entry point allocates a `NotifyQueue` (a `ConcurrentQueue` +
consumer condvar) to carry the task's produced values to its receiver
(`multi/mod.rs:1083`, `stream_iter_with_config`):

```rust
let iter_chan: Arc<NotifyQueue<Stream<Done, Pending>>> = match self.channel_capacity {
    Some(capacity) => Arc::new(NotifyQueue::bounded(capacity)),
    None           => Arc::new(NotifyQueue::unbounded()),   // ← default: no backpressure at all
};
```

- **Consumer side — wired.** `NotifyQueue::push` sets the condvar and
  `notify_one()`s; `wait_for_item` blocks the receiver efficiently until an item
  lands (`local.rs:254-325`).
- **Producer side — NOT wired.** When the queue is bounded and full,
  `StreamConsumingIter::next` stashes the value and returns a **bare pending**
  (`task_iters.rs:123`):

  ```rust
  Err(PushError::Full(msg)) => {
      self.pending_msg = Some(msg);      // retain — good, nothing dropped
      return Some(State::Pending(None)); // ← BARE pending: executor busy re-polls,
  }                                      //   never parks on vacancy
  ```

  `NotifyQueue::push` notifies **only consumers**; the receiver
  (`NotifyQueueStreamIterator::next`) just `pop()`s and wakes no one. So a full
  bounded delivery queue degrades to exactly the busy-poll Decision 00 exists to
  remove — and unbounded-by-default means a fast task can run memory away from a
  slow consumer.

### 2. The fan-out queue drops instead of backpressuring

`split_collector` / `split_collect_one` / `split_collect_until*` /
`split_collector_map` route matched items to an observer branch through a bounded
`ConcurrentQueue`. On a full queue the continuation **overwrites the oldest item**
(`sendable.rs:1923`):

```rust
if let Err(e) = self.queue.force_push(stream_item) { ... }  // force_push = drop-oldest
```

and the observer returns `Stream::Ignore` when empty rather than parking
(`sendable.rs:1880`). So a slow observer **silently loses matched items**, and
the continuation never stalls to let it catch up. Harmless for a 1-slot head
peel; **data corruption** for a body fan-out.

### 3. Consequence for the transport seam

Because valtron's own delivery/fan-out couldn't be trusted to backpressure or
even to *not drop*, F23/F44 hand-rolled two `Pipe`s (`head`, `recv_body`) plus
the pump's `map_ready` side-effect closure — re-deriving, by hand, the
two-sided-wake bounded queue that `Pipe` already is. The fan-out (head vs. body)
that `split_collect_one` expresses natively was instead done manually.

## What valtron ALREADY has (do not reinvent)

- **`QueueVacancyReadiness<T>`** (`task.rs:148`) — `EventReadiness` that is ready
  when `!queue.is_full()` (closed-aware). Its own doc comment already describes
  this exact fix and cites Decision 00 §L1b. **Implemented; just unused by the
  delivery/split layers.**
- **`AnyReadiness`** (`task.rs:203`) — OR-composition so a producer can park on
  "vacancy **or** cancelled/closed" while still returning a single `Depends`.
- **`Sleepable::Readiness` + `Sleepers`** (`local.rs`) — the executor already
  parks a task on an arbitrary `EventReadiness` and re-checks `is_ready` on a
  bounded cadence (`with_max_readiness_wait`). A consumer's `pop` that frees a
  slot makes `QueueVacancyReadiness::is_ready` true → the executor unparks the
  producer. **No new wake wiring is required on the task path** (the futures/pipe
  path additionally stashes wakers for promptness, per 00-F4).
- **`TaskStatus::Depends` → `State::Depends`** is already the path these
  iterators use for `TaskStatus::Depends` (`task_iters.rs:299`).

So this feature is **wiring an existing readiness into two existing hot paths**,
not new executor machinery.

## Design

### Part A — Delivery-layer backpressure (`*ConsumingIter`)

For every consuming iterator that pushes into `iter_chan`
(`StreamConsumingIter`, `ConsumingIter`, `ReadyConsumingIter`, and their
`non_sendable` twins), replace the `PushError::Full` arm:

```rust
// BEFORE
Err(PushError::Full(msg)) => {
    self.pending_msg = Some(msg);
    return Some(State::Pending(None));           // bare pending → busy re-poll
}

// AFTER
Err(PushError::Full(msg)) => {
    self.pending_msg = Some(msg);                // still retain the value
    return Some(State::Depends(Arc::new(
        QueueVacancyReadiness::new(self.channel.queue().clone()),
    )));                                          // park until the consumer drains a slot
}
```

- `NotifyQueue` gains a `queue()` accessor returning the inner
  `Arc<ConcurrentQueue<T>>` (it already stores one; `local.rs:348` exposes a
  `&ConcurrentQueue` — widen to a cloneable `Arc` for the readiness).
- The existing `pending_msg` retry at the top of `next` (`task_iters.rs:120`) is
  kept: when the executor re-runs the parked task (vacancy true), it retries the
  stashed push first.
- **Closed-aware:** `QueueVacancyReadiness::is_ready` already returns ready on a
  closed queue, so a producer parked on a full-then-closed channel unparks and
  its next push observes `Closed` → `State::Done`. No permanent park.

#### A0 — Full `ExecutionIterator` audit (the wrappers for tasks)

Part A is scoped as an audit of **every** `ExecutionIterator` impl (the wrappers
that adapt a task into the executor), not just the three delivery iterators. The
**discrimination rule**: a wrapper returns `Depends` **iff it itself holds the
readiness for the wait** — otherwise it must not, or it parks a task on a
non-signal (park-forever). Applying the rule to the full inventory:

| Impl | Class | Action |
|---|---|---|
| `Box<M>`, `&mut M`, `Arc<Mutex<Box<M>>>`, `Mutex<Box<M>>`, `RefCell<Box<M>>`, `Rc<RefCell<Box<M>>>`, `CanCloneExecutionIterator` (`task.rs`) | pass-through | **Confirm** they forward `Depends` verbatim (they do); no change. |
| `StreamConsumingIter`, `ConsumingIter`, `ReadyConsumingIter` (`task_iters.rs`, native + `non_sendable`) | delivery (owns a queue) | **Change**: every `PushError::Full` arm → `Depends(QueueVacancyReadiness)`, not bare `Pending(None)`. |
| `OnNext`, `DoNext`, `CollectNext` (`on_next.rs`/`do_next.rs`/`collect_next.rs`) | task→State map (no queue) | **Confirm** `TaskStatus::Depends → State::Depends` already forwarded (`do_next.rs:111`, `on_next.rs:204`, `collect_next.rs:117`); no change. |
| `DualSequeunce…` / `FinishChildBeforeParent…` (`dependent_lift.rs`) | linked (forwards child/parent state) | **Confirm** they propagate the child/parent `Depends`; no change (their default policy is Decision 15's concern, not this feature's). |

**Do NOT convert these bare `Pending(None)`s** — there is no event to wait on, so
`Depends` would park forever:
- `TaskStatus::Pending(_)` — the readiness (if any) belongs to the *task*, which
  returns `TaskStatus::Depends` itself; the wrapper forwards it. A spinning task
  is fixed task-side, not in the wrapper.
- `TaskStatus::Init` / `Ignore` / `Spread` — in the delivery `*ConsumingIter` the
  **payload is passed through to the consumer** (`channel.push(Stream::Init /
  Ignore / Spread)`); the wrapper then returns `State::Pending(None)` **only
  because `State` has no `Init`/`Ignore`/`Spread` variant** (`task.rs:761`) — it's
  a bare "re-poll me, nothing to wait on" scheduling tick, not a rewrite of the
  value. There is no resource to park on, so it must **not** become `Depends`.

So the only edits are the `PushError::Full` arms in the `*ConsumingIter`; every
other impl is confirmed-correct once the task side supplies `Depends`.

### Part B — Bound the `sequenced` delivery queue (Decision 15, folded in)

Backpressure only bites if the delivery queue is bounded. Mode selection is
already explicit (`lift()` vs `sequenced()`), so there is **no global default to
flip and no caller migration** — the one queue that should be bounded but isn't
is `sequenced`'s. Per [Decision 15](../../decisions/15-bounded-delivery-and-sequenced-lifting.md):

- **`sequenced` → bounded.** `sequenced_iter` (`builders.rs:319`) and
  `stream_sequenced_iter_with_config` (`builders.rs:368`) currently hardcode
  `NotifyQueue::unbounded()`. Switch them to
  `NotifyQueue::bounded(DEFAULT_DELIVERY_CAPACITY)` (new const in `constants.rs`).
  `sequenced`'s parent drains as the child produces, so Part A's vacancy-park now
  actually paces the child — memory stays bounded, no deadlock.
- **`lift` stays unbounded.** `FinishChildBeforeParent` can't drain until the
  child is `Done`; a bound there would deadlock (child parks on vacancy that never
  comes). Unbounded is load-bearing under `lift`, so it is untouched.
- **Everything else unchanged.** `execute` / `schedule` / explicit non-`sequenced`
  paths keep `unbounded` by default; the vacancy-park is a no-op there
  (`is_full()` always false), so Part A is behaviourally inert until a queue is
  bounded. A `*_with_capacity` override tunes the `sequenced` bound for bursty
  producers.

### Part C — Fan-out backpressure + N-way split

**C1 — replace `force_push` with a park.** The `SplitCollectorContinuation`
(and the `Until` / `Map` variants) are themselves `Iterator<Item = TaskStatus>`,
so they can return `Depends`. On a full observer queue, stash the matched item
and return `TaskStatus::Depends(QueueVacancyReadiness(observer_queue))` instead
of `force_push`:

```rust
if (self.predicate)(value) {
    match self.observer_queue.push(Stream::Next(value.clone())) {
        Ok(())  => {}
        Err(PushError::Full(item)) => {
            self.pending_observer = Some(item);     // stash
            return Some(TaskStatus::Depends(Arc::new(
                QueueVacancyReadiness::new(self.observer_queue.clone()))));
        }
        Err(PushError::Closed(_)) => { /* observer gone: drop-or-close per policy */ }
    }
}
```

- The observer (`CollectorStreamIterator::next`) is unchanged in shape (empty →
  `Ignore`, closed → `None`), but now the source genuinely waits for it.
- **Observer-dropped policy:** if the observer branch is dropped, its queue
  closes; the continuation's push sees `Closed`. Decision: **the continuation
  keeps forwarding to its own continuation and simply stops copying** (a dropped
  observer means "no one is listening," not "kill the stream"). Document this
  explicitly — it is the one behavioural choice in C1.

**C2 — N-way split (`split_n` / `fanout`).** Generalize the 2-way split to N
observer branches:

```rust
/// Fan each matched item out to N observer branches (each gets a clone).
/// Returns N observers + the continuation. Requires Ready: Clone.
fn split_n<P>(self, n: usize, predicate: P, queue_size: usize)
    -> (Vec<CollectorStreamIterator<Self::Ready, Self::Pending>>, SplitNContinuation<Self>)
where Self::Ready: Clone, Self::Pending: Clone,
      P: Fn(&Self::Ready) -> bool + Send + 'static;
```

- Holds `Vec<Arc<ConcurrentQueue<Stream<Ready,Pending>>>>`; a matched item is
  cloned into each (cheap when `Ready` is `Arc`/`Bytes`-backed).
- Backpressure composes across all N: if **any** branch is full, the source
  parks on a `QueueVacancyReadiness` for that branch (or an `AnyReadiness`/“all
  have vacancy” composite — see Open Question 2). One slow branch backpressures
  the source; it does not drop.
- The existing 2-way `split_collector` becomes `split_n(2, …)`'s degenerate case
  (or stays as a thin wrapper for source/API stability).

### Part D — Shrink Pipe in the transport seam (the payoff) — **design D1**

With A + C in place, rework the H1 pump (and the WASM pump) so the response side
uses native fan-out instead of hand-rolled pipes. **The split is an internal
detail of the transport; it does not leak through `TransportStream`.**

#### The load-bearing idea: `TransportStream` speaks FutureStream, not splits

The key realization (ratified 2026-07-08): **any `StreamIterator` bridges into a
`futures_core::Stream`/`Future`** via the existing `StreamIteratorExt` adapters
(`into_ready_future`, `into_next_stream` — blanket-implemented for *every*
`StreamIterator`). So `TransportStream`'s output fields are typed as **erased,
awaitable FutureStream handles**, not as the concrete `SplitCollectorMapObserver`
types. Each transport fills them from whatever mechanism fits — h1 from splits,
WASM from the Fetch body stream, a future h2 from its body — and none of that
leaks past the boxed trait object. The `TransportStream` contract is
"transport-agnostic, `.await`-friendly," exactly as Decision 11 intends.

New `TransportStream` output shape (sendable/non_sendable cfg-twinned like the
rest of valtron — h1/native boxes `+ Send`, WASM boxes without). **Both outputs
are `Stream`s** — see "Why head is a stream, not a future" below:

```rust
pub type HeadStream = Pin<Box<dyn futures_core::Stream<
    Item = Result<(Status, SimpleHeaders), TransportError>> + Send>>;
pub type BodyStream = Pin<Box<dyn futures_core::Stream<
    Item = Result<Bytes, TransportError>> + Send>>;

pub struct TransportStream {
    pub send_body: ByteSink,   // the ONE surviving Pipe (caller→task input)
    pub head: HeadStream,      // .next().await → head(s) or transport error
    pub recv_body: BodyStream, // .next().await → bytes chunk or transport error
}
```

#### Why `head` is a stream, not a future

The pre-D `head` was a `PipeReceiver<(Status, SimpleHeaders)>` whose `receive()` is
**repeatable** — structurally a stream. HTTP/2 and HTTP/3 can deliver *interim*
heads (`100 Continue`, `103 Early Hints`) before the final response head, so a
header channel is inherently multi-item. Modelling `head` as a `Stream` (symmetric
with `recv_body`) preserves that capability and avoids a public-API change when an
h2/h3 transport starts surfacing interim heads. RPC protocols (Connect/gRPC/
gRPC-Web) emit exactly one head today, so their consumers just take the first item
(`head.next().await`); trailers are handled separately as `Frame::EndStream`, not
on `head`. For h1 the head split's `CollectionState::Close(true)` closes the head
observer the instant the head is delivered, so `head` yields the one head then ends.

#### h1 pump internals (hidden behind the erased fields)

- The pump task's `Ready` type is already `HttpExchange` (Head / BodyChunk /
  Failed). **`split_collect_until_map`** peels the head (or a *pre-head* `Failed`)
  into an observer whose item is `Result<(Status, SimpleHeaders), TransportError>`
  and closes after the first; the continuation carries body chunks.
- **`split_collector_map`** on the continuation projects `BodyChunk → Ok(bytes)`
  and a *mid-body* `Failed → Err(TransportError)` into a second observer whose
  item is `Result<Bytes, TransportError>`.
- Both observers are bridged with **`into_next_stream()`** and boxed into
  `HeadStream`/`BodyStream` — head and body are handled identically (the head split
  simply closes its observer after the first head, so its stream is length-1 today).
- The final continuation still yields the original `HttpExchange` values; it is
  terminated with **`.map_ready(|_| ())`** before `valtron::send` so body chunks
  are not buffered a second time into an undrained delivery queue (Resolution 8).
- No `head` Pipe, no `recv_body` Pipe, no `map_ready` side-effect routing closure.

#### Invariants

- **`Pipe` survives for exactly one role:** `send_body` — the **caller→task
  input** channel. Splits fan *outputs* out; nothing here injects inputs, so this
  is the irreducible Pipe.
- **Error propagation rides the payload** (the whole point of Res 7's
  `TransportError: Clone` + `HttpExchange::Failed: Arc`): a `Failed` that fires
  **pre-head** is delivered as `Err` on `head`; one that fires **mid-body** is
  delivered as `Err` on `recv_body` — never a silent `None`/dropped error (the
  original F23 objection). `send_body` stays plain `Bytes` (no back-flowing error).
- **Internals don't leak:** downstream code (`read_response_frames`, `round_trip`)
  consumes `head`/`recv_body` purely through their `Stream` surface and cannot
  observe whether a split, a pipe, or a Fetch body produced them.
- **A pipe→stream bridge unifies producers.** `body_stream_from_pipe` /
  `head_stream_from_pipe` (`transport/base.rs`) wrap a `PipeReceiver` as an
  all-`Ok` `BodyStream`/`HeadStream`, so pipe-fed producers (server-side readers,
  test harnesses) feed the *same* reader as split-fed transports — the concrete
  demonstration that "any source → FutureStream."

#### Consumer adaptation (the only protocol-layer churn)

Exactly one **client-side** reader per protocol consumes the transport body:
`read_response_frames` (`protocol/connect.rs`) and grpc_web's shared `read_frames`.
They change from `body: ByteSource` + `body.receive().await → Option<Bytes>` to
`body: BodyStream` + `body.next().await → Option<Result<Bytes, TransportError>>`
(an `Err` chunk surfaces as a `ConnectError`, then closes). grpc_web's `read_frames`
is shared by its client and server `new_conn`; the server path (pipe-fed) wraps its
`ByteSource` with `body_stream_from_pipe`, so one reader serves both. The
**server-side** `read_request_frames` (Connect, `connect.rs`) is fed by an internal
`PipeReceiver`, never touches `TransportStream`, and is **unchanged** — proof the
transport internals don't leak. `round_trip` (`transport/base.rs`) takes the one
head via `stream.head.next().await` and drains the body via
`while let Some(chunk) = stream.recv_body.next().await`.

## Scope

- Part A (+A0): ✅ **done.** Audit all `ExecutionIterator` impls per the
  discrimination rule; `PushError::Full` arms in `StreamConsumingIter` /
  `ConsumingIter` / `ReadyConsumingIter` (native + `non_sendable`) →
  `Depends(QueueVacancyReadiness)`; `NotifyQueue::queue()` Arc accessor added.
  Pass-throughs and `OnNext`/`DoNext`/`CollectNext` confirmed forwarding
  `Depends`; `Init`/`Ignore`/`Spread`/`Pending` untouched.
- Part B (Decision 15): ✅ **done.** `sequenced_iter` /
  `stream_sequenced_iter*` allocate `NotifyQueue::bounded(DEFAULT_DELIVERY_CAPACITY)`;
  `DEFAULT_DELIVERY_CAPACITY` const added; `lift` untouched.
- Part C1: ✅ **done.** `force_push` → `Depends(QueueVacancyReadiness)` across
  the 4 task-split continuation types (both twins); observer `Drop` impls close
  queue; dropped-observer policy (stop copying, keep forwarding).
- **C1b — Observer async compatibility (Resolutions 2, 3):** ✅ **done.** All
  split observer structs (task + stream twins) yield `Stream::Wait` on empty-open
  instead of `Stream::Ignore` (makes `into_ready_future()` / `into_pending_future()`
  / `into_next_stream()` work correctly over observers). `fn readiness(&self) ->
  QueueReadiness<Stream<D, P>>` accessor added to every observer.
- **C1c — Stream splits (Resolution 4):** ✅ **done.** The 3 stream-split
  continuation types × 2 twins replace `force_push` with stash
  (`pending_push`/`pending_forward`/`pending_close`) + `Stream::Wait` cooperative
  yield; helper `sstream_observer_push`. **Zero `force_push` in `extensions/`.**
- **C1d — `into_next_stream()` adapter (Resolution 5):** ✅ **done.** New
  `StreamNextStream` (`futures_core::Stream<Item = D>`) in `stream_future.rs` +
  `into_next_stream()` on both `StreamIteratorExt` twins, same self-wake mechanics
  as the existing bridges.
- **Pre-D — Error Clone (Resolution 7):** ✅ **done.** `TransportError` is `Clone`
  via Arc-wrapped `Io`/`Connect`; `HttpExchange::Failed` carries
  `Arc<dyn Error + Send + Sync>` in `foundation_netio` (boxed error `Arc::from`'d
  at the public boundary).
- Part D (**design D1**): ✅ **done.** `TransportStream.head`/`.recv_body` are
  erased `Stream` handles (`HeadStream`/`BodyStream` = boxed `futures_core::Stream`
  of `Result<(Status,SimpleHeaders), TransportError>` / `Result<Bytes, TransportError>`;
  `+ Send` for h1/native). `head` is a stream (not a future) to preserve the pre-D
  repeatable-receive capability and future-proof h2/h3 interim (`1xx`) heads.
  `H1Transport::open` fills both from `split_collect_until_map` (head) +
  `split_collector_map` (body), each bridged via `into_next_stream()`, with the
  final continuation `.map_ready(|_| ())` (Res 8). `send_body` is the only `Pipe`.
  Added `body_stream_from_pipe`/`head_stream_from_pipe` bridges so pipe-fed readers
  and test harnesses feed the same reader. Consumers: client `read_response_frames`
  (Connect) + shared `read_frames` (grpc_web) + `round_trip` adopt `.next().await`
  with the `Err` lane surfacing as `ConnectError`; server `read_request_frames`
  untouched. Protocol + real-socket F44 tests updated and green (full connectrpc
  suite passes). WASM transport is an F24 stub — not wired here.

## Out of scope

- Changing the **global** `execute`/`schedule` default from `unbounded` to
  `bounded` (Part B bounds only the `sequenced` path per Decision 15; every other
  path keeps `unbounded`).
- The Decision 11 seam `FramePipe` itself (00-F4 — already done for the seam).
- The reactor / real-I/O parking (Decision 00 §L2, `foundation_nativeapis`).
- ~~N-way broadcast / `split_n` / `broadcast_lossy` (carved to follow-on feature).~~
  **Now implemented** on both `TaskIteratorExt` twins — see "N-way broadcast" below.
  (A `StreamIteratorExt` parity twin remains a possible follow-on.)
- Embedding `Pipe` inside split combinators (explicitly rejected — the raw
  `ConcurrentQueue` + `Wait` fix + existing bridges suffice).

## Resolutions ratified during implementation (2026-07-07)

The initial design proposed embedding `Pipe` inside every split combinator and
flattening the channel element type. The session reviewed both decisions and
reversed them: the task path doesn't need Pipe (the executor's sleeper
re-checks `EventReadiness::is_ready()` on its own cadence), the async path is
already served by the existing `into_ready_future()`/`into_pending_future()`
bridges, and `Stream<D, P>` carries real in-band data (`Pending`) that must not
be silently discarded.

The corrected architecture — **no Pipe in splits, no element-type flattening** —
with the actual fixes needed:

1. **Split channels stay on raw `ConcurrentQueue`; Pipe is NOT embedded.**
   The task path parks natively via `Depends(QueueVacancyReadiness)` — the
   executor's `Sleepers` re-checks `is_ready()` on its cadence; a `push()` that
   fills a slot / a `pop()` that frees one flips the readiness, and the parked
   task unparks on the next check. No waker, no Pipe, no wiring needed (this is
   exactly what Decision 00 §L1b and Feature 00-F4 already describe for the task
   path). The async path is served by the **existing** `into_ready_future()` /
   `into_pending_future()` bridges (`stream_future.rs`) — those bridges already
   implement the self-wake pattern that `FutureTask` sanctions (yield to
   executor via `wake_by_ref()` + `Poll::Pending`). The only obstacle blocking
   them from working over split observers was the observer signal choice
   (Resolution 2). C1's `Depends(QueueVacancyReadiness)` park and the manual
   observer `Drop` impls remain as landed.

2. **Observer empty-open yields `Stream::Wait`, not `Stream::Ignore`.**
   The `into_ready_future()` bridge loops on `Stream::Ignore` *inside* `poll()`
   without yielding to the executor (`stream_future.rs:99`); it only yields on
   `Wait`/`Pending`/`Delayed`/`Init` via `wake_by_ref()` + `Poll::Pending`. But
   all 10 split observer structs (4 task + 2 stream × 2 twins) yield
   `Stream::Ignore` when their queue is empty-but-open — so awaiting an observer
   via `into_ready_future()` hard-spins inside poll and the pump task never gets
   a turn. **Fix:** every observer yields `Stream::Wait` on empty-open, matching
   `NotifyQueueStreamIterator`'s existing convention (`local.rs:595`). This
   single-line change per observer makes the existing bridges work correctly
   over observers — no new wrapper type, no Pipe, no refit.

3. **Split observers gain a `readiness()` accessor.** Task-path consumers park
   via `Depends(QueueReadiness)`. But the observer structs don't expose their
   inner `Arc<ConcurrentQueue<Stream<D, P>>>`, so a consumer literally can't
   construct the readiness. Add a trivial `fn readiness(&self) ->
   QueueReadiness<Stream<D, P>>` to every observer. Zero wake wiring — the
   executor's sleeper cadence handles it, exactly as Decision 00 §L1b describes.

4. **Stream splits: `force_push` → stash + `Stream::Wait`.**
   The 4 stream-split continuation types (2 twins × 4 sites = 8 call sites)
   currently call `force_push` — silent drop on a full observer queue. `Stream`
   has no `Depends` variant, so a stream continuation physically cannot park.
   Fix: add `pending_item: Option<Stream<D,P>>` stash; on Full, stash +
   return `Stream::Wait` (the stream model's native lossless cooperative yield,
   ~4ms setTimeout on JS); retry stashed push on next `next()`. Acceptance
   criterion: **zero `force_push` anywhere in `extensions/`** (tasks already
   park via C1; streams get `Wait`; the count goes to zero in both families).

5. **`into_next_stream()` adapter — a `futures_core::Stream<Item = D>`.**
   `into_ready_future()` gives one value + remaining iterator — works for the
   one-shot head but is ceremonial for body chunks (re-wrap loop).
   `into_future_stream()` is a raw lens that yields items as-is (including
   `Wait`) and never returns `Poll::Pending`, so it can't serve as an awaitable
   stream. The new adapter: yields `Next(v)` as `Ready(Some(v))`, self-wakes on
   `Wait`/`Pending`/`Delayed`/`Init`, and returns `Ready(None)` on exhaustion.
   Same self-wake mechanics as the existing bridges. Enables clean
   `while let Some(chunk) = body.next().await` body consumption in Part D.

6. **Channel element type stays `Stream<D, P>`.** Earlier the design proposed
   carrying bare `D`/`M` directly (since continuations only push
   `Stream::Next(v)`). But `Pending` is in-band data in the stream family
   (progress states, intermediate markers), and flattening the channel type
   would silently delete that lane from every observer. The channel carries the
   full `Stream<D, P>` item; observers keep their `Iterator<Item = Stream<D,
   P>>` surface. The `*_map` variants project what crosses (Part D's head split
   forwards only the head/error and never `Pending`), but the primitive never
   discards.

7. **Clone bounds — `TransportError` and `HttpExchange::Failed`.**
   `TransportError` becomes `Clone` by Arc-wrapping its non-`Clone` payloads:
   `Io(Arc<std::io::Error>)`, `Connect(Arc<dyn Error + Send + Sync>)`,
   `Protocol(String)` (already `Clone`), others are unit variants.
   `HttpExchange::Failed` switches from `SendableBoxedError` (`Box<dyn Error +
   Send + Sync>`) to `Arc<dyn Error + Send + Sync>` in `foundation_netio`, so a
   pre-head failure clones losslessly into **both** split branches without
   stringification at the transform.

8. **No double-buffering on the drive task.** After the two splits the final
   continuation still yields the original `HttpExchange` values; it is
   terminated with `.map_ready(|_| ())` before spawning so body chunks are not
   buffered a second time into an undrained delivery queue.

## Open questions (resolve during implementation)

1. **`NotifyQueue` producer wake for promptness — RESOLVED: rely on sleeper
   cadence.** Task-path parking re-checks `is_ready` on the sleeper cadence
   (`DEFAULT_READINESS_WAIT` = **10ms**, `constants.rs:56`) — the executor's
   `Sleepers` polls readiness on a bounded interval and unparks when the condition
   flips true. This cadence is correct for the task path (Decision 00 §L1b already
   describes it) and carries zero per-push tax. Instant producer wake via `pop()`
   interrupting the executor would tax the hot `pop` path and add a `NotifyQueue →
   executor` coupling edge. Since only **bounded** queues ever park a producer
   (unbounded's `is_full()` is never true, so the vacancy-park is inert), and
   bounded queues are opt-in, the 10ms cadence is acceptable for the common case.
   If profiling shows it stalls seam streaming, a bounded-gated wake can be added
   later — but it's a latency optimization, not a correctness requirement.
2. **N-way park composition — RESOLVED: slowest gates, lockstep.** Semantics:
   **the slowest consumer blocks everyone** — every branch receives and delivers
   each value before the source advances; no straggler, all in sync. Mechanism:
   park on the first full branch's vacancy and **retry-all on wake** (the stashed
   item is re-pushed to every branch; whichever is still full re-parks). This is
   the simplest correct semantics and is what `queue_size = 1` makes strict
   lockstep. Confirm no livelock when two branches alternate full.
3. **Observer-dropped semantics — RESOLVED.** Default (`broadcast` /
   `split_collector`): a full branch **backpressures** (never drops); a *dropped
   receiver* closes its queue and the source **stops copying to it but keeps
   forwarding** (dropped observer ≠ kill stream). Any variant that *drops a value*
   for a slow-but-live observer is a **separate, loudly-named opt-in**
   (`broadcast_lossy` / `*_lossy`) whose docs state plainly that an observer may
   miss values if it can't keep up in sync. No silent drop anywhere in the default
   path.
4. **`Result` payload vs. the split's `Clone` bound — RESOLVED: Arc-wrap the
   non-`Clone` error payloads.** The split combinators (`split_collector_map`,
   `split_collect_until_map`) require the mapped observer type `M: Clone`.
   `TransportError::Io(std::io::Error)` and `Connect(SendableBoxedError)` are not
   `Clone`. Resolution (see Resolution 7): Arc-wrap both —
   `Io(Arc<std::io::Error>)`, `Connect(Arc<dyn Error + Send + Sync>)`. In netio,
   `HttpExchange::Failed` switches from `SendableBoxedError` to `Arc<dyn Error +
   Send + Sync>`, so a pre-head `Failed` clones losslessly into both split
   branches. The error rides the payload; no stringification at the transform.

### N-way broadcast — ✅ **implemented** (2026-07-08, was a carve-out)

Part D itself only needs the **existing 2-way** `split_collect_until_map` (head
peel) + `split_collector_map` (body continuation). The **N-way** broadcast had no
Part-D consumer, so it was originally carved to a follow-on — but it has since been
**built and tested** (both `TaskIteratorExt` twins) even without a consumer, on
request. It reuses the 2-way `CollectorStreamIterator` as the branch observer (so it
inherits the Resolution 2 `Wait` + Resolution 3 `readiness()` behaviour) plus a new
`BroadcastContinuation` holding `Vec<Arc<ConcurrentQueue<Stream<Ready,Pending>>>>`.
Two policies:

- **`broadcast(n, predicate, queue_size)`** — backpressured, zero loss, slowest
  gates. Mechanism: **all-or-nothing** — a matched value is delivered to *no* branch
  until **every** open branch has vacancy (checked via `is_full`); if any is full the
  source parks on that branch's `QueueVacancyReadiness` and holds the value +
  source item back, so there is no double-delivery on resume and nothing is dropped.
  `queue_size = 1` = strict lockstep; `> 1` = buffered slack. The safe default.
- **`broadcast_lossy(n, predicate, queue_size)`** — explicit, loud opt-in; a full
  branch loses its oldest (`force_push`) so the source never stalls on a slow
  observer. The one sanctioned `force_push`, confined to this loudly-named variant.

A **dropped** observer closes its branch queue; the continuation skips it
(`first_full_branch` / `deliver` ignore closed queues) but keeps forwarding and
delivering to survivors — never a deadlock. Tests:
`tests/valtron/broadcast_tests.rs` (9 tests, green on both cfgs): fan-out to all,
predicate filter, zero-loss lockstep, deterministic park+resume (no double-deliver),
dropped-observer, lossy-never-stalls, `n=0` passthrough, `queue_size=0` clamp.

Active **eviction** (removing a branch after K misses) is still deferred — dropping
the receiver already gives a clean manual opt-out. A `StreamIteratorExt` (stream
family) twin of `broadcast` is a natural parity follow-on (not yet built).

### Related decision

[Decision 15](../../decisions/15-bounded-delivery-and-sequenced-lifting.md) —
"`sequenced` delivery is bounded; `lift` stays unbounded" — is **realized inside
this feature** (Part B), now that it's a two-line builder change rather than a
default flip. Part A's park-on-vacancy is what makes that bound safe (park, not
spin).

## Acceptance criteria

- A task producing into a **full bounded** `iter_chan` parks via
  `Depends(QueueVacancyReadiness)` — **turn-count stays flat while full** (the
  Decision 00 metric), and unparks when the consumer drains. No bare `Pending`
  on pipe-full anywhere in the delivery iterators.
- Unbounded delivery queues are behaviourally unchanged (vacancy-park is inert).
- A slow observer branch **backpressures the source and loses zero items**
  (replaces the `force_push` drop); a dropped observer does not kill the stream
  (source stops copying to it, keeps forwarding).
- **Zero `force_push` calls anywhere in `extensions/`** — task splits park via
  `Depends(QueueVacancyReadiness)` (C1); stream splits stash + yield
  `Stream::Wait` (Resolution 4).
- Split observers yield `Stream::Wait` on empty-open (not `Ignore`), so the
  existing `into_ready_future()` / `into_pending_future()` bridges await
  correctly over observers without busy-spinning.
- Split observers expose a `fn readiness(&self) -> QueueReadiness<Stream<D,
  P>>` accessor, so task-path consumers can park natively via
  `Depends(observer.readiness())`.
- `StreamIteratorExt` gains `fn into_next_stream(self) ->
  impl futures_core::Stream<Item = Self::D>`, enabling clean
  `while let Some(chunk) = body.next().await` body consumption.
- `TransportError` is `Clone` (Arc-wrapped `Io` and `Connect` variants).
- `HttpExchange::Failed` carries `Arc<dyn Error + Send + Sync>`.
- `H1Transport::open` uses `split_collect_until_map` (head peel) +
  `split_collector_map` (body continuation) with `Result` payloads, each bridged
  to an erased `Stream` via `into_next_stream()`; a pre-head **and** a mid-body
  `HttpExchange::Failed` are observed as `Err` on the `head` / `recv_body` stream
  respectively (no silent `None`). `TransportStream.head` is a `HeadStream`
  (symmetric with `recv_body`, future-proofing h2/h3 interim heads), consumed via
  `head.next().await`. `send_body` is the only remaining `Pipe` in the transport.
- All existing valtron tests green on `single` (wasm) and `multi` (native);
  use `#[valtron_test]`, never `#[test]`/`#[serial]`.

## Module references

- `backends/foundation_core/src/valtron/executors/task_iters.rs` — `*ConsumingIter` Full arms (Part A ✅ done)
- `backends/foundation_core/src/valtron/executors/{on_next,do_next,collect_next}.rs` — audit only (already forward `Depends`)
- `backends/foundation_core/src/valtron/executors/dependent_lift.rs` — linked-task wrappers (audit only)
- `backends/foundation_core/src/valtron/task.rs` — pass-through `ExecutionIterator` impls (audit only); `QueueVacancyReadiness` (reused)
- `backends/foundation_core/src/valtron/executors/local.rs` — `NotifyQueue` (`queue()` accessor), receiver
- `backends/foundation_core/src/valtron/executors/multi/mod.rs` — `iter_chan` allocation, `channel_capacity`
- `backends/foundation_core/src/valtron/executors/builders/mod.rs` — `sequenced` bounded queue (Part B ✅ done)
- `backends/foundation_core/src/valtron/executors/constants.rs` — `DEFAULT_DELIVERY_CAPACITY` (Part B ✅ done)
- `backends/foundation_core/src/valtron/extensions/tasks/sendable.rs` + `non_sendable.rs` — split family: C1 park ✅ done; observer `Wait` fix + `readiness()` accessor (Resolutions 2, 3)
- `backends/foundation_core/src/valtron/extensions/streams/sendable.rs` + `non_sendable.rs` — stream split family: `force_push` → stash+`Wait` (Resolution 4); observer `Wait` fix + `readiness()` (Resolutions 2, 3)
- `backends/foundation_core/src/valtron/stream_future.rs` — existing `into_ready_future()`/`into_pending_future()` bridges (read-only); add `into_next_stream()` (Resolution 5)
- `backends/foundation_netio/src/simple_http/client/shared/request_task.rs` — `HttpExchange::Failed` → `Arc<dyn Error>` (Resolution 7)
- `backends/foundation_connectrpc/src/transport/base.rs` — `TransportError` → `Clone` (Resolution 7); `TransportStream`, `ByteSource`/`HeadSource` types
- `backends/foundation_connectrpc/src/transport/h1.rs` — Part D: split-based head/body + `Result` payloads
- `backends/foundation_connectrpc/src/transport/wasm.rs` — Part D: WASM pump (same split pattern)

## Language Stack

- **Rust** — all implementation

---

_Created: 2026-07-07_
