---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-valtron-async-iterators"
feature_directory: "specifications/08-valtron-async-iterators/features/08-iterator-ext-completion"
this_file: "specifications/08-valtron-async-iterators/features/08-iterator-ext-completion/feature.md"

status: pending
priority: high
created: 2026-03-25

depends_on:
  - 01-foundation
  - 02-task-iterator-ext
  - 03-stream-iterator-ext
  - 04-collectors
  - 05-split-collectors
  - 06-map-iter

tasks:
  completed: 0
  uncompleted: 82
  total: 82
  completion_percentage: 0%
---

# Iterator Extension Completion Feature

## Overview

Implement missing standard `Iterator` trait methods for both `TaskIteratorExt` and `StreamIteratorExt`.

**Key Design Principles:**

1. **Core methods accept full `TaskStatus`/`Stream`** - Users get complete state access
2. **Convenience wrappers for Ready/Next** - Common case is simple
3. **Minimal generic parameters** - Types inferred from iterator's associated types
4. **Non-blocking semantics** - All methods preserve async states

---

## WHY Section

### Problem Statement

Standard `std::Iterator` assumes simple `Option<T>`:
- `Some(T)` = item available
- `None` = exhausted

Valtron uses multi-state types:

**TaskStatus<D, P, S>**:
- `Ready(D)`, `Pending(P)`, `Delayed(Duration)`, `Init`, `Spawn(S)`, `Ignore`

**Stream<D, P>**:
- `Next(D)`, `Pending(P)`, `Delayed(Duration)`, `Init`, `Ignore`

### Why Standard Methods Don't Work

1. **State Preservation**: Must forward non-terminal states (`Pending`, `Delayed`, etc.)
2. **Non-Blocking**: Methods like `count()`, `all()` must remain non-blocking
3. **Filter Behavior**: Filtered items return `Ignore`, not `None`

---

## WHAT Section

### Design Philosophy

| Layer | Method Pattern | Description |
|-------|---------------|-------------|
| **Core** | `*_state()` | Accept full `TaskStatus`/`Stream` - users get complete state access |
| **Convenience** | Ready/Next wrappers | Built on core methods - common case is simple |
| **Type-changing** | `flatten_*()`, `flat_map_*()`, `enumerate()`, `zip()`, `chain()` | Cannot be wrappers - change associated types |

**Key Principles:**
1. Core methods operate on complete state types
2. Convenience wrappers delegate to core methods
3. Reduction methods (`count()`, `all()`, `any()`, `fold()`) remain non-blocking
4. Flatten/flat_map methods store inner iterator and drain over multiple `next()` calls
5. No `loop {}` in `next()` - return `Ignore` when waiting for more data

### Method Inventory

**TaskIteratorExt Methods:**

| Category | Core Methods (_state) | Convenience Wrappers | Type-Changing |
|----------|----------------------|---------------------|---------------|
| **Transformation** | `map_state()`, `inspect_state()` | `map_ready()`, `inspect_ready()` | - |
| **Filtering** | `filter_state()` | `filter_ready()` | - |
| **Limiting** | `take_state()`, `take_while_state()` | `take()`, `take_all()`, `take_while()`, `take_while_any()` | - |
| **Skipping** | `skip_state()`, `skip_while_state()` | `skip()`, `skip_all()`, `skip_while()`, `skip_while_any()` | - |
| **Indexing** | `enumerate_state()` | `enumerate()` | Yes (D → (usize, D)) |
| **Flattening** | - | - | `flatten_ready()`, `flatten_pending()`, `flat_map_ready()`, `flat_map_pending()` |
| **Search** | `find_state()`, `find_map_state()` | `find()`, `find_map()` | - |
| **Reduction** | `fold_state()`, `all_state()`, `any_state()`, `count_state()` | `fold()`, `all()`, `any()`, `count()`, `count_all()` | - |
| **Branching** | `branch_state()` | `branch()` | - |

**StreamIteratorExt Methods:**

| Category | Core Methods (_state) | Convenience Wrappers | Type-Changing |
|----------|----------------------|---------------------|---------------|
| **Transformation** | `map_state()`, `inspect_state()` | `map_next()`, `inspect_next()` | - |
| **Filtering** | `filter_state()` | `filter_next()` | - |
| **Limiting** | `take_state()`, `take_while_state()` | `take()`, `take_all()`, `take_while()`, `take_while_any()` | - |
| **Skipping** | `skip_state()`, `skip_while_state()` | `skip()`, `skip_all()`, `skip_while()`, `skip_while_any()` | - |
| **Indexing** | `enumerate_state()` | `enumerate()` | Yes (D → (usize, D)) |
| **Flattening** | - | - | `flatten_next()`, `flatten_pending()`, `flat_map_next()`, `flat_map_pending()` |
| **Search** | `find_state()`, `find_map_state()` | `find()`, `find_map()` | - |
| **Reduction** | `fold_state()`, `all_state()`, `any_state()`, `count_state()` | `fold()`, `all()`, `any()`, `count()`, `count_all()` | - |
| **Branching** | `branch_state()` | `branch()` | - |
| **Combination** | - | - | `zip()`, `chain()` |

---

### Core Methods

#### 1. map_state() - Transform Any States

