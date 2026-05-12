---
feature: iterator-semantics
description: Fix TransformIterator filter_map semantics, add TransformUntilIterator for all iterator types, fix CollectAllStream O(n) remove
status: pending
priority: high
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0
dependencies: []
---

# Feature 05: Iterator Semantics Fixes

## Problem

### 1. TaskIterator/Iterator Blanket Impl Recursion Trap (DOCUMENTED)

**Location:** `task.rs:311-401`

**Verdict:** Intentional design — document it, don't change it. Add a warning to
`foundation_core/src/valtron/README.md` (create if missing) as a must-know.

### 2. TransformIterator Conflates Filtering With Termination (HIGH)

**Location:** `iterators.rs:194-202, 220-228`

When the transformer closure returns `None`, the iterator signals completion (returns `None`
from `next()`). Users expect `filter_map` semantics (skip `None`, continue) but get
`take_while` semantics (stop on first `None`).

Current code (`iterators.rs:197-201`):
```rust
fn next(&mut self) -> Option<Self::Item> {
    match self.source.next() {
        Some(item) => (self.transformer)(item),  // None from transformer = iterator done
        _ => None,
    }
}
```

### 3. No TransformUntilIterator (HIGH)

The old `TransformIterator` behavior (stop on first `None` from transformer) is valid for
some use cases. After fixing `TransformIterator` to use filter_map semantics, we need a
`TransformUntilIterator` that preserves the take_while-on-None behavior for callers that
want it.

This must be available at all three iterator levels:
- **Base iterators** (`iterators.rs`): `TransformUntilIterator`, `TransformUntilSendIterator`
- **TaskIterator** (`task.rs`): `TTransformUntil` combinator via `TaskIteratorExt::transform_until()`
- **StreamIterator** (`streams.rs`): `STransformUntil` combinator via `StreamIteratorExt::transform_until()`

### 4. CollectAllStream Uses O(n) remove Instead of swap_remove (HIGH)

**Location:** `unified.rs:752`

`self.sources.remove(idx)` is O(n) per call. `CollectNextFromAllStream` already uses
`swap_remove` correctly.

## Approach

### TransformIterator fix

Change `TransformIterator::next()` and `TransformSendIterator::next()` to loop until the
transformer returns `Some` or the underlying source is exhausted:

```rust
fn next(&mut self) -> Option<Self::Item> {
    loop {
        match self.source.next() {
            Some(item) => {
                if let Some(transformed) = (self.transformer)(item) {
                    return Some(transformed);
                }
                // transformer returned None — skip this item, try next
            }
            None => return None,
        }
    }
}
```

### TransformUntilIterator — base level

New structs in `iterators.rs` that preserve the old take_while-on-None behavior:

- `TransformUntilIterator<T, V>` — non-Send variant
- `TransformUntilSendIterator<T: Send, V: Send>` — Send variant

Same struct layout as `TransformIterator`/`TransformSendIterator` (transformer + source).
The `Iterator::next()` impl returns `None` when the transformer returns `None` (current
behavior), terminating the iterator.

### TTransformUntil — TaskIterator level

New combinator on `TaskIteratorExt` following the existing pattern (`TTakeWhileState`,
`TFilterState`, etc.):

```rust
pub struct TTransformUntil<I: TaskIterator, F> {
    inner: I,
    transformer: F,
    done: bool,
}
```

Where `F: Fn(TaskStatus<I::Ready, I::Pending, I::Spawner>) -> Option<TaskStatus<R, I::Pending, I::Spawner>>`.

- When transformer returns `Some(status)` — yield the transformed status
- When transformer returns `None` — set `done = true`, return `None` (iterator finished)
- Non-Ready states can be passed through or transformed by the closure

Added to `TaskIteratorExt` as:
```rust
fn transform_until<F, R>(self, f: F) -> TTransformUntil<Self, F>
where
    F: Fn(TaskStatus<Self::Ready, Self::Pending, Self::Spawner>)
       -> Option<TaskStatus<R, Self::Pending, Self::Spawner>> + Send + 'static,
    R: Send + 'static;
```

### STransformUntil — StreamIterator level

Same pattern for `StreamIteratorExt`:

```rust
pub struct STransformUntil<I: StreamIterator, F> {
    inner: I,
    transformer: F,
    done: bool,
}
```

Where `F: Fn(Stream<I::D, I::P>) -> Option<Stream<R, I::P>>`.

Added to `StreamIteratorExt` as:
```rust
fn transform_until<F, R>(self, f: F) -> STransformUntil<Self, F>
where
    F: Fn(Stream<Self::D, Self::P>) -> Option<Stream<R, Self::P>> + Send + 'static,
    R: Send + 'static;
```

### CollectAllStream fix

Replace `self.sources.remove(idx)` with `self.sources.swap_remove(idx)` at `unified.rs:752`.

## Tasks

### Documentation

- [ ] TASK-05-01: Add recursion trap warning to `foundation_core/src/valtron/README.md` (create if missing) documenting the `TaskIterator`/`Iterator` blanket impl bidirectional coherence at `task.rs:311-401` — explain that wrapper types must NOT implement both traits, and that `Box<dyn TaskIterator>` breaks the cycle via `as_mut().next_status()`

### TransformIterator fix (filter_map semantics)

- [ ] TASK-05-02: Fix `TransformSendIterator::next()` (`iterators.rs:194-202`) to loop until transformer returns `Some` or source returns `None`
- [ ] TASK-05-03: Fix `TransformIterator::next()` (`iterators.rs:220-228`) with same filter_map loop
- [ ] TASK-05-04: Verify call sites in `wire/simple_http/impls.rs:470,496` — confirm they don't rely on None-terminates behavior (both use encoder closures that should always return `Some`)

### TransformUntilIterator — base iterators

- [ ] TASK-05-05: Add `TransformUntilIterator<T, V>` and `TransformUntilSendIterator<T: Send, V: Send>` to `iterators.rs` — same struct layout as `TransformIterator`/`TransformSendIterator` but `next()` returns `None` when transformer returns `None` (old take_while-on-None behavior); add `SendableIterator<V>` impl for the Send variant

### TransformUntil — TaskIterator combinator

- [ ] TASK-05-06: Add `TTransformUntil<I, F>` struct to `task.rs` (near `TTakeWhileState` at line 3538); impl `Iterator` for it following the `TTakeWhileState` pattern — transformer receives full `TaskStatus`, returns `Option<TaskStatus<R, P, S>>`, `None` terminates
- [ ] TASK-05-07: Add `transform_until()` method to `TaskIteratorExt` trait declaration and impl block (trait: near `take_while_state` at line 1939; impl: near line 2442)

### TransformUntil — StreamIterator combinator

- [ ] TASK-05-08: Add `STransformUntil<I, F>` struct to `streams.rs` (near `STakeWhileState` at line 2649); impl `Iterator` for it — transformer receives full `Stream<D, P>`, returns `Option<Stream<R, P>>`, `None` terminates
- [ ] TASK-05-09: Add `transform_until()` method to `StreamIteratorExt` trait declaration and impl block (trait: near `take_while_state` at line 762; impl: near line 1184)

### CollectAllStream fix

- [ ] TASK-05-10: Replace `self.sources.remove(idx)` with `self.sources.swap_remove(idx)` in `CollectAllStream` (`unified.rs:752`)
