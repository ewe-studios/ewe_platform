---
feature: error-handling-api
description: Fix error swallowing in StateMachineTask, executor panic-on-init, MapAllPendingAndDoneStream, and Drain bounds
status: pending
priority: medium
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
dependencies: []
---

# Feature 06: Error Handling & API Quality

## Problem

Four related error handling and API quality issues:

### 1. StateMachineTask Silently Swallows Errors (MEDIUM)

**Location:** `state_machine.rs:113-119`

`StateTransition::Error` logs a warning and returns `None`. The caller sees iterator
exhaustion with no way to distinguish success from failure. If `Output` is `Result<T, E>`,
errors should propagate through `TaskStatus::Ready(Err(e))`.

### 2. Global Executor Panics on Uninitialized Access (MEDIUM)

**Location:** `single/mod.rs:63-66, 125-129, 148-153`

**Verdict:**: we want a panic, its intended

Every public function (`run_once`, `run_until`, `spawn`) panics if the pool isn't
initialized. In a large application, if any code path reaches valtron before
`initialize_pool()` is called, you get a hard panic with no recovery.

### 3. MapAllPendingAndDoneStream Discards Exhausted Sources (MEDIUM)

**Location:** `unified.rs:1286-1293`

When a source returns `None`, it is skipped and the `states` vector becomes shorter than
`sources`. The mapper function receives a different-length vector on each poll. If the
mapper indexes by position, it silently gets wrong results after any source completes early.

### 4. Drain Trait Blanket Impl Requires Unnecessary Bounds (MEDIUM)

**Location:** `drain.rs:5-8`

The blanket impl requires `Clone + Send + 'static` on the iterator, but draining only needs
`Iterator`. The `Clone` bound excludes most real-world iterators.

## Approach

1. **StateMachineTask**: Propagate `StateTransition::Error` through `TaskStatus::Ready(err)`
   instead of returning `None`. This requires the error type to be convertible to the output
   type, or adding an error variant to the output.

3. **MapAllPendingAndDoneStream**: Either maintain a fixed-length `Vec<Option<Stream>>`
   with `None` entries for exhausted sources, or document clearly that the mapper must not
   rely on positional indexing. 

    Document clearly on the type and trait methods. But if use Option improves speed because no removal is needed, then use Vec<Option<Stream>>, same for task Iterators.

4. **Drain trait**: Remove `Clone` and `Send` bounds from the blanket impl.

## Tasks

- [ ] TASK-06-01: Propagate `StateTransition::Error` through `TaskStatus::Ready` in `StateMachineTask` instead of returning `None` (`state_machine.rs:113-119`)
- [ ] TASK-06-02: Evaluate switching single-threaded executor public functions from panic to `Result<_, ExecutorError>` (`single/mod.rs`); document migration path if feasible
- [ ] TASK-06-03: Fix `MapAllPendingAndDoneStream` to use `Vec<Option<Stream>>` maintaining fixed source positions, or add runtime documentation and assertion that mapper must not index by position (`unified.rs:1286-1293`)
- [ ] TASK-06-04: Remove `Clone + Send + 'static` bounds from `Drain` blanket impl, keeping only `Iterator` (`drain.rs:5-8`)
- [ ] TASK-06-05: Add tests for StateMachineTask error propagation and Drain with non-Clone iterators
