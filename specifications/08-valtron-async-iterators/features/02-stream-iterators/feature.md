---
feature: "Stream Iterators"
description: "StreamIteratorExt trait with state-aware combinators using custom wrapper types with embedded mappers"
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

# Stream Iterators Feature

## WHY: Problem Statement

`Stream<Done, Pending>` from Valtron represents async computations that may be pending. Standard Iterator methods cannot handle `Pending` states - they block waiting for `Done`. Need state-aware iterator extensions that:

- Handle `Pending`, `Delayed`, `Init`, and `Done` states explicitly
- Never block waiting for completion
- Enable transformations based on state (e.g., "map only when done")
- Use custom wrapper types that contain both iterator and mapper function

## WHAT: Solution Overview

Implement `StreamIteratorExt` trait providing state-aware combinators using custom wrapper types:

### Key Design Pattern: Custom Wrapper Types with Embedded Mappers

Instead of creating generic wrapper types, each combinator is a custom struct that:
1. Holds the source iterator(s)
2. Holds the mapper function
3. Applies mapper on each `next()` call with appropriate state awareness

```rust
/// Map values only when all sources reach Done state
///
/// This custom type holds both the source iterators and the mapper function,
/// applying the mapper when all sources complete.
pub struct MapAllDoneStreamIterator<I, F> {
    sources: Vec<I>,
    mapper: F,
    buffer: Vec<Option<I::Item>>,
}

impl<I, F, O> Iterator for MapAllDoneStreamIterator<I, F>
where
    I: Iterator<Item = Stream<I::Item, P>>,
    F: Fn(Vec<I::Item>) -> O,
{
    type Item = Stream<O, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // Poll all sources, buffer values
        // When all done: apply self.mapper(buffer), return Stream::Next(result)
        // While any pending: return Stream::Pending
    }
}
```

### StreamState - Use Existing `Stream<D, P>`

**Note**: Do NOT create a new `StreamState` enum. Use the existing `Stream<D, P>` from `synca/mpp.rs`:

```rust
pub enum Stream<D, P> {
    Init,
    Ignore,
    Delayed(Duration),
    Pending(P),
    Next(D),  // This is "Done"
}
```

All combinators should pattern match on `Stream<D, P>` directly:

```rust
match stream_item {
    Stream::Next(value) => { /* done with value */ }
    Stream::Pending(ctx) => { /* pending with context */ }
    Stream::Delayed(dur) => { /* delayed by duration */ }
    Stream::Init => { /* initializing */ }
    Stream::Ignore => { /* internal event, ignore */ }
}
```

### Helper trait for Stream state checks

```rust
/// Extension trait for Stream<D, P> with convenience methods
impl<D, P> Stream<D, P> {
    pub fn is_done(&self) -> bool { matches!(self, Stream::Next(_)) }
    pub fn is_pending(&self) -> bool { matches!(self, Stream::Pending(_)) }
    pub fn is_delayed(&self) -> bool { matches!(self, Stream::Delayed(_)) }
    pub fn is_init(&self) -> bool { matches!(self, Stream::Init) }
}
```

### Key Trait Definition

