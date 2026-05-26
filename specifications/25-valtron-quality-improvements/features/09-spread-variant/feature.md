---
feature: spread-variant
description: Add SpreadDone and SpreadPending variants to TaskStatus and Stream enums allowing tasks to emit multiple values in a single poll that get delivered individually at the delivery point
status: pending
priority: high
created: 2026-05-26
tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0
dependencies: []
---

# Feature 09: SpreadDone and SpreadPending Variants for TaskStatus and Stream

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

Add type-specific spread variants to both `TaskStatus` and `Stream` that carry a `Vec` of
**raw values** (not wrapped variants). At delivery points, each value in the `Vec` is wrapped
in its corresponding variant and pushed individually.

### TaskStatus variants

```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    // ... existing variants ...

    /// Emit multiple ready values at once.
    /// Each element is delivered individually as `TaskStatus::Ready(value)`.
    SpreadDone(Vec<D>),

    /// Emit multiple pending values at once.
    /// Each element is delivered individually as `TaskStatus::Pending(value)`.
    SpreadPending(Vec<P>),
}
```

### Stream variants

```rust
pub enum Stream<D, P> {
    // ... existing variants ...

    /// Emit multiple next values at once.
    /// Each element is delivered individually as `Stream::Next(value)`.
    SpreadDone(Vec<D>),

    /// Emit multiple pending values at once.
    /// Each element is delivered individually as `Stream::Pending(value)`.
    SpreadPending(Vec<P>),
}
```

### Conversion

The `From<TaskStatus<D, P, S>> for Stream<D, P>` impl maps the spread variants directly:

```rust
impl<D, P, S: ExecutionAction> From<TaskStatus<D, P, S>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P, S>) -> Self {
        match val {
            TaskStatus::SpreadDone(items) => Stream::SpreadDone(items),
            TaskStatus::SpreadPending(items) => Stream::SpreadPending(items),
            // ... existing arms unchanged ...
        }
    }
}
```

### PartialEq

`SpreadDone` matches `SpreadDone` when both contain the same number of elements and all
elements pairwise equal (`D: PartialEq`). `SpreadPending` matches `SpreadPending` similarly
(`P: PartialEq`).

### Where spreading happens

Spreading happens at **delivery points** in `task_iters.rs` (the three `ExecutionIterator`
impls that push values to `NotifyQueue`), and in **async futures** (`stream_future.rs`) where
futures poll `StreamIterator` and need to handle spread items.

There are three delivery points in `task_iters.rs`:

1. **`StreamConsumingIter::next()`** — pushes `Stream` to `NotifyQueue<Stream<Done, Pending>>`
2. **`ConsumingIter::next()`** — pushes `TaskStatus` to `NotifyQueue<TaskStatus<...>>`
3. **`ReadyConsumingIter::next()`** — pushes only `Ready`/`Wait` `TaskStatus` variants

### Design rationale

**Why `SpreadDone(Vec<D>)` and `SpreadPending(Vec<P>)` instead of `Spread(Vec<Stream<D, P>>)`?**

A `Spread(Vec<Stream<D, P>>)` carries full stream items, which means mappers that change type
parameters (`Stream<D, P>` → `Stream<R, P>`) must recursively map every inner item. This
creates complex type inference issues and combinatorial explosion across 50+ match sites.

`SpreadDone(Vec<D>)` and `SpreadPending(Vec<P>)` carry only raw values. Mappers handle them
the same way they handle `Next(D)` and `Pending(P)` — just apply the mapper function to the
`Vec<D>` or `Vec<P>`. No nested structures, no recursive flattening, no type parameter
complexity.

**Why not just let the iterator yield multiple items?**

`Iterator::next()` returns a single `Option<T>`. A task cannot yield two values from one
`next()` call. Spread provides a way to emit multiple discrete signals through a single
`next()` return, which the delivery point then expands.

**Why Vec?**

