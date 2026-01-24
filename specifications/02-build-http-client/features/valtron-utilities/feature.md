---
feature: valtron-utilities
description: Reusable ExecutionAction types, unified executor wrapper, state machine helpers, Future adapter, and retry/timeout wrappers
status: pending
priority: high
depends_on: []
estimated_effort: medium
created: 2026-01-19
last_updated: 2026-01-24
author: Main Agent
tasks:
  completed: 0
  uncompleted: 30
  total: 30
  completion_percentage: 0
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
---

# Valtron Utilities Feature

## Overview

Create reusable valtron patterns that will be used by the task-iterator feature and can be reused across the entire codebase. This feature adds generic utilities to the valtron module (not simple_http), providing foundational building blocks for async-like execution patterns.

## Dependencies

This feature depends on:
- None (foundational feature)

This feature is required by:
- `task-iterator` - Uses state machine helpers and reusable action types
- Future valtron-based features across the codebase

## Implementation Location

**IMPORTANT**: These utilities go into the valtron module, not simple_http:

```
backends/foundation_core/src/valtron/executors/
├── actions.rs       (NEW - Reusable ExecutionAction types)
├── unified.rs       (NEW - Feature-gated unified executor)
├── state_machine.rs (NEW - State machine helpers)
├── future_task.rs   (NEW - Future-to-TaskIterator adapter)
└── wrappers.rs      (NEW - Retry/timeout wrappers)
```

## Requirements

### A. Reusable ExecutionAction Types

Generic action types that can be reused across different TaskIterator implementations:

#### LiftAction

Lifts a task to the top of the local queue (priority scheduling):

```rust
pub struct LiftAction<T: TaskIterator + Send + 'static> {
    task: T,
}

impl<T: TaskIterator + Send + 'static> LiftAction<T> {
    pub fn new(task: T) -> Self {
        Self { task }
    }
}

impl<T: TaskIterator + Send + 'static> ExecutionAction for LiftAction<T> {
    fn apply(self, parent_key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        engine.lift(Box::new(DoNext::new(self.task)), Some(parent_key))?;
        Ok(())
    }
}
```

#### ScheduleAction

Schedules a task to the bottom of the local queue (normal scheduling):

```rust
pub struct ScheduleAction<T: TaskIterator + Send + 'static> {
    task: T,
}

impl<T: TaskIterator + Send + 'static> ScheduleAction<T> {
    pub fn new(task: T) -> Self {
        Self { task }
    }
}

impl<T: TaskIterator + Send + 'static> ExecutionAction for ScheduleAction<T> {
    fn apply(self, _parent_key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        engine.schedule(Box::new(DoNext::new(self.task)))?;
        Ok(())
    }
}
```

#### BroadcastAction

Broadcasts a task to the global queue (cross-thread scheduling):

```rust
pub struct BroadcastAction<T: TaskIterator + Send + 'static> {
    task: T,
}

impl<T: TaskIterator + Send + 'static> BroadcastAction<T> {
    pub fn new(task: T) -> Self {
        Self { task }
    }
}

impl<T: TaskIterator + Send + 'static> ExecutionAction for BroadcastAction<T> {
    fn apply(self, _parent_key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        engine.broadcast(Box::new(DoNext::new(self.task)))?;
        Ok(())
    }
}
```

#### CompositeAction

Enum combining all action types plus a custom action slot:

```rust
pub enum CompositeAction<L, S, B, C>
where
    L: TaskIterator + Send + 'static,
    S: TaskIterator + Send + 'static,
    B: TaskIterator + Send + 'static,
    C: ExecutionAction,
{
    None,
    Lift(LiftAction<L>),
    Schedule(ScheduleAction<S>),
    Broadcast(BroadcastAction<B>),
    Custom(C),
}

impl<L, S, B, C> ExecutionAction for CompositeAction<L, S, B, C>
where
    L: TaskIterator + Send + 'static,
    S: TaskIterator + Send + 'static,
    B: TaskIterator + Send + 'static,
    C: ExecutionAction,
{
    fn apply(self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        match self {
            Self::None => Ok(()),
            Self::Lift(action) => action.apply(key, engine),
            Self::Schedule(action) => action.apply(key, engine),
            Self::Broadcast(action) => action.apply(key, engine),
            Self::Custom(action) => action.apply(key, engine),
        }
    }
}
```