```rust
/// map_state() - Transform TaskStatus with full state access
fn map_state<F>(self, f: F) -> TMapState<Self, F>
where
    F: Fn(TaskStatus<Self::Ready, Self::Pending, Self::Spawner>)
       -> TaskStatus<Self::Ready, Self::Pending, Self::Spawner> + Send + 'static;

pub struct TMapState<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F> Iterator for TMapState<I, F>
where
    I: TaskIterator,
    F: Fn(TaskStatus<I::Ready, I::Pending, I::Spawner>)
       -> TaskStatus<I::Ready, I::Pending, I::Spawner> + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        Some((self.mapper)(status))
    }
}
```

**Convenience: map_ready()**
```rust
fn map_ready<F, R>(self, f: F) -> TMapReady<Self, R>
where
    F: Fn(Self::Ready) -> R + Send + 'static,
{
    self.map_state(|status| match status {
        TaskStatus::Ready(v) => TaskStatus::Ready(f(v)),
        other => other,
    })
}
```

---

#### 2. filter_state() - Filter Based on Any State

```rust
/// filter_state() - Filter based on full TaskStatus
/// Non-matching items return TaskStatus::Ignore
fn filter_state<F>(self, f: F) -> TFilterState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TFilterState<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> Iterator for TFilterState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let status = self.inner.next_status()?;
            if (self.predicate)(&status) {
                return Some(status);
            } else {
                return Some(TaskStatus::Ignore);
            }
        }
    }
}
```

**Convenience: filter_ready()**
```rust
fn filter_ready<F>(self, f: F) -> Self
where
    F: Fn(&Self::Ready) -> bool + Send + 'static,
{
    self.filter_state(|status| match status {
        TaskStatus::Ready(v) => f(v),
        _ => true,
    })
}
```

---

#### 3. inspect_state() - Side-Effects on Any State

```rust
/// inspect_state() - Side-effect for each TaskStatus
fn inspect_state<F>(self, f: F) -> TInspectState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) + Send + 'static;

pub struct TInspectState<I, F> {
    inner: I,
    inspector: F,
}

impl<I, F> Iterator for TInspectState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        (self.inspector)(&status);
        Some(status)
    }
}
```

---

### Filtering Methods

#### 4. take_state() - Limit Based on State Predicate (CORE)

```rust
/// take_state() - Take items while state_predicate returns true AND count allows
///
/// The state_predicate receives the full TaskStatus and decides if this item
/// should count toward the limit. The item is only yielded if both:
/// 1. state_predicate returns true (this item "counts")
/// 2. We haven't exhausted the count yet
fn take_state<F>(self, n: usize, state_predicate: F) -> TTakeState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TTakeState<I, F> {
    inner: I,
    remaining: usize,
    state_predicate: F,
}

impl<I, F> Iterator for TTakeState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 { return None; }
        let status = self.inner.next_status()?;
        if (self.state_predicate)(&status) {
            self.remaining -= 1;
            if self.remaining == 0 {
                // This was the last allowed item
                return Some(status);
            }
        }
        Some(status)
    }
}
```

**Convenience: take() - Count only Ready items**
```rust
/// take(n) - Take at most n Ready items, pass through non-Ready unchanged
fn take(self, n: usize) -> TTakeState<Self, impl Fn(&TaskStatus<..>) -> bool> {
    self.take_state(n, |s| matches!(s, TaskStatus::Ready(_)))
}
```

**Convenience: take_all(n) - Count all states**
```rust
/// take_all(n) - Take at most n items of any state
fn take_all(self, n: usize) -> TTakeState<Self, impl Fn(&TaskStatus<..>) -> bool> {
    self.take_state(n, |_| true)
}
```

---

#### 5. skip_state() - Skip Based on State Predicate (CORE)

```rust
/// skip_state() - Skip items while state_predicate returns true
///
/// The state_predicate receives the full TaskStatus and decides if this item
/// should be skipped.
fn skip_state<F>(self, state_predicate: F) -> TSkipState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TSkipState<I, F> {
    inner: I,
    to_skip: usize,
    state_predicate: F,
}

impl<I, F> Iterator for TSkipState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let status = self.inner.next_status()?;
            if self.to_skip > 0 && (self.state_predicate)(&status) {
                self.to_skip -= 1;
                continue; // Skip this one
            }
            return Some(status);
        }
    }
}
```

**Convenience: skip() - Skip only Ready items**
```rust
/// skip(n) - Skip first n Ready items, return all others unchanged
fn skip(self, n: usize) -> TSkipState<Self, impl Fn(&TaskStatus<..>) -> bool> {
    self.skip_state(n, |s| matches!(s, TaskStatus::Ready(_)))
}
```

**Convenience: skip_all(n) - Skip all states**
```rust
/// skip_all(n) - Skip first n items of any state
fn skip_all(self, n: usize) -> TSkipState<Self, impl Fn(&TaskStatus<..>) -> bool> {
    self.skip_state(n, |_| true)
}
```

---

#### 6. take_while_state() - Take While State Predicate True (CORE)

```rust
/// take_while_state() - Take items while predicate on full TaskStatus returns true
fn take_while_state<F>(self, predicate: F) -> TTakeWhileState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TTakeWhileState<I, F> {
    inner: I,
    predicate: F,
    done: bool,
}

impl<I, F> Iterator for TTakeWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done { return None; }
        let status = self.inner.next_status()?;
        if (self.predicate)(&status) {
            Some(status)
        } else {
            self.done = true;
            None
        }
    }
}
```

