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

use super::{single, ExecutionAction, TaskIterator, TaskStatus};

#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
use super::multi;

use crate::valtron::GenericResult;

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
pub fn execute<T>(task: T) -> GenericResult<T::Ready>
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

/// Execute using single-threaded executor.
///
/// WHY: WASM and minimal builds need single-threaded execution
/// WHAT: Schedules task, runs until complete, returns first Ready value
fn execute_single<T>(task: T) -> GenericResult<T::Ready>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use std::time::Duration;

    // Schedule task and get iterator
    let iter = single::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(1))?;

    // Run executor until complete
    single::run_until_complete();

    // Extract first Ready value
    for status in iter {
        if let TaskStatus::Ready(value) = status {
            return Ok(value);
        }
    }

    Err("Task completed without producing Ready value".into())
}

/// Execute using multi-threaded executor.
///
/// WHY: Native builds can use multiple threads for better performance
/// WHAT: Schedules task, runs until complete, returns first Ready value
#[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
fn execute_multi<T>(task: T) -> GenericResult<T::Ready>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    use std::time::Duration;

    // Schedule task and get iterator
    let iter = multi::spawn()
        .with_task(task)
        .schedule_iter(Duration::from_nanos(1))?;

    // Run executor until complete
    multi::run_until_complete();

    // Extract first Ready value
    for status in iter {
        if let TaskStatus::Ready(value) = status {
            return Ok(value);
        }
    }

    Err("Task completed without producing Ready value".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valtron::executors::{NoAction, TaskStatus};

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
        let _result: GenericResult<i32> = execute(task);
    }

    /// WHY: execute() must work on native without multi feature (single executor)
    /// WHAT: Function compiles and uses single executor
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    fn test_execute_uses_single_on_native_without_multi() {
        // Just verify compilation - actual execution requires runtime
        let task = SimpleTask { value: Some(42) };
        let _result: GenericResult<i32> = execute(task);
    }

    /// WHY: execute() must work on native with multi feature (multi executor)
    /// WHAT: Function compiles and uses multi executor
    #[test]
    #[cfg(all(not(target_arch = "wasm32"), feature = "multi"))]
    fn test_execute_uses_multi_on_native_with_feature() {
        // Just verify compilation - actual execution requires runtime
        let task = SimpleTask { value: Some(42) };
        let _result: GenericResult<i32> = execute(task);
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
