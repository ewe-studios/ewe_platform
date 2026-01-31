---
feature: valtron-utilities
description: Reusable ExecutionAction types, unified executor wrapper, state machine helpers, Future adapter, and retry/timeout wrappers
status: in-progress
priority: high
depends_on: []
estimated_effort: medium
created: 2026-01-19
last_updated: 2026-01-24
author: Main Agent
tasks:
  completed: 33
  uncompleted: 0
  total: 33
  completion_percentage: 100
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/11-skills-usage.md
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

### A. Reusable ExecutionAction Types for Spawning Child Tasks

**Purpose**: These action types are used by `TaskIterator`s to spawn child tasks during execution. When a TaskIterator returns `TaskStatus::Spawn(action)`, the executor calls `action.apply(parent_key, engine)` to schedule the child task.

**CRITICAL DISTINCTION**: These are NOT for initial task submission. They are ONLY for spawning children from within a running task.

```rust,ignore
// ❌ WRONG - Don't use actions for initial submission
let action = SpawnWithLift::new(my_task);
// ... nowhere to call action.apply()

// ✅ RIGHT - Use builder for initial submission
engine.spawn()
    .with_task(my_task)
    .lift()  // or .schedule() or .broadcast()
```

#### Spawning Strategies Comparison

| Action               | Engine Method        | Queue          | Priority   | Use When                                |
| -------------------- | -------------------- | -------------- | ---------- | --------------------------------------- |
| `SpawnWithLift`      | `engine.lift()`      | Local (top)    | High       | Child must run before other queued work |
| `SpawnWithSchedule`  | `engine.schedule()`  | Local (bottom) | Normal     | Standard child task                     |
| `SpawnWithBroadcast` | `engine.broadcast()` | Global         | Background | Work can be picked up by any thread     |

#### SpawnWithLift (formerly LiftAction)

**Purpose**: Spawns a child task with HIGH PRIORITY (top of local queue).

**When to Use**:

- The parent task needs the child to complete before continuing
- The child task is a dependency for subsequent processing
- You want to prioritize this child over other queued work

**How It Works**:

1. Wraps the child iterator in `DoNext` (converts `TaskIterator` → `ExecutionIterator`)
2. Calls `engine.lift()` to add to **TOP** of local queue
3. Links child to parent via the `parent_key`
4. Child task processes before other queued work

**Example**:

```rust
use valtron::{TaskIterator, TaskStatus, SpawnWithLift};

struct ParentTask {
    needs_config: bool,
}

struct FetchConfigTask;

impl TaskIterator for ParentTask {
    type Ready = ProcessedData;
    type Pending = ();
    type Spawner = SpawnWithLift<FetchConfigTask>;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.needs_config {
            self.needs_config = false;
            // Spawn high-priority child to fetch config
            Some(TaskStatus::Spawn(SpawnWithLift::new(FetchConfigTask)))
        } else {
            // Process data using config
            Some(TaskStatus::Ready(self.process()))
        }
    }
}
```

**Implementation Signature**:

```rust
pub struct SpawnWithLift<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>> + 'static,
    D: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    iter: Option<I>,
    _marker: PhantomData<(D, P, S)>,
}

impl<I, D, P, S> SpawnWithLift<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>> + 'static,
    D: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter: Some(iter),
            _marker: PhantomData,
        }
    }
}

impl<I, D, P, S> ExecutionAction for SpawnWithLift<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>> + 'static,
    D: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    fn apply(
        mut self,
        key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let Some(iter) = self.iter.take() {
            let task = LiftTask::new(iter);
            let exec_iter: BoxedExecutionIterator = DoNext::new(task).into();
            // Use lift() - adds to TOP of queue and links to parent
            executor.lift(exec_iter, Some(key))?;
        }
        Ok(())
    }
}
```

#### SpawnWithSchedule (formerly ScheduleAction)

**Purpose**: Spawns a child task with NORMAL PRIORITY (bottom of local queue).

