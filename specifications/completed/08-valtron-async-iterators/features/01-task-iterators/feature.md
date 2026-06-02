---
feature: "Task Iterators"
description: "TaskIteratorExt extension trait for builder-style chaining on any T: TaskIterator"
status: "complete"
priority: "high"
depends_on: ["00-foundation"]
estimated_effort: "medium"
created: 2026-03-20
author: "Main Agent"
tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
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
    .map_ready(|m| m.with_enhanced_metadata())  // Transform Ready values
    .filter_pending(|p| !p.is_transient());      // Filter Pending states

// Execute returns StreamIterator for end users
let stream = execute(task)?;  // StreamIterator consumed by end users
```

End users never deal with TaskIterator directly - they receive `StreamIterator` from `execute()`.

## WHAT: Solution Overview

### TaskIteratorExt Trait (Builder-Style Chaining)

```rust
/// Extension trait providing builder-style combinator methods.
///
/// Implemented for any T: TaskIterator with proper bounds.
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
pub trait TaskIteratorExt: TaskIterator + Sized {
    /// Map Ready values using the provided function.
    ///
    /// Forwards Pending, Delayed, Init, Spawn unchanged.
    fn map_ready<F, O>(self, mapper: F) -> MapReady<Self, F>
    where
        F: Fn(Self::Ready) -> O + Send + 'static,
        O: Send + 'static;

    /// Map Pending values using the provided function.
    ///
    /// Forwards Ready, Delayed, Init, Spawn unchanged.
    fn map_pending<F, O>(self, mapper: F) -> MapPending<Self, F>
    where
        F: Fn(Self::Pending) -> O + Send + 'static,
        O: Send + 'static;

    /// Filter Ready values, returning None for filtered items.
    ///
    /// Forwards Pending, Delayed, Init, Spawn unchanged.
    fn filter_ready<F>(self, predicate: F) -> FilterReady<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Stream-collect that gathers all Ready values.
    ///
    /// Unlike std::Iterator::collect(), this does NOT block waiting for all items.
    /// It passes through Pending, Delayed, Init, Spawn states unchanged,
    /// and only yields the collected Vec<Ready> when the inner iterator returns None.
    ///
    /// # Example
    ///
    /// ```rust
    /// let task = fetch_models_dev_task(client)
    ///     .stream_collect();
    ///
    /// // Execute returns StreamIterator
    /// for item in execute(task)? {
    ///     match item {
    ///         Stream::Pending(_) => println!("Still fetching..."),
    ///         Stream::Delayed(d) => println!("Delayed by {:?}", d),
    ///         Stream::Next(all_items) => println!("Done! Got {} items", all_items.len()),
    ///         _ => {}
    ///     }
    /// }
    /// ```
    fn stream_collect(self) -> StreamCollect<Self>
    where
        Self::Ready: Clone + Send + 'static;
}
```

### StreamCollect Type (Simplified - No StreamCollectStatus)

```rust
/// Stream-based collector that gathers all Ready values.
///
/// Unlike std::Iterator::collect(), this does NOT block waiting for all items.
/// It passes through Pending, Delayed, Init, Spawn states unchanged,
/// and yields Vec<Ready> only when the inner iterator completes.
///
/// This is a TaskIterator combinator - apply BEFORE execute().
pub struct StreamCollect<I> {
    inner: I,
    collected: Vec<I::Ready>,
    done: bool,
}