`Vec` is the simplest, zero-dependency ordered collection. Order matters — the elements
inside spread are delivered in sequence, preserving their original order.

## Testing

All tests go in a new file: `backends/foundation_core/tests/valtron/spread.rs`.
Inline tests in source files (`#[cfg(test)]` modules) are acceptable for pure type/conversion tests.

The new file must be registered in `backends/foundation_core/tests/mod.rs` with a conditional
`mod spread;` declaration matching the existing pattern.

### Conversion tests

Verify the `From<TaskStatus<D, P, S>> for Stream<D, P>` impl correctly maps spread variants:

- `TaskStatus::SpreadDone(vec![x, y])` → `Stream::SpreadDone(vec![x, y])`
- `TaskStatus::SpreadPending(vec![a, b])` → `Stream::SpreadPending(vec![a, b])`

### Delivery tests

Verify that at the `NotifyQueue` boundary, spread variants are expanded into individual messages:

- **`TaskStatus::SpreadDone` delivery**: A task yielding `SpreadDone(vec![1, 2, 3])` results
  in the `NotifyQueue` receiving three separate `Ready(1)`, `Ready(2)`, `Ready(3)` messages.
- **`TaskStatus::SpreadPending` delivery**: A task yielding `SpreadPending(vec!["a", "b"])`
  results in the `NotifyQueue` receiving two separate `Pending("a")`, `Pending("b")` messages.
- **`Stream::SpreadDone` delivery**: A stream yielding `SpreadDone(vec![1, 2])` results in the
  `NotifyQueue` receiving two separate `Next(1)`, `Next(2)` messages.
- **`Stream::SpreadPending` delivery**: A stream yielding `SpreadPending(vec!["x"])` results
  in the `NotifyQueue` receiving one `Pending("x")` message.
- **Order preservation**: Elements are delivered in the same order they appear in the `Vec`.

### Edge case tests

- Empty `SpreadDone(vec![])` — delivers nothing, queue is not polluted.
- Empty `SpreadPending(vec![])` — delivers nothing, queue is not polluted.
- Single-element `SpreadDone(vec![1])` — delivers exactly one `Ready(1)` message.
- Single-element `SpreadPending(vec!["x"])` — delivers exactly one `Pending("x")` message.

### Async future tests

The `stream_future.rs` module contains futures that poll `StreamIterator`s.

- **`StreamCollectFuture`**: Verify that `SpreadDone(vec![v1, v2, v3])` contributes all
  values to the collected output. `SpreadPending` inside a collect returns `Poll::Pending`
  (consistent with original `Pending` behavior).
- **`StreamReadyFuture`**: Verify that `SpreadDone(vec![v1, v2])` returns the first value.
  If the `Vec` has multiple items, the first is returned and remaining are discarded (this is
  a single-return Future). `SpreadPending` returns `Poll::Pending`.
- **`StreamPendingFuture`**: Verify that `SpreadPending(vec![p1, p2])` returns the first
  pending value. `SpreadDone` returns `Poll::Pending` (consistent with original `Next` behavior).
- **`StreamAsFutureStream`**: Verify that `SpreadDone` items are yielded individually across
  multiple `poll_next` calls. Same for `SpreadPending`. Needs a `pending_spread` buffer
  (`VecDeque<Stream<D, P>>`) to hold items between `poll_next` calls.

## Tasks

### Type changes