### B. Feature-Gated Unified Executor

Single entry point for executing tasks that auto-selects executor based on platform and features:

```rust
/// Execute a task using the appropriate executor for the current platform/features.
///
/// | Platform | Feature | Executor Used |
/// |----------|---------|---------------|
/// | WASM     | any     | `single`      |
/// | Native   | none    | `single`      |
/// | Native   | `multi` | `multi`       |
pub fn execute<T>(task: T) -> Result<T::Ready, ExecutorError>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        execute_single(task)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        {
            execute_multi(task)
        }

        #[cfg(not(feature = "multi"))]
        {
            execute_single(task)
        }
    }
}

fn execute_single<T>(task: T) -> Result<T::Ready, ExecutorError>
where
    T: TaskIterator + Send + 'static,
{
    single::spawn(task)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(task: T) -> Result<T::Ready, ExecutorError>
where
    T: TaskIterator + Send + 'static,
{
    multi::spawn(task)
}
```

### C. State Machine Helpers

Trait and utilities for building state machine-based TaskIterators:

#### StateTransition Enum

```rust
/// Result of a state machine transition
pub enum StateTransition<S, O, E, A = NoAction>
where
    A: ExecutionAction,
{
    /// Continue processing in new state (non-blocking)
    Continue(S),

    /// Yield a value and transition to new state
    Yield(O, S),

    /// Task complete with final value
    Complete(O),

    /// Task failed with error
    Error(E),

    /// Delay before continuing (for retries, backoff)
    Delay(Duration, S),

    /// Spawn a child task and continue
    Spawn(A, S),
}
```

#### StateMachine Trait

```rust
/// Trait for implementing state machine logic
pub trait StateMachine {
    /// The state type for this machine
    type State;

    /// The output type produced by this machine
    type Output;

    /// The error type for failures
    type Error;

    /// The action type for spawning (use NoAction if not spawning)
    type Action: ExecutionAction;

    /// Perform one transition from the current state
    fn transition(
        &mut self,
        state: Self::State,
    ) -> StateTransition<Self::State, Self::Output, Self::Error, Self::Action>;

    /// Get the initial state
    fn initial_state(&self) -> Self::State;
}
```

#### StateMachineTask Wrapper

```rust
/// Wrapper that implements TaskIterator for any StateMachine
pub struct StateMachineTask<M: StateMachine> {
    machine: M,
    current_state: Option<M::State>,
}

impl<M: StateMachine> StateMachineTask<M> {
    pub fn new(machine: M) -> Self {
        let initial = machine.initial_state();
        Self {
            machine,
            current_state: Some(initial),
        }
    }
}

impl<M> TaskIterator for StateMachineTask<M>
where
    M: StateMachine + Send + 'static,
    M::State: Send,
    M::Output: Send,
    M::Error: Into<BoxedError>,
    M::Action: Send + 'static,
{
    type Ready = M::Output;
    type Pending = M::State;
    type Spawner = M::Action;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let state = self.current_state.take()?;

        match self.machine.transition(state) {
            StateTransition::Continue(next) => {
                self.current_state = Some(next.clone());
                Some(TaskStatus::Pending(next))
            }
            StateTransition::Yield(output, next) => {
                self.current_state = Some(next);
                Some(TaskStatus::Ready(output))
            }
            StateTransition::Complete(output) => {
                Some(TaskStatus::Ready(output))
            }
            StateTransition::Error(err) => {
                Some(TaskStatus::Error(err.into()))
            }
            StateTransition::Delay(duration, next) => {
                self.current_state = Some(next.clone());
                Some(TaskStatus::Delayed(duration, next))
            }
            StateTransition::Spawn(action, next) => {
                self.current_state = Some(next.clone());
                Some(TaskStatus::Spawn(action, next))
            }
        }
    }
}
```

### D. Retry/Timeout Wrappers

Composable wrappers for adding retry logic, timeouts, and backoff to any TaskIterator:

