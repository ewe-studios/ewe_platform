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

use crate::synca::mpp::StreamRecvIterator;
use crate::valtron::{single, ExecutionAction, ProgressIndicator, State, TaskIterator, TaskStatus};

use crate::{synca::mpp::RecvIterator, valtron::GenericResult};

pub const DEFAULT_WAIT_CYCLE: std::time::Duration = std::time::Duration::from_millis(10);

/// [`initialize_pool`] provides a unified method to initialize the underlying
/// thread pool for both the single and multi-threaded instances.
pub fn initialize_pool(seed_for_rng: u64, _user_thread_num: Option<usize>) {
    #[cfg(target_arch = "wasm32")]
    {
        single::initialize_pool(seed_for_rng);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(feature = "multi")]
        {
            use crate::valtron::multi;
            multi::initialize_pool(seed_for_rng, _user_thread_num);
        }

        #[cfg(not(feature = "multi"))]
        {
            single::initialize_pool(seed_for_rng);
        }
    }
}

// ===========================================
// Execution Methods
// ===========================================

/// [`run_until_next_state`] this is a no-op in multi-threaded situations (i.e multi feature flag is on), but
/// under multi=off or wasm execution, this will  executing the execution engine until the next valid
/// state is seen to indicate task has reported progress as [`State`].
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
pub fn run_until_next_state() {
    run_until(|indicator: ProgressIndicator| {
        if let ProgressIndicator::CanProgress(Some(state)) = indicator {
            tracing::debug!("Valtron: Seen new state value from state={state:?}");
            true
        } else {
            false
        }
    });
}

/// [`run_until_ready_state`] this is a no-op in multi-threaded situations (i.e multi feature flag is on), but
/// under multi=off or wasm execution, this will  executing the execution engine until the next valid
/// state ready is seen to indicate task has reported progress as [`State::ReadyValue`].
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
pub fn run_until_ready_state() {
    run_until(|indicator: ProgressIndicator| {
        if let ProgressIndicator::CanProgress(Some(State::ReadyValue(task_id))) = indicator {
            tracing::debug!("Valtron: Seen ready value from task id={task_id:?}");
            true
        } else {
            false
        }
    });
}

/// [`run_until`] provides a unified method to attempt to execute the `run_until`.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
pub fn run_until<T>(checker: T)
where
    T: Fn(ProgressIndicator) -> bool,
{
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single-threaded stream in no-wasm");
        single::run_until(checker);
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single stream in wasm");
        single::run_until(checker);
    }
}

/// [`run_until_complete`] provides a unified method to attempt to execute the `run_until_complete`.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
pub fn run_until_complete() {
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single-threaded stream in no-wasm");
        single::run_until_complete();
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single stream in wasm");
        single::run_until_complete();
    }
}

/// [`run_once`] provides a unified method to attempt to execute the `run_once`.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
pub fn run_once() {
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single-threaded stream in no-wasm");
        let _ = single::run_once();
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single stream in wasm");
        let _ = single::run_once();
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
#[allow(clippy::type_complexity)]
pub fn execute<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
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

/// [`execute_stream`] unlike [`execute`] returns a [`StreamIterator`]
/// which hides away the underlying mechanics of dealing with a [`TaskStatus`]
/// with a type of [`ExecutionIterator`] which will properly manage the different
/// task status, sending the needed spawn events to the executor as needed and
/// providing a simpler representation of the results for the caller.
pub fn execute_stream<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<StreamRecvIterator<T::Ready, T::Pending>>
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
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(iter)
}

/// Execute using single-threaded executor returning a stream iterator.
///
/// WHY: WASM and minimal builds need single-threaded execution
/// WHAT: Schedules task, returns a stream iterator.
#[allow(clippy::type_complexity)]
fn execute_single_stream<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<StreamRecvIterator<T::Ready, T::Pending>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .scheduled_stream_iter(wait_cycle.unwrap_or(DEFAULT_WAIT_CYCLE))?;

    Ok(iter)
}

/// Execute using multi-threaded executor.
///
/// WHY: Native builds can use multiple threads for better performance
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
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

    Ok(iter)
}