**Convenience: take_while() - While predicate true on Ready**
```rust
/// take_while() - Take while predicate true on Ready values, pass through non-Ready
fn take_while<F>(self, f: F) -> TTakeWhileState<Self, impl Fn(&TaskStatus<..>) -> bool>
where
    F: Fn(&Self::Ready) -> bool + Send + 'static,
{
    self.take_while_state(move |s| match s {
        TaskStatus::Ready(v) => f(v),
        _ => true, // Pass through non-Ready states
    })
}
```

**Convenience: take_while_any() - Stop when any state matches**
```rust
/// take_while_any() - Take while predicate true on ANY state
fn take_while_any<F>(self, f: F) -> TTakeWhileState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
{
    self.take_while_state(f)
}
```

---

#### 7. skip_while_state() - Skip While State Predicate True (CORE)

```rust
/// skip_while_state() - Skip items while predicate on full TaskStatus returns true
fn skip_while_state<F>(self, predicate: F) -> TSkipWhileState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TSkipWhileState<I, F> {
    inner: I,
    predicate: F,
    done_skipping: bool,
}

impl<I, F> Iterator for TSkipWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let status = self.inner.next_status()?;
            if !self.done_skipping && (self.predicate)(&status) {
                continue; // Still skipping
            }
            self.done_skipping = true;
            return Some(status);
        }
    }
}
```

**Convenience: skip_while() - Skip while predicate true on Ready**
```rust
/// skip_while() - Skip while predicate true on Ready values, return all others
fn skip_while<F>(self, f: F) -> TSkipWhileState<Self, impl Fn(&TaskStatus<..>) -> bool>
where
    F: Fn(&Self::Ready) -> bool + Send + 'static,
{
    self.skip_while_state(move |s| match s {
        TaskStatus::Ready(v) => f(v),
        _ => false, // Stop skipping on non-Ready
    })
}
```

**Convenience: skip_while_any() - Skip while any state matches**
```rust
/// skip_while_any() - Skip while predicate true on ANY state
fn skip_while_any<F>(self, f: F) -> TSkipWhileState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
{
    self.skip_while_state(f)
}
```

---

### Transformation Methods

#### 8. enumerate_state() - Add Index Based on State Predicate (CORE)

```rust
/// enumerate_state() - Add index to items matching state_predicate
///
/// Only items where state_predicate returns true get indexed and transformed.
/// Other items pass through unchanged.
fn enumerate_state<F, T>(self, state_predicate: F, transformer: T) -> TEnumerateState<Self, F, T>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    T: Fn(usize, TaskStatus<Self::Ready, Self::Pending, Self::Spawner>)
       -> TaskStatus<Self::Ready, Self::Pending, Self::Spawner> + Send + 'static;

pub struct TEnumerateState<I, F, T> {
    inner: I,
    count: usize,
    state_predicate: F,
    transformer: T,
}

impl<I, F, T> Iterator for TEnumerateState<I, F, T>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
    T: Fn(usize, TaskStatus<I::Ready, I::Pending, I::Spawner>)
       -> TaskStatus<I::Ready, I::Pending, I::Spawner> + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        if (self.state_predicate)(&status) {
            let item = (self.transformer)(self.count, status);
            self.count += 1;
            Some(item)
        } else {
            Some(status)
        }
    }
}
```

**Note**: `enumerate()` inherently changes the Ready type from `D` to `(usize, D)`, so it cannot be built on `enumerate_state()`. It is provided as a dedicated core method:

```rust
/// enumerate() - Add index to Ready items, changing Ready type from D to (usize, D)
fn enumerate(self) -> TEnumerate<Self> {
    TEnumerate { inner: self, count: 0 }
}

pub struct TEnumerate<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for TEnumerate<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<(usize, I::Ready), I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        match status {
            TaskStatus::Ready(v) => {
                let item = TaskStatus::Ready((self.count, v));
                self.count += 1;
                Some(item)
            }
            other => Some(other), // Pass through unchanged
        }
    }
}
```

---

#### 9. flatten_ready() - Flatten Ready Values That Implement IntoIterator

Like `std::Iterator::flatten()` - flattens nested iterables in Ready values.

```rust
/// flatten_ready() - Flatten Ready values that implement IntoIterator
///
/// Input:  TaskIterator<Ready = Vec<M>, Pending = P, Spawner = S>
/// Output: TaskIterator<Ready = M, Pending = P, Spawner = S>
fn flatten_ready(self) -> TFlattenReady<Self>
where
    Self::Ready: IntoIterator,
{
    TFlattenReady { inner: self, current_inner: None }
}

pub struct TFlattenReady<I> {
    inner: I,
    current_inner: Option<<I::Ready as IntoIterator>::IntoIter>,
}

impl<I> Iterator for TFlattenReady<I>
where
    I: TaskIterator,
    I::Ready: IntoIterator,
{
    type Item = TaskStatus<<I::Ready as IntoIterator>::Item, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            // Inner exhausted - fall through to get next from outer
            self.current_inner = None;
        }

        // Get next from outer iterator
        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Ready(iterable) => {
                // Start draining this inner iterator on NEXT call
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, no Ready value yet"
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Ready states unchanged
            other => Some(other),
        }
    }
}
```

