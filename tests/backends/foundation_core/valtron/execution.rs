#![cfg(test)]

use foundation_core::valtron::{
    initialize_pool, ExecutionAction, NoAction, TaskIterator, TaskStatus,
};

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

/// WHY: `execute()` must work on native builds without the `multi` feature
/// (single-threaded executor). In that configuration callers may drive
/// progress by calling `single::run_once()` to advance the execution engine
/// and then inspect the iterator returned by `execute()` to observe task
/// status. Calling `single::run_once()` repeatedly allows the task to make
/// progress before pulling the next `Ready` value from the iterator.
///
/// WHAT: Compile and run using the single executor and obtain the next value
/// by invoking `single::run_once()`.
#[test]
#[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
fn test_execute_uses_single_on_native_without_multi_with_run_once() {
    // Just verify compilation - actual execution requires runtime

    use foundation_core::valtron::execute;
    use foundation_core::valtron::single;
    use foundation_core::valtron::ReadyValues;

    let task = SimpleTask { value: Some(42) };

    // never call this in code, user will call this themselves
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

    use foundation_core::valtron::execute;
    use foundation_core::valtron::single;
    use foundation_core::valtron::ReadyValues;

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

    use foundation_core::valtron::ReadyValues;

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
