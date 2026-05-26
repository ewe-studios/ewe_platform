---
feature: spread-variant
description: Add Spread variant to TaskStatus and Stream enums allowing tasks to emit multiple values in a single poll that get delivered individually at the delivery point
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

# Feature 09: Spread Variant for TaskStatus and Stream

## Problem

When a mapper or task wants to emit multiple `Stream<D, P>` or `TaskStatus<D, P, S>` values
in a single poll cycle, the only option is to wrap them in a `Vec` — changing the type
parameters from `D` to `Vec<D>`. This forces all downstream consumers to handle the wrapped
type, and the values arrive as a single batched message rather than individual signals.

For example, the current `MapAllPendingAndDoneStream` mapper returns `Vec<Stream<O, R>>`,
which means downstream consumers receive `Stream::Next(Vec<Stream<O, R>>)`. If a user wants
each element delivered as its own message to a `NotifyQueue`, they must manually unpack and
re-push each element.

**Impact:** Users cannot emit multiple discrete signals from a single `next()` call while
preserving the original `D` and `P` types. The type parameter must change to accommodate the
batched form.

## Approach

Add a `Spread` variant to both `TaskStatus` and `Stream` that carries a `Vec` of inner
values. At delivery points — where values are pushed to a `NotifyQueue`, written to a
channel, or handed to the executor's consumer — the `Spread` is detected and each inner
element is delivered individually.

### TaskStatus::Spread

```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    // ... existing variants ...

    /// Emit multiple task statuses at once.
    /// Each element is delivered individually at the delivery point
    /// (e.g., pushed as separate messages to a NotifyQueue).
    Spread(Vec<TaskStatus<D, P, S>>),
}
```

### Stream::Spread

```rust
pub enum Stream<D, P> {
    // ... existing variants ...

    /// Emit multiple stream states at once.
    /// Each element is delivered individually at the delivery point.
    Spread(Vec<Stream<D, P>>),
}
```

### Conversion

The `From<TaskStatus<D, P, S>> for Stream<D, P>` impl must handle `Spread` by mapping each
inner `TaskStatus` into its corresponding `Stream` variant and wrapping them in
`Stream::Spread`:

```rust
impl<D, P, S: ExecutionAction> From<TaskStatus<D, P, S>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P, S>) -> Self {
        match val {
            TaskStatus::Spread(items) => {
                Stream::Spread(items.into_iter().map(Stream::from).collect())
            }
            // ... existing arms unchanged ...
        }
    }
}
```

### PartialEq

`Spread` matches `Spread` when both contain the same number of elements and all elements
pairwise equal (requires `D: PartialEq, P: PartialEq`).

### Spreading strategy: VecDeque with recursive flattening

At delivery points and in async futures, items from a `Spread` are stored in a `VecDeque`
and consumed from the front for O(1) operations. If an item popped from the deque is itself
a `Spread`, its inner items are extended into the back of the deque. This recursively
flattens arbitrarily nested `Spread` structures without recursion or stack overflow.

Example at delivery point:
```
Stream::Spread(vec![
    Stream::Next(1),
    Stream::Spread(vec![Stream::Next(2), Stream::Spread(vec![Stream::Next(3)])]),
    Stream::Next(4),
])
```

Deque operations:
1. Pop `Next(1)` → deliver
2. Pop `Spread([Next(2), Spread([Next(3)])])` → extend deque with `[Next(2), Spread([Next(3)])]`
3. Pop `Next(2)` → deliver
4. Pop `Spread([Next(3)])` → extend deque with `[Next(3)]`
5. Pop `Next(3)` → deliver
6. Pop `Next(4)` → deliver

**Note:** `Vec` cannot contain cycles in Rust, so this is guaranteed to terminate.

### Where spreading happens

Spreading happens at **delivery points** in `task_iters.rs` (the three `ExecutionIterator`
impls that push values to `NotifyQueue`), and in **async futures** (`stream_future.rs`) where
futures poll `StreamIterator` and need to handle `Spread` items properly.

There are three delivery points in `task_iters.rs`:

1. **`StreamConsumingIter::next()`** — pushes `Stream` to `NotifyQueue<Stream<Done, Pending>>`
2. **`ConsumingIter::next()`** — pushes `TaskStatus` to `NotifyQueue<TaskStatus<...>>`
3. **`ReadyConsumingIter::next()`** — pushes only `Ready`/`Wait` `TaskStatus` variants