**When to Use**:

- Standard child task that doesn't need priority
- Child task can wait for other queued work
- Most common spawning pattern

**How It Works**:

1. Wraps the child closure in `ScheduleTask`
2. Calls `engine.schedule()` to add to **BOTTOM** of local queue
3. Child task processes after other queued work
4. No parent linkage required

**Example**:

```rust
use valtron::{TaskIterator, TaskStatus, SpawnWithSchedule};

struct ParentTask {
    needs_cleanup: bool,
}

impl TaskIterator for ParentTask {
    type Ready = Result;
    type Pending = ();
    type Spawner = SpawnWithSchedule<CleanupTask>;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.needs_cleanup {
            self.needs_cleanup = false;
            // Spawn normal-priority cleanup task
            Some(TaskStatus::Spawn(SpawnWithSchedule::new(CleanupTask::new())))
        } else {
            Some(TaskStatus::Ready(self.get_result()))
        }
    }
}
```

**Implementation Signature**:

```rust
pub struct ScheduleTask<F>
where
    F: FnOnce(),
{
    closure: Option<F>,
}

impl<F> TaskIterator for ScheduleTask<F>
where
    F: FnOnce(),
{
    type Pending = ();
    type Ready = ();
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if let Some(closure) = self.closure.take() {
            closure();
            Some(TaskStatus::Ready(()))
        } else {
            None
        }
    }
}

pub struct SpawnWithSchedule<F>
where
    F: FnOnce() + 'static,
{
    closure: Option<F>,
}

impl<F> SpawnWithSchedule<F>
where
    F: FnOnce() + 'static,
{
    pub fn new(closure: F) -> Self {
        Self {
            closure: Some(closure),
        }
    }
}

impl<F> ExecutionAction for SpawnWithSchedule<F>
where
    F: FnOnce() + 'static,
{
    fn apply(
        mut self,
        _key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let Some(closure) = self.closure.take() {
            let task = ScheduleTask::new(closure);
            let exec_iter: BoxedExecutionIterator = DoNext::new(task).into();
            // Use schedule() - adds to BOTTOM of queue
            executor.schedule(exec_iter)?;
        }
        Ok(())
    }
}
```

#### SpawnWithBroadcast (formerly BroadcastAction)

**Purpose**: Spawns a child task to GLOBAL QUEUE (any thread can pick up).

**When to Use**:

- Work can be distributed across threads
- Task doesn't need to run on the same thread as parent
- Background processing or parallel work distribution

**How It Works**:

1. Wraps the child task in `BroadcastTask`
2. Calls `engine.broadcast()` to add to **GLOBAL** queue
3. Any executor thread can pick up the task
4. Enables cross-thread work distribution

**Example**:

```rust
use valtron::{spawn_builder, TaskIterator, TaskStatus, SpawnWithBroadcast};

struct DataProcessor {
    chunks: Vec<DataChunk>,
    current: usize,
}

impl TaskIterator for DataProcessor {
    type Ready = ProcessedData;
    type Pending = ();
    type Spawner = SpawnWithBroadcast<ProcessChunkTask>;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current < self.chunks.len() {
            let chunk = self.chunks[self.current].clone();
            self.current += 1;

            // Spawn child task to global queue for any thread to process
            Some(TaskStatus::Spawn(
                SpawnWithBroadcast::new(ProcessChunkTask::new(chunk))
            ))
        } else {
            None // All chunks spawned
        }
    }
}
```

**Implementation Signature**:

