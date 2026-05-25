#![allow(clippy::type_complexity)]
#![cfg(feature = "multi")]

use crate::valtron::multi;

use crate::valtron::FutureTask;
use crate::valtron::ReadyValues;
use crate::valtron::StreamConfig;
use crate::valtron::StreamTask;
use crate::valtron::TaskStatusMapper;
use crate::valtron::ThreadedValue;
use crate::valtron::{
    collect_one, collect_result, ExecutionAction, GenericResult, NotificationItem,
    NotifyQueueStreamIterator, NotifyRecvIterator, Stream,
    StreamIterator, TaskIterator, TaskStatus,
};
use core::future::Future;

use crate::valtron::{DEFAULT_MAX_TURNS, DEFAULT_PARK_DURATION, DEFAULT_WAIT_CYCLE};

use crate::valtron::{InlineSendAction, InlineSendActionBehaviour};

#[must_use]
pub fn initialize_pool(
    seed_for_rng: u64,
    user_thread_num: Option<usize>,
) -> crate::valtron::multi::PoolGuard {
    tracing::debug!("Starting under feature=multi");
    multi::initialize_pool(seed_for_rng, user_thread_num)
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
    tracing::debug!("Executing as a multi-threaded stream in no-wasm with multi=off");

    use crate::valtron::multi;

    // Schedule task and get iterator
    let iter = multi::spawn()
        .with_task(task)
        .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(drive_receiver(iter))
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
    tracing::debug!("Executing as a multi-threaded stream in no-wasm");

    // Schedule task and get iterator with defaults
    // Note: wait_cycle is not used directly; DEFAULT_PARK_DURATION is used for park_duration
    let iter = multi::spawn().with_task(task).stream_iter_with_config(
        wait_cycle.unwrap_or(DEFAULT_PARK_DURATION),
        DEFAULT_MAX_TURNS,
    )?;

    Ok(drive_stream(iter))
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
    use crate::valtron::multi;

    tracing::debug!("Executing as a multi-threaded stream in no-wasm with config");

    // Schedule task and get iterator with config
    // Note: wait_cycle parameter is used as park_duration in ConcurrentQueueStreamIterator
    let iter = multi::spawn()
        .with_task(task)
        .stream_iter_with_config(config.park_duration, config.max_turns)?;

    Ok(drive_stream(iter))
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
    tracing::debug!("Executing as a multi-threaded stream in no-wasm");

    // Schedule task and get iterator
    multi::spawn().with_task(task).schedule()?;

    Ok(())
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

/// Execute a future using the unified executor (native - requires Send).
///
/// WHY: Simplest way to run async code through valtron
/// WHAT: Wraps future in `FutureTask` and executes via unified executor
///
/// Note: This requires the unified executor to be available and properly configured.
pub fn run_future<F>(future: F) -> crate::valtron::GenericResult<Vec<F::Output>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let task = FutureTask::new(future);

    let iter: DrivenRecvIterator<FutureTask<F>> = execute_as_task(task, None)?;
    let values_iter = ReadyValues::new(iter);
    let values: Vec<F::Output> = values_iter
        .filter_map(super::super::task::ReadyValue::inner)
        .collect();

    Ok(values)
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Wrap a stream into a `TaskIterator` (native - requires Send).
///
/// WHY: Convenient helper to create `StreamTask`
/// WHAT: Returns `StreamTask` wrapping the given stream
pub fn from_stream<S>(stream: S) -> StreamTask<S>
where
    S: futures_core::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    StreamTask::new(stream)
}

/// Wrap a future into a `TaskIterator` (native - requires Send).
///
/// WHY: Convenient helper to create `FutureTask`
/// WHAT: Returns `FutureTask` wrapping the given future
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

/// `from_future` creates a new [`DrivenTaskIterator<FutureTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with `run_until`
/// types of function calls and abstract out that portion.
///
/// It relies on the `drive_iter` method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
pub fn drive_future<F>(future: F) -> DrivenTaskIterator<FutureTask<F>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    drive_iterator(crate::valtron::from_future(future))
}

