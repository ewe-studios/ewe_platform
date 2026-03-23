---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-valtron-async-iterators"
this_file: "specifications/08-valtron-async-iterators/features/06b-map-iter-combinator/start.md"
created: 2026-03-23
---

# Start: map_iter Combinator Feature

## Feature Overview

**Goal**: Implement the `map_iter()` combinator for nested iterator patterns.

**Problem**: When an outer iterator yields inner iterators (e.g., `Stream::Next(inner_stream)`), there's no combinator to flatten this pattern. Current code requires manual `for` loops.

**Solution**: `map_iter()` takes a mapper that transforms each `Done` value into an inner iterator, then drains that inner iterator before polling the outer for more.

---

## Workflow

### Step 1: Read Prerequisites
1. Read `../02-stream-iterators/feature.md` - Understand `StreamIterator` trait and `Stream` enum
2. Read existing combinator implementations (e.g., `map_done()`) for pattern reference

### Step 2: Implement map_iter

1. **Create `MapIter` wrapper struct**:
   ```rust
   pub struct MapIter<I, F, InnerIter>
   where
       I: StreamIterator,
       F: Fn(I::Done) -> InnerIter,
       InnerIter: Iterator,
   {
       outer: I,
       mapper: F,
       current_inner: Option<InnerIter>,
   }
   ```

2. **Implement `StreamIterator` for `MapIter`**:
   - On `next()`, first drain `current_inner` until `None`
   - When inner exhausts, poll outer for next item
   - Use mapper to create new inner, store in `current_inner`
   - Continue draining

3. **Add `map_iter()` to `StreamIteratorExt`**:
   ```rust
   fn map_iter<F, InnerIter>(self, mapper: F) -> MapIter<Self, F, InnerIter>
   where
       F: Fn(Self::Done) -> InnerIter,
       InnerIter: Iterator,
   ```

### Step 3: Apply to ClientRequest

Refactor lines 393-420 in `client/api.rs`:
```rust
// Before: Manual for loop
for next_element in stream { ... }

// After: Declarative map_iter
let body_stream = execute(task)?.map_iter(|done| { ... });
```

### Step 4: Test

Write unit tests with `#[traced_test]` to verify:
- Inner iterator is drained completely
- Outer is polled for next inner when current exhausts
- `Stream::Ignore` only when outer itself returns `None`

---

## Files to Modify

- `backends/foundation_core/src/valtron/stream_iterators.rs` (or equivalent) - Add `MapIter` and extension method
- `backends/foundation_core/src/wire/simple_http/client/api.rs` - Refactor body read to use `map_iter()`

---

## Verification

```bash
cargo check -p foundation_core
cargo test -p foundation_core map_iter
cargo clippy -p foundation_core -- -D warnings
cargo fmt -p foundation_core -- --check
```

---

**Next**: After completing this feature, proceed to `../06c-gen-model-descriptors-parallel-fetch/start.md`

---

_Created: 2026-03-23_