#### RetryingTask

```rust
pub struct RetryingTask<T, D>
where
    T: TaskIterator,
    D: RetryDecider<T::Ready>,
{
    inner: T,
    decider: D,
    max_retries: u32,
    current_attempt: u32,
}

pub trait RetryDecider<T> {
    /// Returns true if the task should be retried based on the result
    fn should_retry(&self, result: &T, attempt: u32) -> bool;

    /// Create a fresh task for retry
    fn create_retry_task(&self) -> Option<Box<dyn TaskIterator<Ready = T>>>;
}

impl<T, D> RetryingTask<T, D>
where
    T: TaskIterator,
    D: RetryDecider<T::Ready>,
{
    pub fn new(inner: T, decider: D, max_retries: u32) -> Self {
        Self {
            inner,
            decider,
            max_retries,
            current_attempt: 0,
        }
    }
}
```

#### TimeoutTask

```rust
pub struct TimeoutTask<T>
where
    T: TaskIterator,
{
    inner: T,
    timeout: Duration,
    started_at: Option<Instant>,
}

impl<T: TaskIterator> TimeoutTask<T> {
    pub fn new(inner: T, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            started_at: None,
        }
    }
}

impl<T> TaskIterator for TimeoutTask<T>
where
    T: TaskIterator + Send + 'static,
{
    type Ready = T::Ready;
    type Pending = TimeoutState<T::Pending>;
    type Spawner = T::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }

        if self.started_at.unwrap().elapsed() > self.timeout {
            return Some(TaskStatus::Error("Task timed out".into()));
        }

        // Delegate to inner task
        self.inner.next().map(|status| match status {
            TaskStatus::Pending(p) => TaskStatus::Pending(TimeoutState::Inner(p)),
            TaskStatus::Ready(r) => TaskStatus::Ready(r),
            TaskStatus::Delayed(d, p) => TaskStatus::Delayed(d, TimeoutState::Inner(p)),
            TaskStatus::Spawn(a, p) => TaskStatus::Spawn(a, TimeoutState::Inner(p)),
            TaskStatus::Error(e) => TaskStatus::Error(e),
        })
    }
}

pub enum TimeoutState<P> {
    Inner(P),
    TimedOut,
}
```

#### BackoffTask

```rust
pub struct BackoffTask<T>
where
    T: TaskIterator,
{
    inner: T,
    strategy: BackoffStrategy,
    current_delay: Duration,
    max_delay: Duration,
}

pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed(Duration),

    /// Exponential backoff: delay * 2^attempt
    Exponential { base: Duration, multiplier: f64 },

    /// Linear backoff: base + (increment * attempt)
    Linear { base: Duration, increment: Duration },
}

impl BackoffStrategy {
    pub fn next_delay(&self, attempt: u32, current: Duration, max: Duration) -> Duration {
        let next = match self {
            Self::Fixed(d) => *d,
            Self::Exponential { base, multiplier } => {
                Duration::from_secs_f64(base.as_secs_f64() * multiplier.powi(attempt as i32))
            }
            Self::Linear { base, increment } => {
                *base + (*increment * attempt)
            }
        };
        next.min(max)
    }
}
```

### E. Future-to-TaskIterator Adapter

Wrap any Rust `Future` and poll it through the TaskIterator pattern, enabling seamless integration of async code with the valtron executor system.

#### FutureTask Wrapper

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};

/// Wraps a Future and implements TaskIterator to poll it
///
/// This adapter allows any async code to be executed through valtron's
/// executor system without requiring a full async runtime.
///
/// # Example
/// ```rust
/// let future = async {
///     // async operations
///     42
/// };
///
/// let task = FutureTask::new(future);
/// let result = unified::execute(task)?; // Returns 42
/// ```
pub struct FutureTask<F>
where
    F: Future,
{
    /// The wrapped future, pinned for polling
    future: Pin<Box<F>>,
    /// Tracks if the future has completed
    completed: bool,
}

