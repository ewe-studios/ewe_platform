---
description: Redesign multi-threaded executor to use per-thread thread_local! pools
  with a global thread handle registry, drop-based lifecycle, and OnSignal-based shutdown
status: in-progress
priority: high
created: 2026-03-23
author: Main Agent
context_optimization: true
compact_context_file: ./COMPACT_CONTEXT.md
context_reload_required: true
metadata:
  version: '3.0'
  last_updated: 2026-03-23
  estimated_effort: large
  tags:
  - rust
  - threading
  - executor
  - thread-local
  - architecture
  stack_files:
  - .agents/stacks/rust.md
  skills: []
  tools:
  - Rust
  - cargo
  - clippy
builds_on: []
related_specs: []
has_features: true
has_fundamentals: false
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0
---

# Multi-Threaded Executor — Thread-Local Pool Architecture

## Overview

Replace the global `OnceLock<ThreadPool>` singleton in `multi/mod.rs` with a **per-thread `thread_local!` pool** architecture. Each managed thread owns its own `LocalThreadExecutor`. A global **thread handle registry** (replacing `ThreadPool`) tracks liveness and coordinates shutdown via the existing `OnSignal` / `LockSignal` / `ThreadActivity` signal system. A **`PoolGuard` drop handle** provides deterministic cleanup — when it drops, it signals all threads to die via `OnSignal` and blocks until they're all cleaned up via a WaitGroup.

**See**: `analysis.md` for the full architectural analysis, original issue breakdown, and `threads.rs` component review.

## Target Files

**Primary**: `backends/foundation_core/src/valtron/executors/multi/mod.rs`
**Simplified**: `backends/foundation_core/src/valtron/executors/threads.rs` (~530 lines removed)
**Updated**: `backends/foundation_core/src/valtron/executors/unified.rs`

## Key Design Principles

1. **`get_pool()` works per-thread** — Returns a `LocalPoolHandle` for spawning tasks, backed by the shared queue
2. **Registry holds thread handles only** — `JoinHandle` + liveness metadata, not pools
3. **`OnSignal` broadcasts kill** — Uses existing `OnSignal::turn_on()` + `LockSignal::signal_all()` to wake and kill all threads
4. **`PoolGuard` drop handle** — `initialize_pool` returns a `PoolGuard`. On drop: signals kill, blocks via WaitGroup, joins handles. Usable in tests and `main()`
5. **No `ctrlc::set_handler`** — Removed from `initialize_pool`. Caller manages signal handling via `PoolGuard`
6. **`ThreadPoolTaskBuilder` reused unchanged** — Only needs `SharedTaskQueue` + `Arc<LockSignal>`, both from the registry
7. **`ThreadPool` removed from threads.rs** — Replaced by `ThreadRegistry`. Reusable primitives (`ThreadYielder`, `ThreadId`, `ThreadActivity`, `ThreadRef`, `ThreadPoolTaskBuilder`) kept intact

## Features

| # | Feature | Depends On | Effort | Description |
|---|---------|-----------|--------|-------------|
| 01 | [Fix Test Helper Locks](features/01-fix-test-helper-locks/feature.md) | — | Small | DCounter 3→1 locks, Counter release-before-send |
| 02 | [WaitGroup and PoolGuard](features/02-waitgroup-and-pool-guard/feature.md) | — | Medium | WaitGroup (uses LockSignal), PoolGuard drop handle |
| 03 | [ThreadRegistry](features/03-thread-registry/feature.md) | 02 | Large | Replaces ThreadPool as coordinator |
| 04 | [Thread-Local Executor & API](features/04-thread-local-executor-and-api/feature.md) | 02, 03 | Large | thread_local! storage, LocalPoolHandle, rewrite multi/mod.rs |
| 05 | [Simplify threads.rs](features/05-simplify-threads-rs/feature.md) | 03, 04 | Medium | Remove ThreadPool struct, ~530 lines of dead code |
| 06 | [Update Callers & Tests](features/06-update-callers-and-tests/feature.md) | 04 | Medium | unified.rs, gen_model_descriptors, test migration, new tests |