- [ ] TASK-09-01: Add `SpreadDone(Vec<D>)` variant to `TaskStatus` enum in `task.rs`. Include doc comments explaining it wraps multiple done values delivered individually at the delivery point
- [ ] TASK-09-02: Add `SpreadPending(Vec<P>)` variant to `TaskStatus` enum in `task.rs`. Include doc comments explaining it wraps multiple pending values delivered individually at the delivery point
- [ ] TASK-09-03: Add `SpreadDone(Vec<D>)` variant to `Stream` enum in `streams.rs`. Include doc comments explaining it wraps multiple next values delivered individually at the delivery point
- [ ] TASK-09-04: Add `SpreadPending(Vec<P>)` variant to `Stream` enum in `streams.rs`. Include doc comments explaining it wraps multiple pending values delivered individually at the delivery point
- [ ] TASK-09-05: Update `From<TaskStatus> for Stream` impl in `task.rs` to handle `SpreadDone` → `SpreadDone` and `SpreadPending` → `SpreadPending`
- [ ] TASK-09-06: Update `PartialEq` impl for `TaskStatus` in `task.rs` to handle `SpreadDone` and `SpreadPending` matching (pairwise element equality). `D: PartialEq, P: PartialEq` bounds required

### Non-delivery-point match arms

Add passthrough or `State::Pending(None)` fallback arms for the two new variants in all
non-delivery-point matches across the valtron module:

- [ ] TASK-09-07: `do_next.rs` — add `TaskStatus::SpreadDone(_) | SpreadPending(_) => State::Pending(None)`
- [ ] TASK-09-08: `collect_next.rs` — add same fallback
- [ ] TASK-09-09: `on_next.rs` — add same fallback
- [ ] TASK-09-10: `wrappers.rs` — add passthrough `SpreadDone(s) => SpreadDone(s), SpreadPending(s) => SpreadPending(s)`
- [ ] TASK-09-11: `non_sendables.rs` (executors) — add `Stream::SpreadDone(_)/SpreadPending(_)` arms
- [ ] TASK-09-12: `extensions/streams/non_sendable.rs` — add spread arms to mapper impls. For simple mappers like `MapDone`, map `SpreadDone(v)` → `SpreadDone(v.iter().map(mapper).collect())`. For `MapPending`, map `SpreadPending(v)` similarly. For iterator-focused mappers (`MapIterDone`, `MapIterPending`), pass spread through as-is
- [ ] TASK-09-13: `extensions/tasks/non_sendable.rs` — add `TaskStatus::SpreadDone/SpreadPending` passthrough arms

### Delivery point spreading

Spreading happens in the three `ExecutionIterator` impls in `task_iters.rs` where values are
pushed to the `NotifyQueue`. **Do not modify the `NotifyQueue` struct itself** — it is generic
over what it sends. The spread logic goes at the point where we decide what to push.

There are three delivery points, all in `backends/foundation_core/src/valtron/executors/task_iters.rs`:

1. **`StreamConsumingIter::next()`** (line ~175-266): Matches on `TaskStatus` variants, converts
   them to `Stream` variants and pushes to `NotifyQueue<Stream<Done, Pending>>`.
   - `TaskStatus::SpreadDone(items)` → for each item: push `Stream::Next(item)` to channel
   - `TaskStatus::SpreadPending(items)` → for each item: push `Stream::Pending(item)` to channel
   - If channel goes full mid-spread, re-wrap remaining elements as the appropriate spread
     variant and store in `pending_msg` for retry.

2. **`ConsumingIter::next()`** (line ~434-534): Matches on `TaskStatus` variants and pushes
   `TaskStatus` variants to `NotifyQueue<TaskStatus<Done, Pending, Action>>`.
   - `TaskStatus::SpreadDone(items)` → for each item: push `TaskStatus::Ready(item)` to channel
   - `TaskStatus::SpreadPending(items)` → for each item: push `TaskStatus::Pending(item)` to channel
   - Same backpressure: re-wrap remaining elements in `pending_msg`.

