//! Unified executor interface that auto-selects the appropriate executor.
//!
//! This module provides a single `execute()` function that automatically chooses
//! between single-threaded and multi-threaded executors based on the target
//! platform and enabled features.
//!
//! ## Selection Logic
//!
//! - **WASM**: Always uses `single` executor (no threading support)
//! - **Native without `multi` feature**: Uses `single` executor
//! - **Native with `multi` feature**: Uses `multi` executor

#![allow(clippy::type_complexity)]

use core::future::Future;

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::valtron::FutureTask;

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::valtron::StreamTask;

#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
use crate::valtron::DrivenSendTaskIterator;

use crate::valtron::{
    CollectAllStream, DrivenNonSendRecvIterator, DrivenNonSendStreamIterator,
    DrivenNonSendTaskIterator, DrivenRecvIterator, DrivenStreamIterator, ExecutionAction,
    GenericResult, InlineSendAction, InlineSendActionBehaviour, NotifyQueueStreamIterator,
    NotifyRecvIterator, TaskIterator, TaskStatus, TaskStatusMapper, ThreadedValue,
};

use crate::valtron::executors::DEFAULT_WAIT_CYCLE;

/// Configuration for stream execution with fine-grained control over iterator behavior.
///
/// # Fields
///
/// * `wait_cycle` - Polling/wait duration for executor (defaults to [`DEFAULT_WAIT_CYCLE`])
/// * `max_turns` - Max poll attempts before yielding in `ConcurrentQueueStreamIterator`
///   (defaults to [`crate::valtron::executors::DEFAULT_MAX_TURNS`])
/// * `park_duration` - Thread park duration when queue is empty, passed to
///   `ConcurrentQueueStreamIterator` (defaults to [`crate::valtron::executors::DEFAULT_PARK_DURATION`])
///
/// # Example
///
/// ```ignore
/// use crate::valtron::executors::{StreamConfig, DEFAULT_MAX_TURNS};
/// use std::time::Duration;
///
/// let config = StreamConfig::default()
///     .with_max_turns(50)  // Higher throughput
///     .with_park_duration(Duration::from_nanos(50));
///
/// let result = execute_with_config(task, config)?;
/// ```
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Wait cycle duration for executor polling
    pub wait_cycle: std::time::Duration,
    /// Max turns for ConcurrentQueueStreamIterator
    pub max_turns: usize,
    /// Park duration for ConcurrentQueueStreamIterator (used as wait_cycle)
    pub park_duration: std::time::Duration,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            wait_cycle: crate::valtron::executors::DEFAULT_WAIT_CYCLE,
            max_turns: crate::valtron::executors::DEFAULT_MAX_TURNS,
            park_duration: crate::valtron::executors::DEFAULT_PARK_DURATION,
        }
    }
}

impl StreamConfig {
    /// Create a new StreamConfig with custom wait_cycle.
    #[must_use]
    pub fn with_wait_cycle(mut self, wait_cycle: std::time::Duration) -> Self {
        self.wait_cycle = wait_cycle;
        self
    }

    /// Create a new StreamConfig with custom max_turns.
    #[must_use]
    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Create a new StreamConfig with custom park_duration.
    #[must_use]
    pub fn with_park_duration(mut self, park_duration: std::time::Duration) -> Self {
        self.park_duration = park_duration;
        self
    }
}

/// Execute a task using the appropriate executor for the current platform/features.
///
/// ## Platform Selection
///
/// | Platform | Feature | Executor Used |
/// |----------|---------|---------------|
/// | WASM     | any     | `single`      |
/// | Native   | none    | `single`      |
/// | Native   | `multi` | `multi`       |
///
/// ## Example
///
/// ```ignore
/// let task = MyTask::new();
/// let result = execute(task)?;
/// ```
///
/// WHY: Provides single API that works across all platforms/configurations
/// WHAT: Auto-selects executor based on compile-time configuration
///
/// This function selects the correct executor at compile time:
/// - On `wasm32` targets it uses the single-threaded executor.
/// - On native targets without the `multi` feature it uses the single-threaded executor.
/// - On native targets with the `multi` feature it uses the multi-threaded executor.
///
/// # Arguments
///
/// - `task`: The task to execute. It must implement the [`TaskIterator`] trait and satisfy the
///   required `Send + 'static` bounds for the selected executor.
/// - `wait_cycle`: Optional polling/wait duration used by the executor when creating the iterator.
///   If `None` the function uses [`DEFAULT_WAIT_CYCLE`].
///
/// # Returns
///
/// Returns a [`GenericResult`] wrapping a `RecvIterator` over `TaskStatus`. On success the
/// `Ok` variant contains an iterator that yields `TaskStatus::Ready` / `TaskStatus::Pending`
/// values produced by the scheduled task. On error the `Err` variant contains an error from the
/// underlying scheduling/spawn operation.
///
/// # Errors
///
/// This function returns an error if scheduling the task with the chosen executor fails. Possible
/// reasons include executor initialization issues or errors returned by the spawn builder
/// (`schedule_iter` implementation). The concrete error type is the one used by [`GenericResult`]
/// in this crate and will contain additional context about the failure.
///
/// # Panics
///
/// This function does not panic under normal operation. However, panics can occur if:
/// - The provided task implementation panics when polled or when executed by the executor.
/// - The underlying executor implementation or thread pool panics internally.
///
/// In general, avoid panics in task implementations to prevent terminating worker threads or the
/// host process.
///
/// # Type bounds
///
/// The required trait bounds ensure the task and produced values are safe to send between
/// threads when the multi-threaded executor is selected.
///
/// WHY: Provides single API that works across all platforms/configurations
/// WHAT: Auto-selects executor based on compile-time configuration
#[cfg(not(feature = "multi"))]
pub fn execute_as_task<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenNonSendRecvIterator<T>>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    use crate::valtron::{drive_non_send_receiver, single};

    tracing::debug!("Executing as a single-threaded stream in no-wasm");

    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(drive_non_send_receiver(iter))
}