/// Execute using multi-threaded executor.
///
/// WHY: Native builds can use multiple threads for better performance
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi_stream<T>(
    task: T,
    wait_cycle: Option<std::time::Duration>,
) -> GenericResult<StreamRecvIterator<T::Ready, T::Pending>>
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

    Ok(iter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valtron::{initialize_pool, NoAction, TaskStatus};

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

    /// WHY: execute() must work on WASM (single executor only)
    /// WHAT: Function compiles and has correct signature for WASM
    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_execute_available_on_wasm() {
        let task = SimpleTask { value: Some(42) };
        // Just verify it compiles on WASM
        // Actual execution would require a WASM runtime
        initialize_pool(20, None);

        let values_iter = ReadyValues::new(execute(task, None).expect("should create task"));
        let values: Vec<i32> = values_iter.flat_map(|item| item.inner()).collect();
        assert_eq!(values, vec![42]);
    }

    /// WHY: execute() must work on native without multi feature (single executor)
    /// where we run get the next ready value by executing  single::run_once when
    /// executing in multi featured=off secrenario, we can call `single::run_once`
    /// multiple times to get the task to make progress and check the values of `values_iter.next()`
    /// to get the next status of the tasks, sometimes we may not even care about pending, and delayed and ]
    /// only ready  `TaskStatus` and other times we might, but `single::run_once` in a
    /// single threaded context ensures it makes progress before we pull the relevant result.
    /// The nice part of this is we can use this in a single threaded context to pull just enough
    /// result for our computation before making more progress for the task.
    ///
    /// WHAT: Function compiles and uses single executor and  gets next value with
    /// single::run_once())
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    fn test_execute_uses_single_on_native_without_multi_with_run_once() {
        // Just verify compilation - actual execution requires runtime

        use crate::valtron::single;
        use crate::valtron::ReadyValues;

        let task = SimpleTask { value: Some(42) };

        // never call this in code, user will call this themsevles
        // in the main function but we do this here since its a test
        initialize_pool(20, None);

        let mut values_iter = ReadyValues::new(execute(task, None).expect("should create task"));

        let _ = single::run_once();

        let value = values_iter.next();
        let inner = value.expect("get inner").inner();

        assert_eq!(inner, Some(42));
    }

    /// WHY: execute() must work on native without multi feature (single executor)
    /// where the result should all be ready fully before we read it all out.
    /// WHAT: Function compiles and uses single executor
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    fn test_execute_uses_single_on_native_without_multi() {
        // Just verify compilation - actual execution requires runtime

        use crate::valtron::single;
        use crate::valtron::ReadyValues;

        let task = SimpleTask { value: Some(42) };

        // never call this in code, user will call this themsevles
        // in the main function but we do this here since its a test
        initialize_pool(20, None);

        let values_iter = ReadyValues::new(execute(task, None).expect("should create task"));

        single::run_until_complete();

        let values: Vec<i32> = values_iter.filter_map(|item| item.inner()).collect();
        assert_eq!(values, vec![42]);
    }

    /// WHY: execute() must work on native with multi feature (multi executor)
    /// WHAT: Function compiles and uses multi executor
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
    fn test_execute_uses_multi_on_native_with_feature() {
        // Just verify compilation - actual execution requires runtime

        use crate::valtron::ReadyValues;

        initialize_pool(20, None);

        let task = SimpleTask { value: Some(42) };
        let values_iter = ReadyValues::new(execute(task, None).expect("should create task"));
        let values: Vec<i32> = values_iter
            .flat_map(|item: crate::valtron::ReadyValue<_>| item.inner())
            .collect();
        assert_eq!(values, vec![42]);
    }

    /// WHY: execute() signature must match TaskIterator trait requirements
    /// WHAT: Verify Send + 'static bounds are correct
    #[test]
    fn test_execute_signature_accepts_task_iterator() {
        // This is a compile-time test
        fn _assert_compiles<T>(_task: T)
        where
            T: TaskIterator + Send + 'static,
            T::Ready: Send + 'static,
            T::Spawner: ExecutionAction + Send + 'static,
        {
            // execute() should accept this
        }
    }
}
