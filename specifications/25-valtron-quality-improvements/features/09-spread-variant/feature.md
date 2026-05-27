---
feature: spread-variant
description: Add a unified Spread variant to TaskStatus and Stream enums using TaskSpread/StreamSpread types, allowing tasks to emit multiple done and pending values in a single poll that get delivered individually at the delivery point
status: complete
priority: high
created: 2026-05-26
completed: 2026-05-27
tasks:
  completed: 29
  uncompleted: 0
  total: 29
  completion_percentage: 100
dependencies: []
---

# Feature 09: Unified Spread Variant for TaskStatus and Stream

## Problem

When a mapper or task wants to emit multiple discrete values in a single poll cycle, the
only option is to wrap them in a `Vec` — changing the type parameters from `D` to `Vec<D>`.
This forces all downstream consumers to handle the wrapped type, and the values arrive as a
single batched message rather than individual signals.

For example, the current `MapAllPendingAndDoneStream` mapper returns `Vec<Stream<O, R>>`,
which means downstream consumers receive `Stream::Next(Vec<Stream<O, R>>)`. If a user wants
each element delivered as its own message to a `NotifyQueue`, they must manually unpack and
re-push each element.

**Impact:** Users cannot emit multiple discrete signals from a single `next()` call while
preserving the original `D` and `P` types. The type parameter must change to accommodate the
batched form.

## Approach

Add a single `Spread` variant to both `TaskStatus` and `Stream` that carries a `Vec` of
**TaskSpread/StreamSpread** items — a lightweight enum that distinguishes `Done(D)` from
`Pending(P)` at the element level. At delivery points, each element is individually unwrapped
and pushed as `Ready`/`Next` or `Pending`.

### TaskSpread type

```rust
pub enum TaskSpread<D, P> {
    Ready(D),
    Pending(P),
}
```

### TaskStatus variant

```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    // ... existing variants ...

    /// Emit multiple values at once.
    /// Each TaskSpread element is delivered individually:
    ///   TaskSpread::Ready(d)  → TaskStatus::Ready(d)
    ///   TaskSpread::Pending(p) → TaskStatus::Pending(p)
    Spread(Vec<TaskSpread<D, P>>),
}
```

### StreamSpread type

```rust
pub enum StreamSpread<D, P> {
    Done(D),
    Pending(P),
}
```

### Stream variant

```rust
pub enum Stream<D, P> {
    // ... existing variants ...

    /// Emit multiple values at once.
    /// Each StreamSpread element is delivered individually:
    ///   StreamSpread::Done(d)    → Stream::Next(d)
    ///   StreamSpread::Pending(p) → Stream::Pending(p)
    Spread(Vec<StreamSpread<D, P>>),
}
```

### Conversion

The `From<TaskStatus<D, P, S>> for Stream<D, P>` impl maps `TaskSpread` elements to
`StreamSpread` elements:

```rust
impl<D, P, S: ExecutionAction> From<TaskStatus<D, P, S>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P, S>) -> Self {
        match val {
            TaskStatus::Spread(items) => {
                Stream::Spread(items.into_iter().map(|item| match item {
                    TaskSpread::Ready(d) => StreamSpread::Done(d),
                    TaskSpread::Pending(p) => StreamSpread::Pending(p),
                }).collect())
            }
            // ... existing arms unchanged ...
        }
    }
}
```

### PartialEq

`TaskSpread<D, P>` implements `PartialEq` when `D: PartialEq` and `P: PartialEq`:
`Ready(d1) == Ready(d2)` iff `d1 == d2`, `Pending(p1) == Pending(p2)` iff `p1 == p2`,
cross-type comparisons are `false`.

Similarly for `StreamSpread<D, P>`.

`TaskStatus::Spread` compares element-wise via the derived `PartialEq` on `Vec<TaskSpread>`.
Same for `Stream::Spread`.

### Where spreading happens

Spreading happens at **delivery points** in `task_iters.rs` (the three `ExecutionIterator`
impls that push values to `NotifyQueue`), and in **async futures** (`stream_future.rs`) where
futures poll `StreamIterator` and need to handle spread items.

There are three delivery points in `task_iters.rs`:

1. **`StreamConsumingIter::next()`** — pushes `Stream` to `NotifyQueue<Stream<Done, Pending>>`
2. **`ConsumingIter::next()`** — pushes `TaskStatus` to `NotifyQueue<TaskStatus<...>>`
3. **`ReadyConsumingIter::next()`** — pushes only `Ready`/`Wait` `TaskStatus` variants

### Design rationale

**Why a unified `Spread(Vec<TaskSpread<D, P>>)` instead of `SpreadDone(Vec<D>)` + `SpreadPending(Vec<P>)`?**

