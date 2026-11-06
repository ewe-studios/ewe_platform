# Panic Unwinding in Valtron

## Overview

Valtron uses `std::panic::catch_unwind` extensively to isolate panicking tasks from
killing executor threads. This document explains the design, its limitations under
the Cranelift codegen backend, and how tests handle the incompatibility.

## How Panic Protection Works

### Executor Iterators (task_iters, collect_next, on_next, do_next)

Every `ExecutionIterator::next()` implementation wraps the task's `next_status()` call:

```rust
match std::panic::catch_unwind(|| self.task.lock().unwrap().next_status()) {
    Ok(inner) => inner,
    Err(panic_error) => {
        if let Some(panic_handler) = &self.panic_handler {
            (panic_handler)(panic_error);
        }
        return Some(State::Panicked);
    }
}
```

When a task panics:
1. `catch_unwind` catches the panic payload
2. Optional `panic_handler` callback is invoked (for logging/metrics)
3. `State::Panicked` is returned to the `LocalThreadExecutor`
4. The executor removes the task from its queue (unparks and takes)
5. Remaining tasks continue executing normally

### Spawn builders and inline actions

`TaskSpawnConfig::with_panic_handler` installs its handler on every terminal.
`spawn` and `broadcast` set it on `DoNext` or `OnNext`. `recv` and `stream` set
it on `ConsumingIter` or `StreamConsumingIter`. `action()` carries it into the
`InlineAction`, and `apply` sets it on the child iterator. Before issue #244,
the config stored the handler and no terminal read it.

### BackgroundJobRegistry (background.rs)

Worker threads wrap each job in `catch_unwind`:

```rust
let result = std::panic::catch_unwind(AssertUnwindSafe(job));
if let Err(panic_info) = result {
    tracing::error!("Background job panicked on worker {worker_id}: {panic_info:?}");
}
```

Workers survive panicking jobs and continue processing the queue.

### ThreadRegistry (threads.rs)

Each worker thread's entire execution is wrapped:

```rust
match panic::catch_unwind(|| { /* worker execution */ }) {
    Ok(()) => { /* clean exit */ }
    Err(err) => {
        sender.send(ThreadActivity::Panicked(id, err)).expect("should send event");
    }
}
```

If a worker panics, exactly ONE exit event, `ThreadActivity::Panicked`, is sent.
Its listener arm does the same accounting as the clean-exit `Stopped` event
(decrement `live_threads`, unregister the thread, drop its handle) and then
respawns the worker. Sending both events made the listener decrement twice, so
`live_threads` drifted down by one per panic. The `WaitGroup` guard (`_wg`) is
dropped on both the clean and the panic path, ensuring shutdown never hangs.

The respawn runs only when something reads the activity queue.
`initialize_pool_with_config_and_ctx` starts one monitor thread
(`ThreadRegistry::start_activity_monitor`) for the life of the pool, and shutdown
stops and joins it before the workers stop. Without the monitor nothing read the
queue during normal operation, so a worker-level panic lost a worker for good and
the `idle_threads` counter stayed frozen (issue #243).

### Single-threaded executor (single/mod.rs)

`run_background_job` uses `catch_unwind` for API consistency:

```rust
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
```

Returns `Err` instead of aborting the single-threaded event loop.

## Mutex Poisoning

When `next_status()` panics while holding the Mutex, the Mutex becomes poisoned.
This is safe because `State::Panicked` causes immediate task eviction — the poisoned
Mutex is never accessed again.

## Cranelift Incompatibility

The Cranelift codegen backend (`dev` builds under mise: `CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift` in its `[env]`)
does not fully support panic unwinding on all platforms. Under Cranelift:

- `catch_unwind` does NOT catch panics
- Any panic triggers "fatal runtime error: failed to initiate panic, error 5" and aborts
- `#[should_panic]` tests still work (test harness handles them specially)
- All panic isolation in executor code becomes inert (panics kill the thread/process)

### Impact on Production

Release builds use the LLVM backend (`[profile.release]`) where unwinding works
correctly. The Cranelift limitation only affects development builds.

### Impact on Development

During `cargo test` with Cranelift active:
- A panicking task in `BackgroundJobRegistry` kills the worker thread (not just the job)
- A panicking task in the executor kills the executor thread
- Tests that use explicit `catch_unwind` abort the test process

### How Tests Handle This

A `build.rs` in `foundation_core` detects Cranelift in the workspace `Cargo.toml`
and sets `cfg(cranelift_backend)`. Tests that rely on unwinding are annotated:

```rust
#[cfg_attr(cranelift_backend, ignore = "cranelift does not support panic unwinding")]
```

Tests that verify panic behavior can often use `#[should_panic]` instead of
`catch_unwind`, since `#[should_panic]` works under Cranelift.

### Affected Tests

| Test | File | Fix |
|------|------|-----|
| `test_panic_recovery` | `executors/background.rs` | `cfg_attr` ignore |
| `test_waitgroup_guard_calls_done_during_panic` | `executors/threads.rs` | `cfg_attr` ignore |

## Design Decisions

1. **Why `AssertUnwindSafe`?** — Task closures cross an unwind boundary. We assert
   unwind safety because the executor handles the aftermath (task eviction) rather
   than requiring tasks to be `UnwindSafe`.

2. **Why not remove Cranelift?** — Cranelift provides significantly faster codegen
   during development. The tradeoff (no panic isolation in dev) is acceptable because
   panicking tasks indicate bugs that should be fixed regardless.

3. **Why per-task catch rather than per-thread?** — Per-task catch allows the executor
   to continue running other tasks. Per-thread catch (as in `ThreadRegistry`) is a
   last resort that reports the failure but loses all in-progress work on that thread.