/// Execute a task using the appropriate executor for the current platform/features.
///
/// ## Platform Selection
///
/// | Platform | Feature | Executor Used |
/// |----------|---------|---------------|
/// | WASM     | any     | `single`      |
/// | Native   | none    | `single`      |
/// | Native   | `multi` | `multi`       |
///
/// ## Example
///
/// ```ignore
/// let task = MyTask::new();
/// let result = execute(task)?;
/// ```
///
/// WHY: Provides single API that works across all platforms/configurations
/// WHAT: Auto-selects executor based on compile-time configuration
///
/// This function selects the correct executor at compile time:
/// - On `wasm32` targets it uses the single-threaded executor.
/// - On native targets without the `multi` feature it uses the single-threaded executor.
/// - On native targets with the `multi` feature it uses the multi-threaded executor.
///
/// # Arguments
///
/// - `task`: The task to execute. It must implement the [`TaskIterator`] trait and satisfy the
///   required `Send + 'static` bounds for the selected executor.
/// - `wait_cycle`: Optional polling/wait duration used by the executor when creating the iterator.
///   If `None` the function uses [`DEFAULT_WAIT_CYCLE`].
///
/// # Returns
///
/// Returns a [`GenericResult`] wrapping a `RecvIterator` over `TaskStatus`. On success the
/// `Ok` variant contains an iterator that yields `TaskStatus::Ready` / `TaskStatus::Pending`
/// values produced by the scheduled task. On error the `Err` variant contains an error from the
/// underlying scheduling/spawn operation.
///
/// # Errors
///
/// This function returns an error if scheduling the task with the chosen executor fails. Possible
/// reasons include executor initialization issues or errors returned by the spawn builder
/// (`schedule_iter` implementation). The concrete error type is the one used by [`GenericResult`]
/// in this crate and will contain additional context about the failure.
///
/// # Panics
///
/// This function does not panic under normal operation. However, panics can occur if:
/// - The provided task implementation panics when polled or when executed by the executor.
/// - The underlying executor implementation or thread pool panics internally.
///
/// In general, avoid panics in task implementations to prevent terminating worker threads or the
/// host process.
///
/// # Type bounds
///
/// The required trait bounds ensure the task and produced values are safe to send between
/// threads when the multi-threaded executor is selected.
///
/// WHY: Provides single API that works across all platforms/configurations
/// WHAT: Auto-selects executor based on compile-time configuration
#[cfg(feature = "multi")]
pub fn execute_as_task<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenRecvIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single-threaded stream in wasm with multi=off");

        // Schedule task and get iterator
        let iter = single::spawn()
            .with_task(task)
            .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

        Ok(drive_receiver(iter))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        tracing::debug!("Executing as a multi-threaded stream in no-wasm with multi=off");

        use crate::valtron::multi;

        // Schedule task and get iterator
        let iter = multi::spawn()
            .with_task(task)
            .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

        Ok(drive_receiver(iter))
    }
}

