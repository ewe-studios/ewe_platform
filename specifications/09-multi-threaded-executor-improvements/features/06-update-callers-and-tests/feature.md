---
feature: "Update Callers and Tests"
description: "Update unified.rs, gen_model_descriptors, and all tests to use new API with PoolGuard"
status: "pending"
priority: "high"
depends_on: ["04-thread-local-executor-and-api"]
estimated_effort: "medium"
created: 2026-03-23
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# Update Callers and Tests

## WHY: Problem Statement

Features 02-04 change the public API:

| Old | New |
|-----|-----|
| `get_pool() -> &'static ThreadPool` | `get_pool() -> LocalPoolHandle` |
| `initialize_pool(seed, num) -> &'static ThreadPool` | `initialize_pool(seed, num) -> PoolGuard` |
| `block_on(seed, num, FnOnce(&ThreadPool))` | `block_on(seed, num, FnOnce(LocalPoolHandle))` |
| Kill via `get_pool().kill()` | Kill via `guard.shutdown()` or `handle.kill()` or drop guard |

All callers must be updated: `unified.rs`, `gen_model_descriptors/mod.rs`, test modules, and README docs.

## WHAT: Solution

### unified.rs Changes

The unified executor functions call `multi::spawn()` which still returns `ThreadPoolTaskBuilder` — the same type as before. These should work **unchanged**:

```rust
// execute_multi_as_task — calls multi::spawn() → ThreadPoolTaskBuilder
fn execute_multi_as_task<T>(task: T, wait_cycle: Option<Duration>) -> GenericResult<...> {
    let iter = multi::spawn()           // still returns ThreadPoolTaskBuilder
        .with_task(task)
        .schedule_iter(wait_cycle)?;    // same method as before
    Ok(drive_receiver(iter))
}
```

Verify all three: `execute_multi_as_task`, `execute_multi_stream`, `execute_multi`.

### gen_model_descriptors Changes

Currently (line ~1827):
```rust
valtron::initialize_pool(100, None);
// ... uses valtron::execute_collect_all(tasks, wait_cycle)
```

New:
```rust
let _guard = valtron::initialize_pool(100, None);
// ... uses valtron::execute_collect_all(tasks, wait_cycle)
// _guard dropped at scope end → cleanup
```

The key change is storing the `PoolGuard` so it lives for the duration of work.

### Test Pattern Changes

**Old pattern** — spawn a thread to call `get_pool().kill()`:
```rust
let handler_kill = thread::spawn(move || {
    thread::sleep(time::Duration::from_secs(5));
    get_pool().kill();  // or receiver.recv(); get_pool().kill();
});
block_on(seed, None, |pool| {
    pool.spawn().with_task(task).schedule().expect("ok");
});
handler_kill.join().expect("should finish");
```

**New pattern** — `block_on` handles lifecycle, or use `PoolGuard`:
```rust
// Pattern A: block_on manages everything
block_on(seed, None, |handle| {
    handle.spawn().with_task(counter)
        .with_resolver(Box::new(FnReady::new(|item, _| {
            tracing::info!("Received: {item:?}");
        })))
        .schedule()
        .expect("should schedule");
});
// block_on returns after all tasks complete

// Pattern B: explicit guard for tests needing kill control
let guard = initialize_pool(seed, None);
let handle = get_pool();
handle.spawn().with_task(task).schedule().unwrap();
// ... do stuff ...
guard.shutdown();  // or just drop guard
```

**For `can_finish_even_when_task_panics`** — the panicking task causes the thread to catch_unwind and report `ThreadActivity::Panicked`, then the registry respawns a replacement. The test can use a timeout-based approach:

```rust
let guard = initialize_pool(seed, None);
let handle = get_pool();
handle.spawn().with_task(PanicCounter).schedule().unwrap();
// Wait a bit for the panic to be handled, then shut down
thread::sleep(Duration::from_secs(2));
guard.shutdown();
```

Or better, use `block_on` with a signal:
```rust
block_on(seed, None, |handle| {
    handle.spawn().with_task(PanicCounter)
        .with_resolver(Box::new(FnReady::new(|item, _| {
            tracing::info!("Received: {item:?}");
        })))
        .schedule()
        .expect("should schedule");
});
```

### New Tests to Add

```rust
#[test]
fn pool_guard_drop_kills_threads() {
    {
        let _guard = initialize_pool(seed, Some(4));
        let handle = get_pool();
        handle.spawn().with_task(counter).schedule().unwrap();
        // guard drops here
    }
    // All threads should be dead after this point
}

#[test]
fn pool_guard_shutdown_is_idempotent() {
    let guard = initialize_pool(seed, Some(2));
    guard.shutdown();
    guard.shutdown();  // second call should be a no-op
}

#[test]
fn panicked_thread_does_not_hang_pool_guard() {
    let guard = initialize_pool(seed, Some(2));
    get_pool().spawn().with_task(PanicCounter).schedule().unwrap();
    guard.shutdown();  // should return, not hang
}

#[test]
fn dead_thread_gets_replaced() {
    let guard = initialize_pool(seed, Some(2));
    // Spawn a panicking task
    get_pool().spawn().with_task(PanicCounter).schedule().unwrap();
    thread::sleep(Duration::from_millis(500));
    // Registry should have respawned the dead thread
    // Spawn a normal task — should succeed
    let (counter, receiver) = Counter::new(5, shared_list.clone());
    get_pool().spawn().with_task(counter).schedule().unwrap();
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    guard.shutdown();
}
```

---

## Tasks

### Unified Executor
- [ ] Verify `execute_multi_as_task` compiles unchanged (uses `multi::spawn()`)
- [ ] Verify `execute_multi_stream` compiles unchanged
- [ ] Verify `execute_multi` (send-only) compiles unchanged
- [ ] Verify `execute_collect_all` works end-to-end

### External Callers
- [ ] Update `gen_model_descriptors/mod.rs` to store `PoolGuard`
- [ ] Search for any other `multi::initialize_pool` or `multi::get_pool` callers, update them

### Existing Test Migration
- [ ] Migrate `can_finish_even_when_task_panics` to new pattern
- [ ] Migrate `can_queue_and_complete_task` to new pattern
- [ ] Migrate `inline_execution` tests to new pattern (use `PoolGuard`)

### New Tests
- [ ] Test: `PoolGuard` drop kills all threads and returns cleanly
- [ ] Test: panicked thread doesn't hang `PoolGuard::shutdown()`
- [ ] Test: dead thread gets replaced, new tasks succeed

## Files Changed

- `backends/foundation_core/src/valtron/executors/unified.rs` (verify/minor updates)
- `backends/foundation_core/src/valtron/executors/multi/mod.rs` (test rewrites)
- `bin/platform/src/gen_model_descriptors/mod.rs` (store PoolGuard)

## Verification

```bash
cargo build --package foundation_core
cargo test --package foundation_core -- multi_threaded_tests
cargo test --package foundation_core -- unified
cargo test --package foundation_core
cargo clippy --package foundation_core -- -D warnings
```
