---
description: Address footguns, semantic bugs, architectural issues, and quality improvements in valtron module identified during deep-dive review
status: in_progress
priority: high
created: 2026-05-12
author: Main Agent
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: false
metadata:
  version: '2.2'
  last_updated: 2026-05-12
  estimated_effort: large
  tags:
  - valtron
  - quality
  - iterators
  - executors
  - correctness
  - architecture
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
has_features: true
has_fundamentals: false
tasks:
  completed: 18
  uncompleted: 21
  total: 48
  rejected: 9
  completion_percentage: 38
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

The review identified issues across two categories:

1. **Architectural issues** (features 00-04): Fundamental execution-model problems including
   CPU livelocks, stale state panics, lost signals, queue starvation, and polling inefficiency.

2. **Code quality issues** (features 05-08): Iterator semantics bugs, error handling gaps,
   API ergonomics, and naming/cleanup.

## Scope

- Module: `backends/foundation_core/src/valtron/`
- Sub-modules: core types, streams, executors (single, multi, unified, drivers)
- Related: `iterators.rs`, `task.rs`, `streams.rs`, `drain.rs`, `funcs.rs`, `branches.rs`
- Key files: `local.rs`, `threads.rs`, `drivers.rs`, `dependent_lift.rs`, `task_iters.rs`

---

## Features

| # | Feature | Priority | Status | Description | Tasks | Dependencies |
|---|---------|----------|--------|-------------|-------|--------------|
| 00 | [pending-none-livelock](./features/00-pending-none-livelock/) | CRITICAL | **rejected** | Fix unbounded CPU spin when tasks return Pending(None) | 9 | None |
| 01 | [sleeper-lifecycle-safety](./features/01-sleeper-lifecycle-safety/) | CRITICAL | **completed** | Fix stale sleeper entries that can panic executor on task combination | 6 | None |
| 02 | [linked-task-state-propagation](./features/02-linked-task-state-propagation/) | HIGH | **completed** | Fix DualSequence silently discarding parent State signals | 5 | 01 |
| 03 | [global-queue-fairness](./features/03-global-queue-fairness/) | HIGH | **completed** | Add fairness mechanism for global queue pickup | 5 | None |
| 04 | [notification-based-waiting](./features/04-notification-based-waiting/) | HIGH | pending | Replace spin+sleep polling with CondVar notification | 6 | None |
| 05 | [iterator-semantics](./features/05-iterator-semantics/) | HIGH | pending | Fix TaskIterator recursion trap, TransformIterator, CollectAllStream | 10 | None |
| 06 | [error-handling-api](./features/06-error-handling-api/) | MEDIUM | pending | Fix error swallowing, executor panics, Drain bounds | 5 | None |
| 07 | [channel-backpressure](./features/07-channel-backpressure/) | MEDIUM | pending | Add bounded queue option and EntryList slot reuse safety | 4 | 04 |
| 08 | [code-quality](./features/08-code-quality/) | LOW | pending | WASM variable fix, BranchPath naming, alias cleanup | 3 | None |

**Total Tasks:** 48 (18 completed, 21 remaining, 9 rejected)

**Note:** Feature 00 was rejected after design review - workers own tasks to completion by design, and the Pending(None) behavior is intentional. See feature file for details.

**Note:** Feature 01 is now complete. The `Sleepers` data structure was rewritten to use `HashMap<Entry, T>` keyed by task entry directly, eliminating entry ID collision bugs.

**Note:** Feature 03 is now complete. Added fairness mechanism that checks global queue every N calls to `schedule_next()`, preventing long-running local tasks from starving global queue. Tasks acquired via fairness are pushed to back of processing queue.

---

## Notes (Not Issues)

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

The review identified issues across two categories:

1. **Architectural issues** (features 00-04): Fundamental execution-model problems including
   CPU livelocks, stale state panics, lost signals, queue starvation, and polling inefficiency.

2. **Code quality issues** (features 05-08): Iterator semantics bugs, error handling gaps,
   API ergonomics, and naming/cleanup.

## Scope

- Module: `backends/foundation_core/src/valtron/`
- Sub-modules: core types, streams, executors (single, multi, unified, drivers)
- Related: `iterators.rs`, `task.rs`, `streams.rs`, `drain.rs`, `funcs.rs`, `branches.rs`
- Key files: `local.rs`, `threads.rs`, `drivers.rs`, `dependent_lift.rs`, `task_iters.rs`

---

## Features

| # | Feature | Priority | Status | Description | Tasks | Dependencies |
|---|---------|----------|--------|-------------|-------|--------------|
| 00 | [pending-none-livelock](./features/00-pending-none-livelock/) | CRITICAL | **rejected** | Fix unbounded CPU spin when tasks return Pending(None) | 9 | None |
| 01 | [sleeper-lifecycle-safety](./features/01-sleeper-lifecycle-safety/) | CRITICAL | **in_progress** | Fix stale sleeper entries that can panic executor on task combination | 6 | None |
| 02 | [linked-task-state-propagation](./features/02-linked-task-state-propagation/) | HIGH | pending | Fix DualSequence silently discarding parent State signals | 5 | 01 |
| 03 | [global-queue-fairness](./features/03-global-queue-fairness/) | HIGH | **completed** | Add fairness mechanism for global queue pickup | 5 | None |
| 04 | [notification-based-waiting](./features/04-notification-based-waiting/) | HIGH | pending | Replace spin+sleep polling with CondVar notification | 6 | None |
| 05 | [iterator-semantics](./features/05-iterator-semantics/) | HIGH | pending | Fix TaskIterator recursion trap, TransformIterator, CollectAllStream | 10 | None |
| 06 | [error-handling-api](./features/06-error-handling-api/) | MEDIUM | pending | Fix error swallowing, executor panics, Drain bounds | 5 | None |
| 07 | [channel-backpressure](./features/07-channel-backpressure/) | MEDIUM | pending | Add bounded queue option and EntryList slot reuse safety | 4 | 04 |
| 08 | [code-quality](./features/08-code-quality/) | LOW | pending | WASM variable fix, BranchPath naming, alias cleanup | 3 | None |

**Total Tasks:** 48 (0 completed, 39 remaining, 9 rejected)

**Note:** Feature 00 was rejected after design review - workers own tasks to completion by design, and the Pending(None) behavior is intentional. See feature file for details.

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

## Implementation Order

Recommended sequence based on dependencies and priority:

1. **Phase 1 (Critical):** Feature 01 — fix the panic from stale sleeper entries first (Feature 00 was rejected as intentional design)
2. **Phase 2 (High):** Features 02, 03, 04, 05 — architectural and semantic fixes
3. **Phase 3 (Medium/Low):** Features 06, 07, 08 — quality and cleanup

Features within the same phase can be implemented in parallel unless they share a dependency.

---

*Last updated: 2026-05-12*
