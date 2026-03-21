---
feature: "Task Iterators"
description: "TaskStatusIterator trait with into_stream() conversion and TaskIteratorExt for builder-style chaining"
status: "pending"
priority: "high"
depends_on: ["00-foundation"]
estimated_effort: "medium"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Task Iterators Feature

## WHY: Problem Statement

Rust's `std::iter::Iterator` is synchronous and blocking - calling `next()` blocks until the element is ready. This conflicts with Valtron's async execution model where tasks can be in `Pending`, `Delayed`, `Init`, `Spawn`, or `Ready` states.

**Current limitation**: No way to iterate over `TaskStatus` values without blocking, preventing:
- Parallel task execution with state visibility
- Non-blocking collection from multiple sources
- Integration with Valtron's executor scheduling

**Key Design Principle: TaskIterators Are Inputs**

TaskIterators are for implementers defining async tasks. Combinators are applied BEFORE `execute()`:

```rust
// Implementer defines task with builder combinators
let task = fetch_models_dev_task(client)
    .map_models(|m| m.with_enhanced_metadata())  // Transform Ready values
    .filter_pending(|p| !p.is_transient());      // Filter Pending states

// Execute returns StreamIterator for end users
let stream = execute(task)?;  // StreamIterator consumed by end users
```

End users never deal with TaskIterator directly - they receive `StreamIterator` from `execute()`.

## WHAT: Solution Overview

### TaskStatusIterator Trait (Foundation)

```rust
/// Core trait for TaskStatus-aware iteration.
///
/// This is the foundation trait that all TaskIterators implement.
/// Implementers define async tasks that yield TaskStatus variants.
///
/// Key principle: TaskIterators are INPUTS to execute().
/// End users receive StreamIterator from execute(), never TaskIterator directly.
pub trait TaskStatusIterator: Iterator<Item = TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
    type Ready;
    type Pending;
    type Spawner: ExecutionAction;
}
```

### TaskIteratorExt Trait (Builder-Style Chaining)

```rust
/// Extension trait providing builder-style combinator methods.
///
/// Each method:
/// 1. Consumes self and wraps it internally
/// 2. Forwards TaskStatus variants it doesn't care about unchanged
/// 3. Transforms only the TaskStatus variant(s) it targets
///
/// Key principle: These combinators are applied BEFORE execute().
/// The result is still a TaskIterator, ready for execute().
///
/// # Example
///
/// ```rust
/// let task = fetch_models_dev_task(client)
///     .map_ready(|models| models.into_iter().filter(|m| m.is_enabled()).collect())
///     .map_pending(|pending| pending.with_timestamp(Instant::now()))
///     .filter_ready(|models| !models.is_empty());
///
/// // Then execute - returns StreamIterator for end users
/// let stream = execute(task)?;
/// ```
pub trait TaskIteratorExt: TaskStatusIterator + Sized {
    /// Map Ready values using the provided function.
    ///
    /// Forwards Pending, Delayed, Init, Spawn unchanged.
    fn map_ready<F, O>(self, mapper: F) -> MapReady<Self, F>
    where
        F: Fn(Self::Ready) -> O;

    /// Map Pending values using the provided function.
    ///
    /// Forwards Ready, Delayed, Init, Spawn unchanged.
    fn map_pending<F, O>(self, mapper: F) -> MapPending<Self, F>
    where
        F: Fn(Self::Pending) -> O;

    /// Filter Ready values, returning None for filtered items.
    ///
    /// Forwards Pending, Delayed, Init, Spawn unchanged.
    fn filter_ready<F>(self, predicate: F) -> FilterReady<Self, F>
    where
        F: Fn(&Self::Ready) -> bool;