/// `execute_stream` unlike [`execute_as_task`] returns a `DrivenStreamIterator`
/// which hides the underlying mechanics of handling `TaskStatus`. The stream
/// iterator will internally manage different task states, send any required
/// spawn events to the executor as tasks request additional work, and present a
/// simpler, higher-level sequence of produced values to the caller.
///
/// This function follows the same platform/feature selection as [`execute`]:
/// - On `wasm32` targets it uses the single-threaded executor.
/// - On native targets without the `multi` feature it uses the single-threaded executor.
/// - On native targets with the `multi` feature it uses the multi-threaded executor.
///
/// # Arguments
///
/// - `task`: The task to execute. Must implement [`TaskIterator`] and satisfy the
///   required `Send + 'static` bounds for the selected executor.
/// - `wait_cycle`: Optional polling/wait duration used by the executor when
///   creating the stream iterator. If `None`, the function uses
///   [`DEFAULT_WAIT_CYCLE`]. This value controls how long the executor will
///   wait/poll between checks for task progress in single-threaded modes
///   (and may be used by multi-threaded executors to control scheduling
///   behavior).
///
/// # Returns
///
/// Returns a [`GenericResult`] wrapping a `DrivenStreamIterator` over the task's
/// produced values. On success the `Ok` variant contains a stream-style
/// iterator that yields ready values (and may represent pending/ready events
/// internally). On error the `Err` variant contains an error from the
/// underlying scheduling/spawn operation.
///
/// # Errors
///
/// This function returns an error if scheduling the task with the chosen
/// executor fails. Possible reasons include:
/// - Executor or thread pool initialization problems.
/// - Errors returned by the spawn/schedule builder used by the executor
///   (for example failures constructing the iterator).
///
/// The concrete error type is the one used by [`GenericResult`] in this crate
/// and will contain additional context about the failure.
///
/// # Panics
///
/// This function does not intentionally panic. However, panics may occur if:
/// - The provided task implementation panics while being polled or executed by
///   the executor.
/// - The underlying executor implementation or thread pool panics internally.
///
/// Avoid panics in task implementations to prevent terminating worker threads
/// or the host process.
///
/// # Type bounds
///
/// The required trait bounds ensure the task and produced values are safe to
/// send between threads when the multi-threaded executor is selected:
/// - `T: TaskIterator + Send + 'static`
/// - `T::Ready: Send + 'static`
/// - `T::Pending: Send + 'static`
/// - `T::Spawner: ExecutionAction + Send + 'static`
///
/// WHY: Provides single API that works across all platforms/configurations.
/// WHAT: Auto-selects executor based on compile-time configuration and returns a
/// higher-level stream iterator that simplifies consuming produced values.
#[cfg(not(feature = "multi"))]
pub fn execute<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenNonSendStreamIterator<T>>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    use super::single;
    use crate::valtron::executors::{DEFAULT_MAX_TURNS, DEFAULT_PARK_DURATION};

    tracing::debug!("Executing as a single-threaded stream in no-wasm with multi=off");

    // Schedule task and get iterator with defaults
    // Note: wait_cycle is not used directly; DEFAULT_PARK_DURATION is used for park_duration
    let iter = single::spawn()
        .with_task(task)
        .scheduled_stream_iter_with_config(
            wait_cycle.unwrap_or(DEFAULT_PARK_DURATION),
            DEFAULT_MAX_TURNS,
        )?;

    Ok(drive_non_send_stream(iter))
}