## Implementation Order

```
01-fix-test-helper-locks  (independent, do first)
        │
02-waitgroup-and-pool-guard  (independent of 01)
        │
03-thread-registry  (depends on 02)
        │
04-thread-local-executor-and-api  (depends on 02 + 03)
        │
   ┌────┴────┐
   │         │
05-simplify  06-update-callers
threads.rs   and-tests
```

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Caller (main / test)                                     │
│                                                           │
│  let guard = initialize_pool(seed, num_threads);          │
│  // guard: PoolGuard — drop kills + waits                 │
│                                                           │
│  block_on(seed, num_threads, |handle| {                   │
│      handle.spawn().with_task(t).schedule();              │
│  });                                                      │
│  // PoolGuard dropped → OnSignal::turn_on() → wait → join│
├──────────────────────────────────────────────────────────┤
│              ThreadRegistry (replaces ThreadPool)         │
│  shared_tasks: SharedTaskQueue                            │
│  latch: Arc<LockSignal>        (wake idle threads)        │
│  kill_signal: Arc<OnSignal>    (broadcast death)          │
│  kill_latch: Arc<LockSignal>   (shutdown sync)            │
│  waitgroup: WaitGroup          (completion barrier)       │
│  activity: mpp channel         (lifecycle events)         │
│  thread_handles: HashMap<ThreadId, JoinHandle>            │
├──────────────────────────────────────────────────────────┤
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐     │
│  │  Worker 1     │ │  Worker 2     │ │  Worker N     │    │
│  │ thread_local! │ │ thread_local! │ │ thread_local! │    │
│  │ LocalThread   │ │ LocalThread   │ │ LocalThread   │    │
│  │ Executor      │ │ Executor      │ │ Executor      │    │
│  │ ThreadYielder │ │ ThreadYielder │ │ ThreadYielder │    │
│  │ WaitGroupGuard│ │ WaitGroupGuard│ │ WaitGroupGuard│    │
│  └───────────────┘ └───────────────┘ └───────────────┘    │
│         pulls from SharedTaskQueue                        │
│         checks kill_signal (OnSignal)                     │
└──────────────────────────────────────────────────────────┘
```

## API Breaking Changes

| Old | New |
|-----|-----|
| `get_pool() -> &'static ThreadPool` | `get_pool() -> LocalPoolHandle` |
| `initialize_pool(seed, num) -> &'static ThreadPool` | `initialize_pool(seed, num) -> PoolGuard` |
| `block_on(seed, num, FnOnce(&ThreadPool))` | `block_on(seed, num, FnOnce(LocalPoolHandle))` |
| ctrlc handler registered automatically | Removed — caller manages signals via PoolGuard |
| Kill via `get_pool().kill()` | Kill via `guard.shutdown()` or `handle.kill()` or drop guard |

## Verification Commands

```bash
cargo build --package foundation_core
cargo clippy --package foundation_core -- -D warnings
cargo fmt -- --check
cargo test --package foundation_core -- multi_threaded_tests
cargo test --package foundation_core -- unified
cargo test --package foundation_core
```

## Agent Rules Reference

### Mandatory Rules
- `.agents/rules/01-rule-naming-and-structure.md`
- `.agents/rules/02-rules-directory-policy.md`
- `.agents/rules/03-dangerous-operations-safety.md`
- `.agents/rules/04-work-commit-and-push-rules.md`

### Role-Specific
- `.agents/rules/13-implementation-agent-guide.md`
- `.agents/stacks/rust.md`

## File Organization

1. `requirements.md` — This file
2. `analysis.md` — Architectural analysis, original issues, threads.rs component review
3. `start.md` — Agent workflow entry point
4. `features/` — Feature specifications (one directory per feature)
5. `LEARNINGS.md` — Learnings (created during work)
6. `PROGRESS.md` — Current status (delete at 100%)
7. `COMPACT_CONTEXT.md` — Compact context for agents

---

*Created: 2026-03-23*
*Last Updated: 2026-03-23*
*Status: In Progress*