/// `drive_future_stream` creates a new [`DrivenTaskIterator<StreamTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with `run_until`
/// types of function calls and abstract out that portion.
///
/// It relies on the `drive_iter` method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
pub fn drive_future_stream<S>(stream: S) -> DrivenTaskIterator<StreamTask<S>>
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

/// [`inlined_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using boxed [`TaskStatusMapper`].
///
/// You then are able to receive the output of that task from the returned
/// channel [`RecvIterator<TaskStatus<Done, Pending, Action>>`].
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

/// [`drive_iterator`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_iterator<T>(incoming: T) -> DrivenTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    DrivenTaskIterator::new(incoming)
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

// ============================================================================
// Sync boundary helpers
// ============================================================================

/// WHY: Many operations produce a single result. Callers want `Result<T>`,
/// not `Result<Vec<T>>`.
///
/// WHAT: Schedules a `TaskIterator` on the Valtron executor, blocks until
/// the first `Stream::Next` value, and returns it directly.
///
/// HOW: Calls `execute(task, None)` to schedule, then `collect_one` to
/// extract the first result.
///
/// Use sparingly — prefer returning streams to callers. This is the
/// single-value counterpart to `sync_one` (which returns `Vec`).
///
/// # Errors
///
/// Returns an error if the task could not be scheduled, or if the stream
/// exhausted without producing any `Next` value.
///
/// # Examples
///
/// ```ignore
/// let value: MyValue = sync_collect_one(my_task)?;
/// ```
pub fn sync_collect_one<T>(task: T) -> GenericResult<T::Ready>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    let stream = execute(task, None)?;
    collect_one(stream).ok_or_else(|| "sync_collect_one: stream produced no result".into())
}

/// WHY: Provides an ergonomic sync escape hatch for executing a single task
/// and blocking until all results are collected.
///
/// WHAT: Schedules a `TaskIterator` on the Valtron executor via `execute()`,
/// then drains the returned stream with `collect_result`.
///
/// HOW: Calls `execute(task, None)` to schedule, then `collect_result` to
/// block and collect all `Stream::Next` values.
///
/// Use sparingly — prefer returning streams to callers so they can compose
/// and parallelize. This is appropriate for one-shot initializations,
/// migrations, or CLI tools where there is nothing to parallelize.
///
/// # Errors
///
/// Returns an error if the task could not be scheduled on the executor
/// (e.g., pool not initialized).
///
/// # Examples
///
/// ```ignore
/// let results: Vec<MyValue> = sync_one(my_task)?;
/// ```
pub fn sync_one<T>(task: T) -> GenericResult<Vec<T::Ready>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    let stream = execute(task, None)?;
    Ok(collect_result(stream))
}

/// WHY: Enables parallel execution of multiple homogeneous tasks with a
/// single blocking collection point.
///
/// WHAT: Schedules all tasks in parallel via `execute_collect_all`, then
/// blocks until every task has produced its result.
///
/// HOW: `execute_collect_all` buffers results internally and yields a single
/// `Stream::Next(Vec<T::Ready>)` when all tasks complete. We drain with
/// `collect_result` and flatten into the final `Vec<T::Ready>`.
///
/// For heterogeneous tasks (different `Ready` types), call `execute()`
/// on each individually — they still run in parallel on the pool — then
/// `collect_result` each stream at the boundary.
///
/// # Errors
///
/// Returns an error if any task could not be scheduled on the executor.
///
/// # Examples
///
/// ```ignore
/// let tasks = vec![task_a, task_b, task_c];
/// let all_results: Vec<MyValue> = sync_all(tasks)?;
/// ```
pub fn sync_all<T>(tasks: Vec<T>) -> GenericResult<Vec<T::Ready>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    if tasks.is_empty() {
        return Err("empty tasks not allowed".into());
    }

    let mut stream = execute_collect_all(tasks, None)?;
    // execute_collect_all yields Pending(count) while in flight, then a single
    // Next(Vec<T::Ready>) when all complete. find_map skips Pending items.
    stream
        .find_map(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        })
        .ok_or_else(|| "sync_all: no results produced by execute_collect_all".into())
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
) -> GenericResult<CollectAllTaskStream<T>>
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

    Ok(CollectAllTaskStream::new(streams))
}

// ===========================================
// Iterator Execution Drivenrs: Making seamless call to next without
// bothering about driving execution directly in your business logic.
// ===========================================

