---
feature: "Delivery & fan-out backpressure — enforce Decision 00 §L1b in valtron's own queues; N-way split; shrink Pipe"
description: "Wire QueueVacancyReadiness into the executor's result-delivery iterators and the split combinators (no bare Pending / no force_push drop), add an N-way split, then migrate the transport seam off hand-rolled Pipes for the output/fan-out direction — Pipe survives only for caller→task input"
status: "pending"
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

### Part D — Shrink Pipe in the transport seam (the payoff)

With A + C in place, rework the H1 pump (and the WASM pump) so the response side
uses native fan-out instead of hand-rolled pipes:

- The pump task's `Ready` type is already `HttpExchange` (Head / BodyChunk /
  Failed). **`split_collect_one(|x| matches!(x, HttpExchange::Head{..}))`** peels
  the head into an observer; the continuation carries body chunks. No `head`
  Pipe, no `recv_body` Pipe, no `map_ready` side-effect closure.
- `TransportStream.head` / `.recv_body` become the split's observer / continuation
  receivers (bounded via `queue_size`, backpressured via C1).
- **`Pipe` survives for exactly one role:** `send_body` — the **caller→task
  input** channel (request bytes pushed into the running pump). Splits fan
  *outputs* out; nothing here injects inputs, so this is the irreducible Pipe.
- **Error propagation rides the payload** (this is where the earlier discussion
  lands): the head observer item type is
  `Result<(Status, SimpleHeaders), TransportError>` and the body continuation item
  is `Result<Bytes, TransportError>`, so a `HttpExchange::Failed` (which can fire
  **pre-head** *and* **mid-body**, per `http_exchange_task.rs:133,172,217`) is
  delivered as `Err`, not a silent `None`. `send_body` stays plain `Bytes`
  (caller→task; no back-flowing error).

## Scope

- Part A (+A0): audit **all** `ExecutionIterator` impls (the task wrappers) per
  the discrimination rule; the only edits are the `PushError::Full` arms in
  `StreamConsumingIter` / `ConsumingIter` / `ReadyConsumingIter` (native +
  `non_sendable`) → `Depends(QueueVacancyReadiness)`; add `NotifyQueue::queue()`
  Arc accessor. Confirm pass-throughs and `OnNext`/`DoNext`/`CollectNext` forward
  `Depends` (no change); do **not** touch `Init`/`Ignore`/`Spread`/`Pending`.
- Part B (Decision 15): `sequenced_iter` / `stream_sequenced_iter*` allocate
  `NotifyQueue::bounded(DEFAULT_DELIVERY_CAPACITY)` instead of `unbounded()`; add
  the const. `lift` and all other paths untouched.
- Part C1: replace `force_push` with vacancy-park across the existing `split_*`
  family; define observer-dropped behaviour (default = backpressure; drop is a
  separate loud opt-in).
- Part D: migrate `H1Transport::open` (and WASM pump) to `split_collect_one` for
  head/body; `Result`-typed head/body payloads; keep `send_body` as the only
  Pipe. Update `round_trip` + F44 transport tests accordingly.

## Out of scope

- Changing the **global** `execute`/`schedule` default from `unbounded` to
  `bounded` (Part B bounds only the `sequenced` path per Decision 15; every other
  path keeps `unbounded`).
- The Decision 11 seam `FramePipe` itself (00-F4 — already done for the seam).
- The reactor / real-I/O parking (Decision 00 §L2, `foundation_nativeapis`).
- HTTP/2/3/WS transports (they inherit the shrink once D lands for H1).

## Open questions (resolve during implementation)