impl<F> FutureTask<F>
where
    F: Future,
{
    /// Create a new FutureTask wrapping the given future
    pub fn new(future: F) -> Self {
        Self {
            future: Box::pin(future),
            completed: false,
        }
    }

    /// Create from an already-pinned future
    pub fn from_pinned(future: Pin<Box<F>>) -> Self {
        Self {
            future,
            completed: false,
        }
    }
}
```

#### Polling State

```rust
/// State reported while future is being polled
#[derive(Debug, Clone)]
pub enum FuturePollState {
    /// Future returned Poll::Pending
    Pending,
    /// Future is being polled
    Polling,
}
```

#### No-Op Waker

The TaskIterator executor drives polling, so we need a minimal waker that does nothing (the executor will re-poll on next iteration):

```rust
/// Creates a no-op waker for use with Future polling
///
/// Since the valtron executor drives the polling loop, we don't need
/// the waker to actually wake anything - the executor will continue
/// polling on the next iteration.
fn create_noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RAW_WAKER,           // clone
        |_| {},                   // wake
        |_| {},                   // wake_by_ref
        |_| {},                   // drop
    );
    const RAW_WAKER: RawWaker = RawWaker::new(std::ptr::null(), &VTABLE);

    // SAFETY: The vtable functions are all no-ops and handle null pointers
    unsafe { Waker::from_raw(RAW_WAKER) }
}

/// Alternative: Thread-local cached waker for efficiency
thread_local! {
    static NOOP_WAKER: Waker = create_noop_waker();
}

fn get_noop_waker() -> Waker {
    NOOP_WAKER.with(|w| w.clone())
}
```

#### TaskIterator Implementation

```rust
impl<F> TaskIterator for FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    type Ready = F::Output;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.completed {
            return None;
        }

        // Create context with no-op waker
        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        // Poll the future
        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => {
                self.completed = true;
                Some(TaskStatus::Ready(output))
            }
            Poll::Pending => {
                Some(TaskStatus::Pending(FuturePollState::Pending))
            }
        }
    }
}
```

#### Convenience Functions

```rust
/// Wrap a future into a TaskIterator
pub fn from_future<F>(future: F) -> FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    FutureTask::new(future)
}

/// Execute a future using the unified executor
///
/// This is the simplest way to run async code through valtron:
/// ```rust
/// let result = run_future(async {
///     some_async_operation().await
/// })?;
/// ```
pub fn run_future<F>(future: F) -> Result<F::Output, ExecutorError>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let task = FutureTask::new(future);
    execute(task)
}
```

#### Async Block Helper

```rust
/// Macro for inline async execution (optional convenience)
///
/// ```rust
/// let result = valtron_async! {
///     let data = fetch_data().await;
///     process(data).await
/// };
/// ```
#[macro_export]
macro_rules! valtron_async {
    ($($body:tt)*) => {
        $crate::valtron::executors::run_future(async { $($body)* })
    };
}
```

#### Stream Adapter (For Async Streams)

For async streams that yield multiple values:

```rust
use futures_core::Stream;

/// Wraps an async Stream and yields values through TaskIterator
pub struct StreamTask<S>
where
    S: Stream,
{
    stream: Pin<Box<S>>,
    exhausted: bool,
}

impl<S> StreamTask<S>
where
    S: Stream,
{
    pub fn new(stream: S) -> Self {
        Self {
            stream: Box::pin(stream),
            exhausted: false,
        }
    }
}

/// State for stream polling
#[derive(Debug, Clone)]
pub enum StreamPollState {
    /// Stream returned Pending
    Pending,
    /// Polling stream
    Polling,
}

impl<S> TaskIterator for StreamTask<S>
where
    S: Stream + Send + 'static,
    S::Item: Send + 'static,
{
    type Ready = Option<S::Item>;
    type Pending = StreamPollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.exhausted {
            return None;
        }

        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        match self.stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(item)) => {
                Some(TaskStatus::Ready(Some(item)))
            }
            Poll::Ready(None) => {
                self.exhausted = true;
                Some(TaskStatus::Ready(None))
            }
            Poll::Pending => {
                Some(TaskStatus::Pending(StreamPollState::Pending))
            }
        }
    }
}