A unified `Spread` variant reduces the number of match arms from two to one across every
non-delivery-point site. The `TaskSpread`/`StreamSpread` inner enum cleanly separates done
from pending at the element level, so delivery points know exactly what to do with each item.

This eliminates the previous issue of trait ambiguity between `Iterator::any/count/fold`
and `TaskIteratorExt::any/count/fold` — with a single `Spread` variant there are fewer
specialized combinator paths, and the blanket `TaskIterator` impl for `Iterator` types works
correctly.

**Why not `Spread(Vec<Stream<D, P>>)` (full nested stream items)?**

Full nesting creates recursive complexity: mappers that change type parameters must
recursively map every inner item, creating complex type inference issues and combinatorial
explosion across 50+ match sites. `TaskSpread<D, P>` carries only raw values — mappers handle
them the same way they handle `Ready(D)` and `Pending(P)`.

**Why Vec?**

`Vec` is the simplest, zero-dependency ordered collection. Order matters — the elements
inside spread are delivered in sequence, preserving their original order.

## Testing

All tests go in a new file: `backends/foundation_core/tests/valtron/spread.rs`.
Inline tests in source files (`#[cfg(test)]` modules) are acceptable for pure type/conversion tests.

The new file must be registered in `backends/foundation_core/tests/valtron/mod.rs` with a
`mod spread;` declaration matching the existing pattern.

### Conversion tests

Verify the `From<TaskStatus<D, P, S>> for Stream<D, P>` impl correctly maps spread:

- `TaskStatus::Spread(vec![TaskSpread::Ready(x), TaskSpread::Pending(y)])` →
  `Stream::Spread(vec![StreamSpread::Done(x), StreamSpread::Pending(y)])`
- `TaskStatus::Spread(vec![TaskSpread::Ready(a), TaskSpread::Ready(b)])` →
  `Stream::Spread(vec![StreamSpread::Done(a), StreamSpread::Done(b)])`

### Delivery tests

Verify that at the `NotifyQueue` boundary, spread variants are expanded into individual messages:

- **`TaskStatus::Spread` delivery**: A task yielding `Spread(vec![TaskSpread::Ready(1),
  TaskSpread::Ready(2), TaskSpread::Ready(3)])` results in the `NotifyQueue` receiving three
  separate `Ready(1)`, `Ready(2)`, `Ready(3)` messages.
- **`TaskStatus::Spread` with mixed**: `Spread(vec![TaskSpread::Ready(1), TaskSpread::Pending("x")])`
  results in `Ready(1)` then `Pending("x")`.
- **`Stream::Spread` delivery**: A stream yielding `Spread(vec![StreamSpread::Done(1),
  StreamSpread::Done(2)])` results in the `NotifyQueue` receiving two separate `Next(1)`, `Next(2)` messages.
- **Order preservation**: Elements are delivered in the same order they appear in the `Vec`.

### Edge case tests

- Empty `Spread(vec![])` — delivers nothing, queue is not polluted.
- Single-element `Spread(vec![TaskSpread::Ready(1)])` — delivers exactly one `Ready(1)` message.
- All-pending spread — delivers only `Pending` messages.

### Mapper tests

Verify mappers transform spread elements correctly:

- **`map_done`**: `Spread(vec![TaskSpread::Ready(1), TaskSpread::Ready(2)])` →
  `Spread(vec![TaskSpread::Ready(10), TaskSpread::Ready(20)])` when mapper is `x * 10`.
  `TaskSpread::Pending` elements pass through unchanged.
- **`map_pending`**: `Spread(vec![TaskSpread::Pending("hi")])` →
  `Spread(vec![TaskSpread::Pending(2)])` when mapper is `|s| s.len()`.
  `TaskSpread::Ready` elements pass through unchanged.

### Async future tests

The `stream_future.rs` module contains futures that poll `StreamIterator`s.

- **`StreamCollectFuture`**: Verify that `Spread` items with `Done` elements contribute to the
  collected output. `Spread` with `Pending` elements returns `Poll::Pending`.
- **`StreamReadyFuture`**: Verify that `Spread` with `Done` elements returns the first done value.
  `Spread` with only `Pending` returns `Poll::Pending`.
- **`StreamPendingFuture`**: Verify that `Spread` with `Pending` elements returns the first
  pending value. `Spread` with only `Done` returns `Poll::Pending`.
- **`StreamAsFutureStream`**: Verify that `Spread` items are yielded individually across
  multiple `poll_next` calls. Needs a `pending_spread` buffer (`VecDeque<Stream<D, P>>`)
  to hold items between `poll_next` calls.

## Tasks

### Type changes