///[`DrivenTaskIterator`] is a wrapper around a [`TaskIterator`] that drives the state of the stream.
/// This is good for send-safe types you want to be send-safe.
///
/// It internally uses the [`run_until_next_state`] function.
pub struct DrivenTaskIterator<T>(Option<T>)
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

impl<T> DrivenTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    pub fn new(task_iterator: T) -> Self {
        Self(Some(task_iterator))
    }
}

unsafe impl<T> Send for DrivenTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
}

impl<T> Iterator for DrivenTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    type Item = TaskStatus<T::Ready, T::Pending, T::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            tracing::trace!("Run: run_until_next_state");

            let next_value = task_iterator.next_status();

            // if the next value is Some(_) then set back the
            // executor for the next call.
            if next_value.is_some() {
                self.0.replace(task_iterator);
            }

            next_value
        } else {
            None
        }
    }
}

pub struct DrivenStreamIterator<T>(Option<NotifyQueueStreamIterator<T::Ready, T::Pending>>)
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

impl<T> DrivenStreamIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[must_use]
    pub fn new(task_iterator: NotifyQueueStreamIterator<T::Ready, T::Pending>) -> Self {
        Self(Some(task_iterator))
    }

    pub fn is_empty(&self) -> bool {
        match &self.0 {
            Some(inner) => inner.is_empty(),
            None => true,
        }
    }
}

unsafe impl<T> Send for DrivenStreamIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
}

impl<T> Iterator for DrivenStreamIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    type Item = Stream<T::Ready, T::Pending>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            tracing::trace!("Run: run_until_next_state");

            let next_value = task_iterator.next();

            // if the next value is Some(_) then set back the
            // executor for the next call.
            if next_value.is_some() {
                self.0.replace(task_iterator);
            }

            next_value
        } else {
            None
        }
    }
}

pub struct DrivenRecvIterator<T>(
    Option<NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>,
)
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

impl<T> DrivenRecvIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[must_use]
    pub fn new(
        task_iterator: NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
    ) -> Self {
        Self(Some(task_iterator))
    }
}

// This is safe to send since it contains a type `NotifyRecvIterator`
// which is safe to send.
unsafe impl<T> Send for DrivenRecvIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
}

impl<T> Iterator for DrivenRecvIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    type Item = TaskStatus<T::Ready, T::Pending, T::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            tracing::trace!("Run: run_until_next_state");

            let next_value = task_iterator.next();

            // Handle NotificationItem from NotifyRecvIterator
            match next_value {
                Some(NotificationItem::Ready(status)) => {
                    self.0.replace(task_iterator);
                    Some(status)
                }
                Some(NotificationItem::None) => {
                    // Queue open but empty — signal Wait to executor
                    self.0.replace(task_iterator);
                    Some(TaskStatus::Wait)
                }
                None => None, // Queue closed
            }
        } else {
            None
        }
    }
}

/// Collects outputs from multiple `TaskIterators` executed via `execute()`.
///
/// This type holds the `DrivenStreamIterator`s returned from `execute()` and
/// polls them one at a time, yielding `Stream::Pending` while any
/// sources are pending, and `Stream::Next(Vec<D>)` when all complete.
pub struct CollectAllTaskStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    sources: Vec<DrivenStreamIterator<T>>,
    collected: Vec<T::Ready>,
    done: bool,
    current_index: usize,
}

impl<T> CollectAllTaskStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    /// Create a new `CollectAllTaskStream` from a vector of `DrivenStreamIterator`s.
    #[must_use]
    pub fn new(sources: Vec<DrivenStreamIterator<T>>) -> Self {
        Self {
            sources,
            collected: Vec::new(),
            done: false,
            current_index: 0,
        }
    }
}