impl<I> Iterator for StreamCollect<I>
where
    I: TaskIterator,
    I::Ready: Clone,
{
    type Item = TaskStatus<Vec<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already yielded the collected result, we're done
        if self.done {
            return None;
        }

        loop {
            match self.inner.next()? {
                TaskStatus::Ready(value) => {
                    self.collected.push(value);
                    // Don't yield yet - keep collecting
                }
                TaskStatus::Pending(p) => {
                    // Pass through Pending with current count info
                    return Some(TaskStatus::Pending(p));
                }
                TaskStatus::Delayed(d) => {
                    return Some(TaskStatus::Delayed(d));
                }
                TaskStatus::Init => {
                    return Some(TaskStatus::Init);
                }
                TaskStatus::Spawn(s) => {
                    return Some(TaskStatus::Spawn(s));
                }
            }
        }
        // When inner returns None, we fall through and yield the collected result
        // But we need to track that we've yielded it, so set done = true first
        // This requires restructuring - see actual implementation
    }
}
```

**Simplified approach**: Just collect all Ready values silently, pass through all other states, and only yield the final Vec<Ready> when done. No intermediate StreamCollectStatus.

### Custom Combinator Types (Hold Source + Operation)

```rust
/// Maps Ready values, forwards all other TaskStatus variants unchanged.
pub struct MapReady<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F, O> Iterator for MapReady<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> O + Send + 'static,
    O: Send + 'static,
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

impl<I, F, O> TaskIterator for MapReady<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> O + Send + 'static,
    O: Send + 'static,
{
    type Ready = O;
    type Pending = I::Pending;
    type Spawner = I::Spawner;
}

/// Maps Pending values, forwards all other TaskStatus variants unchanged.
pub struct MapPending<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F, O> Iterator for MapPending<I, F>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> O + Send + 'static,
    O: Send + 'static,
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

impl<I, F, O> TaskIterator for MapPending<I, F>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> O + Send + 'static,
    O: Send + 'static,
{
    type Ready = I::Ready;
    type Pending = O;
    type Spawner = I::Spawner;
}

/// Filters Ready values, returns None for non-matching.
pub struct FilterReady<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> Iterator for FilterReady<I, F>
where
    I: TaskIterator,
    F: Fn(&I::Ready) -> bool + Send + 'static,
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

impl<I, F> TaskIterator for FilterReady<I, F>
where
    I: TaskIterator,
    F: Fn(&I::Ready) -> bool + Send + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;
    type Spawner = I::Spawner;
}
```

## HOW: Implementation Approach

1. Define `TaskIteratorExt` trait with builder-style chaining methods for any `T: TaskIterator`
2. Each combinator is a custom struct holding `inner: I` + operation (`mapper`, `predicate`, etc.)
3. Each combinator's `next()` forwards unknown `TaskStatus` variants, transforms targeted ones
4. Each combinator also implements `TaskIterator` to enable chaining
5. Provide blanket implementation of `TaskIteratorExt` for all `T: TaskIterator` with proper bounds
6. Execute via `execute()` which returns `StreamIterator` for end users

## Requirements

1. **TaskIteratorExt trait** - Builder-style chaining methods (map_ready, map_pending, filter_ready, stream_collect)
2. **Custom combinator types** - Each holds inner iterator + operation, forwards unknown states
3. **Combinators implement TaskIterator** - Enables chaining multiple combinators
4. **Blanket implementations** - Auto-implement for all `T: TaskIterator` with proper bounds
5. **Unit tests** - Verify combinators forward/transform correctly
6. **Integration test** - Chain multiple combinators, execute via `execute()` returning StreamIterator

## Tasks

1. [x] Define `TaskIteratorExt` trait with builder-style methods (extend T: TaskIterator)
2. [x] Implement `TMapReady<I, F>` combinator - transforms Ready, forwards rest
3. [x] Implement `TMapPending<I, F>` combinator - transforms Pending, forwards rest
4. [x] Implement `TFilterReady<I, F>` combinator - filters Ready, forwards rest
5. [x] Implement `TStreamCollect<I>` combinator - collects Ready values, passes through rest
6. [x] Write unit tests for each combinator type
7. [x] Write integration test chaining multiple combinators, then execute()

## Verification

```bash
cargo test -p foundation_core -- valtron::task_iterators
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 7 tasks completed
- `TaskIteratorExt` trait compiles with zero errors
- All combinator types correctly forward/transform TaskStatus variants
- All combinators implement TaskIterator for chaining
- Unit tests pass for each combinator
- Integration test demonstrates chaining combinators then execute() returning StreamIterator
- Zero clippy warnings

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (v3.1: Removed TaskStatusIterator, simplified StreamCollect)_