/// Wrap an async stream into a TaskIterator
pub fn from_stream<S>(stream: S) -> StreamTask<S>
where
    S: Stream + Send + 'static,
    S::Item: Send + 'static,
{
    StreamTask::new(stream)
}
```

#### Feature Gate for futures-core

```toml
[dependencies]
# futures-core with no_std support
futures-core = { version = "0.3", default-features = false }

[features]
default = ["std"]
std = ["futures-core/std"]
alloc = ["futures-core/alloc"]  # For no_std with alloc
```

**Feature Configurations:**

| Feature | Environment | Notes |
|---------|-------------|-------|
| `std` (default) | Standard library | Full functionality |
| `alloc` | no_std + alloc | Heap allocation available |
| (none) | Pure no_std | Limited functionality, no Box/Pin heap allocation |

**WASM Compatibility**: `futures-core` is a minimal, no-std compatible crate that works on all platforms including WASM. It only provides the `Future` and `Stream` traits without any runtime dependencies.

#### no_std Support

The Future adapter supports no_std environments:

```rust
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

#[cfg(feature = "std")]
use std::boxed::Box;

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};
```

#### FutureTask with no_std

```rust
/// FutureTask requires either `std` or `alloc` feature for Box<T>
#[cfg(any(feature = "std", feature = "alloc"))]
pub struct FutureTask<F>
where
    F: Future,
{
    future: Pin<Box<F>>,
    completed: bool,
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<F> FutureTask<F>
where
    F: Future,
{
    pub fn new(future: F) -> Self {
        Self {
            future: Box::pin(future),
            completed: false,
        }
    }
}

/// For pure no_std without alloc, provide a stack-pinned variant
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub struct FutureTaskRef<'a, F>
where
    F: Future,
{
    future: Pin<&'a mut F>,
    completed: bool,
}

#[cfg(not(any(feature = "std", feature = "alloc")))]
impl<'a, F> FutureTaskRef<'a, F>
where
    F: Future,
{
    /// Create from a pinned reference (user must pin the future)
    pub fn new(future: Pin<&'a mut F>) -> Self {
        Self {
            future,
            completed: false,
        }
    }
}
```

#### No-Op Waker (no_std compatible)

The no-op waker uses only `core` types:

```rust
use core::task::{Waker, RawWaker, RawWakerVTable};

/// Creates a no-op waker for use with Future polling
///
/// Uses only core types - works in no_std
fn create_noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RAW_WAKER,           // clone
        |_| {},                   // wake
        |_| {},                   // wake_by_ref
        |_| {},                   // drop
    );
    const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);

    // SAFETY: The vtable functions are all no-ops and handle null pointers
    unsafe { Waker::from_raw(RAW_WAKER) }
}

/// Thread-local cached waker (only available with std)
#[cfg(feature = "std")]
thread_local! {
    static NOOP_WAKER: Waker = create_noop_waker();
}

#[cfg(feature = "std")]
fn get_noop_waker() -> Waker {
    NOOP_WAKER.with(|w| w.clone())
}

/// For no_std, create waker on each call (no thread-locals)
#[cfg(not(feature = "std"))]
fn get_noop_waker() -> Waker {
    create_noop_waker()
}
```

#### Instant/Duration for Timeouts

TimeoutTask needs platform-aware time handling:

```rust
// std: Use std::time::Instant
#[cfg(feature = "std")]
use std::time::{Duration, Instant};

// no_std: User must provide time source or disable timeout features
#[cfg(not(feature = "std"))]
pub use core::time::Duration;

// TimeoutTask only available with std (requires Instant)
#[cfg(feature = "std")]
pub struct TimeoutTask<T> { ... }

// For no_std, provide a poll-count based "timeout"
#[cfg(not(feature = "std"))]
pub struct PollLimitTask<T>
where
    T: TaskIterator,
{
    inner: T,
    max_polls: usize,
    current_polls: usize,
}
```

#### StreamTask with no_std

```rust
#[cfg(any(feature = "std", feature = "alloc"))]
use futures_core::Stream;

#[cfg(any(feature = "std", feature = "alloc"))]
pub struct StreamTask<S>
where
    S: Stream,
{
    stream: Pin<Box<S>>,
    exhausted: bool,
}

