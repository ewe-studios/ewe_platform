# Learnings

This file captures learnings, discoveries, and design decisions made during implementation of Valtron Async Iterators.

## Pending Learnings

_No learnings recorded yet._

---

## Design Decisions

### Why TaskStatusIterator instead of modifying Iterator?

The standard `Iterator` trait cannot be modified and is fundamentally synchronous. By creating a new `TaskStatusIterator` trait, we:
- Keep `std::iter::Iterator` unchanged for synchronous cases
- Make async state (`Pending`, `Delayed`, `Ready`) explicit in the type system
- Enable Valtron executor integration without blocking

### Why collect_all() returns TaskStatus<Vec<T>> not Vec<T>?

Returning `TaskStatus<Vec<T>>` instead of `Vec<T>` allows the collection itself to be async-aware:
- Caller sees `Pending` while any sources are still pending
- Caller sees `Ready(vec)` only when all sources complete
- Enables further composition (map, filter) on the collected result

---

_Created: 2026-03-20_
_Updated: 2026-03-20 (v3.0: TaskIterator/StreamIterator separation)_

## Design Decisions

### Why TaskStatusIterator instead of modifying Iterator?

The standard `Iterator` trait cannot be modified and is fundamentally synchronous. By creating a new `TaskStatusIterator` trait, we:
- Keep `std::iter::Iterator` unchanged for synchronous cases
- Make async state (`Pending`, `Delayed`, `Ready`) explicit in the type system
- Enable Valtron executor integration without blocking

### Why collect_all() Returns TaskStatus<Vec<T>> Not Vec<T>?

Returning `TaskStatus<Vec<T>>` instead of `Vec<T>` allows the collection itself to be async-aware:
- Caller sees `Pending` while any sources are still pending
- Caller sees `Ready(vec)` only when all sources complete
- Enables further composition (map, filter) on the collected result

### Why execute() Returns StreamIterator Not TaskIterator? (v3.0 Architecture)

**Critical Design Decision**: `execute()` takes `TaskIterator` as input and returns `StreamIterator` as output. This separation provides:

1. **Clear Boundary**: TaskIterators are for implementers defining async tasks; StreamIterators are for end users consuming results
2. **Hidden Complexity**: The executor engine (delays, actions, spawner) is hidden inside `execute()`
3. **Simplified End User Experience**: End users work with `Stream<D, P>` variants (Init, Pending, Delayed, Next) without TaskStatus complexity
4. **Opt-in for TaskStatus**: `execute_as_task()` exists for rare cases needing TaskStatus output

```rust
// Implementer defines task with combinators (BEFORE execute)
let task = fetch_task().map_ready(transform).map_pending(log);

// execute() is the boundary - returns StreamIterator for end users
let stream = execute(task)?;  // StreamIterator, not TaskIterator

// End user consumes results (AFTER execute)
for item in stream {
    match item {
        Stream::Pending(_) => { /* still waiting */ }
        Stream::Next(value) => { /* got result */ }
    }
}
```

### Why Extension Traits for Combinators?

Extension traits (`TaskIteratorExt`, `StreamIteratorExt`) provide builder-style combinators:

1. **Automatic Availability**: Any type implementing `TaskStatusIterator` automatically gets all combinator methods
2. **Builder Pattern**: Enables chaining like `.map_ready(f).map_pending(g).filter_ready(h)`
3. **Internal Wrappers**: Each combinator wraps the iterator in a custom struct (MapReady, FilterReady, etc.)
4. **Type Safety**: Compiler enforces correct usage through trait bounds

### Why Wrapper Types for Combinators?

Each combinator (MapReady, MapPending, FilterReady, etc.) is a custom struct that:
1. **Holds the Source**: `inner: I` stores the wrapped iterator
2. **Holds the Operation**: `mapper: F` or `predicate: F` stores the transformation
3. **Forwards Unknown States**: `Pending`, `Delayed`, `Init`, `Spawn` pass through unchanged
4. **Transforms Target States**: Only the targeted variant (e.g., `Ready` for `MapReady`) is transformed

This pattern enables:
- **Composability**: Multiple combinators can be chained
- **Lazy Evaluation**: Transformations happen on `next()`, not at construction
- **Zero-Cost Abstraction**: No runtime overhead beyond the transformation itself

### Why Stream<D, P> Instead of StreamState?

We use the existing `Stream<D, P>` enum from `synca/mpp.rs` rather than creating a new `StreamState` type:

```rust
// Already exists - use it!
pub enum Stream<D, P> {
    Init,
    Ignore,
    Delayed(Duration),
    Pending(P),
    Next(D),  // This is "Done"
}
```

This avoids:
- Duplicate type definitions
- Confusion about which type to use
- Unnecessary trait conversions

### Why split_collector() Is Available on Both Traits?

The `split_collector()` combinator works on both `TaskIterator` (before `execute()`) and `StreamIterator` (after `execute()`):

1. **TaskIterator Context**: Split before execution, then continue with `execute(continuation)`
2. **StreamIterator Context**: Split after execution for pure stream processing
3. **Observer Pattern**: One branch observes intermediate values, the other continues the chain
4. **ClientRequest Use Case**: Get intro/headers first (observer), then read body (continuation)

```rust
// On TaskIterator (before execute)
let (observer, continuation) = task.split_collect_one(predicate);
let stream = execute(continuation)?;

// On StreamIterator (after execute)
let (observer, continuation) = stream.split_collect_one(predicate);
```

### Why Clone Bounds for split_collector()?

The `split_collector()` method requires `Ready: Clone` and `Pending: Clone` because:

1. **Observer Gets a Copy**: The observer branch receives cloned items via `ConcurrentQueue`
2. **Continuation Gets Original**: The continuation continues with the original iterator
3. **Trade-off Accepted**: Clone requirement is acceptable for the clean observer/continuation pattern

This is a deliberate trade-off that enables the ClientRequest intro/body pattern without manual state machines.