impl<T> Iterator for CollectAllTaskStream<T>
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

        if self.sources.is_empty() {
            self.done = true;
            if self.collected.is_empty() {
                return None;
            }
            return Some(Stream::Next(std::mem::take(&mut self.collected)));
        }

        // Poll exactly ONE source per next() call
        let idx = self.current_index;
        match self.sources[idx].next() {
            Some(Stream::Next(value)) => {
                self.collected.push(value);
                self.current_index = (self.current_index + 1) % self.sources.len();
                Some(Stream::Pending(self.sources.len()))
            }
            Some(Stream::Pending(_)) => {
                self.current_index = (self.current_index + 1) % self.sources.len();
                Some(Stream::Pending(self.sources.len()))
            }
            Some(Stream::Delayed(d)) => {
                self.current_index = (self.current_index + 1) % self.sources.len();
                Some(Stream::Delayed(d))
            }
            Some(Stream::Init) => {
                self.current_index = (self.current_index + 1) % self.sources.len();
                Some(Stream::Pending(self.sources.len()))
            }
            Some(Stream::Ignore) => {
                self.current_index = (self.current_index + 1) % self.sources.len();
                Some(Stream::Pending(self.sources.len()))
            }
            Some(Stream::Wait) => {
                self.current_index = (self.current_index + 1) % self.sources.len();
                Some(Stream::Pending(self.sources.len()))
            }
            None => {
                // Source exhausted - remove it using swap_remove for O(1) complexity
                self.sources.swap_remove(idx);
                if self.current_index >= self.sources.len() && !self.sources.is_empty() {
                    self.current_index = 0;
                }
                // Check if all done
                if self.sources.is_empty() {
                    self.done = true;
                    if self.collected.is_empty() {
                        return None;
                    }
                    Some(Stream::Next(std::mem::take(&mut self.collected)))
                } else {
                    Some(Stream::Pending(self.sources.len()))
                }
            }
        }
    }
}

// impl<T> crate::synca::mpp::StreamIterator for CollectAllStream<T>
// where
//     T: TaskIterator + Send + 'static,
//     T::Ready: + Send + 'static,
//     T::Pending: + Send + 'static,
//     T::Spawner: ExecutionAction + Send + 'static,
// {
//     type D = Vec<T::Ready>;
//     type P = usize;
// }

// ============================================================================
// StreamIterator collect_all
// ============================================================================

/// Collect results from multiple `StreamIterator`s executed in parallel.
///
/// This function takes a vector of `StreamIterator`s and combines them into
/// a single stream that yields all results when complete. The returned iterator
/// yields:
/// - `Stream::Pending(count)` while any sources are still pending
/// - `Stream::Next(Vec<D>)` when all sources complete
/// - `Stream::Delayed(duration)` if any source is delayed
///
/// # Arguments
///
/// * `streams` - Vector of `StreamIterator`s to execute in parallel
///
/// # Returns
///
/// Returns a `CollectAllStreams` that combines all sources.
///
/// # Example
///
/// ```ignore
/// let streams = vec![stream1, stream2, stream3];
/// let combined = collect_all_streams(streams);
///
/// for item in combined {
///     match item {
///         Stream::Pending(count) => println!("{count} still pending..."),
///         Stream::Next(results) => process(results),
///         Stream::Delayed(dur) => continue,
///     }
/// }
/// ```
pub fn collect_all_streams<S>(streams: Vec<S>) -> CollectAllStreams<S>
where
    S: StreamIterator<P = usize> + Send + 'static,
    S::D: Send + 'static,
{
    CollectAllStreams::new(streams)
}

/// Create a StreamIterator that yields a single value and then completes.
///
/// This is useful for creating error streams or wrapping a single result
/// in a StreamIterator for combinator chains.
///
/// # Arguments
///
/// * `value` - The value to yield as Stream::Next
///
/// # Returns
///
/// Returns a OneShotStream that yields the value once then completes.
///
/// # Example
///
/// ```ignore
/// let stream = one_shot(Ok::<_, MyError>(path_buf));
/// for item in stream {
///     match item {
///         Stream::Next(v) => println!("Got value: {:?}", v),
///         _ => {}
///     }
/// }
/// ```
pub fn one_shot<D, P>(value: D) -> OneShotStream<D, P>
where
    D: Send + 'static,
    P: Send + 'static,
{
    OneShotStream {
        value: Some(value),
        _phantom: std::marker::PhantomData,
    }
}

/// A StreamIterator that yields a single value and then completes.
pub struct OneShotStream<D, P> {
    value: Option<D>,
    _phantom: std::marker::PhantomData<P>,
}

