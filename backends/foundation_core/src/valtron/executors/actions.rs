//! Reusable ExecutionAction types for common task patterns.
//!
//! This module provides pre-built `ExecutionAction` implementations that can be
//! composed and reused across different task types. These actions encapsulate
//! common patterns like lifting iterators to tasks, scheduling callbacks, and
//! broadcasting results.

use super::{
    BoxedExecutionEngine, BoxedExecutionIterator, DoNext, ExecutionAction, NoAction, TaskIterator,
    TaskStatus,
};
use crate::valtron::GenericResult;
use std::marker::PhantomData;

// ============================================================================
// LiftAction - Convert Iterator to TaskIterator
// ============================================================================

/// Task that wraps an iterator and yields items as TaskStatus::Ready.
///
/// WHY: Allows standard iterators to be used as TaskIterators in the executor.
/// WHAT: Each `next()` call wraps the iterator's item in TaskStatus::Ready.
pub struct LiftTask<I, T>
where
    I: Iterator<Item = T>,
{
    iter: I,
}

impl<I, T> LiftTask<I, T>
where
    I: Iterator<Item = T>,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I, T> TaskIterator for LiftTask<I, T>
where
    I: Iterator<Item = T>,
{
    type Pending = ();
    type Ready = T;
    type Spawner = NoAction;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        self.iter.next().map(TaskStatus::Ready)
    }
}

/// Action that spawns a LiftTask wrapping an iterator.
///
/// WHY: Provides a reusable way to schedule iterators as tasks.
/// WHAT: Creates a DoNext executor for the lifted iterator.
pub struct LiftAction<I, T>
where
    I: Iterator<Item = T> + 'static,
    T: 'static,
{
    iter: Option<I>,
    _marker: PhantomData<T>,
}

impl<I, T> LiftAction<I, T>
where
    I: Iterator<Item = T> + 'static,
    T: 'static,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter: Some(iter),
            _marker: PhantomData,
        }
    }
}

impl<I, T> ExecutionAction for LiftAction<I, T>
where
    I: Iterator<Item = T> + 'static,
    T: 'static,
{
    fn apply(
        mut self,
        _key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let Some(iter) = self.iter.take() {
            let task = LiftTask::new(iter);
            let exec_iter: BoxedExecutionIterator = DoNext::new(task).into();
            executor.schedule(exec_iter)?;
        }
        Ok(())
    }
}

// ============================================================================
// ScheduleAction - Execute closures as tasks
// ============================================================================

/// Task that executes a closure once and completes.
///
/// WHY: Allows running one-shot operations as tasks.
/// WHAT: Executes the closure on first poll, then returns None.
pub struct ScheduleTask<F>
where
    F: FnOnce(),
{
    closure: Option<F>,
}

impl<F> ScheduleTask<F>
where
    F: FnOnce(),
{
    pub fn new(closure: F) -> Self {
        Self {
            closure: Some(closure),
        }
    }
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

/// Action that schedules a closure to run as a task.
///
/// WHY: Provides reusable pattern for scheduling arbitrary code.
/// WHAT: Wraps closure in a ScheduleTask and schedules it.
pub struct ScheduleAction<F>
where
    F: FnOnce() + 'static,
{
    closure: Option<F>,
}

impl<F> ScheduleAction<F>
where
    F: FnOnce() + 'static,
{
    pub fn new(closure: F) -> Self {
        Self {
            closure: Some(closure),
        }
    }
}

impl<F> ExecutionAction for ScheduleAction<F>
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
            executor.schedule(exec_iter)?;
        }
        Ok(())
    }
}

// ============================================================================
// BroadcastAction - Send values to multiple receivers
// ============================================================================

/// Task that broadcasts a value to multiple callbacks.
///
/// WHY: Allows notifying multiple listeners of a result.
/// WHAT: Calls all callbacks with clones of the value.
pub struct BroadcastTask<T>
where
    T: Clone,
{
    value: Option<T>,
    callbacks: Vec<Box<dyn FnOnce(T) + Send>>,
}

impl<T> BroadcastTask<T>
where
    T: Clone,
{
    pub fn new(value: T, callbacks: Vec<Box<dyn FnOnce(T) + Send>>) -> Self {
        Self {
            value: Some(value),
            callbacks,
        }
    }
}

impl<T> TaskIterator for BroadcastTask<T>
where
    T: Clone,
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