```rust
pub trait StreamIteratorExt<D, P>: Iterator<Item = Stream<D, P>> {
    /// Collect all outputs from multiple StreamIterators
    ///
    /// Returns custom CollectAll type that holds sources and aggregates results.
    fn collect_all<I>(iterators: Vec<I>) -> CollectAllStreamIterator<I>
    where
        I: StreamIteratorExt<Item = D, Pending = P>;

    /// Map all values - only when all sources reach Done state
    ///
    /// Returns custom MapAllDone type that holds sources + mapper.
    fn map_all_done<I, F, O>(iterators: Vec<I>, mapper: F) -> MapAllDoneStreamIterator<I, F>
    where
        I: StreamIteratorExt<Item = D, Pending = P>,
        F: Fn(Vec<D>) -> O + Send + 'static;

    /// Map all values processing both Pending and Done states together
    ///
    /// Returns custom MapAllPendingDone type that holds sources + mapper.
    fn map_all_pending_and_done<I, F, O>(
        iterators: Vec<I>,
        mapper: F,
    ) -> MapAllPendingDoneStreamIterator<I, F>
    where
        I: StreamIteratorExt<Item = D, Pending = P>,
        F: Fn(Vec<StreamState<D, P>>) -> O + Send + 'static;

    /// Stream-collect that yields StreamCollectStatus while gathering items.
    ///
    /// Unlike std::Iterator::collect(), this does NOT block waiting for all items.
    /// Instead, it yields Stream variants showing collection progress:
    /// - Stream::Pending(count) - still gathering, have `count` items so far
    /// - Stream::Next(Vec<D>) - collection complete with all items
    ///
    /// # Example
    ///
    /// ```rust
    /// let stream = fetch_models_stream(client)
    ///     .stream_collect();
    ///
    /// for status in stream {
    ///     match status {
    ///         Stream::Pending(count) => println!("Collected {count} so far..."),
    ///         Stream::Next(all_items) => println!("Done! Got {} items", all_items.len()),
    ///         Stream::Delayed(_) => continue,
    ///     }
    /// }
    /// ```
    fn stream_collect(self) -> StreamCollect<Self>
    where
        Self: Sized,
        D: Clone;
}
```

### StreamCollect Type

```rust
/// Stream-based collector for Stream iterators that yields progress while gathering.
///
/// Unlike std::Iterator::collect(), this does NOT block waiting for all items.
/// It yields Stream::Pending(count) as items arrive, finally yielding Stream::Next(vec).
pub struct StreamCollect<I> {
    inner: I,
    collected: Vec<I::Item>,
}

impl<I> Iterator for StreamCollect<I>
where
    I: Iterator<Item = Stream<I::Item, P>>,
    I::Item: Clone,
{
    type Item = Stream<Vec<I::Item>, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Stream::Next(value) => {
                self.collected.push(value);
                Some(Stream::Pending(self.collected.len()))
            }
            Stream::Pending(_) => Some(Stream::Pending(self.collected.len())),
            Stream::Delayed(d) => Some(Stream::Delayed(d)),
            Stream::Init => Some(Stream::Init),
            // When inner is exhausted, yield the final collection
            None => {
                if self.collected.is_empty() {
                    None
                } else {
                    let collected = std::mem::take(&mut self.collected);
                    Some(Stream::Next(collected))
                }
            }
        }
    }
}
```

## HOW: Implementation Approach

1. Define `StreamState<D, P>` enum in `stream_iterators.rs`
2. Implement `StreamIteratorExt` trait with all methods returning custom wrapper types
3. Each wrapper type holds its sources and mapper function
4. Add tests for state-aware behavior

## Requirements

1. **StreamState enum** - Unified state representation with helper methods
2. **StreamIteratorExt trait** - Extension trait with all combinators
3. **Custom wrapper types** - Each combinator is a struct holding sources + mapper
4. **collect_all()** - Aggregate multiple StreamIterators
5. **map_all_done()** - Map when all sources complete (holds mapper)
6. **map_all_pending_and_done()** - Map with Pending+Done visibility (holds mapper)
7. **collect_nonblocking()** - Non-blocking collect yielding progress states

## Tasks

1. [ ] Define `StreamState<D, P>` enum with Init, Pending, Delayed, Done variants
2. [ ] Define `StreamIteratorExt<D, P>` trait with all methods
3. [ ] Implement `collect_all()` returning CollectAllStreamIterator wrapper type
4. [ ] Implement `map_all_done()` returning MapAllDoneStreamIterator wrapper type
5. [ ] Implement `map_all_pending_and_done()` returning MapAllPendingDoneStreamIterator wrapper type
6. [ ] Implement `stream_collect()` returning StreamCollect wrapper type (stream_collect method)
7. [ ] Write unit tests for each combinator
8. [ ] Verify wrapper types hold sources + mapper correctly
9. [ ] Run clippy and fmt checks

## Verification

```bash
cargo test -p foundation_core -- valtron::stream_iterators
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

## Success Criteria

- All 9 tasks completed
- `StreamIteratorExt` trait compiles with zero errors
- All combinators functional with proper state handling
- Wrapper types correctly hold sources and mappers
- `collect_nonblocking()` yields progress states without blocking
- Unit tests pass for state-aware behavior
- Zero clippy warnings

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (Custom wrapper types with embedded mappers)_