3. **`ReadyConsumingIter::next()`** (line ~688-726): Only pushes `TaskStatus::Ready` and
   `TaskStatus::Wait` to `NotifyQueue<TaskStatus<Done, Pending, Action>>`. All other variants
   map to `State::Pending`.
   - `TaskStatus::SpreadDone(items)` → for each item: push `TaskStatus::Ready(item)` (only Ready is pushed)
   - `TaskStatus::SpreadPending(items)` → consume without pushing (maps to State::Pending, consistent with this iterator's filtering)
   - Handle backpressure: re-wrap remaining elements in `pending_msg`.

- [ ] TASK-09-14: `StreamConsumingIter::next()` — add spread arms, iterate and push individually, handle backpressure
- [ ] TASK-09-15: `ConsumingIter::next()` — add spread arms, iterate and push individually, handle backpressure
- [ ] TASK-09-16: `ReadyConsumingIter::next()` — add spread arms, push only Ready from SpreadDone, consume SpreadPending as Pending, handle backpressure

### Async future handling in `stream_future.rs`

- **`StreamCollectFuture`**: For loop over `SpreadDone(items)`, push each into `collected`.
  `SpreadPending` → `Poll::Pending` (iterator signaled not-ready).
- **`StreamReadyFuture`**: For loop over `SpreadDone(items)`, return first value.
  `SpreadPending` → `Poll::Pending`.
- **`StreamPendingFuture`**: For loop over `SpreadPending(items)`, return first value.
  `SpreadDone` → `Poll::Pending` (consistent with original `Next` behavior).
- **`StreamAsFutureStream`**: Needs `pending_spread: VecDeque<Stream<D, P>>` field — the only
  construct requiring additional state because one spread yield maps to many `poll_next` calls.
  When iterator yields `SpreadDone(items)`, extend as `items.into_iter().map(Stream::Next)`.
  When iterator yields `SpreadPending(items)`, extend as `items.into_iter().map(Stream::Pending)`.
  Each `poll_next` pops one item from deque and returns it.

### stream_future.rs tasks

- [ ] TASK-09-17: Update `StreamCollectFuture` — handle `SpreadDone` by collecting items, `SpreadPending` returns `Poll::Pending`
- [ ] TASK-09-18: Update `StreamReadyFuture` — handle `SpreadDone` by returning first item, `SpreadPending` returns `Poll::Pending`
- [ ] TASK-09-19: Update `StreamPendingFuture` — handle `SpreadPending` by returning first item, `SpreadDone` returns `Poll::Pending`
- [ ] TASK-09-20: Update `StreamAsFutureStream` — add `pending_spread: VecDeque` field, expand `SpreadDone` into `Next` items, `SpreadPending` into `Pending` items, yield one-per-`poll_next`

### Testing

- [ ] TASK-09-21: Add conversion test — verify `TaskStatus::SpreadDone/Pending` convert to `Stream::SpreadDone/Pending`
- [ ] TASK-09-22: Add `Stream::SpreadDone` delivery test — verify `SpreadDone(vec![1, 2, 3])` yields three separate `Next` messages
- [ ] TASK-09-23: Add `Stream::SpreadPending` delivery test — verify `SpreadPending(vec!["a", "b"])` yields two separate `Pending` messages
- [ ] TASK-09-24: Add `TaskStatus::SpreadDone` delivery test — verify `SpreadDone(vec![1, 2])` yields two separate `Ready` messages
- [ ] TASK-09-25: Add `TaskStatus::SpreadPending` delivery test — verify `SpreadPending(vec!["x"])` yields one `Pending` message
- [ ] TASK-09-26: Add empty spread edge case tests — verify empty spread delivers nothing
- [ ] TASK-09-27: Add single-element spread edge case tests — verify single-element spread delivers exactly one message
- [ ] TASK-09-28: Add `StreamCollectFuture` spread test — verify `SpreadDone` values all collected, `SpreadPending` triggers `Poll::Pending`
- [ ] TASK-09-29: Add `StreamReadyFuture` spread test — verify `SpreadDone` returns first value, `SpreadPending` triggers `Poll::Pending`
- [ ] TASK-09-30: Add `StreamPendingFuture` spread test — verify `SpreadPending` returns first value, `SpreadDone` triggers `Poll::Pending`
- [ ] TASK-09-31: Add `StreamAsFutureStream` spread test — verify spread items yielded individually across multiple `poll_next` calls