**Example:**
```rust
// Vec<Vec<i32>> -> Vec<i32>
let nested = vec![vec![1, 2, 3], vec![4, 5, 6]];
let task = make_task(nested); // TaskStatus::Ready(Vec<Vec<i32>>)
let flattened = task.flatten_ready();
// Yields: Ignore, Ready(1), Ready(2), Ready(3), Ignore, Ready(4), ...
```

---

#### 10. flatten_pending() - Flatten Pending Values That Implement IntoIterator

```rust
/// flatten_pending() - Flatten Pending values that implement IntoIterator
///
/// Input:  TaskIterator<Pending = Vec<M>, Ready = D, Spawner = S>
/// Output: TaskIterator<Pending = M, Ready = D, Spawner = S>
fn flatten_pending(self) -> TFlattenPending<Self>
where
    Self::Pending: IntoIterator,
{
    TFlattenPending { inner: self, current_inner: None }
}

pub struct TFlattenPending<I> {
    inner: I,
    current_inner: Option<<I::Pending as IntoIterator>::IntoIter>,
}

impl<I> Iterator for TFlattenPending<I>
where
    I: TaskIterator,
    I::Pending: IntoIterator,
{
    type Item = TaskStatus<I::Ready, <I::Pending as IntoIterator>::Item, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Pending(iterable) => {
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            other => Some(other),
        }
    }
}
```

---

#### 11. flat_map_ready() - Map Ready to IntoIterator, Then Flatten

Like `std::Iterator::flat_map()` - maps Ready values to iterables, then flattens.

```rust
/// flat_map_ready() - Map Ready to IntoIterator, then flatten
///
/// Input:  TaskIterator<Ready = T, Pending = P, Spawner = S>
/// Output: TaskIterator<Ready = U, Pending = P, Spawner = S>
/// where F: Fn(T) -> U, U: IntoIterator
fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
where
    F: Fn(Self::Ready) -> U + Send + 'static,
    U: IntoIterator,
{
    TFlatMapReady {
        inner: self,
        mapper: f,
        current_inner: None
    }
}

pub struct TFlatMapReady<I, F, U> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for TFlatMapReady<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> U + Send + 'static,
    U: IntoIterator,
{
    type Item = TaskStatus<U::Item, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Ready(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            other => Some(other),
        }
    }
}
```

**Example:**
```rust
// Expand each number into [x, x*10]
let nums = vec![1, 2, 3];
let expanded = task.flat_map_ready(|x| vec![x, x * 10]);
// Yields: Ignore, Ready(1), Ready(10), Ignore, Ready(2), Ready(20), ...
```

---

#### 12. flat_map_pending() - Map Pending to IntoIterator, Then Flatten

```rust
/// flat_map_pending() - Map Pending to IntoIterator, then flatten
fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
where
    F: Fn(Self::Pending) -> U + Send + 'static,
    U: IntoIterator,
{
    TFlatMapPending {
        inner: self,
        mapper: f,
        current_inner: None
    }
}

pub struct TFlatMapPending<I, F, U> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for TFlatMapPending<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> U + Send + 'static,
    U: IntoIterator,
{
    type Item = TaskStatus<I::Ready, U::Item, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Pending(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            other => Some(other),
        }
    }
}
```

---

### Search Methods

#### 13. find_state() - Find First Matching Based on State Predicate (CORE)

```rust
/// find_state() - Find first item where state_predicate + value_predicate both return true
///
/// Returns TaskStatus<Option<D>, ..> where None = not found, Some(v) = found
fn find_state<P, V>(self, state_predicate: P, value_predicate: V) -> TFindState<Self, P, V>
where
    P: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    V: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TFindState<I, P, V> {
    inner: I,
    state_predicate: P,
    value_predicate: V,
    found: Option<I::Ready>,
    yielded: bool,
}

impl<I, P, V> Iterator for TFindState<I, P, V>
where
    I: TaskIterator,
    P: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
    V: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<Option<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.yielded { return None; }
        loop {
            let status = self.inner.next_status()?;
            if (self.state_predicate)(&status) && (self.value_predicate)(&status) {
                if let TaskStatus::Ready(v) = &status {
                    self.found = Some(v.clone());
                    self.yielded = true;
                    return Some(TaskStatus::Ready(self.found.take()));
                }
            }
            // Pass through non-matching states
            return Some(match status {
                TaskStatus::Ready(_) => TaskStatus::Ignore,
                other => other,
            });
        }
    }
}
```

**Convenience: find() - Find first Ready matching predicate**
```rust
/// find() - Find first Ready item matching predicate
fn find<F>(self, f: F) -> TFindState<Self, impl Fn(&TaskStatus<..>) -> bool, F>
where
    F: Fn(&Self::Ready) -> bool + Send + 'static,
{
    self.find_state(
        |s| matches!(s, TaskStatus::Ready(_)), // Only consider Ready states
        move |s| match s {
            TaskStatus::Ready(v) => f(v),
            _ => false,
        },
    )
}
```

---

#### 12. find_map_state() - Find and Transform Based on State (CORE)