/// `execute_stream` unlike [`execute_as_task`] returns a `DrivenStreamIterator`
/// which hides the underlying mechanics of handling `TaskStatus`. The stream
/// iterator will internally manage different task states, send any required
/// spawn events to the executor as tasks request additional work, and present a
/// simpler, higher-level sequence of produced values to the caller.
///
/// This function follows the same platform/feature selection as [`execute`]:
/// - On `wasm32` targets it uses the single-threaded executor.
/// - On native targets without the `multi` feature it uses the single-threaded executor.
/// - On native targets with the `multi` feature it uses the multi-threaded executor.
///
/// # Arguments
///
/// - `task`: The task to execute. Must implement [`TaskIterator`] and satisfy the
///   required `Send + 'static` bounds for the selected executor.
/// - `wait_cycle`: Optional polling/wait duration used by the executor when
///   creating the stream iterator. If `None`, the function uses
///   [`DEFAULT_WAIT_CYCLE`]. This value controls how long the executor will
///   wait/poll between checks for task progress in single-threaded modes
///   (and may be used by multi-threaded executors to control scheduling
///   behavior).
///
/// # Returns
///
/// Returns a [`GenericResult`] wrapping a `DrivenStreamIterator` over the task's
/// produced values. On success the `Ok` variant contains a stream-style
/// iterator that yields ready values (and may represent pending/ready events
/// internally). On error the `Err` variant contains an error from the
/// underlying scheduling/spawn operation.
///
/// # Errors
///
/// This function returns an error if scheduling the task with the chosen
/// executor fails. Possible reasons include:
/// - Executor or thread pool initialization problems.
/// - Errors returned by the spawn/schedule builder used by the executor
///   (for example failures constructing the iterator).
///
/// The concrete error type is the one used by [`GenericResult`] in this crate
/// and will contain additional context about the failure.
///
/// # Panics
///
/// This function does not intentionally panic. However, panics may occur if:
/// - The provided task implementation panics while being polled or executed by
///   the executor.
/// - The underlying executor implementation or thread pool panics internally.
///
/// Avoid panics in task implementations to prevent terminating worker threads
/// or the host process.
///
/// # Type bounds
///
/// The required trait bounds ensure the task and produced values are safe to
/// send between threads when the multi-threaded executor is selected:
/// - `T: TaskIterator + Send + 'static`
/// - `T::Ready: Send + 'static`
/// - `T::Pending: Send + 'static`
/// - `T::Spawner: ExecutionAction + Send + 'static`
///
/// WHY: Provides single API that works across all platforms/configurations.
/// WHAT: Auto-selects executor based on compile-time configuration and returns a
/// higher-level stream iterator that simplifies consuming produced values.
#[cfg(feature = "multi")]
pub fn execute<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenStreamIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        use super::single;
        use crate::valtron::executors::{DEFAULT_MAX_TURNS, DEFAULT_PARK_DURATION};

        tracing::debug!("Executing as a single stream in wasm");

        // Schedule task and get iterator with defaults
        // Note: wait_cycle is not used directly; DEFAULT_PARK_DURATION is used for park_duration
        let iter = single::spawn()
            .with_task(task)
            .scheduled_stream_iter_with_config(
                wait_cycle.unwrap_or(DEFAULT_PARK_DURATION),
                DEFAULT_MAX_TURNS,
            )?;

        Ok(drive_stream(iter))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::valtron::executors::{DEFAULT_MAX_TURNS, DEFAULT_PARK_DURATION};
        use crate::valtron::multi;

        tracing::debug!("Executing as a multi-threaded stream in no-wasm");

        // Schedule task and get iterator with defaults
        // Note: wait_cycle is not used directly; DEFAULT_PARK_DURATION is used for park_duration
        let iter = multi::spawn().with_task(task).stream_iter_with_config(
            wait_cycle.unwrap_or(DEFAULT_PARK_DURATION),
            DEFAULT_MAX_TURNS,
        )?;

        Ok(drive_stream(iter))
    }
}

/// Execute a task with custom [`StreamConfig`] for fine-grained control.
///
/// This function allows configuring:
/// - `wait_cycle`: Executor polling duration
/// - `max_turns`: Poll attempts before yielding in ConcurrentQueueStreamIterator
/// - `park_duration`: Thread park duration when queue is empty
///
/// # Arguments
///
/// - `task`: The task to execute
/// - `config`: StreamConfig with execution parameters
///
/// # Returns
///
/// Returns a [`GenericResult`] wrapping a `DrivenStreamIterator`.
///
/// # Errors
///
/// Returns an error if scheduling the task fails.
///
/// # Example
///
/// ```ignore
/// use crate::valtron::executors::{execute_with_config, StreamConfig};
/// use std::time::Duration;
///
/// let config = StreamConfig::default()
///     .with_max_turns(50)
///     .with_park_duration(Duration::from_nanos(50));
///
/// let result = execute_with_config(my_task, config)?;
/// ```
#[cfg(not(feature = "multi"))]
pub fn execute_with_config<T>(
    task: T,
    config: StreamConfig,
) -> GenericResult<DrivenStreamIterator<T>>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    use super::single;

    tracing::debug!("Executing as a single stream in wasm with config");

    // schedule task and get iterator with config
    // note: wait_cycle parameter is used as park_duration in concurrentqueuestreamiterator
    let iter = single::spawn()
        .with_task(task)
        .scheduled_stream_iter_with_config(config.park_duration, config.max_turns)?;

    ok(drive_non_send_stream(iter))
}

/// Execute a task with custom [`StreamConfig`] for fine-grained control.
///
/// This function allows configuring:
/// - `wait_cycle`: Executor polling duration
/// - `max_turns`: Poll attempts before yielding in ConcurrentQueueStreamIterator
/// - `park_duration`: Thread park duration when queue is empty
///
/// # Arguments
///
/// - `task`: The task to execute
/// - `config`: StreamConfig with execution parameters
///
/// # Returns
///
/// Returns a [`GenericResult`] wrapping a `DrivenStreamIterator`.
///
/// # Errors
///
/// Returns an error if scheduling the task fails.
///
/// # Example
///
/// ```ignore
/// use crate::valtron::executors::{execute_with_config, StreamConfig};
/// use std::time::Duration;
///
/// let config = StreamConfig::default()
///     .with_max_turns(50)
///     .with_park_duration(Duration::from_nanos(50));
///
/// let result = execute_with_config(my_task, config)?;
/// ```
#[cfg(feature = "multi")]
pub fn execute_with_config<T>(
    task: T,
    config: StreamConfig,
) -> GenericResult<DrivenStreamIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        use super::single;

        tracing::debug!("Executing as a single stream in wasm with config");

        // schedule task and get iterator with config
        // note: wait_cycle parameter is used as park_duration in concurrentqueuestreamiterator
        let iter = single::spawn()
            .with_task(task)
            .scheduled_stream_iter_with_config(config.park_duration, config.max_turns)?;

        ok(drive_stream(iter))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::valtron::multi;

        tracing::debug!("Executing as a multi-threaded stream in no-wasm with config");

        // Schedule task and get iterator with config
        // Note: wait_cycle parameter is used as park_duration in ConcurrentQueueStreamIterator
        let iter = multi::spawn()
            .with_task(task)
            .stream_iter_with_config(config.park_duration, config.max_turns)?;

        Ok(drive_stream(iter))
    }
}