/// Action that broadcasts a value to multiple receivers.
///
/// WHY: Reusable pattern for fan-out notifications.
/// WHAT: Schedules a BroadcastTask that notifies all callbacks.
pub struct BroadcastAction<T>
where
    T: Clone + 'static,
{
    value: Option<T>,
    callbacks: Vec<Box<dyn FnOnce(T) + Send>>,
}

impl<T> BroadcastAction<T>
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

impl<T> ExecutionAction for BroadcastAction<T>
where
    T: Clone + 'static,
{
    fn apply(
        mut self,
        _key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let (Some(value), true) = (self.value.take(), !self.callbacks.is_empty()) {
            let callbacks = std::mem::take(&mut self.callbacks);
            let task = BroadcastTask::new(value, callbacks);
            let exec_iter: BoxedExecutionIterator = DoNext::new(task).into();
            executor.schedule(exec_iter)?;
        }
        Ok(())
    }
}

// ============================================================================
// CompositeAction - Enum combining action types
// ============================================================================

/// Enum that combines all action types plus a custom action slot.
///
/// WHY: Allows using different action types through a single enum type
/// WHAT: Enum with variants for each action type, delegates to inner action
pub enum CompositeAction<I, T, F, V, C>
where
    I: Iterator<Item = T> + 'static,
    T: 'static,
    F: FnOnce() + 'static,
    V: Clone + 'static,
    C: ExecutionAction,
{
    None,
    Lift(LiftAction<I, T>),
    Schedule(ScheduleAction<F>),
    Broadcast(BroadcastAction<V>),
    Custom(C),
}

