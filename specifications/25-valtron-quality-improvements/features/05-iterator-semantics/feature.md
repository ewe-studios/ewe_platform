---
feature: iterator-semantics
description: Fix TaskIterator/Iterator recursion trap, TransformIterator semantics, and CollectAllStream O(n) remove
status: pending
priority: high
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0
dependencies: []
---

# Feature 05: Iterator Semantics Fixes

## Problem

Three distinct iterator semantic issues:

### 1. TaskIterator/Iterator Blanket Impl Recursion Trap (HIGH)

**Location:** `task.rs:311-401`

The blanket impl `impl TaskIterator for M where M: Iterator<Item = TaskStatus<R, P, S>>`
combined with `impl Iterator for Box<dyn TaskIterator>` creates a bidirectional coherence
loop. The existing code breaks the cycle using `as_mut().next_status()`, but this is fragile.

Anyone adding a new wrapper type that implements both `Iterator` and `TaskIterator` will
silently enter infinite recursion at runtime with no compile error.

### 2. TransformIterator Conflates Filtering With Termination (HIGH)

**Location:** `iterators.rs:194-202, 220-228`

When the transformer closure returns `None`, the iterator signals completion (returns `None`
from `next()`). Users expect `filter_map` semantics (skip `None`, continue) but get
`take_while` semantics (stop on first `None`).

### 3. CollectAllStream Uses O(n) remove Instead of swap_remove (HIGH)

**Location:** `unified.rs:751`

`self.sources.remove(idx)` is O(n) per call. When collecting from many sources that exhaust
at different times, this produces quadratic behavior. `CollectNextFromAllStream` already
uses `swap_remove` correctly.

## Approach

1. **TaskIterator blanket impl**: Evaluate removing the blanket impl entirely and requiring
   explicit `TaskIterator` implementations. If removal breaks too much, add a newtype wrapper
   pattern and prominent documentation about the recursion trap.

2. **TransformIterator**: Change `next()` to loop until the transformer returns `Some` or
   the underlying source is exhausted, providing `filter_map` semantics. If
   termination-on-None is desired, add a separate `TransformUntilIterator`.

3. **CollectAllStream**: Replace `remove(idx)` with `swap_remove(idx)`.

## Tasks

- [ ] TASK-05-01: Evaluate feasibility of removing the blanket `impl TaskIterator for M where M: Iterator` (`task.rs:311`); identify all call sites that depend on it
- [ ] TASK-05-02: If removal not feasible, add compile-time safeguard (e.g., sealed trait pattern or marker) and add `// WARNING:` doc comment explaining the recursion trap
- [ ] TASK-05-03: Fix `TransformIterator::next()` (`iterators.rs:194-202`) to loop until transformer returns `Some` or source returns `None` (filter_map semantics)
- [ ] TASK-05-04: Fix `TransformSendIterator::next()` (`iterators.rs:220-228`) with same filter_map fix
- [ ] TASK-05-05: Replace `self.sources.remove(idx)` with `self.sources.swap_remove(idx)` in `CollectAllStream` (`unified.rs:751`)
- [ ] TASK-05-06: Add tests: TransformIterator with closure that returns None for some items verifies remaining items are still yielded; CollectAllStream with many sources verifies correct behavior with swap_remove
