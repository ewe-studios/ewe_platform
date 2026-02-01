//! HTTP request executor wrapper for valtron execution.
//!
//! WHY: Provides a unified interface for executing HTTP request tasks that auto-selects
//! the appropriate executor (single-threaded or multi-threaded) based on platform
//! and feature flags. Abstracts executor complexity from HTTP client users.
//!
//! WHAT: Implements `execute_task()` function that spawns HTTP request tasks and returns
//! an iterator of task status updates. Handles platform differences (WASM vs native)
//! and feature-based selection (multi-threading support).
//!
//! HOW: Uses valtron's unified executor interface with `schedule_iter()` to spawn tasks.
//! Returns `RecvIterator<TaskStatus>` which users can poll for progress.
//! CRITICAL: In single mode (WASM or multi=off), users MUST call `run_once/run_until_complete`.
//! In multi mode (multi=on), executor threads run automatically.

use crate::synca::mpp::RecvIterator;
use crate::valtron::{single, ExecutionAction, GenericResult, TaskIterator, TaskStatus};
use std::time::Duration;

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
use crate::valtron::multi;

/// Execute an HTTP request task using the appropriate executor.
///
/// WHY: Provides single entry point for HTTP request execution that works across
/// all platforms and configurations. Simplifies HTTP client implementation.
///
/// WHAT: Spawns the task using the platform-appropriate executor and returns an
/// iterator of task status updates. The iterator yields `TaskStatus` variants
/// (Pending, Ready, Spawn, etc.) as the task progresses.
///
/// HOW: Uses valtron's unified executor with `schedule_iter()` for non-blocking
/// task spawning. Returns immediately with an iterator handle.
///
/// # Platform Selection
///
/// | Platform | Feature | Executor Used | User Action Required |
/// |----------|---------|---------------|---------------------|
/// | WASM     | any     | `single`      | Call `run_once()` or `run_until_complete()` |
/// | Native   | none    | `single`      | Call `run_once()` or `run_until_complete()` |
/// | Native   | `multi` | `multi`       | None - threads run automatically |
///
/// # Type Parameters
///
/// * `T` - Task type implementing `TaskIterator`
///
/// # Arguments
///
/// * `task` - The HTTP request task to execute
///
/// # Returns
///
/// An iterator of `TaskStatus` updates that can be polled for progress.
/// Use `ReadyValues::new(iter)` to filter for Ready values only.
///
/// # Errors
///
/// Returns error if task spawning fails.
///
/// # Examples
///
/// ```ignore
/// // Single-threaded mode (WASM or multi=off)
/// let task = HttpRequestTask::new(request, resolver, 5);
/// let iter = execute_task(task)?;
/// single::run_once(); // MUST call to drive execution
/// let ready_values = ReadyValues::new(iter);
///
/// // Multi-threaded mode (multi=on)
/// let task = HttpRequestTask::new(request, resolver, 5);
/// let iter = execute_task(task)?;
/// // Threads run automatically - just consume iterator
/// let ready_values = ReadyValues::new(iter);
/// ```
pub fn execute_task<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
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

/// Execute using single-threaded executor.
///
/// WHY: WASM and minimal builds need single-threaded execution. This function
/// provides explicit single-threaded execution path.
///
/// WHAT: Schedules task using `single::spawn()` with a short polling interval.
/// Returns iterator immediately - caller must drive execution with `run_once()`.
///
/// HOW: Uses `schedule_iter()` with 5 nanosecond interval for tight polling.
/// The interval represents how often the executor checks for new work.
///
/// CRITICAL: Caller MUST call `single::run_once()` or `single::run_until_complete()`
/// to actually drive the executor forward. Without this, the task will not progress.
fn execute_single<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(5))?;

    Ok(iter)
}