/// Send a task for execution without a need to get back any replies
/// using the appropriate executor for the current platform/features.
///
/// # Errors
///
/// Returns an error if scheduling the task with the chosen executor fails. Possible
/// reasons include executor initialization issues or errors returned by the spawn builder.
///
/// WHY: Provides single API that works across all platforms/configurations
/// WHAT: Auto-selects executor based on compile-time configuration
#[cfg(not(feature = "multi"))]
pub fn send<T>(task: T) -> GenericResult<()>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    use super::single;

    tracing::debug!("Executing as a single stream in wasm");

    // Schedule task and get iterator
    single::spawn().with_task(task).schedule()?;

    Ok(())
}

/// Send a task for execution without a need to get back any replies
/// using the appropriate executor for the current platform/features.
///
/// # Errors
///
/// Returns an error if scheduling the task with the chosen executor fails. Possible
/// reasons include executor initialization issues or errors returned by the spawn builder.
///
/// WHY: Provides single API that works across all platforms/configurations
/// WHAT: Auto-selects executor based on compile-time configuration
#[cfg(feature = "multi")]
pub fn send<T>(task: T) -> GenericResult<()>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        use super::single;

        tracing::debug!("Executing as a single stream in wasm");

        // Schedule task and get iterator
        single::spawn().with_task(task).schedule()?;

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        tracing::debug!("Executing as a multi-threaded stream in no-wasm");
        use crate::valtron::multi;

        // Schedule task and get iterator
        multi::spawn().with_task(task).schedule()?;

        Ok(())
    }
}

/// Submit a blocking closure for execution on the appropriate background executor.
///
/// WHY: Unified API for background work across all platforms/configurations
/// WHAT: Delegates to `multi::run_background_job` or `single::run_background_job`
/// HOW: Uses the same `#[cfg]` dispatch pattern as `execute()` and `send()`
///
/// ## Platform Selection
///
/// | Platform | Feature | Behavior |
/// |----------|---------|----------|
/// | WASM     | any     | Inline execution (single-threaded) |
/// | Native   | none    | Inline execution (single-threaded) |
/// | Native   | `multi` | Submitted to `BackgroundJobRegistry` pool |
///
/// # Errors
///
/// Returns an error if the job cannot be submitted (pool not initialized or shut down),
/// or if the closure panics in single-threaded mode.
#[cfg(not(feature = "multi"))]
pub fn run_background_job(job: impl FnOnce() + Send + 'static) -> GenericResult<()> {
    super::single::run_background_job(job)
}

/// Submit a blocking closure for execution on the appropriate background executor.
///
/// WHY: Unified API for background work across all platforms/configurations
/// WHAT: Delegates to `multi::run_background_job` or `single::run_background_job`
/// HOW: Uses the same `#[cfg]` dispatch pattern as `execute()` and `send()`
///
/// ## Platform Selection
///
/// | Platform | Feature | Behavior |
/// |----------|---------|----------|
/// | WASM     | any     | Inline execution (single-threaded) |
/// | Native   | none    | Inline execution (single-threaded) |
/// | Native   | `multi` | Submitted to `BackgroundJobRegistry` pool |
///
/// # Errors
///
/// Returns an error if the job cannot be submitted (pool not initialized or shut down),
/// or if the closure panics in single-threaded mode.
#[cfg(feature = "multi")]
pub fn run_background_job(job: impl FnOnce() + Send + 'static) -> GenericResult<()> {
    #[cfg(target_arch = "wasm32")]
    {
        super::single::run_background_job(job)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        super::multi::run_background_job(job)
    }
}

// ============================================================================
// Feature 03: Collection Combinators
// ============================================================================

