# Decision 15: `sequenced` Delivery Is Bounded (Backpressured Interleave)

> **Status:** narrow correctness fix, not a semantics change. Task-execution mode
> is already chosen explicitly at the call site (`lift()` vs `sequenced()`), and
> that selection stays exactly as-is. The **only** change: the `sequenced`
> delivery queue becomes **bounded** so the interleaved parent actually
> backpressures the child, instead of the child buffering without limit through
> an unbounded hand-off. Depends on **Feature 45 Part A** (a full bounded queue
> must park on vacancy, not busy-spin). Small blast radius — `lift`, `execute`,
> and all explicit selections are untouched.

## Context

Callers already state how a task runs — nothing is implicit:

- **`lift()` → `FinishChildBeforeParentTask`** (`dependent_lift.rs`, `local.rs:1229,1281`):
  the child is driven `next`-until-`None` **before the parent makes any
  progress**. The parent (typically the downstream consumer) does not run while
  the child produces.
- **`sequenced()` → `DualSequeunceChildAndParentLinkedTask`** (`local.rs:1318`):
  each child `next` also drives one parent `next` — **the parent consumes as the
  child produces**. This type exists specifically to pace the child against the
  parent.

So mode selection is explicit and correct. The defect is one level down, in the
queue each mode hands off through.

### The defect: `sequenced` hands off through an *unbounded* queue

The `sequenced` builders hardcode an unbounded delivery queue
(`builders.rs:319` `sequenced_iter`, `builders.rs:368`
`stream_sequenced_iter_with_config`):

```rust
let iter_chan = Arc::new(NotifyQueue::unbounded());   // ← sequenced, unbounded
```

`sequenced`'s whole premise is that the parent drains as the child produces — but
with an unbounded hand-off, a child that outruns the interleaved parent still
**buffers without limit**. The pacing the mode promises isn't actually enforced;
the interleave only bounds memory if the parent happens to keep up.

### Why this is a `sequenced`-only change — the coupling

Bounded-ness is correct for `sequenced` and **wrong** for `lift`:

| | **unbounded** | **bounded** |
|---|---|---|
| **`sequenced` (parent drains as child produces)** | today: buffers if parent lags | ✅ **the fix** — child parks on vacancy, parent drains, paced |
| **`lift` (finish child before parent)** | ✅ **required** — parent can't drain until child `Done` | 💀 deadlock (child parks on vacancy that never comes) |

Under `lift`, a bounded queue would **deadlock**: with Feature 45 Part A a full
bounded queue parks the child on `QueueVacancyReadiness`, but the parent — the
only drainer — can't run until the child is `Done`, so vacancy never comes. So
`lift` keeps its unbounded queue (it is load-bearing there, not a footgun), and
only `sequenced` moves to bounded.

## Decision

1. **Keep explicit mode selection unchanged.** `lift()` stays
   `FinishChildBeforeParent` + unbounded; `sequenced()` stays `DualSequence`;
   `execute()` / `schedule` / everything else is untouched. No default flip, no
   caller migration.

2. **`sequenced` delivery queue becomes bounded.** The `sequenced_iter` /
   `stream_sequenced_iter*` builders allocate
   `NotifyQueue::bounded(DEFAULT_DELIVERY_CAPACITY)` instead of `unbounded()`.
   Paired with Feature 45 Part A (the child's `*ConsumingIter` parks on
   `QueueVacancyReadiness` when full), this makes the interleave genuinely
   backpressured — the child pauses when the parent lags, memory stays bounded.

3. **`DEFAULT_DELIVERY_CAPACITY`** — a small, documented default that favours
   pacing over buffering; a `*_with_capacity` override is available for bursty
   producers. Err small (a large default is unbounded with extra steps).

4. **Record the coupling as an invariant** (rationale for anyone touching this):
   `sequenced` ⇒ bounded, `lift` ⇒ unbounded. The two illegal cells
   (`lift`+bounded = deadlock, `sequenced`+unbounded = the buffering this fixes)
   are not offered as configurations.

## Ordering

`sequenced`-bounded is only viable once a full bounded queue **parks** the
producer instead of returning bare `Pending(None)` and busy-spinning — that is
Feature 45 Part A. So Part A lands first, then this one-line queue change.

## Consequences

- **`sequenced` pipelines now pace to the parent** and stay memory-bounded — the
  guarantee the mode was always meant to give.
- **A `sequenced` pipeline whose "parent" doesn't actually drain in lockstep** (a
  degenerate wiring) now *parks* where it previously grew memory. This is a
  louder, better failure than silent unbounded growth, but it is a behaviour
  change for such a pipeline — document "a bounded `sequenced` stage with no live
  drainer parks; that's a wiring bug surfaced, not a regression."
- **`lift` unchanged**, so nothing that relied on finish-before + unbounded
  buffering is affected. No audit of existing callers needed.
- **Relation to Feature 45 / Decision 00 §L1b.** This decision just points the
  `sequenced` builders at a bounded queue; Feature 45 supplies the mechanism
  (park-on-vacancy) that makes that bound safe rather than a spin.

## Change surface

- `backends/foundation_core/src/valtron/executors/builders.rs` — `sequenced_iter`
  (`:319`) and `stream_sequenced_iter_with_config` (`:368`): `NotifyQueue::unbounded()`
  → `NotifyQueue::bounded(DEFAULT_DELIVERY_CAPACITY)`.
- `backends/foundation_core/src/valtron/executors/constants.rs` — add
  `DEFAULT_DELIVERY_CAPACITY`.
- (No changes to `lift`, `execute`, `schedule`, or the linked-task types.)

## Success criteria

- `sequenced` delivers through a bounded queue; a child that outruns the
  interleaved parent **parks on vacancy** (turn-count flat while full) and resumes
  as the parent drains — memory stays bounded.
- `lift` still uses an unbounded queue and its finish-before behaviour is
  unchanged; no `lift` pipeline deadlocks.
- No behaviour change for `execute` / `schedule` / explicit non-`sequenced` paths.
- All valtron tests green on `single` (wasm) and `multi` (native); `#[valtron_test]` only.

## Module references

- `backends/foundation_core/src/valtron/executors/builders.rs` — `sequenced_iter`, `stream_sequenced_iter*` (the two-line change)
- `backends/foundation_core/src/valtron/executors/dependent_lift.rs` — `FinishChildBeforeParentTask` (`lift`), `DualSequeunceChildAndParentLinkedTask` (`sequenced`)
- `backends/foundation_core/src/valtron/executors/local.rs` — `SpawnType::{Lifted,Sequenced}` wiring (1229/1281/1318)
- `specifications/41-connectrpc/features/45-delivery-fanout-backpressure/` — Part A (park-on-vacancy) that this depends on

---

_Created: 2026-07-07_