    /// Transform any TaskStatus variant the combinator cares about.
    ///
    /// Generic mapping over TaskStatus itself.
    fn map_task_status<F, R, P>(self, mapper: F) -> MapTaskStatus<Self, F>
    where
        F: Fn(TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> TaskStatus<R, P, Self::Spawner>;

    /// Stream-collect that yields StreamCollectStatus while gathering items.
    ///
    /// Unlike std::Iterator::collect(), this does NOT block waiting for all items.
    /// Instead, it yields StreamCollectStatus variants showing collection progress:
    /// - StreamCollectStatus::Pending(count) - still gathering, have `count` items so far
    /// - StreamCollectStatus::Ready(Vec<D>) - collection complete with all items
    ///
    /// # Example
    ///
    /// ```rust
    /// let task = fetch_models_dev_task(client)
    ///     .stream_collect();
    ///
    /// // Execute returns StreamIterator
    /// for status in execute(task)? {
    ///     match status {
    ///         Stream::Pending(count) => println!("Collected {count} so far..."),
    ///         Stream::Next(all_items) => println!("Done! Got {} items", all_items.len()),
    ///     }
    /// }
    /// ```
    fn stream_collect(self) -> StreamCollect<Self>
    where
        Self::Ready: Clone;
}
```

### StreamCollectStatus Type

```rust
/// Progress states for stream-based collection
pub enum StreamCollectStatus<D, P> {
    /// Still gathering items, have `count` items so far
    Pending { count: usize, pending_info: P },
    /// Collection complete with all items
    Ready(Vec<D>),
}
```

### StreamCollect Type

```rust
/// Stream-based collector that yields StreamCollectStatus while gathering items.
///
/// Unlike std::Iterator::collect(), this does NOT block waiting for all items.
/// It yields status updates as items arrive, finally yielding the complete collection.
///
/// This is a TaskIterator combinator - apply BEFORE execute().
pub struct StreamCollect<I> {
    inner: I,
    collected: Vec<I::Ready>,
}

impl<I> Iterator for StreamCollect<I>
where
    I: TaskStatusIterator,
    I::Ready: Clone,
{
    type Item = TaskStatus<StreamCollectStatus<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            TaskStatus::Ready(value) => {
                self.collected.push(value);
                Some(TaskStatus::Pending(StreamCollectStatus::Pending {
                    count: self.collected.len(),
                    pending_info: None,
                }))
            }
            TaskStatus::Pending(p) => {
                Some(TaskStatus::Pending(StreamCollectStatus::Pending {
                    count: self.collected.len(),
                    pending_info: Some(p),
                }))
            }
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
        // Note: When inner returns None, we yield TaskStatus::Ready(collected)
        // This requires tracking completion state
    }
}
```

### Custom Combinator Types (Hold Source + Operation)

```rust
/// Maps Ready values, forwards all other TaskStatus variants unchanged.
pub struct MapReady<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F, O> Iterator for MapReady<I, F>
where
    I: TaskStatusIterator,
    F: Fn(I::Ready) -> O,
{
    type Item = TaskStatus<O, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            TaskStatus::Ready(value) => Some(TaskStatus::Ready((self.mapper)(value))),
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

/// Maps Pending values, forwards all other TaskStatus variants unchanged.
pub struct MapPending<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F, O> Iterator for MapPending<I, F>
where
    I: TaskStatusIterator,
    F: Fn(I::Pending) -> O,
{
    type Item = TaskStatus<I::Ready, O, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            TaskStatus::Ready(value) => Some(TaskStatus::Ready(value)),
            TaskStatus::Pending(p) => Some(TaskStatus::Pending((self.mapper)(p))),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

/// Filters Ready values, returns None for non-matching.
pub struct FilterReady<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> Iterator for FilterReady<I, F>
where
    I: TaskStatusIterator,
    F: Fn(&I::Ready) -> bool,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next()? {
                TaskStatus::Ready(value) if (self.predicate)(&value) => {
                    return Some(TaskStatus::Ready(value));
                }
                TaskStatus::Ready(_) => continue, // Filtered out, get next
                other => return Some(other), // Forward unchanged
            }
        }
    }
}
```

## HOW: Implementation Approach

1. Define `TaskStatusIterator` trait as the foundation for TaskStatus-aware iteration
2. Define `TaskIteratorExt` trait with builder-style chaining methods
3. Each combinator is a custom struct holding `inner: I` + operation (`mapper`, `predicate`, etc.)
4. Each combinator's `next()` forwards unknown `TaskStatus` variants, transforms targeted ones
5. Provide blanket implementation of `TaskIteratorExt` for all `TaskStatusIterator` types
6. Execute via `execute()` which returns `StreamIterator` for end users

## Requirements

1. **TaskStatusIterator trait** - Core trait for TaskStatus-aware iteration (input to execute())
2. **TaskIteratorExt trait** - Builder-style chaining methods (map_ready, map_pending, filter_ready, etc.)
3. **Custom combinator types** - Each holds inner iterator + operation, forwards unknown states
4. **Blanket implementations** - Auto-implement for all `TaskStatusIterator` types
5. **Unit tests** - Verify combinators forward/transform correctly
6. **Integration test** - Chain multiple combinators, execute via `execute()` returning StreamIterator

## Tasks

1. [ ] Define `TaskStatusIterator` trait (no into_stream method - execute() is separate)
2. [ ] Define `TaskIteratorExt` trait with builder-style methods
3. [ ] Implement `MapReady<I, F>` combinator - transforms Ready, forwards rest
4. [ ] Implement `MapPending<I, F>` combinator - transforms Pending, forwards rest
5. [ ] Implement `FilterReady<I, F>` combinator - filters Ready, forwards rest
6. [ ] Implement `MapTaskStatus<I, F>` combinator - transforms any TaskStatus variant
7. [ ] Implement `StreamCollect<I>` combinator - yields StreamCollectStatus while gathering
8. [ ] Write unit tests for each combinator type
9. [ ] Write integration test chaining multiple combinators, then execute()

## Verification

```bash
cargo test -p foundation_core -- valtron::task_iterators
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 9 tasks completed
- `TaskStatusIterator` and `TaskIteratorExt` traits compile with zero errors
- All combinator types correctly forward/transform TaskStatus variants
- Unit tests pass for each combinator
- Integration test demonstrates chaining combinators then execute() returning StreamIterator
- Zero clippy warnings

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (v3.0: TaskIterator is input to execute(), which returns StreamIterator)_