impl<D, P> Iterator for OneShotStream<D, P>
where
    D: Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.value.take().map(Stream::Next)
    }
}

// Note: StreamIterator impl is provided by the blanket impl in synca/mpp.rs

/// Collects outputs from multiple `StreamIterator`s in parallel.
///
/// This type holds the `StreamIterator`s and polls them one at a time,
/// yielding `Stream::Pending` while any sources are pending, and
/// `Stream::Next(Vec<D>)` when all complete.
pub struct CollectAllStreams<S>
where
    S: StreamIterator<P = usize> + Send + 'static,
    S::D: Send + 'static,
{
    sources: Vec<S>,
    collected: Vec<S::D>,
    done: bool,
    current_index: usize,
}

impl<S> CollectAllStreams<S>
where
    S: StreamIterator<P = usize> + Send + 'static,
    S::D: Send + 'static,
{
    /// Create a new `CollectAllStreams` from a vector of `StreamIterator`s.
    #[must_use]
    pub fn new(sources: Vec<S>) -> Self {
        Self {
            sources,
            collected: Vec::new(),
            done: false,
            current_index: 0,
        }
    }
}

impl<S> Iterator for CollectAllStreams<S>
where
    S: StreamIterator<P = usize> + Send + 'static,
    S::D: Send + 'static,
{
    type Item = Stream<Vec<S::D>, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if self.sources.is_empty() {
            self.done = true;
            if self.collected.is_empty() {
                return None;
            }
            return Some(Stream::Next(std::mem::take(&mut self.collected)));
        }

        let mut exhausted_indices = Vec::new();

        // One pass through sources starting at current_index
        for _ in 0..self.sources.len() {
            let idx = self.current_index;
            match self.sources[idx].next() {
                Some(Stream::Next(value)) => {
                    self.collected.push(value);
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Pending(self.sources.len()));
                }
                Some(Stream::Pending(_)) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Pending(self.sources.len()));
                }
                Some(Stream::Delayed(d)) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Delayed(d));
                }
                Some(Stream::Init) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Pending(self.sources.len()));
                }
                Some(Stream::Ignore) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    // Continue to next source in same pass
                }
                Some(Stream::Wait) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    // Continue to next source in same pass
                }
                None => {
                    exhausted_indices.push(idx);
                    self.current_index = (self.current_index + 1) % self.sources.len();
                }
            }
        }

        // Remove exhausted sources after the pass
        // Sort in descending order so swap_remove doesn't invalidate subsequent indices
        exhausted_indices.sort_by(|a, b| b.cmp(a));
        for idx in exhausted_indices {
            self.sources.swap_remove(idx);
        }

        // Adjust current_index if it now points beyond the new length
        if !self.sources.is_empty() && self.current_index >= self.sources.len() {
            self.current_index = 0;
        }

        if self.sources.is_empty() {
            self.done = true;
            if self.collected.is_empty() {
                return None;
            }
            return Some(Stream::Next(std::mem::take(&mut self.collected)));
        }

        Some(Stream::Pending(self.sources.len()))
    }
}
// Note: StreamIterator is auto-implemented via blanket impl for Iterator<Item = Stream<D, P>>

// ============================================================================
// Feature 04: Mapping Combinators
// ============================================================================

/// Execute multiple `TaskIterators` and apply a mapper when all complete.
///
/// This function takes a vector of `TaskIterators` and a mapper function,
/// executes them in parallel, and applies the mapper only when all sources
/// have produced their values. The returned iterator yields:
/// - `Stream::Pending(count)` while any sources are still pending
/// - `Stream::Next(O)` when all sources complete and mapper is applied
/// - `Stream::Delayed(duration)` if any source is delayed
///
/// # Arguments
///
/// * `tasks` - Vector of `TaskIterators` to execute in parallel
/// * `mapper` - Function that transforms `Vec<T::Ready>` into output type `O`
/// * `wait_cycle` - Optional polling duration (defaults to `DEFAULT_WAIT_CYCLE`)
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

/// Maps values from multiple `TaskIterators` only when all sources reach Done state.
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
{
    sources: Vec<DrivenStreamIterator<T>>,
    mapper: F,
    buffer: Vec<Option<T::Ready>>,
    done: bool,
    _phantom: std::marker::PhantomData<O>,
}

