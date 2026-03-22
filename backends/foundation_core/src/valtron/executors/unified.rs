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
    drive_receiver, drive_stream, DrivenRecvIterator, DrivenStreamIterator, ExecutionAction,
    TaskIterator,
};

use crate::synca::mpp::Stream;
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
        tracing::debug!("Executing as a single stream in wasm");
        execute_single_as_task(task, wait_cycle)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        {
            tracing::debug!("Executing as a multi-threaded stream in no-wasm");
            execute_multi_as_task(task, wait_cycle)
        }

        #[cfg(not(feature = "multi"))]
        {
            tracing::debug!("Executing as a single-threaded stream in no-wasm");
            execute_single_as_task(task, wait_cycle)
        }
    }
}

/// [`execute_stream`] unlike [`execute_as_task`] returns a [`StreamRecvIterator`]
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
pub fn send<T>(task: T) -> GenericResult<()>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        tracing::debug!("Executing as a single stream in wasm");
        execute_single(task)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        {
            tracing::debug!("Executing as a multi-threaded stream in no-wasm");
            execute_multi(task)
        }

        #[cfg(not(feature = "multi"))]
        {
            tracing::debug!("Executing as a single-threaded stream in no-wasm");
            execute_single(task)
        }
    }
}

/// Execute using single-threaded executor.
///
/// WHY: WASM and minimal builds need single-threaded execution
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[allow(clippy::type_complexity)]
fn execute_single_as_task<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<DrivenRecvIterator<T>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use super::single;

    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(drive_receiver(iter))
}

#[allow(clippy::type_complexity)]
fn execute_single<T>(task: T) -> GenericResult<()>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use super::single;

    // Schedule task and get iterator
    single::spawn().with_task(task).schedule()?;

    Ok(())
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
    use super::single;
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
fn execute_multi_as_task<T>(
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

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(task: T) -> GenericResult<()>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use crate::valtron::multi;

    // Schedule task and get iterator
    multi::spawn().with_task(task).schedule()?;

    Ok(())
}

// ============================================================================
// Feature 03: Collection Combinators
// ============================================================================

/// Execute multiple TaskIterators in parallel and collect their results.
///
/// This function takes a vector of TaskIterators, executes them in parallel
/// using `execute()`, and returns a `CollectAllStream` that aggregates their
/// outputs. The returned iterator yields `Stream<Vec<D>, P>` variants:
/// - `Stream::Pending(count)` while any sources are still pending
/// - `Stream::Next(Vec<D>)` when all sources complete
/// - `Stream::Delayed(duration)` if any source is delayed
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `wait_cycle` - Optional polling duration (defaults to DEFAULT_WAIT_CYCLE)
///
/// # Returns
///
/// Returns a `CollectAllStream` that aggregates outputs from all tasks.
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
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(CollectAllStream::new(streams))
}

/// Collects outputs from multiple TaskIterators executed via `execute()`.
///
/// This type holds the `DrivenStreamIterator`s returned from `execute()` and
/// polls them in round-robin fashion, yielding `Stream::Pending` while any
/// sources are pending, and `Stream::Next(Vec<D>)` when all complete.
pub struct CollectAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    sources: Vec<DrivenStreamIterator<T>>,
    collected: Vec<T::Ready>,
    done: bool,
}

impl<T> CollectAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    /// Create a new `CollectAllStream` from a vector of `DrivenStreamIterator`s.
    pub fn new(sources: Vec<DrivenStreamIterator<T>>) -> Self {
        Self {
            sources,
            collected: Vec::new(),
            done: false,
        }
    }
}

impl<T> Iterator for CollectAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    type Item = Stream<Vec<T::Ready>, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut all_done = true;
        let mut has_pending = false;
        let mut max_delayed: Option<std::time::Duration> = None;

        // Poll all sources in round-robin
        for source in &mut self.sources {
            match source.next() {
                Some(Stream::Next(value)) => {
                    self.collected.push(value);
                    all_done = false;
                }
                Some(Stream::Pending(_)) => {
                    all_done = false;
                    has_pending = true;
                }
                Some(Stream::Delayed(d)) => {
                    all_done = false;
                    max_delayed = Some(match max_delayed {
                        Some(current) => current.max(d),
                        None => d,
                    });
                }
                Some(Stream::Init) => {
                    all_done = false;
                }
                Some(Stream::Ignore) => {
                    // Ignore internal events, keep collecting
                    all_done = false;
                }
                None => {
                    // This source is exhausted
                }
            }
        }

        if all_done {
            // All sources exhausted, yield collected results
            self.done = true;
            if self.collected.is_empty() {
                return None;
            }
            Some(Stream::Next(std::mem::take(&mut self.collected)))
        } else if let Some(delay) = max_delayed {
            Some(Stream::Delayed(delay))
        } else if has_pending {
            Some(Stream::Pending(self.collected.len()))
        } else {
            // Still collecting, return pending with count
            Some(Stream::Pending(self.collected.len()))
        }
    }
}

