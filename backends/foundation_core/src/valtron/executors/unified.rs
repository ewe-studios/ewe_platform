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

use crate::valtron::{
    drive_receiver, drive_stream, single, DrivenRecvIterator, DrivenStreamIterator,
    ExecutionAction, TaskIterator,
};

use crate::valtron::GenericResult;

pub const DEFAULT_WAIT_CYCLE: std::time::Duration = std::time::Duration::from_millis(10);

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
/// Returns a [`GenericResult`] wrapping a [`RecvIterator`] over [`TaskStatus`]. On success the
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
pub fn execute<T>(
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
        tracing::debug!("Executing as a single stream in wasm");
        execute_single(task, wait_cycle)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        {
            tracing::debug!("Executing as a multi-threaded stream in no-wasm");
            execute_multi(task, wait_cycle)
        }

        #[cfg(not(feature = "multi"))]
        {
            tracing::debug!("Executing as a single-threaded stream in no-wasm");
            execute_single(task, wait_cycle)
        }
    }
}

/// [`execute_stream`] unlike [`execute`] returns a [`StreamRecvIterator`]
/// which hides the underlying mechanics of handling [`TaskStatus`]. The stream
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
/// Returns a [`GenericResult`] wrapping a [`StreamRecvIterator`] over the task's
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
/// The concrete error type is the one used by [`GenericResult`] in this crate
/// and will contain additional context about the failure.
///
/// # Panics
///
/// This function does not intentionally panic. However, panics may occur if:
/// - The provided task implementation panics while being polled or executed by
///   the executor.
/// - The underlying executor implementation or thread pool panics internally.
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
pub fn execute_stream<T>(
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
        tracing::debug!("Executing as a single stream in wasm");
        execute_single_stream(task, wait_cycle)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        {
            tracing::debug!("Executing as a multi-threaded stream in no-wasm");
            execute_multi_stream(task, wait_cycle)
        }

        #[cfg(not(feature = "multi"))]
        {
            tracing::debug!("Executing as a single-threaded stream in no-wasm");
            execute_single_stream(task, wait_cycle)
        }
    }
}

/// Execute using single-threaded executor.
///
/// WHY: WASM and minimal builds need single-threaded execution
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[allow(clippy::type_complexity)]
fn execute_single<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenRecvIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(drive_receiver(iter))
}

/// Execute using single-threaded executor returning a stream iterator.
///
/// WHY: WASM and minimal builds need single-threaded execution
/// WHAT: Schedules task, returns a stream iterator.
#[allow(clippy::type_complexity)]
fn execute_single_stream<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenStreamIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .scheduled_stream_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(drive_stream(iter))
}

/// Execute using multi-threaded executor.
///
/// WHY: Native builds can use multiple threads for better performance
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenRecvIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use crate::valtron::multi;

    // Schedule task and get iterator
    let iter = multi::spawn()
        .with_task(task)
        .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(drive_receiver(iter))
}

/// Execute using multi-threaded executor.
///
/// WHY: Native builds can use multiple threads for better performance
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi_stream<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenStreamIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use crate::valtron::multi;

    // Schedule task and get iterator
    let iter = multi::spawn()
        .with_task(task)
        .stream_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(drive_stream(iter))
}