- [x] TASK-09-01: Fix `StreamSpread` duplicate `PartialEq` impl — remove the `#[derive(PartialEq)]` since there's a manual impl below it
- [x] TASK-09-02: Update `From<TaskStatus> for Stream` impl — map `TaskSpread` elements to `StreamSpread` elements within the single `Spread` variant
- [x] TASK-09-03: Update `PartialEq` impl for `TaskStatus` — handle `Spread` matching via element-wise comparison
- [x] TASK-09-04: Update `PartialEq` impl for `Stream` — handle `Spread` matching via element-wise comparison
- [x] TASK-09-05: Fix `Display`/`Debug` impls for `Stream` and `TaskStatus` — update `Spread` arm from `SpreadDone`/`SpreadPending` to `Spread`

### Non-delivery-point match arms

Replace all `SpreadDone(_)/SpreadPending(_)` match arms with a single `Spread(_)` arm across:

- [x] TASK-09-06: `task.rs` — From impl, PartialEq, TStatus helper enums, wrappers
- [x] TASK-09-07: `task_iters.rs` — three `ExecutionIterator` impls (StreamConsumingIter, ConsumingIter, ReadyConsumingIter)
- [x] TASK-09-08: `non_sendables.rs` (executors) — buffer and round-robin executor spread arms
- [x] TASK-09-09: `wrappers.rs` — passthrough spread arms
- [x] TASK-09-10: `extensions/streams/non_sendable.rs` — mapper impls. For `MapDone`, map `Done(d)` elements through mapper, leave `Pending` elements unchanged. For `MapPending`, map `Pending(p)` elements, leave `Done` unchanged
- [x] TASK-09-11: `extensions/tasks/non_sendable.rs` — all task combinators. `TMapReady` maps `Ready` elements in spread, passthrough `Pending`. `TMapPending` maps `Pending` elements, passthrough `Ready`. Combinators like `TAll`, `TAny`, `TCount`, `TFold`, `TFind`, etc. iterate spread elements and apply their logic to `Ready`/`Done` elements only
- [x] TASK-09-12: `store_state_task.rs` (foundation_db) — type conversion for spread elements
- [x] TASK-09-13: `notification_based_waiting.rs` (tests) — fix stream iterator spread pattern
- [x] TASK-09-14: `collect_next.rs`, `do_next.rs`, `on_next.rs` — add `Spread(_) => State::Pending(None)` fallback

### Delivery point spreading

Spreading happens in the three `ExecutionIterator` impls in `task_iters.rs`:

- [x] TASK-09-15: `StreamConsumingIter::next()` — iterate `Spread` items, map `TaskSpread::Ready(d)` → `Stream::Next(d)`, `TaskSpread::Pending(p)` → `Stream::Pending(p)`, push individually, handle backpressure
- [x] TASK-09-16: `ConsumingIter::next()` — iterate `Spread` items, map `TaskSpread::Ready(d)` → `TaskStatus::Ready(d)`, `TaskSpread::Pending(p)` → `TaskStatus::Pending(p)`, push individually, handle backpressure
- [x] TASK-09-17: `ReadyConsumingIter::next()` — iterate `Spread` items, push only `Ready` from `TaskSpread::Ready`, consume `TaskSpread::Pending` as `State::Pending`, handle backpressure

### Async future handling in `stream_future.rs`

- [x] TASK-09-18: Update `StreamCollectFuture` — handle `Spread` by collecting `Done` items, `Pending` items return `Poll::Pending`
- [x] TASK-09-19: Update `StreamReadyFuture` — handle `Spread` by returning first `Done` item
- [x] TASK-09-20: Update `StreamPendingFuture` — handle `Spread` by returning first `Pending` item
- [x] TASK-09-21: Update `StreamAsFutureStream` — handle `Spread` by iterating elements inline, returning first `Done`/`Pending` as appropriate

### Testing

- [x] TASK-09-22: Rewrite test file `spread.rs` with unified `Spread` + `TaskSpread`/`StreamSpread` types
- [x] TASK-09-23: Conversion test — `TaskStatus::Spread` → `Stream::Spread` with mixed elements
- [x] TASK-09-24: Delivery tests — spread expands to individual messages at `NotifyQueue` boundary
- [x] TASK-09-25: Edge cases — empty spread, single-element spread, all-pending spread
- [x] TASK-09-26: Mapper tests — `map_done`/`map_pending` transform correct spread elements
- [x] TASK-09-27: Async future tests — `StreamCollectFuture`, `StreamReadyFuture`, `StreamPendingFuture`, `StreamAsFutureStream`
- [x] TASK-09-28: Tokio and smol async tests with spread
- [x] TASK-09-29: Combinator tests — `enumerate`, `find`, `fold`, `all`, `any`, `count` with spread