```rust
pub struct BroadcastTask<T>
where
    T: Clone + Send,
{
    value: Option<T>,
    callbacks: Vec<Box<dyn FnOnce(T) + Send>>,
}

impl<T> TaskIterator for BroadcastTask<T>
where
    T: Clone + Send,
{
    type Pending = ();
    type Ready = ();
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if let Some(value) = self.value.take() {
            // Call each callback with a clone
            for callback in self.callbacks.drain(..) {
                callback(value.clone());
            }
            Some(TaskStatus::Ready(()))
        } else {
            None
        }
    }
}

pub struct SpawnWithBroadcast<T>
where
    T: Clone + 'static,
{
    value: Option<T>,
    callbacks: Vec<Box<dyn FnOnce(T) + Send>>,
}

impl<T> SpawnWithBroadcast<T>
where
    T: Clone + 'static,
{
    pub fn new(value: T, callbacks: Vec<Box<dyn FnOnce(T) + Send>>) -> Self {
        Self {
            value: Some(value),
            callbacks,
        }
    }
}

impl<T> ExecutionAction for SpawnWithBroadcast<T>
where
    T: Clone + Send + 'static,
{
    fn apply(
        mut self,
        _key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let (Some(value), true) = (self.value.take(), !self.callbacks.is_empty()) {
            let callbacks = std::mem::take(&mut self.callbacks);
            let task = BroadcastTask::new(value, callbacks);
            // Use broadcast() - sends to GLOBAL queue for any thread
            valtron::spawn_builder(executor)
                .with_parent(key.clone())
                .with_task(task)
                .broadcast()?;
        }
        Ok(())
    }
}
```

#### SpawnStrategy (formerly CompositeAction)

**Purpose**: Enum combining all action types plus a custom action slot, allowing a TaskIterator to use different spawning strategies dynamically.

**When to Use**:

- Your task needs to spawn children using different strategies at different times
- You want flexibility in choosing spawn method at runtime
- You need to combine standard actions with custom behavior

**Example**:

```rust
use valtron::{TaskIterator, TaskStatus, SpawnStrategy};

struct FlexibleTask {
    phase: Phase,
}

enum Phase {
    FetchData,
    ProcessLocal,
    BroadcastResults,
}

impl TaskIterator for FlexibleTask {
    type Ready = Result;
    type Pending = ();
    type Spawner = SpawnStrategy<
        std::vec::IntoIter<TaskStatus<i32, (), NoAction>>,  // For Lift
        i32,
        (),
        NoAction,
        fn(),  // For Schedule
        i32,   // For Broadcast
        NoAction,  // For Custom
    >;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self.phase {
            Phase::FetchData => {
                // Use lift for high-priority fetch
                Some(TaskStatus::Spawn(SpawnStrategy::Lift(
                    SpawnWithLift::new(fetch_iter())
                )))
            }
            Phase::ProcessLocal => {
                // Use schedule for normal processing
                Some(TaskStatus::Spawn(SpawnStrategy::Schedule(
                    SpawnWithSchedule::new(|| process())
                )))
            }
            Phase::BroadcastResults => {
                // Use broadcast for distribution
                Some(TaskStatus::Spawn(SpawnStrategy::Broadcast(
                    SpawnWithBroadcast::new(results, callbacks)
                )))
            }
        }
    }
}
```

**Implementation Signature**:

```rust
pub enum SpawnStrategy<IW, TW, IL, DL, PL, SL, F, V, C>
where
    IW: Iterator<Item = TW> + 'static,
    TW: 'static,
    IL: Iterator<Item = TaskStatus<DL, PL, SL>> + 'static,
    DL: 'static,
    PL: 'static,
    SL: ExecutionAction + 'static,
    F: FnOnce() + 'static,
    V: Clone + 'static,
    C: ExecutionAction,
{
    None,
    Wrap(WrapAction<IW, TW>),
    Lift(SpawnWithLift<IL, DL, PL, SL>),
    Schedule(SpawnWithSchedule<F>),
    Broadcast(SpawnWithBroadcast<V>),
    Custom(C),
}

impl<IW, TW, IL, DL, PL, SL, F, V, C> ExecutionAction
    for SpawnStrategy<IW, TW, IL, DL, PL, SL, F, V, C>
where
    IW: Iterator<Item = TW> + 'static,
    TW: 'static,
    IL: Iterator<Item = TaskStatus<DL, PL, SL>> + 'static,
    DL: 'static,
    PL: 'static,
    SL: ExecutionAction + 'static,
    F: FnOnce() + 'static,
    V: Clone + 'static,
    C: ExecutionAction,
{
    fn apply(self, key: crate::synca::Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        match self {
            Self::None => Ok(()),
            Self::Wrap(action) => action.apply(key, engine),
            Self::Lift(action) => action.apply(key, engine),
            Self::Schedule(action) => action.apply(key, engine),
            Self::Broadcast(action) => action.apply(key, engine),
            Self::Custom(action) => action.apply(key, engine),
        }
    }
}
```