impl<T, F, O> MapAllDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    /// Create a new `MapAllDoneStream` from sources and a mapper function.
    pub fn new(sources: Vec<DrivenStreamIterator<T>>, mapper: F) -> Self {
        let len = sources.len();
        Self {
            sources,
            mapper,
            buffer: (0..len).map(|_| None).collect(),
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F, O> Iterator for MapAllDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<T::Ready>) -> O,
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
                Some(Stream::Wait) => {
                    all_done = false;
                }
                None => {
                    // Source exhausted without producing
                }
            }
        }

        // Check if all sources have produced a value
        if self.buffer.iter().all(std::option::Option::is_some) {
            self.done = true;
            let values: Vec<T::Ready> = self.buffer.drain(..).flatten().collect();
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

// impl<T, F, O> crate::synca::mpp::StreamIterator for MapAllDoneStream<T, F, O>
// where
//     T: TaskIterator + Send + 'static,
//     T::Ready: + Send + 'static,
//     T::Pending: + Send + 'static,
//     T::Spawner: ExecutionAction + Send + 'static,
//     F: Fn(Vec<T::Ready>) -> O + Send + 'static,
//     O: + Send + 'static,
// {
//     type D = O;
//     type P = usize;
// }

/// Execute multiple `TaskIterators` with state-aware mapping.
///
/// This function takes a vector of `TaskIterators` and a mapper function that
/// receives the current `Stream<D, P>` state from each source. This enables
/// progress tracking and partial result visibility.
///
/// # Arguments
///
/// * `tasks` - Vector of `TaskIterators` to execute in parallel
/// * `mapper` - Function that transforms `Vec<Stream<D, P>>` into output type `O`.
///   **Warning:** The vector only contains states from active sources. When a
///   source completes, it is removed from the vector. Do not rely on positional
///   indexing - the position of a source in this vector changes as other sources
///   complete.
/// * `wait_cycle` - Optional polling duration (defaults to `DEFAULT_WAIT_CYCLE`)
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

/// Maps values from multiple `TaskIterators` with full state visibility.
///
/// This type holds the `DrivenStreamIterator`s and applies the mapper function
/// to the current state of all sources on each poll. This enables progress
/// tracking and state-aware transformations.
///
/// # Important Note on Positional Indexing
///
/// The mapper receives a `Vec<Stream<T::Ready, T::Pending>>` containing only
/// the states of active (non-exhausted) sources. When a source completes,
/// it is removed from this vector. **Do not rely on positional indexing**
/// into this vector - if you need to track individual source progress,
/// use unique identifiers in your task outputs.
///
/// For example, if source at index 1 completes before source at index 0,
/// the mapper will receive a 1-element vector on the next poll:
/// - Before: `[Stream::Next(0), Stream::Next(1)]` (2 elements)
/// - After source 1 completes: `[Stream::Next(0)]` (1 element, was at index 0)
pub struct MapAllPendingAndDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    sources: Vec<DrivenStreamIterator<T>>,
    mapper: F,
    done: bool,
    _phantom: std::marker::PhantomData<O>,
}