### Design rationale

**Why not just let the iterator yield multiple items?**

`Iterator::next()` returns a single `Option<T>`. A task cannot yield two values from one
`next()` call. `Spread` provides a way to emit multiple discrete signals through a single
`next()` return, which the delivery point then expands.

**Why spread only at the notification queue boundary?**

Auto-expanding `Spread` everywhere would break combinators that expect one `next()` call =
one yielded item. The notification queue is the right boundary because it's where values
cross from the producer's control into consumer-facing channels — the consumer naturally
expects discrete, individual messages there.

**Why Vec and not some other collection?**

`Vec` is the simplest, zero-dependency ordered collection. Order matters — the elements
inside `Spread` are delivered in sequence, preserving their original order.

## Testing

All tests go in a new file: `backends/foundation_core/tests/valtron/spread.rs`.
Inline tests in source files (`#[cfg(test)]` modules) are acceptable for pure type/conversion tests.

The new file must be registered in `backends/foundation_core/tests/mod.rs` with a conditional
`mod spread;` declaration matching the existing pattern.

### Conversion tests

Verify the `From<TaskStatus<D, P, S>> for Stream<D, P>` impl correctly maps `Spread` to
`Spread` with each inner element transformed:

- `TaskStatus::Ready(x)` → `Stream::Next(x)`
- `TaskStatus::Pending(x)` → `Stream::Pending(x)`
- `TaskStatus::Init` → `Stream::Init`
- `TaskStatus::Ignore` → `Stream::Ignore`
- `TaskStatus::Wait` → `Stream::Wait`
- `TaskStatus::Spawn(_)` → `Stream::Ignore`

Create a `Spread` containing a mix of these variants and verify each maps correctly.

### Delivery tests

Verify that at the `NotifyQueue` boundary, `Spread` is expanded into individual messages:

- **TaskStatus delivery**: A task yielding `Spread(vec![Ready(1), Ready(2), Ready(3)])`
  results in the `NotifyQueue` receiving three separate `Ready` messages.
- **Stream delivery**: A stream yielding `Spread(vec![Next(1), Next(2)])` results in the
  `NotifyQueue` receiving two separate `Next` messages.
- **Order preservation**: Elements are delivered in the same order they appear in the `Vec`.

### Nested spreading tests

Verify that nested `Spread` items are recursively flattened:

- `Spread(vec![Next(1), Spread(vec![Next(2), Next(3)])])` delivers as three separate messages:
  `Next(1)`, `Next(2)`, `Next(3)`.
- Deeply nested: `Spread(vec![Spread(vec![Spread(vec![Next(1)])])])` delivers one `Next(1)`.

### Edge case tests

- Empty `Spread(vec![])` — delivers nothing, queue is not polluted.
- Single-element `Spread(vec![Next(1)])` — delivers exactly one `Next(1)` message.
- Mixed variants in one `Spread` — e.g., `Spread(vec![Pending(p), Next(n), Init])` delivers
  three separate messages of different types.

### Async future handling in `stream_future.rs`

The `stream_future.rs` module contains futures and a `futures_core::Stream` impl that poll
`StreamIterator`s. Spread handling differs between `Future` impls (single-return) and the
`Stream` impl (multi-return).

**`Future` impls (`StreamCollectFuture`, `StreamReadyFuture`, `StreamPendingFuture`):**
No additional state needed. When the iterator yields `Spread(items)`, a simple for loop
iterates the inner `Vec` directly:

- **`StreamCollectFuture`**: For loop over spread items. `Next(v)` → push into `collected`,
  `Ignore` → skip, nested `Spread(sub)` → recursively extend into the same loop (flatten),
  `Pending/Delayed/Init/Wait` → return `Poll::Pending` (iterator signaled not-ready). If the
  spread exhausts without blocking, continue polling the iterator.
- **`StreamReadyFuture`**: For loop over spread items. Return `Poll::Ready(Some((v, remaining)))`
  on first `Next(v)`. `Ignore` → skip, `Pending/Delayed/Init/Wait` → `Poll::Pending`,
  non-`Next` items → skip. If spread exhausts without finding `Next`, poll iterator again.