/// Execute multiple `TaskIterators` in parallel and collect their results.
///
/// This function takes a vector of `TaskIterators`, executes them in parallel
/// using `execute()`, and returns a `CollectAllStream` that aggregates their
/// outputs. The returned iterator yields `Stream<Vec<D>, P>` variants:
/// - `Stream::Pending(count)` while any sources are still pending
/// - `Stream::Next(Vec<D>)` when all sources complete
/// - `Stream::Delayed(duration)` if any source is delayed
///
/// # Arguments
///
/// * `tasks` - Vector of `TaskIterators` to execute in parallel
/// * `wait_cycle` - Optional polling duration (defaults to `DEFAULT_WAIT_CYCLE`)
///
/// # Returns
///
/// Returns a `CollectAllStream` that aggregates outputs from all tasks.
///
/// # Errors
///
/// Returns an error if scheduling any of the provided tasks fails. This includes
/// executor initialization issues or errors returned by the spawn builder for
/// any individual task in the vector.
///
/// # Example
///
/// ```ignore
/// let tasks = vec![task1, task2, task3];
/// let collected = execute_collect_all(tasks, None)?;
///
/// for stream_item in collected {
///     match stream_item {
///         Stream::Pending(count) => println!("{count} still pending..."),
///         Stream::Next(results) => process(results),
///         Stream::Delayed(dur) => continue,
///     }
/// }
/// ```
pub fn execute_collect_all<T>(
    tasks: Vec<T>,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<CollectAllStream<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(not(feature = "multi"))]
    {
        let streams: Vec<DrivenNonSendStreamIterator<T>> = tasks
            .into_iter()
            .map(|t| execute(t, wait_cycle))
            .collect::<GenericResult<_>>()?;

        Ok(CollectAllStream::new(streams))
    }

    #[cfg(feature = "multi")]
    {
        let streams: Vec<DrivenStreamIterator<T>> = tasks
            .into_iter()
            .map(|t| execute(t, wait_cycle))
            .collect::<GenericResult<_>>()?;

        Ok(CollectAllStream::new(streams))
    }
}

// ============================================================================
// run_future_iter - Unified Future executor (all feature configurations)
// ============================================================================

/// Execute a future that produces an iterator, returning results as an Iterator.
///
/// WHY: Single unified API across all feature configurations (multi, std, no_std)
/// WHAT: Creates ThreadedIterFuture with configured queue_size and backpressure,
///       calls execute(), and returns the resulting iterator
///
/// In multi-threaded mode: Spawns background job, returns Result<Iterator, Error>
/// In single-threaded/no-std mode: Polls inline, returns Ok(Iterator)
#[cfg(not(feature = "multi"))]
pub fn run_future_iter<F, Fut, I, T, E>(
    future_fn: F,
    queue_size: Option<usize>,
    backpressure_sleep: Option<std::time::Duration>,
) -> crate::valtron::GenericResult<impl Iterator<Item = ThreadedValue<T, E>>>
where
    F: FnOnce() -> Fut + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: 'static,
    E: 'static,
{
    // Single-threaded and no_std modes don't use queue_size or backpressure_sleep
    let _ = (queue_size, backpressure_sleep);
    Ok(ThreadedIterFuture::new(future_fn).execute())
}

/// Execute a future that produces an iterator, returning results as an Iterator.
///
/// WHY: Single unified API across all feature configurations (multi, std, no_std)
/// WHAT: Creates ThreadedIterFuture with configured queue_size and backpressure,
///       calls execute(), and returns the resulting iterator
///
/// In multi-threaded mode: Spawns background job, returns Result<Iterator, Error>
/// In single-threaded/no-std mode: Polls inline, returns Ok(Iterator)
#[cfg(feature = "multi")]
pub fn run_future_iter<F, Fut, I, T, E>(
    future_fn: F,
    queue_size: Option<usize>,
    backpressure_sleep: Option<std::time::Duration>,
) -> crate::valtron::GenericResult<impl Iterator<Item = ThreadedValue<T, E>>>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    use crate::valtron::ThreadedIterFuture;

    tracing::debug!("Executing as a multi-threaded future iterator");
    let threaded = ThreadedIterFuture::with_backpressure_sleep(
        future_fn,
        queue_size.unwrap_or(16),
        backpressure_sleep.or(Some(std::time::Duration::from_millis(10))),
    );
    threaded.execute()
}

// ============================================================================
// run_future - Execute Future through unified executor
// ============================================================================

/// Execute a future using the unified executor (WASM — Send required for trait bounds,
/// but no actual cross-thread movement occurs since wasm32 is single-threaded).
#[cfg(not(feature = "multi"))]
pub fn run_future<F>(future: F) -> crate::valtron::GenericResult<Vec<F::Output>>
where
    F: Future + 'static,
    F::Output: 'static,
{
    let task = FutureTask::new(future);
    let iter: crate::valtron::DrivenNonSendRecvIterator<FutureTask<F>> =
        execute_as_task(task, None)?;

    let values_iter = ReadyValues::new(iter);
    let values: Vec<F::Output> = values_iter
        .filter_map(super::super::task::ReadyValue::inner)
        .collect();

    Ok(values)
}