/// Execute using multi-threaded executor.
///
/// WHY: Native builds with multi feature can use multiple threads for better
/// performance and concurrent request handling.
///
/// WHAT: Schedules task using `multi::spawn()` with a minimal polling interval.
/// Executor threads run automatically - no user action required.
///
/// HOW: Uses schedule_iter() with 1 nanosecond interval for maximum responsiveness.
/// Background threads automatically drive execution.
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Schedule task and get iterator
    let iter = multi::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(1))?;

    Ok(iter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valtron::{initialize_pool, NoAction, ReadyValues};

    /// Simple test task that yields a single value
    struct SimpleTask {
        value: Option<i32>,
    }

    impl TaskIterator for SimpleTask {
        type Pending = ();
        type Ready = i32;
        type Spawner = NoAction;

        fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            self.value.take().map(TaskStatus::Ready)
        }
    }

    // ========================================================================
    // Execute Function Tests
    // ========================================================================

    /// WHY: execute_task() must work on WASM (single executor only)
    /// WHAT: Function compiles and has correct signature for WASM
    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_execute_task_available_on_wasm() {
        initialize_pool(20, None);

        let task = SimpleTask { value: Some(42) };
        let status_iter = execute_task(task).expect("should create task");

        single::run_until_complete();

        let ready_values = ReadyValues::new(status_iter);
        let values: Vec<i32> = ready_values.filter_map(|item| item.inner()).collect();
        assert_eq!(values, vec![42]);
    }

    /// WHY: execute_task() must work on native without multi feature (single executor)
    /// with run_once() for controlled execution
    /// WHAT: Function uses single executor and gets next value with single::run_once()
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    fn test_execute_task_uses_single_with_run_once() {
        initialize_pool(20, None);

        let task = SimpleTask { value: Some(42) };
        let mut values_iter = ReadyValues::new(execute_task(task).expect("should create task"));

        // Drive executor forward one step
        let _ = single::run_once();

        // Get next ready value
        let value = values_iter.next();
        let inner = value.expect("should have value").inner();

        assert_eq!(inner, Some(42));
    }

    /// WHY: execute_task() must work on native without multi feature (single executor)
    /// with run_until_complete() for full execution
    /// WHAT: Function uses single executor and completes all work
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    fn test_execute_task_uses_single_with_run_until_complete() {
        initialize_pool(20, None);

        let task = SimpleTask { value: Some(42) };
        let values_iter = ReadyValues::new(execute_task(task).expect("should create task"));

        // Run until all work completes
        single::run_until_complete();

        // Collect all ready values
        let values: Vec<i32> = values_iter.filter_map(|item| item.inner()).collect();
        assert_eq!(values, vec![42]);
    }

    /// WHY: execute_task() must work on native with multi feature (multi executor)
    /// WHAT: Function uses multi executor with automatic thread execution
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
    fn test_execute_task_uses_multi() {
        initialize_pool(20, None);

        let task = SimpleTask { value: Some(42) };
        let values_iter = ReadyValues::new(execute_task(task).expect("should create task"));

        // Threads run automatically - just collect values
        let values: Vec<i32> = values_iter.filter_map(|item| item.inner()).collect();
        assert_eq!(values, vec![42]);
    }

    /// WHY: execute_task() signature must match TaskIterator trait requirements
    /// WHAT: Verify Send + 'static bounds are correct
    #[test]
    fn test_execute_task_signature_accepts_task_iterator() {
        // This is a compile-time test
        fn _assert_compiles<T>(_task: T)
        where
            T: TaskIterator + Send + 'static,
            T::Ready: Send + 'static,
            T::Spawner: ExecutionAction + Send + 'static,
        {
            // execute_task() should accept this
        }
    }

    /// WHY: execute_task() returns correct iterator type
    /// WHAT: Verify return type is RecvIterator<TaskStatus<...>>
    #[test]
    fn test_execute_task_return_type() {
        initialize_pool(20, None);

        let task = SimpleTask { value: Some(42) };

        // Type assertion
        let _iter: RecvIterator<TaskStatus<i32, (), NoAction>> =
            execute_task(task).expect("should create task");
    }

    /// WHY: execute_task() must handle multiple tasks concurrently
    /// WHAT: Spawn multiple tasks and verify they all complete
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_execute_task_multiple_tasks() {
        initialize_pool(20, None);

        let task1 = SimpleTask { value: Some(1) };
        let task2 = SimpleTask { value: Some(2) };
        let task3 = SimpleTask { value: Some(3) };

        let iter1 = ReadyValues::new(execute_task(task1).expect("should create task"));
        let iter2 = ReadyValues::new(execute_task(task2).expect("should create task"));
        let iter3 = ReadyValues::new(execute_task(task3).expect("should create task"));

        #[cfg(feature = "multi")]
        {
            // Multi mode - threads run automatically
            let values1: Vec<i32> = iter1.filter_map(|item| item.inner()).collect();
            let values2: Vec<i32> = iter2.filter_map(|item| item.inner()).collect();
            let values3: Vec<i32> = iter3.filter_map(|item| item.inner()).collect();

            assert_eq!(values1, vec![1]);
            assert_eq!(values2, vec![2]);
            assert_eq!(values3, vec![3]);
        }

        #[cfg(not(feature = "multi"))]
        {
            // Single mode - must drive executor
            single::run_until_complete();

            let values1: Vec<i32> = iter1.filter_map(|item| item.inner()).collect();
            let values2: Vec<i32> = iter2.filter_map(|item| item.inner()).collect();
            let values3: Vec<i32> = iter3.filter_map(|item| item.inner()).collect();

            assert_eq!(values1, vec![1]);
            assert_eq!(values2, vec![2]);
            assert_eq!(values3, vec![3]);
        }
    }
}