```rust
/// find_map_state() - Find first item where transform returns Some
fn find_map_state<F, R>(self, f: F) -> TFindMapState<Self, F, R>
where
    F: Fn(TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> Option<R> + Send + 'static,
    R: Send + 'static;

pub struct TFindMapState<I, F, R> {
    inner: I,
    mapper: F,
    found: Option<R>,
    yielded: bool,
}

impl<I, F, R> Iterator for TFindMapState<I, F, R>
where
    I: TaskIterator,
    F: Fn(TaskStatus<I::Ready, I::Pending, I::Spawner>) -> Option<R> + Send + 'static,
    R: Send + 'static,
{
    type Item = TaskStatus<Option<R>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.yielded { return None; }
        loop {
            let status = self.inner.next_status()?;
            if let Some(result) = (self.mapper)(status) {
                self.found = Some(result);
                self.yielded = true;
                return Some(TaskStatus::Ready(self.found.take()));
            }
            // Keep searching
        }
    }
}
```

**Convenience: find_map() - Find and transform Ready items**
```rust
/// find_map() - Find first Ready where predicate returns Some
fn find_map<F, R>(self, f: F) -> TFindMapState<Self, impl Fn(TaskStatus<..>) -> Option<R>, R>
where
    F: Fn(Self::Ready) -> Option<R> + Send + 'static,
{
    self.find_map_state(move |s| match s {
        TaskStatus::Ready(v) => f(v),
        _ => None,
    })
}
```

---

### Reduction Methods

#### 13. fold_state() - Accumulate Based on State Predicate (CORE)

```rust
/// fold_state() - Accumulate items where state_predicate returns true
///
/// Yields TaskStatus<Acc, ..> when complete (non-blocking during accumulation)
fn fold_state<P, F, Acc>(self, init: Acc, state_predicate: P, f: F) -> TFoldState<Self, P, F, Acc>
where
    P: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    F: Fn(Acc, TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> Acc + Send + 'static,
    Acc: Clone + Send + 'static;

pub struct TFoldState<I, P, F, Acc> {
    inner: I,
    mapper: F,
    state_predicate: P,
    acc: Acc,
    done: bool,
}
```

**Convenience: fold() - Accumulate Ready items**
```rust
/// fold() - Accumulate Ready items with initial value
fn fold<F, Acc>(self, init: Acc, f: F) -> TFoldState<Self, impl Fn(&TaskStatus<..>) -> bool, F, Acc>
where
    F: Fn(Acc, Self::Ready) -> Acc + Send + 'static,
    Acc: Clone + Send + 'static,
{
    self.fold_state(
        init,
        |s| matches!(s, TaskStatus::Ready(_)),
        move |acc, s| match s {
            TaskStatus::Ready(v) => f(acc, v),
            other => acc, // Pass through unchanged
        },
    )
}
```

---

#### 14. all_state() - Check All Items Match State Predicate (CORE)

```rust
/// all_state() - Check if ALL items match state_predicate
///
/// Short-circuits: returns false immediately on first non-match
/// Yields TaskStatus<bool, ..> when determined
fn all_state<F>(self, predicate: F) -> TAllState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TAllState<I, F> {
    inner: I,
    predicate: F,
    done: bool,
}

impl<I, F> Iterator for TAllState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<bool, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done { return None; }
        loop {
            let status = self.inner.next_status()?;
            if !(self.predicate)(&status) {
                self.done = true;
                return Some(TaskStatus::Ready(false)); // Short-circuit
            }
            // Keep checking
        }
    }
}
```

**Convenience: all() - Check all Ready items match**
```rust
/// all() - Check if all Ready items match predicate
fn all<F>(self, f: F) -> TAllState<Self, impl Fn(&TaskStatus<..>) -> bool>
where
    F: Fn(&Self::Ready) -> bool + Send + 'static,
{
    self.all_state(move |s| match s {
        TaskStatus::Ready(v) => f(v),
        _ => true, // Non-Ready states don't affect result
    })
}
```

---

#### 15. any_state() - Check Any Item Matches State Predicate (CORE)

```rust
/// any_state() - Check if ANY item matches state_predicate
///
/// Short-circuits: returns true immediately on first match
fn any_state<F>(self, predicate: F) -> TAnyState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TAnyState<I, F> {
    inner: I,
    predicate: F,
    done: bool,
}

impl<I, F> Iterator for TAnyState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<bool, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done { return None; }
        loop {
            let status = self.inner.next_status()?;
            if (self.predicate)(&status) {
                self.done = true;
                return Some(TaskStatus::Ready(true)); // Short-circuit
            }
        }
    }
}
```

**Convenience: any() - Check any Ready item matches**
```rust
/// any() - Check if any Ready item matches predicate
fn any<F>(self, f: F) -> TAnyState<Self, impl Fn(&TaskStatus<..>) -> bool>
where
    F: Fn(&Self::Ready) -> bool + Send + 'static,
{
    self.any_state(move |s| match s {
        TaskStatus::Ready(v) => f(v),
        _ => false,
    })
}
```

---

#### 16. count_state() - Count Items Based on State Predicate (CORE)

```rust
/// count_state() - Count items where state_predicate returns true
///
/// Yields TaskStatus<usize, ..> when complete (non-blocking during counting)
fn count_state<F>(self, predicate: F) -> TCountState<Self, F>
where
    F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

pub struct TCountState<I, F> {
    inner: I,
    predicate: F,
    count: usize,
    done: bool,
}
```

