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

use crate::valtron::{single, ExecutionAction, ProgressIndicator, TaskIterator, TaskStatus};

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
use super::multi;

use crate::{synca::mpp::RecvIterator, valtron::GenericResult};

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
            multi::initialize_pool(seed_for_rng, _user_thread_num);
        }

        #[cfg(not(feature = "multi"))]
        {
            single::initialize_pool(seed_for_rng);
        }
    }
}

/// [`run_until`] provides a unified method to attempt to execute the run_until.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded and wasm context.
pub fn run_until<T>(checker: T)
where
    T: Fn(ProgressIndicator) -> bool,
{
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;
        single::run_until(checker);
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;
        single::run_until(checker);
    }
}

/// [`run_until_complete`] provides a unified method to attempt to execute the run_until_complete.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded and wasm context.
pub fn run_until_complete() {
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;
        single::run_until_complete();
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;
        single::run_until_complete();
    }
}

/// [`run_once`] provides a unified method to attempt to execute the run_once.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded and wasm context.
pub fn run_once() {
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;
        single::run_once();
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;
        single::run_once();
    }
}

// pub type RecvIter<T> = RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>;
// pub type RecvIterResult<T> = GenericResult<RecvIter<T>>;

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
/// WHY: WASM and minimal builds need single-threaded execution
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[allow(clippy::type_complexity)]
#[allow(dead_code)]
fn execute_single_complete<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use std::time::Duration;

    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(5))?;

    // Run executor until complete
    single::run_until_complete();

    Ok(iter)
}

/// Execute using single-threaded executor.
///
/// WHY: WASM and minimal builds need single-threaded execution
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[allow(clippy::type_complexity)]
fn execute_single<T>(
    task: T,
) -> GenericResult<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use std::time::Duration;

    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(5))?;

    Ok(iter)
}

/// Execute using multi-threaded executor.
///
/// WHY: Native builds can use multiple threads for better performance
/// WHAT: Schedules task, runs until complete, returns first Ready value
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
    use std::time::Duration;

    // Schedule task and get iterator
    let iter = multi::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(1))?;

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

        let values_iter = ReadyValues::new(execute(task).expect("should create task"));
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

        let mut values_iter = ReadyValues::new(execute(task).expect("should create task"));

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

        let values_iter = ReadyValues::new(execute(task).expect("should create task"));

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
        let values_iter = ReadyValues::new(execute(task).expect("should create task"));
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