impl<I, T, F, V, C> ExecutionAction for CompositeAction<I, T, F, V, C>
where
    I: Iterator<Item = T> + 'static,
    T: 'static,
    F: FnOnce() + 'static,
    V: Clone + 'static,
    C: ExecutionAction,
{
    fn apply(self, key: crate::synca::Entry, engine: BoxedExecutionEngine) -> GenericResult<()> {
        match self {
            Self::None => Ok(()),
            Self::Lift(action) => action.apply(key, engine),
            Self::Schedule(action) => action.apply(key, engine),
            Self::Broadcast(action) => action.apply(key, engine),
            Self::Custom(action) => action.apply(key, engine),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    // PHASE 1: LiftAction Tests

    /// WHY: LiftAction must convert Iterator to a TaskIterator for scheduling
    /// WHAT: Construct LiftTask from vector iterator
    #[test]
    fn test_lift_task_from_vec_iterator() {
        let items = vec![1, 2, 3];
        let mut task = LiftTask::new(items.into_iter());

        // First item should be Ready(1)
        match task.next() {
            Some(TaskStatus::Ready(1)) => {}
            other => panic!("Expected Ready(1), got {:?}", other),
        }

        // Second item should be Ready(2)
        match task.next() {
            Some(TaskStatus::Ready(2)) => {}
            other => panic!("Expected Ready(2), got {:?}", other),
        }

        // Third item should be Ready(3)
        match task.next() {
            Some(TaskStatus::Ready(3)) => {}
            other => panic!("Expected Ready(3), got {:?}", other),
        }

        // Iterator exhausted
        assert!(task.next().is_none());
    }

    /// WHY: LiftAction must wrap iterator in ExecutionAction for spawning
    /// WHAT: Create action that spawns a lifted iterator task
    #[test]
    fn test_lift_action_creates_spawnable_task() {
        let items = vec![10, 20, 30];
        let action = LiftAction::new(items.into_iter());

        // Action should be an ExecutionAction
        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    /// WHY: LiftTask should work with different iterator types
    /// WHAT: Test with range iterator
    #[test]
    fn test_lift_task_with_range() {
        let mut task = LiftTask::new(0..3);

        assert_eq!(task.next(), Some(TaskStatus::Ready(0)));
        assert_eq!(task.next(), Some(TaskStatus::Ready(1)));
        assert_eq!(task.next(), Some(TaskStatus::Ready(2)));
        assert_eq!(task.next(), None);
    }

    // PHASE 2: ScheduleAction Tests

    /// WHY: ScheduleAction must allow scheduling closures as one-shot tasks
    /// WHAT: Execute closure when task is polled
    #[test]
    fn test_schedule_task_executes_closure() {
        let executed = Arc::new(StdMutex::new(false));
        let executed_clone = executed.clone();

        let mut task = ScheduleTask::new(move || {
            *executed_clone.lock().unwrap() = true;
        });

        // First poll should execute and return Ready
        match task.next() {
            Some(TaskStatus::Ready(())) => {}
            other => panic!("Expected Ready(()), got {:?}", other),
        }

        assert_eq!(*executed.lock().unwrap(), true);

        // Task is done, should return None
        assert!(task.next().is_none());
    }

    /// WHY: ScheduleAction must create spawnable one-shot tasks
    /// WHAT: Wrap closure in ExecutionAction
    #[test]
    fn test_schedule_action_is_execution_action() {
        let counter = Arc::new(StdMutex::new(0));
        let counter_clone = counter.clone();

        let action = ScheduleAction::new(move || {
            *counter_clone.lock().unwrap() += 1;
        });

        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    /// WHY: ScheduleTask should only execute once
    /// WHAT: Subsequent polls return None
    #[test]
    fn test_schedule_task_runs_once() {
        let counter = Arc::new(StdMutex::new(0));
        let counter_clone = counter.clone();

        let mut task = ScheduleTask::new(move || {
            *counter_clone.lock().unwrap() += 1;
        });

        assert!(task.next().is_some());
        assert_eq!(*counter.lock().unwrap(), 1);

        // Second poll should return None and not execute again
        assert!(task.next().is_none());
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    // PHASE 3: BroadcastAction Tests

    /// WHY: BroadcastAction must send values to multiple receivers
    /// WHAT: All receivers get a clone of the value
    #[test]
    fn test_broadcast_task_sends_to_multiple_receivers() {
        let receiver1 = Arc::new(StdMutex::new(None));
        let receiver2 = Arc::new(StdMutex::new(None));
        let receiver3 = Arc::new(StdMutex::new(None));

        let r1 = receiver1.clone();
        let r2 = receiver2.clone();
        let r3 = receiver3.clone();

        let callbacks: Vec<Box<dyn FnOnce(i32) + Send>> = vec![
            Box::new(move |val| *r1.lock().unwrap() = Some(val)),
            Box::new(move |val| *r2.lock().unwrap() = Some(val)),
            Box::new(move |val| *r3.lock().unwrap() = Some(val)),
        ];

        let mut task = BroadcastTask::new(42, callbacks);

        // First poll sends to all receivers
        match task.next() {
            Some(TaskStatus::Ready(())) => {}
            other => panic!("Expected Ready(()), got {:?}", other),
        }

        assert_eq!(*receiver1.lock().unwrap(), Some(42));
        assert_eq!(*receiver2.lock().unwrap(), Some(42));
        assert_eq!(*receiver3.lock().unwrap(), Some(42));

        // Task is done
        assert!(task.next().is_none());
    }

    /// WHY: BroadcastAction must be spawnable
    /// WHAT: Wrap broadcast task in ExecutionAction
    #[test]
    fn test_broadcast_action_is_execution_action() {
        let action = BroadcastAction::new(100, vec![]);
        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    /// WHY: BroadcastTask should work with zero callbacks
    /// WHAT: Empty callback list should not panic
    #[test]
    fn test_broadcast_task_with_no_callbacks() {
        let mut task = BroadcastTask::new(42, vec![]);

        assert!(task.next().is_some());
        assert!(task.next().is_none());
    }

    // PHASE 4: CompositeAction Tests

    /// WHY: CompositeAction must support different action types via enum
    /// WHAT: CompositeAction::None variant compiles and is an ExecutionAction
    #[test]
    fn test_composite_action_none_variant() {
        // CompositeAction is now an enum, test the None variant
        let composite: CompositeAction<std::ops::Range<i32>, i32, fn(), i32, NoAction> =
            CompositeAction::None;
        let _: Box<dyn ExecutionAction> = Box::new(composite);
    }

    /// WHY: CompositeAction should work with Lift variant
    /// WHAT: Wraps LiftAction in enum
    #[test]
    fn test_composite_action_with_lift() {
        let lift = LiftAction::new(vec![1, 2, 3].into_iter());
        let composite: CompositeAction<_, _, fn(), i32, NoAction> = CompositeAction::Lift(lift);
        let _: Box<dyn ExecutionAction> = Box::new(composite);
    }

    /// WHY: CompositeAction should work with Schedule variant
    /// WHAT: Wraps ScheduleAction in enum
    #[test]
    fn test_composite_action_with_schedule() {
        let schedule = ScheduleAction::new(|| {});
        let composite: CompositeAction<std::ops::Range<i32>, i32, _, i32, NoAction> =
            CompositeAction::Schedule(schedule);
        let _: Box<dyn ExecutionAction> = Box::new(composite);
    }
}