impl<T, F, O> MapAllPendingAndDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    /// Create a new `MapAllPendingAndDoneStream` from sources and a mapper.
    pub fn new(sources: Vec<DrivenStreamIterator<T>>, mapper: F) -> Self {
        Self {
            sources,
            mapper,
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T, F, O> Iterator for MapAllPendingAndDoneStream<T, F, O>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
    F: Fn(Vec<Stream<T::Ready, T::Pending>>) -> O,
{
    type Item = Stream<O, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut states: Vec<Stream<T::Ready, T::Pending>> = Vec::with_capacity(self.sources.len());
        let mut all_exhausted = true;

        for source in &mut self.sources {
            if let Some(state) = source.next() {
                states.push(state);
                all_exhausted = false;
            } else {
                // Source exhausted
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

// impl<T, F, O> crate::synca::mpp::StreamIterator for MapAllPendingAndDoneStream<T, F, O>
// where
//     T: TaskIterator + Send + 'static,
//     T::Ready: + Send + 'static,
//     T::Pending: + Send + 'static,
//     T::Spawner: ExecutionAction + Send + 'static,
//     F: Fn(Vec<Stream<T::Ready, T::Pending>>) -> O + Send + 'static,
//     O: + Send + 'static,
// {
//     type D = O;
//     type P = usize;
// }

// ============================================================================
// Collect Next From All (New Feature)
// ============================================================================

/// Execute multiple `TaskIterator`s and yield individual values as they arrive.
///
/// Unlike `execute_collect_all` which buffers all values and returns them when complete,
/// this function yields each `Stream::Next(D)` value individually as soon as any task
/// produces it. When a task is exhausted (returns `None`), it is removed from the pool.
/// Yields `Stream::Pending(count)` while any tasks are still pending.
/// Returns `None` when all tasks are exhausted.
///
/// # Arguments
///
/// * `tasks` - Vector of `TaskIterator`s to execute in parallel
/// * `wait_cycle` - Optional polling duration (defaults to `DEFAULT_WAIT_CYCLE`)
///
/// # Returns
///
/// Returns a `CollectNextFromAllStream` that yields values as they arrive.
///
/// # Example
///
/// ```ignore
/// let tasks = vec![task1, task2, task3];
/// let stream = execute_collect_next_from_all(tasks, None)?;
///
/// for item in stream {
///     match item {
///         Stream::Next(value) => process_immediately(value),
///         Stream::Pending(count) => println!("{count} tasks still running..."),
///         Stream::Delayed(dur) => continue,
///     }
/// }
/// ```
#[cfg(feature = "multi")]
pub fn execute_collect_next_from_all<T>(
    tasks: Vec<T>,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<CollectNextFromAllStream<T>>
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

    Ok(CollectNextFromAllStream::new(streams))
}

/// Collects outputs from multiple `TaskIterator`s executed via `execute()`,
/// yielding individual values as they arrive and removing completed streams.
///
/// Unlike `CollectAllStream` which buffers all values and returns them when complete,
/// this type yields each `Next(D)` value individually as soon as any stream produces it.
/// When a stream is exhausted (returns `None`), it is removed from the pool.
/// Yields `Stream::Pending(count)` while any streams are still pending.
/// Returns `None` when all streams are exhausted.
pub struct CollectNextFromAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    sources: Vec<DrivenStreamIterator<T>>,
    current_index: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> CollectNextFromAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    /// Create a new `CollectNextFromAllStream` from a vector of `DrivenStreamIterator`s.
    #[must_use]
    pub fn new(sources: Vec<DrivenStreamIterator<T>>) -> Self {
        Self {
            sources,
            current_index: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> Iterator for CollectNextFromAllStream<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    type Item = Stream<T::Ready, usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sources.is_empty() {
            return None;
        }

        let mut exhausted_indices = Vec::new();

        // One pass through sources starting at current_index
        for _ in 0..self.sources.len() {
            let idx = self.current_index;
            match self.sources[idx].next() {
                Some(Stream::Next(value)) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Next(value));
                }
                Some(Stream::Pending(_)) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Pending(self.sources.len()));
                }
                Some(Stream::Delayed(d)) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Delayed(d));
                }
                Some(Stream::Init) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    return Some(Stream::Pending(self.sources.len()));
                }
                Some(Stream::Ignore) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    // Continue to next source in same pass
                }
                Some(Stream::Wait) => {
                    self.current_index = (self.current_index + 1) % self.sources.len();
                    // Continue to next source in same pass
                }
                None => {
                    exhausted_indices.push(idx);
                    self.current_index = (self.current_index + 1) % self.sources.len();
                }
            }
        }

        // Remove exhausted sources after the pass
        // Sort in descending order so swap_remove doesn't invalidate subsequent indices
        exhausted_indices.sort_by(|a, b| b.cmp(a));
        for idx in exhausted_indices {
            self.sources.swap_remove(idx);
        }

        // Adjust current_index if it now points beyond the new length
        if !self.sources.is_empty() && self.current_index >= self.sources.len() {
            self.current_index = 0;
        }

        if self.sources.is_empty() {
            return None;
        }

        Some(Stream::Pending(self.sources.len()))
    }
}
