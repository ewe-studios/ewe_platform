---
feature: "map_iter Combinator"
description: "Simple map_iter combinator for nested iterator patterns - outer iterator yields inner iterators"
status: "pending"
priority: "high"
depends_on: ["01-task-iterators", "02-stream-iterators"]
estimated_effort: "small"
created: 2026-03-23
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# map_iter Combinator Feature

## WHY: Problem Statement

The ClientRequest body read pattern (lines 393-420 in `client/api.rs`) requires a nested iterator pattern:

```rust
// Lines 393-420 in client/api.rs - manual nested iteration
for next_element in stream {
    match next_element {
        Ok(IncomingResponseParts::SizedBody(inner)) => {
            return Ok((conn, inner));  // inner is a ReadState iterator
        }
        // ... other cases ...
    }
}
Err(HttpClientError::InvalidState)  // Fallback if outer yields but inner never produces
```

**The pattern**:
1. Outer iterator (`StreamIterator` from `execute()`) yields `Stream::Next(inner_stream)`
2. Inner iterator (`inner_stream`) must be consumed until `None`
3. When inner exhausts without result, poll outer again for next inner
4. Only return `None` when outer itself returns `None`

**Current limitation**: No combinator to express this - requires manual `for` loops, breaking composability.

---

## WHAT: Solution Overview

The `map_iter` combinator flattens nested iterator patterns where the outer iterator yields inner iterators:

```rust
// Before: Manual nested loop (lines 393-420)
for next_element in stream {
    match next_element {
        Ok(IncomingResponseParts::SizedBody(inner)) => {
            return Ok((conn, inner));
        }
        // ... manual iteration ...
    }
}
Err(HttpClientError::InvalidState)
```

```rust
// After: Simple map_iter combinator
let body_stream = execute(task)?.map_iter(|done| match done {
    RequestIntro::Success { stream, conn, .. } => {
        // Mapper returns the inner iterator directly
        stream.map_done(|parts| match parts {
            Ok(IncomingResponseParts::SizedBody(inner)) => Ok((conn, inner)),
            Ok(_) => Err(HttpClientError::InvalidState),
            Err(e) => Err(HttpClientError::ReaderError(e)),
        })
    }
    RequestIntro::Failed(e) => {
        // Return an iterator that yields one error and exhausts
        std::iter::once(Err(e))
    }
});
```

**How it works**:

**For StreamIterator**:
1. Call `outer.next()` → gets `Stream::Next(inner_iter)`
2. Store `inner_iter`
3. Call `inner_iter.next()` repeatedly - yields `Stream<InnerD, InnerP>` directly
4. When inner returns `None`, call `outer.next()` again for next inner
5. Repeat until outer returns `None`

**For TaskIterator**:
1. Call `outer.next_status()` → gets `TaskStatus::Ready(inner_iter)`
2. Store `inner_iter`
3. Call `inner_iter.next_status()` repeatedly - yields `TaskStatus<InnerR, InnerP, InnerS>` directly
4. When inner returns `None`, call `outer.next_status()` again for next inner
5. Repeat until outer returns `None`

**Signatures**:
```rust
// For StreamIterator - mapper returns an iterator, inner's types become output
fn map_iter<F, InnerIter>(
    mapper: F,
) -> MapIter<Self, F, InnerIter>
where
    F: Fn(Self::Done) -> InnerIter,
    InnerIter: Iterator,
// Output: StreamIterator<Done = InnerIter::Item for Stream::Next, Pending = InnerIter::Pending>

// For TaskIterator - mapper returns a TaskIterator, inner's types become output
fn map_iter<F, InnerIter>(
    mapper: F,
) -> MapIter<Self, F, InnerIter>
where
    F: Fn(Self::Ready) -> InnerIter,
    InnerIter: TaskIterator,
// Output: TaskIterator<Ready = InnerIter::Ready, Pending = InnerIter::Pending, Spawner = InnerIter::Spawner>
```

**Key points**:
- Mapper returns an iterator (`Iterator` for StreamIterator, `TaskIterator` for TaskIterator)
- Inner iterator yields its own `Stream<InnerD, InnerP>` or `TaskStatus<InnerR, InnerP, InnerS>` directly
- Wrapper just forwards inner's values unchanged
- When inner drains, poll outer for next iterator