1. **`NotifyQueue` producer wake for promptness — RESOLVED: gate on bounded.**
   Task-path parking re-checks `is_ready` on the sleeper cadence
   (`DEFAULT_READINESS_WAIT` = **10ms**, `constants.rs:56`) — correct but a 10ms
   worst-case unpark stall, which bites seam streaming. A naive "mirror the
   consumer condvar" **does not work here**: the delivery producer is a *task*
   parked in `Sleepers`, not a *thread* blocked on a condvar, so a producer
   condvar wakes nothing. Instant wake instead requires `pop()` to interrupt the
   producer's executor (`Sleepers::wake` / `yielders.interrupt_all`), which taxes
   the **hot `pop` path** and adds a `NotifyQueue → executor` coupling edge.
   **Resolution:** only **bounded** queues ever park a producer (unbounded's
   `is_full()` is never true), so **gate the producer-wake on `bounded`**. The
   `unbounded` default (the common path) takes the branch never and pays ~nothing;
   bounded queues (opt-in, seam-critical) get instant unpark. "Only seam queues"
   and "always" converge — bounded *is* the fireable set. Still land Part A's
   correct-but-cadence-paced parking first; the wake is a latency optimization on
   top, not a correctness requirement. **Default stance:** rely on
   `QueueVacancyReadiness` alone; only add the bounded-gated producer wake if it
   proves genuinely cost-free (unbounded never parks anyway, so nothing is lost by
   deferring it).
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
4. **`Result` payload vs. the split's `Clone` bound — RESOLVED: fix the bound.**
   The head/body split requires `Ready: Clone`; carry `Result<…, TransportError>`
   by making `TransportError: Clone`, or, if that's undesirable, project a
   cloneable error subset via `split_collect_one_map`. Decide the exact route in
   Part D; either way the error rides the payload.

### Carve-out: N-way broadcast is its own feature

Part D (the Pipe shrink) only needs the **existing 2-way** `split_collect_one`
(head peel + body continue) plus Part C1's backpressure. The **N-way** broadcast
(`split_n` and the two named policies below) has **no consumer in Part D** and is
therefore carved into its own follow-on feature so 45 stays focused on
"backpressure + shrink Pipe." The two policies that feature will expose, per the
resolutions above:

- **`broadcast(n, queue_size)`** — backpressured, zero loss, slowest gates
  (`queue_size = 1` = strict lockstep; `> 1` = buffered slack). The safe default.
- **`broadcast_lossy(n, queue_size)`** — explicit, loud opt-in; a full branch
  loses the item (today's silent `force_push`, made visible). Source never stalls
  on a slow observer.

Active **eviction** (removing a branch after K misses) is deferred until a
concrete best-effort observer needs it — dropping the receiver already gives a
clean manual opt-out.

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
- `H1Transport::open` uses `split_collect_one` for head/body; the only remaining
  `Pipe` in the transport is `send_body`. A pre-head **and** a mid-body
  `HttpExchange::Failed` are observed as `Err` on the head / body receiver
  respectively (no silent `None`).
- All existing valtron tests green on `single` (wasm) and `multi` (native);
  use `#[valtron_test]`, never `#[test]`/`#[serial]`.

## Module references

- `backends/foundation_core/src/valtron/executors/task_iters.rs` — `*ConsumingIter` Full arms (Part A edits)
- `backends/foundation_core/src/valtron/executors/{on_next,do_next,collect_next}.rs` — audit only (already forward `Depends`)
- `backends/foundation_core/src/valtron/executors/dependent_lift.rs` — linked-task wrappers (audit only; default policy = Decision 15)
- `backends/foundation_core/src/valtron/task.rs` — pass-through `ExecutionIterator` impls (audit only)
- `backends/foundation_core/src/valtron/executors/local.rs` — `NotifyQueue` (`queue()` accessor), receiver
- `backends/foundation_core/src/valtron/executors/multi/mod.rs` — `iter_chan` allocation, `channel_capacity`
- `backends/foundation_core/src/valtron/extensions/tasks/sendable.rs` + `non_sendable.rs` — `split_*` family, `force_push` → park, `split_n` (Part C)
- `backends/foundation_core/src/valtron/task.rs` — `QueueVacancyReadiness`, `AnyReadiness` (reused as-is)
- `backends/foundation_connectrpc/src/transport/{base,h1,wasm}.rs` — Pipe shrink + `Result` payloads (Part D)

## Language Stack

- **Rust** — all implementation

---

_Created: 2026-07-07_
