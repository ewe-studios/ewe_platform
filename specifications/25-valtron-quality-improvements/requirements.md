---
description: Address footguns, semantic bugs, and quality issues in valtron module identified during deep-dive review
status: pending
priority: high
created: 2026-05-12
author: Main Agent
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: false
metadata:
  version: '1.0'
  last_updated: 2026-05-12
  estimated_effort: medium
  tags:
  - valtron
  - quality
  - iterators
  - executors
  - correctness
  stack_files:
  - .agents/stacks/rust.md
  skills: []
  tools:
  - Rust
  - cargo
builds_on:
- 23-valtron-executor-deep-dive
related_specs:
- 09-valtron-streamiterator
- 12-background-job-registry
has_features: false
has_fundamentals: false
tasks:
  completed: 0
  uncompleted: 13
  total: 13
  completion_percentage: 0
---

# Valtron Quality Improvements - Requirements

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this specification MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. **Search the codebase** for similar implementations using Grep/Glob
2. **Read existing code** to understand project patterns and conventions
3. **Check stack files** (`.agents/stacks/[language].md`) for language-specific patterns
4. **Read module documentation** for modules you'll modify
5. **Follow discovered patterns** - do NOT invent new patterns without justification
6. **Verify all assumptions** by reading actual code

## Context

This specification captures issues found during a comprehensive deep-dive review of
`backends/foundation_core/src/valtron/` (~21,000 lines, 85 unit tests, 39 source files).

Valtron is a custom cooperative task executor built around synchronous iterators rather
than Rust's Future/async system. Tasks express async-like behavior through `TaskStatus`
variants and are polled by an execution engine that manages scheduling, priority lifting,
and thread distribution.

The issues below are organized by severity. Each represents a real correctness, semantic,
or quality concern that should be addressed.

## Scope

- Module: `backends/foundation_core/src/valtron/`
- Sub-modules: core types, streams, executors (single, multi, unified, drivers)
- Related: `iterators.rs`, `task.rs`, `streams.rs`, `drain.rs`, `funcs.rs`, `branches.rs`

---

## Issue 1 (HIGH): TaskIterator/Iterator Blanket Impl Recursion Trap

**Location:** `task.rs:311-401`

The blanket impl `impl TaskIterator for M where M: Iterator<Item = TaskStatus<R, P, S>>`
combined with `impl Iterator for Box<dyn TaskIterator>` creates a bidirectional coherence
loop. The existing code breaks the cycle using `as_mut().next_status()`, but this is fragile.

**Problem:** Anyone adding a new wrapper type that implements both `Iterator` and
`TaskIterator` will silently enter infinite recursion at runtime with no compile error.

**Recommendation:** Consider removing the blanket impl and requiring explicit `TaskIterator`
implementations, or use a newtype wrapper pattern that makes the direction unambiguous.

### Tasks
- [ ] TASK-01: Evaluate feasibility of removing the blanket `TaskIterator` impl
- [ ] TASK-02: If removal not feasible, add compile-time safeguards or prominent documentation

---

## Issue 2 (HIGH): CollectAllStream Uses O(n) remove Instead of swap_remove

**Location:** `unified.rs:751`

`self.sources.remove(idx)` is O(n) per call. When collecting from many sources and
sources exhaust at different times, this produces quadratic behavior.
`CollectNextFromAllStream` already uses `swap_remove` correctly.

**Recommendation:** Replace `remove(idx)` with `swap_remove(idx)`.

### Tasks
- [ ] TASK-03: Replace `sources.remove(idx)` with `swap_remove` in `CollectAllStream`

---

## Issue 3 (HIGH): TransformIterator Conflates Filtering With Termination

**Location:** `iterators.rs:194-202, 220-228`

When the transformer closure returns `None`, the iterator signals completion (returns
`None` from `next()`). This means a transformer that wants to skip one item accidentally
terminates the entire iterator.

**Problem:** Users expect `filter_map` semantics (skip `None`, continue) but get
`take_while` semantics (stop on first `None`).

**Recommendation:** Loop until the transformer returns `Some` or the source is exhausted,
providing `filter_map` semantics. If termination-on-None is desired, add a separate
`TransformUntilIterator`.

### Tasks
- [ ] TASK-04: Fix `TransformIterator::next()` to use filter_map semantics
- [ ] TASK-05: Fix `TransformSendIterator::next()` to use filter_map semantics

---

## Issue 4 (MEDIUM): Global Executor Panics on Uninitialized Access

**Location:** `single/mod.rs:63-66, 125-129, 148-153`

Every public function (`run_once`, `run_until`, `spawn`) panics if the pool isn't
initialized. In a large application, if any code path reaches valtron before
`initialize_pool()` is called, you get a hard panic with no recovery.

**Recommendation:** Return `Result` instead of panicking, or use lazy initialization
with sensible defaults.

### Tasks
- [ ] TASK-06: Evaluate switching single-threaded executor from panic to Result-based errors

---

## Issue 5 (MEDIUM): StateMachineTask Silently Swallows Errors

**Location:** `state_machine.rs:113-119`

`StateTransition::Error` logs a warning and returns `None`. The caller sees iterator
exhaustion with no way to distinguish success from failure. If `Output` is
`Result<T, E>`, errors should propagate through `TaskStatus::Ready(Err(e))`.

**Recommendation:** Propagate errors through the output type, or add a dedicated error
variant to `TaskStatus`.