**Convenience: count() - Count Ready items**
```rust
/// count() - Count Ready items
fn count(self) -> TCountState<Self, impl Fn(&TaskStatus<..>) -> bool> {
    self.count_state(|s| matches!(s, TaskStatus::Ready(_)))
}
```

**Convenience: count_all() - Count all states**
```rust
/// count_all() - Count all items (any state)
fn count_all(self) -> TCountState<Self, impl Fn(&TaskStatus<..>) -> bool> {
    self.count_state(|_| true)
}
```

---

### StreamIterator Flatten Methods

Like TaskIterator, flatten operations are type-specific and cannot use a generic `*_state()` pattern.

**Important**: Implementations must NOT use `loop {}` - return `Ignore` when waiting for inner iterator to produce.

#### 17. flatten_next() - Flatten Next Values That Implement IntoIterator

```rust
/// flatten_next() - Flatten Next values that implement IntoIterator
///
/// Input:  StreamIterator<D = Vec<M>, P = P>
/// Output: StreamIterator<D = M, P = P>
fn flatten_next(self) -> SFlattenNext<Self>
where
    Self::D: IntoIterator,
{
    SFlattenNext { inner: self, current_inner: None }
}

pub struct SFlattenNext<I> {
    inner: I,
    current_inner: Option<<I::D as IntoIterator>::IntoIter>,
}

impl<I> Iterator for SFlattenNext<I>
where
    I: StreamIterator,
    I::D: IntoIterator,
{
    type Item = Stream<<I::D as IntoIterator>::Item, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Next(item));
            }
            // Inner exhausted - fall through to get next from outer
            self.current_inner = None;
        }

        // Get next from outer iterator
        let stream = self.inner.next()?;

        match stream {
            Stream::Next(iterable) => {
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, no Next value yet"
                Some(Stream::Ignore)
            }
            // Pass through non-Next states unchanged
            other => Some(other),
        }
    }
}
```

---

#### 18. flatten_pending() - Flatten Pending Values That Implement IntoIterator

```rust
/// flatten_pending() - Flatten Pending values that implement IntoIterator
///
/// Input:  StreamIterator<P = Vec<M>, D = D>
/// Output: StreamIterator<P = M, D = D>
fn flatten_pending(self) -> SFlattenPending<Self>
where
    Self::P: IntoIterator,
{
    SFlattenPending { inner: self, current_inner: None }
}

pub struct SFlattenPending<I> {
    inner: I,
    current_inner: Option<<I::P as IntoIterator>::IntoIter>,
}

impl<I> Iterator for SFlattenPending<I>
where
    I: StreamIterator,
    I::P: IntoIterator,
{
    type Item = Stream<I::D, <I::P as IntoIterator>::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Pending(item));
            }
            self.current_inner = None;
        }

        let stream = self.inner.next()?;

        match stream {
            Stream::Pending(iterable) => {
                self.current_inner = Some(iterable.into_iter());
                Some(Stream::Ignore)
            }
            other => Some(other),
        }
    }
}
```

---

#### 19. flat_map_next() - Map Next to IntoIterator, Then Flatten

```rust
/// flat_map_next() - Map Next to IntoIterator, then flatten
///
/// Input:  StreamIterator<D = T, P = P>
/// Output: StreamIterator<D = U, P = P>
/// where F: Fn(T) -> U, U: IntoIterator
fn flat_map_next<F, U>(self, f: F) -> SFlatMapNext<Self, F, U>
where
    F: Fn(Self::D) -> U + Send + 'static,
    U: IntoIterator,
{
    SFlatMapNext {
        inner: self,
        mapper: f,
        current_inner: None
    }
}

pub struct SFlatMapNext<I, F, U> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for SFlatMapNext<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::D) -> U + Send + 'static,
    U: IntoIterator,
{
    type Item = Stream<U::Item, I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Next(item));
            }
            self.current_inner = None;
        }

        let stream = self.inner.next()?;

        match stream {
            Stream::Next(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(Stream::Ignore)
            }
            other => Some(other),
        }
    }
}
```

---

#### 20. flat_map_pending() - Map Pending to IntoIterator, Then Flatten

```rust
/// flat_map_pending() - Map Pending to IntoIterator, then flatten
fn flat_map_pending<F, U>(self, f: F) -> SFlatMapPending<Self, F, U>
where
    F: Fn(Self::P) -> U + Send + 'static,
    U: IntoIterator,
{
    SFlatMapPending {
        inner: self,
        mapper: f,
        current_inner: None
    }
}

pub struct SFlatMapPending<I, F, U> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for SFlatMapPending<I, F, U>
where
    I: StreamIterator,
    F: Fn(I::P) -> U + Send + 'static,
    U: IntoIterator,
{
    type Item = Stream<I::D, U::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(Stream::Pending(item));
            }
            self.current_inner = None;
        }

        let stream = self.inner.next()?;

        match stream {
            Stream::Pending(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(Stream::Ignore)
            }
            other => Some(other),
        }
    }
}
```

---

### Combination Methods

#### 21. zip() - Combine Two StreamIterators

```rust
fn zip<J>(self, other: J) -> SZip<Self, J>
where
    J: StreamIterator;

pub struct SZip<I, J> {
    first: I,
    second: J,
}

impl<I, J> Iterator for SZip<I, J>
where
    I: StreamIterator,
    J: StreamIterator,
{
    type Item = Stream<(I::D, J::D), I::P>;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.first.next(), self.second.next()) {
            (Some(Stream::Next(a)), Some(Stream::Next(b))) => {
                Some(Stream::Next((a, b)))
            }
            _ => None,
        }
    }
}
```