// Same pattern as FutureTask - Box requires alloc
```

#### Complete Feature Matrix

```toml
[package]
name = "foundation_core"

[features]
default = ["std"]

# Standard library support (includes alloc)
std = [
    "futures-core/std",
    "alloc",
]

# Heap allocation without full std
alloc = [
    "futures-core/alloc",
]

# Multi-threaded executor (requires std for threading)
multi = ["std"]

# Optional: embedded-friendly minimal build
minimal = []  # Pure no_std, no alloc
```

#### Conditional Compilation Summary

```rust
// Module-level no_std declaration
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Type imports based on features
#[cfg(feature = "std")]
use std::{boxed::Box, vec::Vec, string::String};

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{boxed::Box, vec::Vec, string::String};

// FutureTask available with heap
#[cfg(any(feature = "std", feature = "alloc"))]
pub use future_task::{FutureTask, from_future, run_future};

// FutureTaskRef available in pure no_std
#[cfg(not(any(feature = "std", feature = "alloc")))]
pub use future_task::{FutureTaskRef, from_future_ref};

// StreamTask requires heap
#[cfg(any(feature = "std", feature = "alloc"))]
pub use future_task::{StreamTask, from_stream};

// TimeoutTask requires std for Instant
#[cfg(feature = "std")]
pub use wrappers::TimeoutTask;

// PollLimitTask available everywhere
pub use wrappers::PollLimitTask;

// Thread-local waker cache only with std
#[cfg(feature = "std")]
fn get_noop_waker() -> Waker { NOOP_WAKER.with(|w| w.clone()) }

#[cfg(not(feature = "std"))]
fn get_noop_waker() -> Waker { create_noop_waker() }
```

#### WASM Considerations

The Future adapter works on WASM with these considerations:

```rust
// No-op waker works identically on WASM
// No platform-specific code needed

// The unified executor already handles WASM:
// - WASM always uses single-threaded executor
// - No threading primitives required

impl<F> TaskIterator for FutureTask<F>
where
    F: Future + Send + 'static,  // Send bound is fine on WASM (single-threaded)
    F::Output: Send + 'static,
{
    // Implementation is platform-agnostic
}

// For WASM-specific futures (e.g., from wasm-bindgen-futures):
#[cfg(target_arch = "wasm32")]
pub fn run_future_wasm<F>(future: F) -> Result<F::Output, ExecutorError>
where
    F: Future + 'static,  // No Send required on WASM
    F::Output: 'static,
{
    let task = FutureTask::new(future);
    execute(task)
}
```

#### Relaxed Send Bounds for WASM

Since WASM is single-threaded, we can provide relaxed bounds:

```rust
#[cfg(target_arch = "wasm32")]
impl<F> TaskIterator for FutureTask<F>
where
    F: Future + 'static,  // No Send required
    F::Output: 'static,
{
    type Ready = F::Output;
    type Pending = FuturePollState;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Same implementation, just relaxed bounds
        if self.completed {
            return None;
        }

        let waker = get_noop_waker();
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => {
                self.completed = true;
                Some(TaskStatus::Ready(output))
            }
            Poll::Pending => {
                Some(TaskStatus::Pending(FuturePollState::Pending))
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<F> TaskIterator for FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    // Same implementation with Send bounds for native
}
```

#### Convenience Functions with Platform-Aware Bounds

```rust
/// Wrap a future into a TaskIterator (native - requires Send)
#[cfg(not(target_arch = "wasm32"))]
pub fn from_future<F>(future: F) -> FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    FutureTask::new(future)
}

/// Wrap a future into a TaskIterator (WASM - no Send required)
#[cfg(target_arch = "wasm32")]
pub fn from_future<F>(future: F) -> FutureTask<F>
where
    F: Future + 'static,
    F::Output: 'static,
{
    FutureTask::new(future)
}

/// Execute a future using the unified executor
#[cfg(not(target_arch = "wasm32"))]
pub fn run_future<F>(future: F) -> Result<F::Output, ExecutorError>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let task = FutureTask::new(future);
    execute(task)
}