impl<T> crate::synca::mpp::StreamIterator<Vec<T::Ready>, usize> for CollectAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
}

// ============================================================================
// Feature 04: Mapping Combinators
// ============================================================================

/// Execute multiple TaskIterators and apply a mapper when all complete.
///
/// This function takes a vector of TaskIterators and a mapper function,
/// executes them in parallel, and applies the mapper only when all sources
/// have produced their values. The returned iterator yields:
/// - `Stream::Pending(count)` while any sources are still pending
/// - `Stream::Next(O)` when all sources complete and mapper is applied
/// - `Stream::Delayed(duration)` if any source is delayed
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `mapper` - Function that transforms `Vec<T::Ready>` into output type `O`
/// * `wait_cycle` - Optional polling duration (defaults to DEFAULT_WAIT_CYCLE)
///
/// # Returns
///
/// Returns a `MapAllDoneStream` that applies the mapper when all complete.
///
/// # Example
///
/// ```ignore
/// let tasks = vec![task1, task2, task3];
/// let merged = execute_map_all(tasks, |results| {
///     results.into_iter().flatten().collect::<Vec<_>>()
/// }, None)?;
///
/// for stream_item in merged {
///     match stream_item {
///         Stream::Pending(count) => println!("{count} still pending..."),
///         Stream::Next(merged) => process(merged),
///         Stream::Delayed(dur) => continue,
///     }
/// }
/// ```
pub fn execute_map_all<T, F, O>(
    tasks: Vec<T>,
    mapper: F,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<MapAllDoneStream<T, F, O>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(MapAllDoneStream::new(streams, mapper))
}

/// Maps values from multiple TaskIterators only when all sources reach Done state.
///
/// This type holds the `DrivenStreamIterator`s returned from `execute()` and
/// buffers values as they arrive. When all sources complete, it applies the
/// mapper function to the collected values and yields the result.
pub struct MapAllDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
    sources: Vec<DrivenStreamIterator<T>>,
    mapper: F,
    buffer: Vec<Option<T::Ready>>,
    done: bool,
}

impl<T, F, O> MapAllDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
    /// Create a new `MapAllDoneStream` from sources and a mapper function.
    pub fn new(sources: Vec<DrivenStreamIterator<T>>, mapper: F) -> Self {
        let len = sources.len();
        Self {
            sources,
            mapper,
            buffer: (0..len).map(|_| None).collect(),
            done: false,
        }
    }
}

impl<T, F, O> Iterator for MapAllDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
    type Item = Stream<O, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut all_done = true;
        let mut has_pending = false;
        let mut max_delayed: Option<std::time::Duration> = None;

        for (i, source) in self.sources.iter_mut().enumerate() {
            if self.buffer[i].is_some() {
                continue; // Already have value from this source
            }

            match source.next() {
                Some(Stream::Next(value)) => {
                    self.buffer[i] = Some(value);
                }
                Some(Stream::Pending(_)) => {
                    all_done = false;
                    has_pending = true;
                }
                Some(Stream::Delayed(d)) => {
                    all_done = false;
                    max_delayed = Some(match max_delayed {
                        Some(current) => current.max(d),
                        None => d,
                    });
                }
                Some(Stream::Init) => {
                    all_done = false;
                }
                Some(Stream::Ignore) => {
                    all_done = false;
                }
                None => {
                    // Source exhausted without producing
                }
            }
        }

        // Check if all sources have produced a value
        if self.buffer.iter().all(|x| x.is_some()) {
            self.done = true;
            let values: Vec<T::Ready> = self.buffer.drain(..).filter_map(|x| x).collect();
            let result = (self.mapper)(values);
            return Some(Stream::Next(result));
        }

        if all_done {
            // All sources exhausted but not all produced values
            self.done = true;
            return None;
        }

        if let Some(delay) = max_delayed {
            Some(Stream::Delayed(delay))
        } else if has_pending {
            let collected: usize = self.buffer.iter().filter(|x| x.is_some()).count();
            Some(Stream::Pending(collected))
        } else {
            Some(Stream::Init)
        }
    }
}

