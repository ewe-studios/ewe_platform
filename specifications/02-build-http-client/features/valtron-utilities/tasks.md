---
feature: valtron-utilities
completed: 0
uncompleted: 24
last_updated: 2026-01-19
tools:
  - Rust
  - cargo
---

# Valtron Utilities - Tasks

## Task List

### Module Setup
- [ ] Create `valtron/executors/actions.rs` - Module for reusable action types
- [ ] Create `valtron/executors/unified.rs` - Module for unified executor
- [ ] Create `valtron/executors/state_machine.rs` - Module for state machine helpers
- [ ] Create `valtron/executors/future_task.rs` - Module for Future-to-TaskIterator adapter
- [ ] Create `valtron/executors/wrappers.rs` - Module for retry/timeout wrappers
- [ ] Update `valtron/executors/mod.rs` - Add new module exports with conditional compilation
- [ ] Add `#![cfg_attr(not(feature = "std"), no_std)]` to crate root

### Reusable Action Types
- [ ] Implement `LiftAction<T>` - Priority task scheduling
- [ ] Implement `ScheduleAction<T>` - Normal task scheduling
- [ ] Implement `BroadcastAction<T>` - Cross-thread task scheduling
- [ ] Implement `CompositeAction<L, S, B, C>` - Combined action enum

### Unified Executor
- [ ] Implement `execute<T>()` - Feature-gated executor selection
- [ ] Add WASM support (always single-threaded)
- [ ] Add multi-threaded feature gate (requires std)

### State Machine Helpers
- [ ] Define `StateTransition<S, O, E, A>` enum
- [ ] Define `StateMachine` trait with associated types
- [ ] Implement `StateMachineTask<M>` wrapper

### Future-to-TaskIterator Adapter
- [ ] Define `FuturePollState` enum for pending state
- [ ] Implement `create_noop_waker()` using only `core` types
- [ ] Implement `get_noop_waker()` with thread-local cache (std) or direct creation (no_std)
- [ ] Implement `FutureTask<F>` struct with Pin<Box<F>> (requires std or alloc)
- [ ] Implement `FutureTaskRef<'a, F>` for pure no_std (stack-pinned)
- [ ] Implement `TaskIterator` for `FutureTask<F>` (native with Send bounds)
- [ ] Implement `TaskIterator` for `FutureTask<F>` (WASM with relaxed bounds)
- [ ] Implement `TaskIterator` for `FutureTaskRef<'a, F>` (no_std)
- [ ] Implement `from_future<F>()` with platform-aware bounds
- [ ] Implement `from_future_ref<F>()` for no_std
- [ ] Implement `run_future<F>()` with platform-aware bounds
- [ ] (Optional) Define `valtron_async!` macro for inline async
- [ ] Define `StreamPollState` enum
- [ ] Implement `StreamTask<S>` for async streams (requires std or alloc)
- [ ] Implement `TaskIterator` for `StreamTask<S>` (with platform-aware bounds)
- [ ] Implement `from_stream<S>()` convenience function

### Cargo.toml Feature Configuration
- [ ] Add `futures-core = { version = "0.3", default-features = false }`
- [ ] Add `std` feature with `futures-core/std` and `alloc`
- [ ] Add `alloc` feature with `futures-core/alloc`
- [ ] Add `multi` feature requiring `std`
- [ ] Ensure `default = ["std"]`

### Retry/Timeout Wrappers
- [ ] Define `RetryDecider<T>` trait
- [ ] Implement `RetryingTask<T, D>` wrapper
- [ ] Implement `TimeoutTask<T>` wrapper (std only, requires Instant)
- [ ] Implement `PollLimitTask<T>` for no_std (poll-count based limit)
- [ ] Implement `BackoffStrategy` enum (Fixed, Exponential, Linear)
- [ ] Implement `BackoffTask<T>` wrapper

## Implementation Order