/// Execute a future using the unified executor (native - requires Send).
///
/// WHY: Simplest way to run async code through valtron
/// WHAT: Wraps future in `FutureTask` and executes via unified executor
///
/// Note: This requires the unified executor to be available and properly configured.
#[cfg(feature = "multi")]
pub fn run_future<F>(future: F) -> crate::valtron::GenericResult<Vec<F::Output>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    use crate::valtron::ReadyValues;

    let task = FutureTask::new(future);

    let iter: crate::valtron::DrivenRecvIterator<FutureTask<F>> = execute_as_task(task, None)?;
    let values_iter = ReadyValues::new(iter);
    let values: Vec<F::Output> = values_iter
        .filter_map(super::super::task::ReadyValue::inner)
        .collect();

    Ok(values)
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Wrap a stream into a TaskIterator (WASM — Send required by unified executor trait,
/// but no actual cross-thread movement occurs since wasm32 is single-threaded).
#[cfg(not(feature = "multi"))]
pub fn from_stream<S>(stream: S) -> StreamTask<S>
where
    S: futures_core::Stream + 'static,
    S::Item: 'static,
{
    StreamTask::new(stream)
}

/// Wrap a stream into a `TaskIterator` (native - requires Send).
///
/// WHY: Convenient helper to create `StreamTask`
/// WHAT: Returns `StreamTask` wrapping the given stream
#[cfg(feature = "multi")]
pub fn from_stream<S>(stream: S) -> StreamTask<S>
where
    S: futures_core::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    StreamTask::new(stream)
}

/// Wrap a future into a `TaskIterator` (wasm32 - no Send required).
///
/// WHY: `JsFuture` and other wasm-bindgen types are `!Send`. On wasm32 there is
/// only one thread so Send is structurally safe but the types don't implement it.
/// WHAT: Returns `FutureTask` without Send bounds for use with `drive_non_send_iterator`.
#[cfg(not(feature = "multi"))]
pub fn from_future<F>(future: F) -> FutureTask<F>
where
    F: Future + 'static,
    F::Output: 'static,
{
    FutureTask::new(future)
}

/// Wrap a future into a `TaskIterator` (native - requires Send).
///
/// WHY: Convenient helper to create `FutureTask`
/// WHAT: Returns `FutureTask` wrapping the given future
#[cfg(feature = "multi")]
pub fn from_future<F>(future: F) -> FutureTask<F>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    FutureTask::new(future)
}

// ===========================================
// Future based iterators methods
// ===========================================

/// `drive_future` creates a new [`DrivenSendTaskIterator<FutureTask<F>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with `run_until`
/// types of function calls and abstract out that portion.
///
/// It relies on the `drive_iter` method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(not(feature = "multi"))]
pub fn drive_future<F>(future: F) -> DrivenNonSendTaskIterator<FutureTask<F>>
where
    F: Future + 'static,
    F::Output: 'static,
{
    drive_non_send_iterator(crate::valtron::from_future(future))
}

/// `from_future` creates a new [`DrivenSendTaskIterator<FutureTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with `run_until`
/// types of function calls and abstract out that portion.
///
/// It relies on the `drive_iter` method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(feature = "multi")]
pub fn drive_future<F>(future: F) -> DrivenSendTaskIterator<FutureTask<F>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    drive_iterator(crate::valtron::from_future(future))
}

/// `from_future_stream` creates a new [`DrivenSendTaskIterator<StreamTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with `run_until`
/// types of function calls and abstract out that portion.
///
/// It relies on the `drive_iter` method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(not(feature = "multi"))]
pub fn drive_future_stream<S>(stream: S) -> DrivenNotSendTaskIterator<StreamTask<S>>
where
    S: futures_core::Stream + 'static,
    S::Item: 'static,
{
    drive_non_send_iterator(crate::valtron::from_stream(stream))
}

/// `drive_future_stream` creates a new [`DrivenSendTaskIterator<StreamTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with `run_until`
/// types of function calls and abstract out that portion.
///
/// It relies on the `drive_iter` method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(feature = "multi")]
pub fn drive_future_stream<S>(stream: S) -> DrivenSendTaskIterator<StreamTask<S>>
where
    S: futures_core::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    drive_iterator(crate::valtron::from_stream(stream))
}

// ===========================================
// inline iterator creation methods
// ===========================================