---

#### 22. chain() - Concatenate Iterators

```rust
fn chain<J>(self, other: J) -> SChain<Self, J>
where
    J: StreamIterator<D = Self::D, P = Self::P>;

pub struct SChain<I, J> {
    first: I,
    second: J,
    first_exhausted: bool,
}
```

---

### Branching Methods

#### 18. BranchPath Type

```rust
/// Controls which branch an item should be routed to
pub enum BranchPath<T> {
    Left(T),
    Right(T),
    Skip,
}
```

---

#### 19. branch() - Split Iterator into Two Paths

```rust
/// branch() - Split into two iterators based on BranchPath
fn branch<F>(self, f: F) -> (BranchLeft<Self, F>, BranchRight<Self, F>)
where
    F: Fn(TaskStatus<Self::Ready, Self::Pending, Self::Spawner>)
       -> BranchPath<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> + Send + 'static;

pub struct BranchLeft<I, F> {
    inner: I,
    branch_fn: F,
}

pub struct BranchRight<I, F> {
    inner: I,
    branch_fn: F,
}

impl<I, F> Iterator for BranchLeft<I, F>
where
    I: TaskIterator,
    F: Fn(TaskStatus<I::Ready, I::Pending, I::Spawner>)
       -> BranchPath<TaskStatus<I::Ready, I::Pending, I::Spawner>> + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let status = self.inner.next_status()?;
            match (self.branch_fn)(status) {
                BranchPath::Left(s) => return Some(s),
                BranchPath::Right(_) => continue,
                BranchPath::Skip => continue,
            }
        }
    }
}
```

---

## HOW Section

### Implementation Patterns

#### 1. Core Methods (Full State Access)
```rust
match (self.mapper)(status) {
    // Return transformed TaskStatus/Stream directly
}
```

#### 2. Filtering Methods (take, skip)
```rust
// Only decrement counter on Ready/Next
if matches!(&status, TaskStatus::Ready(_)) {
    self.remaining -= 1;
}
```

#### 3. Search Methods (find, find_map)
```rust
// Track found + yielded state
if (self.predicate)(&v) {
    self.found = Some(v);
    self.yielded = true;
    return Some(TaskStatus::Ready(self.found.take()));
}
```

#### 4. Reduction Methods (fold, all, any, count)
```rust
// Accumulate internally, yield when done
self.acc = (self.mapper)(self.acc.clone(), v);
// Return Ignore to signal still working
Some(TaskStatus::Ignore)
```

---

## Tasks Checklist

### TaskIteratorExt Implementation

**Core Methods (full TaskStatus access):**
- [ ] `map_state()` - Transform any TaskStatus
- [ ] `filter_state()` - Filter based on any TaskStatus
- [ ] `inspect_state()` - Side-effect on any TaskStatus
- [ ] `take_state()` - Take items while state predicate true AND count allows
- [ ] `skip_state()` - Skip items while state predicate true
- [ ] `take_while_state()` - Take while state predicate true
- [ ] `skip_while_state()` - Skip while state predicate true
- [ ] `enumerate_state()` - Add index based on state predicate (internal use)
- [ ] `find_state()` - Find first item matching state + value predicates
- [ ] `find_map_state()` - Find and transform based on TaskStatus
- [ ] `fold_state()` - Accumulate based on state predicate
- [ ] `all_state()` - Check all items match state predicate
- [ ] `any_state()` - Check any item matches state predicate
- [ ] `count_state()` - Count items matching state predicate
- [ ] `branch_state()` - Split into two paths based on TaskStatus

**Type-Changing Methods (Ready-specific):**
- [ ] `flatten_ready()` - Flatten Ready values (Ready: IntoIterator)
- [ ] `flatten_pending()` - Flatten Pending values (Pending: IntoIterator)
- [ ] `flat_map_ready()` - Map Ready to IntoIterator, then flatten
- [ ] `flat_map_pending()` - Map Pending to IntoIterator, then flatten
- [ ] `enumerate()` - Add index to Ready items (changes type to (usize, D))

**Convenience Methods (Ready-focused wrappers):**
- [ ] `map_ready()` - Map only Ready values
- [ ] `filter_ready()` - Filter only Ready values
- [ ] `take()` - Take N Ready items
- [ ] `take_all()` - Take N items of any state
- [ ] `take_while()` - Take while Ready predicate true
- [ ] `take_while_any()` - Take while any-state predicate true
- [ ] `skip()` - Skip N Ready items
- [ ] `skip_all()` - Skip N items of any state
- [ ] `skip_while()` - Skip while Ready predicate true
- [ ] `skip_while_any()` - Skip while any-state predicate true
- [ ] `find()` - Find first Ready matching predicate
- [ ] `find_map()` - Find and transform Ready
- [ ] `fold()` - Accumulate Ready items
- [ ] `all()` - Check all Ready match
- [ ] `any()` - Check any Ready matches
- [ ] `count()` - Count Ready items
- [ ] `count_all()` - Count all states
- [ ] `branch()` - Split based on Ready/BranchPath