1. **Cargo.toml** - Feature configuration (std, alloc, multi, futures-core)
2. **Crate root** - Add no_std attribute
3. **Module setup** - Create files and update mod.rs with conditional compilation
4. **actions.rs** - Reusable action types (LiftAction, ScheduleAction, BroadcastAction, CompositeAction)
5. **unified.rs** - Feature-gated executor (depends on existing single/multi)
6. **state_machine.rs** - State machine helpers (StateTransition, StateMachine trait, StateMachineTask)
7. **future_task.rs** - Future adapter core (noop waker using core types)
8. **future_task.rs** - FutureTask (std/alloc) and FutureTaskRef (pure no_std)
9. **future_task.rs** - Stream adapter (StreamTask) - requires alloc
10. **wrappers.rs** - TimeoutTask (std only) and PollLimitTask (all platforms)
11. **wrappers.rs** - Retry wrappers (RetryingTask, BackoffTask)

## Notes

### Action Type Pattern
```rust
pub struct LiftAction<T: TaskIterator + Send + 'static> {
    task: T,
}

impl<T: TaskIterator + Send + 'static> ExecutionAction for LiftAction<T> {
    fn apply(self, parent_key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        engine.lift(Box::new(DoNext::new(self.task)), Some(parent_key))?;
        Ok(())
    }
}
```

### Unified Executor Pattern
```rust
pub fn execute<T>(task: T) -> Result<T::Ready, ExecutorError>
where
    T: TaskIterator + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    { execute_single(task) }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        { execute_multi(task) }

        #[cfg(not(feature = "multi"))]
        { execute_single(task) }
    }
}
```

### StateMachine Trait Pattern
```rust
pub trait StateMachine {
    type State;
    type Output;
    type Error;
    type Action: ExecutionAction;

    fn transition(&mut self, state: Self::State) -> StateTransition<...>;
    fn initial_state(&self) -> Self::State;
}
```

### Re-exports in mod.rs
```rust
mod actions;
mod unified;
mod state_machine;
mod future_task;
mod wrappers;

pub use actions::{LiftAction, ScheduleAction, BroadcastAction, CompositeAction};
pub use unified::execute;
pub use state_machine::{StateTransition, StateMachine, StateMachineTask};
pub use future_task::{FutureTask, FuturePollState, from_future, run_future};
pub use future_task::{StreamTask, StreamPollState, from_stream};
pub use wrappers::{RetryingTask, TimeoutTask, BackoffTask, BackoffStrategy, RetryDecider};
```

### FutureTask Pattern
```rust
pub struct FutureTask<F: Future> {
    future: Pin<Box<F>>,
    completed: bool,
}

impl<F: Future + Send + 'static> TaskIterator for FutureTask<F>
where F::Output: Send + 'static {
    type Ready = F::Output;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<...>> {
        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);
        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => Some(TaskStatus::Ready(output)),
            Poll::Pending => Some(TaskStatus::Pending(FuturePollState::Pending)),
        }
    }
}
```

### No-Op Waker Pattern
```rust
use core::task::{Waker, RawWaker, RawWakerVTable};

fn create_noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RAW_WAKER,  // clone
        |_| {},          // wake
        |_| {},          // wake_by_ref
        |_| {},          // drop
    );
    const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(RAW_WAKER) }
}

#[cfg(feature = "std")]
fn get_noop_waker() -> Waker {
    thread_local! { static W: Waker = create_noop_waker(); }
    W.with(|w| w.clone())
}

#[cfg(not(feature = "std"))]
fn get_noop_waker() -> Waker { create_noop_waker() }
```

### Feature-Conditional Imports Pattern
```rust
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;

use core::future::Future;
use core::pin::Pin;
```

### Cargo.toml Feature Pattern
```toml
[dependencies]
futures-core = { version = "0.3", default-features = false }

[features]
default = ["std"]
std = ["futures-core/std", "alloc"]
alloc = ["futures-core/alloc"]
multi = ["std"]
```

---
*Last Updated: 2026-01-19*
