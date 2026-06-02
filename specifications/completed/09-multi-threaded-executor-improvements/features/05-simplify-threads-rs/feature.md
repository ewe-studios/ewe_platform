---
feature: "Simplify threads.rs"
description: "Remove ThreadPool struct and centralized management, keep reusable primitives"
status: "completed"
priority: "high"
depends_on: ["03-thread-registry", "04-thread-local-executor-and-api"]
estimated_effort: "medium"
created: 2026-03-23
completed: 2026-03-26
author: "Main Agent"
tasks:
  completed: 8
  uncompleted: 0
  total: 8
  completion_percentage: 100%
---

# Simplify threads.rs

## WHY: Problem Statement

After Features 03 and 04, `ThreadRegistry` replaces `ThreadPool` as the coordinator. The `ThreadPool` struct and its methods become dead code. `threads.rs` is currently 1424 lines — roughly half of that is `ThreadPool` and its methods which are no longer used by `multi/mod.rs`.

Additionally, several related pieces can be simplified:
- Parked/blocked thread list management (centralized tracking no longer needed)
- The `ThreadPool::schedule()` method (now a free function or part of `LocalPoolHandle`)
- The `ThreadPool::spawn()/spawn2()` methods (just `ThreadPoolTaskBuilder::new()` calls)

## WHAT: Solution

### What to Remove (~500 lines)

```
REMOVE: ThreadPool struct definition                    (518-550)    ~30 lines
REMOVE: ThreadPool::with_rng/with_seed/with_seed_and_threads (556-596) ~40 lines
REMOVE: ThreadPool::new() constructor                   (604-668)    ~65 lines
REMOVE: ThreadPool::create_thread_executor()            (701-826)   ~125 lines  (logic moved to ThreadRegistry)
REMOVE: ThreadPool::schedule()                          (831-849)    ~20 lines  (logic moved to free fn)
REMOVE: ThreadPool::spawn()/spawn2()                    (854-895)    ~40 lines  (callers use TaskBuilder directly)
REMOVE: ThreadPool::kill()                              (914-928)    ~15 lines  (replaced by PoolGuard)
REMOVE: ThreadPool::await_threads()                     (932-963)    ~30 lines  (moved to ThreadRegistry)
REMOVE: ThreadPool::run_until()/block_until()           (975-1002)   ~30 lines  (moved to ThreadRegistry)
REMOVE: ThreadPool parked/blocked list methods          (1004-1026)  ~20 lines
REMOVE: ThreadPool::listen_for_normal_activity()        (1028-1145) ~115 lines  (moved to ThreadRegistry)
                                                        TOTAL:      ~530 lines
```

### What Stays (~900 lines)

```
KEEP: Imports and constants                              (1-65)       ~65 lines
KEEP: get_allocatable_thread_count + tests               (66-197)    ~130 lines
KEEP: ThreadYielder + ProcessController impl             (199-276)    ~80 lines
KEEP: ThreadId                                           (278-305)    ~30 lines
KEEP: ThreadActivity enum + Display/Debug impls          (307-399)    ~90 lines
KEEP: SharedThreadRegistry + ThreadPoolRegistryInner     (402-514)   ~110 lines
KEEP: ThreadRef                                          (456-501)    ~45 lines
KEEP: ThreadExecutionError                               (673-693)    ~20 lines
KEEP: ThreadPoolTaskBuilder + all methods                (1148-1424) ~275 lines
ADD:  ThreadRegistry (from Feature 03)                               ~200 lines
ADD:  WaitGroup + PoolGuard (from Feature 02)                        ~80 lines
                                                         TOTAL:     ~1125 lines
```

Net result: **~1125 lines** (down from 1424), but more importantly the remaining code has clear separation:
- **Primitives**: ThreadYielder, ThreadId, ThreadActivity, ThreadRef (reusable building blocks)
- **Registry**: ThreadRegistry (coordinator, owns shared state)
- **Builder**: ThreadPoolTaskBuilder (task scheduling API, independent)
- **Lifecycle**: WaitGroup, PoolGuard (shutdown coordination)

### Migration Steps

1. Ensure `ThreadRegistry` passes all tests (Feature 03 complete)
2. Ensure `multi/mod.rs` uses `ThreadRegistry` not `ThreadPool` (Feature 04 complete)
3. Search for any remaining `ThreadPool` usages outside `multi/mod.rs`
4. If `ThreadPool` has external consumers: deprecate with `#[deprecated]` annotation
5. If no external consumers: delete the struct and all impl blocks
6. Clean up imports that only served `ThreadPool`
7. Verify the module's public exports are correct

### External Usage Check

Before removing, verify:
```bash
# Check if ThreadPool is used outside its own module
grep -r "ThreadPool" --include="*.rs" \
  --exclude-dir=target \
  backends/foundation_core/src/ \
  bin/
```

Known usages:
- `multi/mod.rs` — will be updated in Feature 04
- `unified.rs` — may reference `ThreadPool` in type signatures
- `gen_model_descriptors/mod.rs` — calls `initialize_pool` which returns `&'static ThreadPool`
- `valtron/mod.rs` — re-exports `ThreadPool`

All of these will be updated in Features 04 and 06.

---

## Tasks

- [ ] Verify no remaining `ThreadPool` usages after Features 03+04+06
- [ ] Remove `ThreadPool` struct definition and all `impl ThreadPool` blocks
- [ ] Remove `parked_threads` / `blocked_threads` centralized list methods
- [ ] Remove convenience constructors (`with_rng`, `with_seed`, etc.)
- [ ] Clean up now-unused imports
- [ ] Update module-level re-exports in `valtron/mod.rs` (replace `ThreadPool` with `ThreadRegistry`, `LocalPoolHandle`, `PoolGuard`)
- [ ] Add `#[deprecated]` to `ThreadPool` if it has external consumers that can't be updated in this spec
- [ ] Verify: `cargo build --package foundation_core` compiles clean

## Files Changed

- `backends/foundation_core/src/valtron/executors/threads.rs` (remove ~530 lines)
- `backends/foundation_core/src/valtron/mod.rs` (update re-exports)

## Verification

```bash
cargo build --package foundation_core
cargo test --package foundation_core
cargo clippy --package foundation_core -- -D warnings
```