### StreamIteratorExt Implementation

**Core Methods (full Stream access):**
- [ ] `map_state()` - Transform any Stream state
- [ ] `filter_state()` - Filter based on any Stream state
- [ ] `inspect_state()` - Side-effect on any Stream state
- [ ] `take_state()` - Take items while state predicate true AND count allows
- [ ] `skip_state()` - Skip items while state predicate true
- [ ] `take_while_state()` - Take while state predicate true
- [ ] `skip_while_state()` - Skip while state predicate true
- [ ] `enumerate_state()` - Add index based on state predicate (internal use)
- [ ] `find_state()` - Find first item matching state + value predicates
- [ ] `find_map_state()` - Find and transform based on Stream
- [ ] `fold_state()` - Accumulate based on state predicate
- [ ] `all_state()` - Check all items match state predicate
- [ ] `any_state()` - Check any item matches state predicate
- [ ] `count_state()` - Count items matching state predicate
- [ ] `branch_state()` - Split into two paths based on Stream

**Type-Changing Methods (Next-specific):**
- [ ] `flatten_next()` - Flatten Next values (D: IntoIterator)
- [ ] `flatten_pending()` - Flatten Pending values (P: IntoIterator)
- [ ] `flat_map_next()` - Map Next to IntoIterator, then flatten
- [ ] `flat_map_pending()` - Map Pending to IntoIterator, then flatten
- [ ] `enumerate()` - Add index to Next items (changes type to (usize, D))
- [ ] `zip()` - Combine two StreamIterators
- [ ] `chain()` - Concatenate iterators

**Convenience Methods (Next-focused wrappers):**
- [ ] `map_next()` - Map only Next values
- [ ] `filter_next()` - Filter only Next values
- [ ] `take()` - Take N Next items
- [ ] `take_all()` - Take N items of any state
- [ ] `take_while()` - Take while Next predicate true
- [ ] `take_while_any()` - Take while any-state predicate true
- [ ] `skip()` - Skip N Next items
- [ ] `skip_all()` - Skip N items of any state
- [ ] `skip_while()` - Skip while Next predicate true
- [ ] `skip_while_any()` - Skip while any-state predicate true
- [ ] `find()` - Find first Next matching predicate
- [ ] `find_map()` - Find and transform Next
- [ ] `fold()` - Accumulate Next items
- [ ] `all()` - Check all Next match
- [ ] `any()` - Check any Next matches
- [ ] `count()` - Count Next items
- [ ] `count_all()` - Count all states
- [ ] `branch()` - Split based on Next/BranchPath

### Testing

- [ ] Unit tests for core methods (map_state, filter_state, inspect_state)
- [ ] Unit tests for filtering methods (take, skip, take_while, skip_while)
- [ ] Unit tests for transformation methods (enumerate)
- [ ] Unit tests for flattening methods (flatten_ready, flatten_pending, flat_map_ready, flat_map_pending)
- [ ] Unit tests for search methods (find, find_map)
- [ ] Unit tests for reduction methods (fold, all, any, count)
- [ ] Unit tests for combination methods (zip, chain)
- [ ] Unit tests for branching methods (branch)
- [ ] Integration tests combining multiple combinators
- [ ] Edge case tests (empty, single item, all filtered)

### Documentation

- [ ] Doc comments for all trait methods
- [ ] Doc comments for all wrapper types
- [ ] Usage examples in doc comments
- [ ] Update module-level documentation

### Quality

- [ ] Zero clippy warnings
- [ ] All tests pass
- [ ] Documentation examples compile
- [ ] Update spec with completion status

---

## Success Criteria

### Implementation Complete
- [ ] **TaskIteratorExt**: 15 core `_state()` methods + 18 convenience wrappers + 4 type-changing (flatten/flat_map) implemented
- [ ] **StreamIteratorExt**: 15 core `_state()` methods + 18 convenience wrappers + 6 type-changing (flatten/flat_map/zip/chain) implemented
- [ ] `BranchPath<T>` enum defined and exported
- [ ] Core methods accept full `TaskStatus`/`Stream`
- [ ] Convenience wrappers delegate to core methods
- [ ] Type-changing methods have dedicated structs (cannot be wrappers)

### Code Quality
- [ ] Zero clippy warnings
- [ ] Minimal generic parameters (types from associated types)
- [ ] Proper `Send + 'static` bounds
- [ ] Consistent naming: `T<Method>` for Task, `S<Method>` for Stream
- [ ] No `loop {}` in `next()` - uses `Ignore` for non-blocking

### Testing
- [ ] Unit test for each combinator
- [ ] Integration test with chained combinators
- [ ] Edge case coverage
- [ ] All tests pass

### Documentation
- [ ] All public items documented
- [ ] Usage examples provided
- [ ] All doc examples compile

---

## Summary

This spec provides:

1. **Core methods** (`*_state()`) - Accept full `TaskStatus`/`Stream`, users get complete state access
2. **Convenience wrappers** - Built on core methods for common Ready/Next operations
3. **Type-changing methods** - `flatten_*()`, `flat_map_*()`, `enumerate()`, `zip()`, `chain()` have dedicated structs
4. **Non-blocking semantics** - All `next()` implementations return `Ignore` instead of looping
5. **IntoIterator pattern** - Flatten/flat_map methods store inner iterator and drain over multiple calls
