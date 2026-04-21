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
        sender.send(ThreadActivity::Stopped(id)).expect("should send event");
    }
}
```

If a worker panics, the `ThreadActivity::Panicked` event is sent, followed by
`ThreadActivity::Stopped`. The `WaitGroup` guard (`_wg`) is dropped in both cases,
ensuring shutdown never hangs.

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

The Cranelift codegen backend (`codegen-backend = "cranelift"` in `[profile.dev]`)
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
| `test_concurrent_queue_stream_iterator_panics_on_zero_max_turns` | `tests/stream_iterators.rs` | Rewritten to `#[should_panic]` |

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