- **`StreamPendingFuture`**: For loop over spread items. Return `Poll::Ready(Some((p, remaining)))`
  on first `Pending(p)`. `Ignore` → skip, `Next` → skip (not what we're looking for),
  `Delayed/Init/Wait` → `Poll::Pending`. If spread exhausts without finding `Pending`, poll
  iterator again.

**`Stream` impl (`StreamAsFutureStream`):**
Needs a `pending_spread: VecDeque<Stream<D, P>>` field. This is the only construct requiring
additional state because one `Spread` yield from the iterator maps to many `poll_next` calls —
the deque buffers items between calls:

- When iterator yields `Spread(items)`, extend all items into `pending_spread` back.
- Each `poll_next` call pops from front. If the popped item is itself a `Spread`, extend its
  items into the back of the same deque (recursive flattening).
- When deque is empty, poll the iterator again.
- When iterator yields `None`, return `Poll::Ready(None)` (stream exhausted).

### Async future tests

- **`StreamCollectFuture`**: Verify that a `Spread` containing `Next` values contributes all
  values to the collected output. Nested `Spread` items are flattened. If spread contains
  `Pending/Delayed/Init/Wait`, the future returns `Poll::Pending`.
- **`StreamReadyFuture`**: Verify that if a `Spread` contains a `Next`, the first one is
  returned immediately. Non-`Next` items in the spread are skipped. If no `Next` exists,
  the iterator is polled again.
- **`StreamPendingFuture`**: Verify that if a `Spread` contains a `Pending`, the first one is
  returned immediately. `Next` items inside the spread are skipped (not what we're looking for).
  If no `Pending` exists, the iterator is polled again.
- **`StreamAsFutureStream`**: Verify that `Spread` items are flattened so each inner item
  is yielded individually across multiple `poll_next` calls. Nested `Spread` items are
  recursively flattened via `VecDeque`.

## Tasks

### Type changes

- [ ] TASK-09-01: Add `Spread(Vec<TaskStatus<D, P, S>>)` variant to `TaskStatus` enum in `task.rs`. Include doc comments explaining it wraps multiple statuses delivered individually at the delivery point
- [ ] TASK-09-02: Add `Spread(Vec<Stream<D, P>>)` variant to `Stream` enum in `streams.rs`. Include doc comments explaining it wraps multiple stream states delivered individually at the delivery point
- [ ] TASK-09-03: Update `From<TaskStatus> for Stream` impl in `task.rs` to handle `Spread` → `Stream::Spread` conversion by mapping each inner element
- [ ] TASK-09-04: Update `PartialEq` impl for `TaskStatus` in `task.rs` to handle `Spread` matching `Spread` (pairwise element equality). `D: PartialEq, P: PartialEq` bound required

### Delivery point spreading

Spreading happens in the three `ExecutionIterator` impls in `task_iters.rs` where values are
pushed to the `NotifyQueue`. **Do not modify the `NotifyQueue` struct itself** — it is generic
over what it sends. The spread logic goes at the point where we decide what to push.

There are three delivery points, all in `backends/foundation_core/src/valtron/executors/task_iters.rs`:

1. **`StreamConsumingIter::next()`** (line ~175-266): Matches on `TaskStatus` variants, converts
   them to `Stream` variants and pushes to `NotifyQueue<Stream<Done, Pending>>`. Add a
   `TaskStatus::Spread` arm that iterates the inner vec and pushes each element individually
   via `self.channel.push()`. If the channel goes full mid-spread, store the remaining elements
   in `pending_msg` (note: `pending_msg` is a single `Stream`, so for partial-spread scenarios
   you may need to wrap remaining elements back into a `Stream::Spread` for retry).

2. **`ConsumingIter::next()`** (line ~434-534): Matches on `TaskStatus` variants and pushes
   `TaskStatus` variants to `NotifyQueue<TaskStatus<Done, Pending, Action>>`. Add a
   `TaskStatus::Spread` arm that iterates and pushes each inner `TaskStatus` individually.
   Same backpressure consideration: store remaining elements as `TaskStatus::Spread` in
   `pending_msg` if the channel goes full mid-spread.

3. **`ReadyConsumingIter::next()`** (line ~688-726): Only pushes `TaskStatus::Ready` and
   `TaskStatus::Wait` to `NotifyQueue<TaskStatus<Done, Pending, Action>>`. All other variants
   map to `State::Pending`. Add a `TaskStatus::Spread` arm that spreads each inner element,
   but only pushes `Ready` and `Wait` elements (consistent with this iterator's filtering
   behavior). Non-`Ready`/non-`Wait` elements from the spread are consumed without pushing.

- [ ] TASK-09-05: In `StreamConsumingIter::next()` (task_iters.rs), add `TaskStatus::Spread` arm that iterates the inner vec, converts each to `Stream`, and pushes individually to the `NotifyQueue`. Handle backpressure: if channel goes full, re-wrap remaining elements as `Stream::Spread` and store in `pending_msg`
- [ ] TASK-09-06: In `ConsumingIter::next()` (task_iters.rs), add `TaskStatus::Spread` arm that iterates and pushes each inner `TaskStatus` individually to the `NotifyQueue`. Handle backpressure: re-wrap remaining elements as `TaskStatus::Spread` in `pending_msg`
- [ ] TASK-09-06a: In `ReadyConsumingIter::next()` (task_iters.rs), add `TaskStatus::Spread` arm that iterates inner elements but only pushes `Ready` and `Wait` variants (matching this iterator's filtering semantics). Handle backpressure: re-wrap remaining elements in `pending_msg`

### Testing

See the **Testing** section above for test categories and details. All tests go in
`backends/foundation_core/tests/valtron/spread.rs` (a new dedicated test file, not `units.rs`).

- [ ] TASK-09-07: Add conversion test — verify `TaskStatus::Spread` with mixed variants converts to `Stream::Spread` with correctly mapped inner elements
- [ ] TASK-09-08: Add `Stream::Spread` delivery test — verify `Spread(vec![Next(1), Next(2), Next(3)])` yields three separate messages on the `NotifyQueue`
- [ ] TASK-09-09: Add `TaskStatus::Spread` delivery test — verify `Spread(vec![Ready(1), Ready(2)])` yields two separate messages on the `NotifyQueue`
- [ ] TASK-09-10: Add nested spreading flattening test — verify `Spread(vec![Next(1), Spread(vec![Next(2), Next(3)])])` yields three separate messages: `Next(1)`, `Next(2)`, `Next(3)`
- [ ] TASK-09-11: Add deeply nested spreading test — verify `Spread(vec![Spread(vec![Spread(vec![Next(1)])])])` yields exactly one `Next(1)`
- [ ] TASK-09-12: Add empty Spread edge case test — verify `Spread(vec![])` delivers nothing, queue is not polluted
- [ ] TASK-09-13: Add single-element Spread edge case test — verify `Spread(vec![Next(1)])` delivers exactly one `Next(1)` message
- [ ] TASK-09-14: Add mixed variants in one Spread test — verify `Spread(vec![Pending(p), Next(n), Init])` delivers three separate messages of different types

### stream_future.rs tasks

- [ ] TASK-09-15: Update `StreamCollectFuture` — add `Spread` handling via for loop over inner `Vec`, collect `Next` values, flatten nested `Spread`, return `Poll::Pending` on blocking variants
- [ ] TASK-09-16: Update `StreamReadyFuture` — add `Spread` handling via for loop over inner `Vec`, return `Poll::Ready` on first `Next`, skip non-`Next` items, poll iterator again if spread exhausts
- [ ] TASK-09-17: Update `StreamPendingFuture` — add `Spread` handling via for loop over inner `Vec`, return `Poll::Ready` on first `Pending`, skip `Next` and other non-`Pending` items, poll iterator again if spread exhausts
- [ ] TASK-09-18: Update `StreamAsFutureStream` — add `pending_spread: VecDeque<Stream<D, P>>` field, expand `Spread` into deque, yield items one-per-`poll_next`, recursively flatten nested spreads
- [ ] TASK-09-19: Add `StreamCollectFuture` spread test — verify spread containing `Next` values all contribute to collected output, nested spreads are flattened
- [ ] TASK-09-20: Add `StreamReadyFuture` spread test — verify first `Next` in spread is returned immediately, non-`Next` items skipped, iterator polled again if no `Next` found
- [ ] TASK-09-21: Add `StreamPendingFuture` spread test — verify first `Pending` in spread is returned immediately, `Next` items inside spread are skipped, iterator polled again if no `Pending` found
- [ ] TASK-09-22: Add `StreamAsFutureStream` spread test — verify `Spread` items are flattened and yielded individually across multiple `poll_next` calls, nested spreads recursively flattened via `VecDeque`