### Tasks
- [ ] TASK-07: Propagate `StateTransition::Error` through `TaskStatus::Ready` instead of swallowing

---

## Issue 6 (MEDIUM): MapAllPendingAndDoneStream Discards Exhausted Sources Silently

**Location:** `unified.rs:1286-1293`

When a source returns `None`, it is skipped and the `states` vector becomes shorter
than `sources`. The mapper function receives a different-length vector on each poll.
If the mapper indexes by position, it silently gets wrong results after any source
completes early.

**Recommendation:** Either maintain a fixed-length vector with `Option<Stream>` entries,
or document clearly that the mapper must not rely on positional indexing.

### Tasks
- [ ] TASK-08: Fix `MapAllPendingAndDoneStream` to use fixed-length `Vec<Option<Stream>>` or document constraint

---

## Issue 7 (MEDIUM): Multi-threaded Stream Wait Uses spin_loop + sleep Polling

**Location:** `drivers.rs:232-236`

```rust
while stream.is_empty() && !stream.is_closed() {
    std::hint::spin_loop();
    std::thread::sleep(Duration::from_micros(100));
}
```

Fixed 100us sleep polling. CondVar or park/unpark would be more efficient.

**Recommendation:** Use CondVar-based notification when items are pushed to the queue.

### Tasks
- [ ] TASK-09: Replace spin+sleep polling with CondVar notification in multi-threaded stream wait

---

## Issue 8 (MEDIUM): Drain Trait Blanket Impl Requires Unnecessary Clone Bound

**Location:** `drain.rs:5-8`

The blanket impl requires `Clone + Send + 'static` on the iterator, but draining only
needs `Iterator`. The `Clone` bound excludes most real-world iterators.

**Recommendation:** Remove `Clone` and `Send` bounds from the blanket impl.

### Tasks
- [ ] TASK-10: Remove unnecessary `Clone + Send + 'static` bounds from `Drain` blanket impl

---

## Issue 9 (LOW): WASM drive_future_stream References Wrong Variable

**Location:** `drivers.rs:394`

```rust
drive_non_send_iterator(crate::valtron::from_stream(future))
//                                                    ^^^^^^ should be `stream`
```

The parameter is `stream` but the body uses `future`. This is a compile error on wasm32
targets — likely dead code since it's behind `#[cfg(target_arch = "wasm32")]`.

### Tasks
- [ ] TASK-11: Fix variable name `future` -> `stream` in wasm `drive_future_stream`

---

## Issue 10 (LOW): BranchPath::SKIP Uses ALL_CAPS Convention

**Location:** `branches.rs:75`

Rust convention reserves ALL_CAPS for constants, not enum variants. Should be `Skip`.

### Tasks
- [ ] TASK-12: Rename `BranchPath::SKIP` to `BranchPath::Skip`

---

## Issue 11 (LOW): Excessive Iterator Type Alias Proliferation

**Location:** `iterators.rs:13-24, 46-56, 159-170`

~30 type aliases for every primitive type (`CloneableI8Iterator`, `SendU16Iterator`,
`CloneableSendI32Iterator`, etc.). Most are unlikely to be used externally.

**Recommendation:** Audit usage and remove unused aliases. Keep only the generic base
types and commonly-used concrete ones.

### Tasks
- [ ] TASK-13: Audit and prune unused iterator type aliases

---

## Notes (Not Issues)

### unsafe impl Send in drivers.rs (INTENTIONAL)

Three `unsafe impl Send` blocks exist at `drivers.rs:709,832,913`. These are intentional
and correct — the inner types are all `Send` given the existing trait bounds. The `unsafe`
is needed because Rust's auto-trait solver cannot prove `Send` through the associated type
indirection on `TaskIterator`. The bounds on the type parameters (`T: Send`, `T::Ready: Send`,
`T::Pending: Send`) are sufficient to guarantee soundness.

### CloneableFn Clone impl commented out (funcs.rs:18-24)

The `Clone for Box<dyn CloneableFn>` impl is commented out due to stack overflow. The
workaround (`WrappedCloneableFnMut`) is adequate. Root cause is likely recursive clone
dispatch through the blanket impl. Not blocking.

### ConcurrentQueueStreamIterator no_std busy-wait (streams.rs:302)

Uses `std::hint::spin_loop()` on no_std. Acceptable given the `max_turns` bound limits
total spins per poll cycle.

---

## Task Summary

| ID | Priority | Description |
|----|----------|-------------|
| TASK-01 | HIGH | Evaluate removing TaskIterator blanket impl |
| TASK-02 | HIGH | Add safeguards for TaskIterator/Iterator recursion trap |
| TASK-03 | HIGH | Fix CollectAllStream O(n) remove -> swap_remove |
| TASK-04 | HIGH | Fix TransformIterator filter_map semantics |
| TASK-05 | HIGH | Fix TransformSendIterator filter_map semantics |
| TASK-06 | MEDIUM | Evaluate single executor panic -> Result |
| TASK-07 | MEDIUM | Propagate StateMachineTask errors |
| TASK-08 | MEDIUM | Fix MapAllPendingAndDoneStream source exhaustion |
| TASK-09 | MEDIUM | Replace spin+sleep with CondVar in multi-threaded wait |
| TASK-10 | MEDIUM | Remove unnecessary Drain bounds |
| TASK-11 | LOW | Fix wasm drive_future_stream variable name |
| TASK-12 | LOW | Rename BranchPath::SKIP to Skip |
| TASK-13 | LOW | Audit/prune unused iterator type aliases |