/// [`inlined_mapped_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using `BoxedTaskAction`
///
/// You then are able to receive the output of that task from the returned
/// channel [`RecvIterator<TaskStatus<Done, Pending, Action>>`].
///
/// This is predominantly when you specifically do not want a Send action and receiver type.
#[cfg(not(feature = "multi"))]
pub fn inlined_mapped_task<Done, Pending, Action, Task, Mapper>(
    behaviour: InlineActionBehaviour,
    mappers: Vec<Mapper>,
    task: Task,
    wait_cycle: std::time::Duration,
) -> (
    InlineAction<Done, Pending, Action, Task, Mapper>,
    DrivenNonSendRecvIterator<Task>,
)
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    let (task_action, task_receiver) = InlineAction::new(behaviour, mappers, task, wait_cycle);
    (task_action, drive_non_send_receiver(task_receiver))
}

/// [`inlined_mapped_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using `BoxedTaskAction`
///
/// You then are able to receive the output of that task from the returned
/// channel [`RecvIterator<TaskStatus<Done, Pending, Action>>`].
#[cfg(feature = "multi")]
pub fn inlined_mapped_task<Done, Pending, Action, Task, Mapper>(
    behaviour: InlineSendActionBehaviour,
    mappers: Vec<Mapper>,
    task: Task,
    wait_cycle: std::time::Duration,
) -> (
    InlineSendAction<Done, Pending, Action, Task, Mapper>,
    DrivenRecvIterator<Task>,
)
where
    Done: Send + 'static,
    Pending: Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    let (task_action, task_receiver) = InlineSendAction::new(behaviour, mappers, task, wait_cycle);
    (task_action, drive_receiver(task_receiver))
}

/// [`inlined_non_send_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using boxed [`TaskStatusMapper`].
///
/// You then are able to receive the output of that task from the returned
/// channel [`RecvIterator<TaskStatus<Done, Pending, Action>>`].
///
/// This is predominantly when you specifically do not want a Send action and receiver type.
#[cfg(not(feature = "multi"))]
pub fn inlined_task<Done, Pending, Action, Task>(
    behaviour: InlineActionBehaviour,
    mappers: Vec<Box<dyn TaskStatusMapper<Done, Pending, Action> + 'static>>,
    task: Task,
    wait_cycle: std::time::Duration,
) -> (
    InlineAction<
        Done,
        Pending,
        Action,
        Task,
        Box<dyn TaskStatusMapper<Done, Pending, Action> + 'static>,
    >,
    DrivenNonSendRecvIterator<Task>,
)
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    let (task_action, task_receiver) =
        InlineAction::boxed_mapper(behaviour, mappers, task, wait_cycle);
    (task_action, drive_non_send_receiver(task_receiver))
}

/// [`inlined_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using boxed [`TaskStatusMapper`].
///
/// You then are able to receive the output of that task from the returned
/// channel [`RecvIterator<TaskStatus<Done, Pending, Action>>`].
#[cfg(feature = "multi")]
pub fn inlined_task<Done, Pending, Action, Task>(
    behaviour: InlineSendActionBehaviour,
    mappers: Vec<Box<dyn TaskStatusMapper<Done, Pending, Action> + Send + 'static>>,
    task: Task,
    wait_cycle: std::time::Duration,
) -> (
    InlineSendAction<
        Done,
        Pending,
        Action,
        Task,
        Box<dyn TaskStatusMapper<Done, Pending, Action> + Send + 'static>,
    >,
    DrivenRecvIterator<Task>,
)
where
    Done: Send + 'static,
    Pending: Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    let (task_action, task_receiver) =
        InlineSendAction::boxed_mapper(behaviour, mappers, task, wait_cycle);
    (task_action, drive_receiver(task_receiver))
}

// ===========================================
// Iterator driver methods
// ===========================================

/// [`drive_non_send_iterator`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_non_send_iterator<T>(incoming: T) -> DrivenNonSendTaskIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    DrivenNonSendTaskIterator::new(incoming)
}

/// [`drive_iterator`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_iterator<T>(incoming: T) -> DrivenSendTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    DrivenSendTaskIterator::new(incoming)
}

/// [`drive_non_send_receiver`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_non_send_receiver<T>(
    incoming: NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
) -> DrivenNonSendRecvIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    DrivenNonSendRecvIterator::new(incoming)
}

/// [`drive_receiver`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_receiver<T>(
    incoming: NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
) -> DrivenRecvIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    DrivenRecvIterator::new(incoming)
}

/// [`drive_non_send_stream`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_non_send_stream<T>(
    incoming: NotifyQueueStreamIterator<T::Ready, T::Pending>,
) -> DrivenNonSendStreamIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    DrivenNonSendStreamIterator::new(incoming)
}

/// [`drive_stream`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_stream<T>(
    incoming: NotifyQueueStreamIterator<T::Ready, T::Pending>,
) -> DrivenStreamIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    DrivenStreamIterator::new(incoming)
}