#### Anti-Patterns and Common Mistakes

**❌ WRONG: Using actions for initial task submission**

```rust
// This doesn't work - actions need to be applied with a parent_key and engine
let action = SpawnWithLift::new(my_task);
// ... but we have no executor context to call action.apply()
```

**✅ RIGHT: Use builder pattern for initial submission**

```rust
engine.spawn()
    .with_task(my_task)
    .lift()  // This is the builder method, not the action
```

**❌ WRONG: Confusing lift/schedule/broadcast builder methods with actions**

```rust
// These are builder methods on SpawnBuilder, not actions:
engine.spawn().lift()       // Builder method
engine.spawn().schedule()   // Builder method
engine.spawn().broadcast()  // Builder method

// These are actions for spawning from within tasks:
SpawnWithLift::new(child)      // Action for TaskStatus::Spawn()
SpawnWithSchedule::new(child)  // Action for TaskStatus::Spawn()
SpawnWithBroadcast::new(child) // Action for TaskStatus::Spawn()
```

**❌ WRONG: Trying to use actions outside of TaskIterator**

```rust
fn some_function(engine: BoxedExecutionEngine) {
    let action = SpawnWithLift::new(task);
    // This won't compile - where does parent_key come from?
    action.apply(???, engine)?;
}
```

**✅ RIGHT: Actions are ONLY for use within TaskIterator::next()**

```rust
impl TaskIterator for MyTask {
    type Spawner = SpawnWithLift<ChildTask>;

    fn next(&mut self) -> Option<TaskStatus<...>> {
        // Inside here, return TaskStatus::Spawn with an action
        Some(TaskStatus::Spawn(SpawnWithLift::new(child_task)))
    }
}
```

**❌ WRONG: Manually calling action.apply()**

```rust
impl TaskIterator for MyTask {
    fn next(&mut self) -> Option<TaskStatus<...>> {
        let action = SpawnWithLift::new(child);
        action.apply(self.parent_key?, self.engine.clone())?;  // DON'T DO THIS
        Some(TaskStatus::Ready(result))
    }
}
```

**✅ RIGHT: Return TaskStatus::Spawn and let executor handle it**

```rust
impl TaskIterator for MyTask {
    fn next(&mut self) -> Option<TaskStatus<...>> {
        // Just return the Spawn status - executor calls apply() for you
        Some(TaskStatus::Spawn(SpawnWithLift::new(child)))
    }
}
```

#### Legacy Names (Deprecated)

For backward compatibility during migration:

| Old Name          | New Name             | Reason for Change                                               |
| ----------------- | -------------------- | --------------------------------------------------------------- |
| `LiftAction`      | `SpawnWithLift`      | Clarifies this is for spawning children with lift strategy      |
| `ScheduleAction`  | `SpawnWithSchedule`  | Clarifies this is for spawning children with schedule strategy  |
| `BroadcastAction` | `SpawnWithBroadcast` | Clarifies this is for spawning children with broadcast strategy |
| `CompositeAction` | `SpawnStrategy`      | Better reflects that it's choosing a spawning strategy          |

**Migration**: The implementation should provide deprecated type aliases:

```rust
#[deprecated(since = "0.x.0", note = "Use `SpawnWithLift` instead")]
pub type LiftAction<I, D, P, S> = SpawnWithLift<I, D, P, S>;

#[deprecated(since = "0.x.0", note = "Use `SpawnWithSchedule` instead")]
pub type ScheduleAction<F> = SpawnWithSchedule<F>;

#[deprecated(since = "0.x.0", note = "Use `SpawnWithBroadcast` instead")]
pub type BroadcastAction<T> = SpawnWithBroadcast<T>;

#[deprecated(since = "0.x.0", note = "Use `SpawnStrategy` instead")]
pub type CompositeAction<IW, TW, IL, DL, PL, SL, F, V, C> =
    SpawnStrategy<IW, TW, IL, DL, PL, SL, F, V, C>;
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
pub fn execute<T>(
    task: T
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
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

fn execute_single<T>(
    task: T
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    single::spawn(task)
}

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(
    task: T
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
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

````rust
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
````

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

````rust
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
````

#### Async Block Helper

````rust
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
````

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

| Feature         | Environment      | Notes                                             |
| --------------- | ---------------- | ------------------------------------------------- |
| `std` (default) | Standard library | Full functionality                                |
| `alloc`         | no_std + alloc   | Heap allocation available                         |
| (none)          | Pure no_std      | Limited functionality, no Box/Pin heap allocation |

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

- [x] `actions.rs` exists with SpawnWithLift, SpawnWithSchedule, SpawnWithBroadcast (previously LiftAction, ScheduleAction, BroadcastAction)
- [x] `SpawnStrategy` enum combines all action types with custom slot (previously CompositeAction)
- [x] All action types implement ExecutionAction correctly
- [x] Deprecated type aliases provided for backward compatibility (LiftAction, ScheduleAction, BroadcastAction, CompositeAction)
- [x] Comprehensive documentation explains when to use each action type
- [x] Anti-patterns section clearly shows what NOT to do
- [x] Examples demonstrate correct usage within TaskIterator::next()
- [x] Clear distinction between actions (for spawning children) and builder pattern (for initial submission)
- [x] `unified.rs` exists with feature-gated execute() function
- [x] WASM always uses single executor
- [x] Native uses single by default, multi with feature flag
- [x] `StateTransition` enum covers all transition types
- [x] `StateMachine` trait is defined with associated types
- [x] `StateMachineTask` wrapper implements TaskIterator
- [x] `FutureTask` wraps Future and implements TaskIterator (std/alloc)
- [x] `FutureTaskRef` provides stack-pinned variant (pure no_std)
- [x] `FuturePollState` enum defined for pending state
- [x] No-op waker uses only `core` types (no_std compatible)
- [x] Thread-local waker cache used with `std` feature
- [x] `from_future()` convenience function works
- [x] `run_future()` executes futures through unified executor
- [x] WASM build works with relaxed Send bounds
- [x] Native build requires Send bounds
- [x] `StreamTask` wraps async Stream and implements TaskIterator
- [x] `from_stream()` convenience function works
- [x] `futures-core` uses `default-features = false`
- [x] `std` feature enables full functionality
- [x] `alloc` feature enables heap allocation without std
- [x] Pure no_std build compiles (no std, no alloc)
- [x] `TimeoutTask` available only with `std` feature
- [x] `PollLimitTask` available in all configurations
- [x] `RetryingTask` wraps TaskIterator with retry logic
- [x] `BackoffTask` supports fixed/exponential/linear strategies
- [x] Type names updated: LiftAction→SpawnWithLift, ScheduleAction→SpawnWithSchedule, BroadcastAction→SpawnWithBroadcast, CompositeAction→SpawnStrategy
- [x] Backward-compatible deprecated type aliases added for old names
- [x] Send bound added to BroadcastTask<T> for thread safety
- [x] All unit tests pass
- [x] Code passes `cargo fmt` and `cargo clippy`

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

_Created: 2026-01-19_
_Last Updated: 2026-01-19_