impl<T, F, O> crate::synca::mpp::StreamIterator<O, usize> for MapAllDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O + Send + 'static,
    O: Send + 'static,
{
}

/// Execute multiple TaskIterators with state-aware mapping.
///
/// This function takes a vector of TaskIterators and a mapper function that
/// receives the current `Stream<D, P>` state from each source. This enables
/// progress tracking and partial result visibility.
///
/// # Arguments
///
/// * `tasks` - Vector of TaskIterators to execute in parallel
/// * `mapper` - Function that transforms `Vec<Stream<D, P>>` into output type `O`
/// * `wait_cycle` - Optional polling duration (defaults to DEFAULT_WAIT_CYCLE)
///
/// # Returns
///
/// Returns a `MapAllPendingAndDoneStream` that applies the mapper each poll.
///
/// # Example
///
/// ```ignore
/// let tasks = vec![task1, task2, task3];
/// let progress = execute_map_all_pending_and_done(tasks, |states| {
///     let done_count = states.iter().filter(|s| matches!(s, Stream::Next(_))).count();
///     format!("Progress: {}/{} complete", done_count, states.len())
/// }, None)?;
/// ```
pub fn execute_map_all_pending_and_done<T, F, O>(
    tasks: Vec<T>,
    mapper: F,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<MapAllPendingAndDoneStream<T, F, O>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<Stream<T::Ready, T::Pending>>) -> O + Send + 'static,
    O: Send + 'static,
{
    let streams: Vec<DrivenStreamIterator<T>> = tasks
        .into_iter()
        .map(|t| execute(t, wait_cycle))
        .collect::<GenericResult<_>>()?;

    Ok(MapAllPendingAndDoneStream::new(streams, mapper))
}

/// Maps values from multiple TaskIterators with full state visibility.
///
/// This type holds the `DrivenStreamIterator`s and applies the mapper function
/// to the current state of all sources on each poll. This enables progress
/// tracking and state-aware transformations.
pub struct MapAllPendingAndDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<Stream<T::Ready, T::Pending>>) -> O + Send + 'static,
    O: Send + 'static,
{
    sources: Vec<DrivenStreamIterator<T>>,
    mapper: F,
    done: bool,
}

impl<T, F, O> MapAllPendingAndDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<Stream<T::Ready, T::Pending>>) -> O + Send + 'static,
    O: Send + 'static,
{
    /// Create a new `MapAllPendingAndDoneStream` from sources and a mapper.
    pub fn new(sources: Vec<DrivenStreamIterator<T>>, mapper: F) -> Self {
        Self {
            sources,
            mapper,
            done: false,
        }
    }
}

impl<T, F, O> Iterator for MapAllPendingAndDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<Stream<T::Ready, T::Pending>>) -> O + Send + 'static,
    O: Send + 'static,
{
    type Item = Stream<O, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut states: Vec<Stream<T::Ready, T::Pending>> = Vec::with_capacity(self.sources.len());
        let mut all_exhausted = true;

        for source in &mut self.sources {
            match source.next() {
                Some(state) => {
                    states.push(state);
                    all_exhausted = false;
                }
                None => {
                    // Source exhausted
                }
            }
        }

        if states.is_empty() {
            self.done = true;
            return None;
        }

        // Check if all sources have produced Next values
        let all_done = states.iter().all(|s| matches!(s, Stream::Next(_)));
        let pending_count = states
            .iter()
            .filter(|s| !matches!(s, Stream::Next(_)))
            .count();

        if all_done && !all_exhausted {
            // All sources produced values, mapper will produce final result
            self.done = true;
        }

        let result = (self.mapper)(states);
        Some(if all_done {
            Stream::Next(result)
        } else {
            Stream::Pending(pending_count)
        })
    }
}

impl<T, F, O> crate::synca::mpp::StreamIterator<O, usize> for MapAllPendingAndDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<Stream<T::Ready, T::Pending>>) -> O + Send + 'static,
    O: Send + 'static,
{
}