---

## HOW: Implementation Steps

1. **Read existing combinator patterns** - Review existing combinators in `stream_iterators.rs` and `task_iterators.rs`
2. **Create `MapIter` wrapper struct** - Holds outer iterator, current inner iterator, mapper
3. **Implement for StreamIterator** - Drain inner until None, then poll outer for new iterator
4. **Implement for TaskIterator** - Same pattern, forwards TaskStatus values
5. **Add `map_iter()` extension method** - To both `StreamIteratorExt` and `TaskIteratorExt` traits
6. **Apply to ClientRequest body read** - Refactor lines 393-420 to use `map_iter()`
7. **Test with #[traced_test]** - Verify nested iteration works correctly

---

## Requirements

1. **MapIter wrapper struct** - Generic over outer iterator, mapper function, and inner iterator type
2. **StreamIterator implementation** - Drain inner iterator until None, then poll outer for next
3. **TaskIterator implementation** - Same pattern for TaskIterator, forwards TaskStatus values
4. **Extension methods** - `map_iter()` added to both `StreamIteratorExt` and `TaskIteratorExt` traits
5. **ClientRequest refactor** - Lines 393-420 expressed using `map_iter()`
6. **Unit tests** - Verify nested iteration pattern with `#[traced_test]`

---

## Tasks

1. [ ] Read existing combinator patterns in `stream_iterators.rs` and `task_iterators.rs`
2. [ ] Create `MapIter<I, F, InnerIter>` wrapper struct
3. [ ] Implement `StreamIterator` for `MapIter` with proper state management
4. [ ] Implement `TaskIterator` for `MapIter` with proper state management
5. [ ] Add `map_iter()` extension method to `StreamIteratorExt`
6. [ ] Add `map_iter()` extension method to `TaskIteratorExt`
7. [ ] Refactor ClientRequest body read (lines 393-420) to use `map_iter()`
8. [ ] Write unit tests for `MapIter` with `#[traced_test]`

---

## Verification

```bash
cargo check -p foundation_core
cargo test -p foundation_core map_iter
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

---

## Success Criteria

- All 8 tasks completed
- `MapIter` wrapper implemented and tested for both `StreamIterator` and `TaskIterator`
- `map_iter()` method available on both `StreamIteratorExt` and `TaskIteratorExt`
- ClientRequest body read refactored to use `map_iter()`
- Nested iteration pattern works without manual `for` loops
- Zero clippy warnings

---

## Relationship to Other Features

| Feature | Role |
|---------|------|
| 02-stream-iterators | Provides `StreamIterator` trait and `Stream` enum |
| 06a-client-request-refactor | Uses `map_iter()` for body read pattern |
| 06c-gen-model-descriptors-parallel-fetch | Can use `map_iter()` after parallel fetch |

---

## Architecture Flow

```
StreamIterator (outer)
      │
      │ .map_iter(mapper)
      ▼
MapIter wrapper
      │
      │ next() called
      ▼
Is there a current inner iterator?
      ├── YES → drain it until None
      │         └─→ yield Stream::Next(item) or map Pending
      │
      └── NO → poll outer for next item
                ├── Stream::Next(d) → mapper(d) returns new inner iterator
                │                     └─→ start draining it
                │
                └── Stream::Ignore → propagate (or wait)
```

---

## Example: ClientRequest Body Read with map_iter

```rust
// Before: Manual nested iteration (lines 393-420)
for next_element in stream { ... }

// After: Declarative map_iter
let body_stream = execute(task)?
    .map_iter(|done| match done {
        RequestIntro::Success { stream, conn, .. } => {
            // Mapper returns the inner iterator directly - no Either
            stream.map_done(|parts| match parts {
                Ok(IncomingResponseParts::SizedBody(inner)) => Ok((conn, inner)),
                Ok(_) => Err(HttpClientError::InvalidState),
                Err(e) => Err(HttpClientError::ReaderError(e)),
            })
        }
        RequestIntro::Failed(e) => {
            // Return an iterator that yields one error and exhausts
            std::iter::once(Err(e))
        }
    });
```

---

_Created: 2026-03-23_