#[cfg(target_arch = "wasm32")]
pub fn run_future<F>(future: F) -> Result<F::Output, ExecutorError>
where
    F: Future + 'static,
    F::Output: 'static,
{
    let task = FutureTask::new(future);
    execute(task)
}
```

#### Usage Examples

```rust
// Simple future execution
let result = run_future(async {
    let a = compute_async().await;
    let b = fetch_data().await;
    a + b
})?;

// Manual task creation for more control
let task = FutureTask::new(some_future);
let task_with_timeout = TimeoutTask::new(task, Duration::from_secs(30));
let result = execute(task_with_timeout)?;

// Stream processing
let stream = async_stream::stream! {
    for i in 0..10 {
        yield i;
    }
};
let task = StreamTask::new(stream);
// Execute returns Option<Item> for each poll
```

## Success Criteria

- [ ] `actions.rs` exists with LiftAction, ScheduleAction, BroadcastAction
- [ ] `CompositeAction` enum combines all action types with custom slot
- [ ] All action types implement ExecutionAction correctly
- [ ] `unified.rs` exists with feature-gated execute() function
- [ ] WASM always uses single executor
- [ ] Native uses single by default, multi with feature flag
- [ ] `StateTransition` enum covers all transition types
- [ ] `StateMachine` trait is defined with associated types
- [ ] `StateMachineTask` wrapper implements TaskIterator
- [ ] `FutureTask` wraps Future and implements TaskIterator (std/alloc)
- [ ] `FutureTaskRef` provides stack-pinned variant (pure no_std)
- [ ] `FuturePollState` enum defined for pending state
- [ ] No-op waker uses only `core` types (no_std compatible)
- [ ] Thread-local waker cache used with `std` feature
- [ ] `from_future()` convenience function works
- [ ] `run_future()` executes futures through unified executor
- [ ] WASM build works with relaxed Send bounds
- [ ] Native build requires Send bounds
- [ ] `StreamTask` wraps async Stream and implements TaskIterator
- [ ] `from_stream()` convenience function works
- [ ] `futures-core` uses `default-features = false`
- [ ] `std` feature enables full functionality
- [ ] `alloc` feature enables heap allocation without std
- [ ] Pure no_std build compiles (no std, no alloc)
- [ ] `TimeoutTask` available only with `std` feature
- [ ] `PollLimitTask` available in all configurations
- [ ] `RetryingTask` wraps TaskIterator with retry logic
- [ ] `BackoffTask` supports fixed/exponential/linear strategies
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings

# Standard tests
cargo test --package foundation_core -- actions
cargo test --package foundation_core -- unified
cargo test --package foundation_core -- state_machine
cargo test --package foundation_core -- future_task

# Feature combinations
cargo build --package foundation_core                              # default (std)
cargo build --package foundation_core --features multi             # std + multi
cargo build --package foundation_core --no-default-features --features alloc  # no_std + alloc
cargo build --package foundation_core --no-default-features        # pure no_std

# WASM
cargo build --package foundation_core --target wasm32-unknown-unknown
cargo build --package foundation_core --target wasm32-unknown-unknown --no-default-features --features alloc
```

## Notes for Agents

### Before Starting
- **MUST READ** `valtron/executors/task.rs` for TaskIterator trait
- **MUST READ** `valtron/executors/executor.rs` for ExecutionAction trait
- **MUST READ** `valtron/executors/single/mod.rs` for single executor
- **MUST READ** `valtron/executors/multi/mod.rs` for multi executor (if exists)
- **MUST VERIFY** existing valtron patterns before adding new ones

### Implementation Guidelines
- These utilities go in `valtron/executors/`, NOT in `simple_http/client/`
- Use generic type parameters extensively
- All types should be `Send + 'static` for executor compatibility
- Feature gate multi-threaded code with `#[cfg(feature = "multi")]`
- Add `#[cfg(not(target_arch = "wasm32"))]` for platform-specific code
- Document all public types and functions

### Integration with task-iterator
- The task-iterator feature will use these utilities
- HttpRequestTask can use StateMachine trait if beneficial
- HttpClientAction can extend CompositeAction if needed
- FutureTask enables wrapping any async code for execution

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
