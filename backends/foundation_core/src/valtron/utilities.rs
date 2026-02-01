//! Reusable utilities for the valtron executor system.
//!
//! This module provides common utilities for implementing the HTTP 1.1 client
//! using iterator-based patterns and valtron executors.
//!
//! The utilities include:
//! - ExecutionAction types for common task patterns
//! - Unified executor for consistent task scheduling
//! - State machine helpers for managing task lifecycle
//!
//! These utilities are designed to be reused across different components
//! of the HTTP client implementation.

use crate::valtron::{
    spawn_broadcaster, spawn_builder, BoxedExecutionEngine, BoxedExecutionIterator, DoNext,
    ExecutionAction, GenericResult, NoAction, TaskIterator, TaskStatus,
};
use std::marker::PhantomData;
use std::time::Duration;

// ============================================================================
// ExecutionAction Types
// ============================================================================

/// Action that spawns a WrapTask wrapping a plain value iterator.
///
/// WHY: Provides a reusable way to schedule plain value iterators as tasks.
/// WHAT: Creates a DoNext executor for the wrapped iterator.
///
/// Use this when your iterator produces plain values (i32, String, etc.)
/// that need to be wrapped in TaskStatus::Ready.
pub struct WrapAction<I, T>
where
    I: Iterator<Item = T> + 'static,
    T: 'static,
{
    iter: Option<I>,
    _marker: PhantomData<T>,
}

impl<I, T> WrapAction<I, T>
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

impl<I, T> ExecutionAction for WrapAction<I, T>
where
    I: Iterator<Item = T> + 'static,
    T: 'static,
{
    fn apply(
        &mut self,
        key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let Some(iter) = self.iter.take() {
            let task = WrapTask::new(iter);

            spawn_builder(executor)
                .with_parent(key)
                .with_task(task)
                .schedule()?;
        }
        Ok(())
    }
}

/// Action that spawns a LiftTask that passes through TaskStatus items.
///
/// WHY: Provides a reusable way to schedule TaskStatus iterators with parent linkage.
/// WHAT: Creates a DoNext executor and calls engine.lift() to link with parent task.
///
/// Use this when your iterator already produces TaskStatus variants
/// and you want to preserve their semantic meaning (Pending, Delayed, etc.)
/// while maintaining task hierarchy through lift().
pub struct SpawnWithLift<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>> + 'static,
    D: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    iter: Option<I>,
    _marker: PhantomData<(D, P, S)>,
}

impl<I, D, P, S> SpawnWithLift<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>> + 'static,
    D: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    pub fn new(iter: I) -> Self {
        Self {
            iter: Some(iter),
            _marker: PhantomData,
        }
    }
}

impl<I, D, P, S> ExecutionAction for SpawnWithLift<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>> + 'static,
    D: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    fn apply(
        &mut self,
        key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let Some(iter) = self.iter.take() {
            let task = LiftTask::new(iter);
            spawn_builder(executor)
                .with_parent(key.clone())
                .with_task(task)
                .lift()?;
        }
        Ok(())
    }
}

/// Action that schedules a closure to run as a task.
///
/// WHY: Provides reusable pattern for scheduling arbitrary code.
/// WHAT: Wraps closure in a ScheduleTask and schedules it.
pub struct SpawnWithSchedule<F>
where
    F: FnOnce() + 'static,
{
    closure: Option<F>,
}

impl<F> SpawnWithSchedule<F>
where
    F: FnOnce() + 'static,
{
    pub fn new(closure: F) -> Self {
        Self {
            closure: Some(closure),
        }
    }
}

impl<F> ExecutionAction for SpawnWithSchedule<F>
where
    F: FnOnce() + 'static,
{
    fn apply(
        &mut self,
        key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let Some(closure) = self.closure.take() {
            let task = ScheduleTask::new(closure);

            spawn_builder(executor)
                .with_parent(key.clone())
                .with_task(task)
                .schedule()?;
        }
        Ok(())
    }
}

/// Action that broadcasts a value to multiple receivers.
///
/// WHY: Reusable pattern for fan-out notifications using global queue.
/// WHAT: Schedules a BroadcastTask via engine.broadcast() for any thread to execute.
///
/// Unlike schedule() which adds to local queue, broadcast() sends the task
/// to the global queue where any executor thread can pick it up. This enables
/// work distribution across threads.
pub struct SpawnWithBroadcast<T>
where
    T: Clone + Send + 'static,
{
    value: Option<T>,
    callbacks: Vec<Box<dyn FnOnce(T) + Send>>,
}

impl<T> SpawnWithBroadcast<T>
where
    T: Clone + Send + 'static,
{
    pub fn new(value: T, callbacks: Vec<Box<dyn FnOnce(T) + Send>>) -> Self {
        Self {
            value: Some(value),
            callbacks,
        }
    }
}

impl<T> ExecutionAction for SpawnWithBroadcast<T>
where
    T: Clone + Send + 'static,
{
    fn apply(
        &mut self,
        key: crate::synca::Entry,
        executor: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        if let (Some(value), true) = (self.value.take(), !self.callbacks.is_empty()) {
            let callbacks = std::mem::take(&mut self.callbacks);
            let task = BroadcastTask::new(value, callbacks);

            // Use broadcast() instead of schedule() - sends to global queue for any thread
            // Note: Requires Send bound on T
            spawn_broadcaster(executor)
                .with_parent(key.clone())
                .with_task(task)
                .broadcast()?;
        }
        Ok(())
    }
}

// ============================================================================
// TaskIterator Implementations
// ============================================================================

/// Task that wraps an iterator of plain values and yields items as TaskStatus::Ready.
///
/// WHY: Allows standard iterators of plain values to be used as TaskIterators.
/// WHAT: Each `next()` call wraps the iterator's item in TaskStatus::Ready.
///
/// NOTE: If your iterator already produces TaskStatus variants, use LiftTask instead
/// to avoid nesting like Ready(Pending(...)). WrapTask is ONLY for plain value iterators.
pub struct WrapTask<I, T>
where
    I: Iterator<Item = T>,
{
    iter: I,
}

impl<I, T> WrapTask<I, T>
where
    I: Iterator<Item = T>,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I, T> TaskIterator for WrapTask<I, T>
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

/// Task that passes through TaskStatus items from an iterator without wrapping.
///
/// WHY: Allows iterators that already produce TaskStatus to be used directly,
/// preserving their semantic meaning (Pending, Delayed, Ready, etc.).
/// WHAT: Each `next()` call passes through the TaskStatus variant as-is.
///
/// This prevents incorrect nesting like `Ready(Pending(...))` which would lose
/// the semantic meaning of the original TaskStatus.
///
/// # Example
///
/// ```ignore
/// let iter = vec![
///     TaskStatus::Pending(()),
///     TaskStatus::Delayed(Duration::from_secs(1)),
///     TaskStatus::Ready(42)
/// ].into_iter();
///
/// let mut task = LiftTask::new(iter);
/// assert_eq!(task.next(), Some(TaskStatus::Pending(())));  // Preserved!
/// assert_eq!(task.next(), Some(TaskStatus::Delayed(Duration::from_secs(1))));
/// assert_eq!(task.next(), Some(TaskStatus::Ready(42)));
/// ```
pub struct LiftTask<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>>,
    S: ExecutionAction,
{
    iter: I,
}

impl<I, D, P, S> LiftTask<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>>,
    S: ExecutionAction,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<I, D, P, S> TaskIterator for LiftTask<I, D, P, S>
where
    I: Iterator<Item = TaskStatus<D, P, S>>,
    S: ExecutionAction,
{
    type Pending = P;
    type Ready = D;
    type Spawner = S;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Pass through TaskStatus as-is, preserving semantic meaning
        self.iter.next()
    }
}

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

/// Task that broadcasts a value to multiple callbacks.
///
/// WHY: Allows notifying multiple listeners of a result.
/// WHAT: Calls all callbacks with clones of the value.
///
/// Note: Requires Send bounds because SpawnWithBroadcast uses engine.broadcast()
/// which sends tasks to the global queue for any thread to pick up.
pub struct BroadcastTask<T>
where
    T: Clone + Send,
{
    value: Option<T>,
    callbacks: Vec<Box<dyn FnOnce(T) + Send>>,
}

impl<T> BroadcastTask<T>
where
    T: Clone + Send,
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
    T: Clone + Send,
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

// ============================================================================
// Unified Executor
// ============================================================================

/// Unified executor for consistent task scheduling.
///
/// This executor provides a consistent interface for scheduling tasks
/// across different execution contexts.
pub struct UnifiedExecutor {
    engine: BoxedExecutionEngine,
}

impl UnifiedExecutor {
    /// Creates a new unified executor.
    pub fn new(engine: BoxedExecutionEngine) -> Self {
        Self { engine }
    }

    /// Schedules a task with the specified parent.
    pub fn schedule_with_parent<T, P, S>(&self, parent: T, task: impl TaskIterator<Ready = P, Pending = S, Spawner = NoAction>) -> GenericResult<()> {
        spawn_builder(self.engine.clone())
            .with_parent(parent)
            .with_task(task)
            .schedule()
    }

    /// Lifts a task with the specified parent.
    pub fn lift_with_parent<T, P, S>(&self, parent: T, task: impl TaskIterator<Ready = P, Pending = S, Spawner = NoAction>) -> GenericResult<()> {
        spawn_builder(self.engine.clone())
            .with_parent(parent)
            .with_task(task)
            .lift()
    }

    /// Broadcasts a task to all available executors.
    pub fn broadcast_with_parent<T, P, S>(&self, parent: T, task: impl TaskIterator<Ready = P, Pending = S, Spawner = NoAction>) -> GenericResult<()> {
        spawn_broadcaster(self.engine.clone())
            .with_parent(parent)
            .with_task(task)
            .broadcast()
    }
}

// ============================================================================
// State Machine Helpers
// ============================================================================

/// State machine helper for managing task lifecycle.
///
/// This helper provides utilities for managing the state transitions of tasks
/// during their execution.
pub struct StateMachine {
    state: TaskState,
}

/// Possible states of a task.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    /// Task is ready to execute.
    Ready,
    /// Task is waiting for a resource.
    Pending,
    /// Task is delayed for a specific duration.
    Delayed(Duration),
    /// Task is sleeping for a specific duration.
    Sleeping(Duration),
    /// Task has completed successfully.
    Completed,
    /// Task has failed.
    Failed,
}

impl StateMachine {
    /// Creates a new state machine with the initial state.
    pub fn new(initial_state: TaskState) -> Self {
        Self { state: initial_state }
    }

    /// Gets the current state of the task.
    pub fn get_state(&self) -> &TaskState {
        &self.state
    }

    /// Sets the state of the task.
    pub fn set_state(&mut self, state: TaskState) {
        self.state = state;
    }

    /// Checks if the task is in a completed state.
    pub fn is_completed(&self) -> bool {
        matches!(self.state, TaskState::Completed)
    }

    /// Checks if the task is in a failed state.
    pub fn is_failed(&self) -> bool {
        matches!(self.state, TaskState::Failed)
    }

    /// Checks if the task is ready to execute.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, TaskState::Ready)
    }

    /// Checks if the task is pending.
    pub fn is_pending(&self) -> bool {
        matches!(self.state, TaskState::Pending)
    }

    /// Checks if the task is delayed.
    pub fn is_delayed(&self) -> bool {
        matches!(self.state, TaskState::Delayed(_))
    }

    /// Checks if the task is sleeping.
    pub fn is_sleeping(&self) -> bool {
        matches!(self.state, TaskState::Sleeping(_))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;

    // Test the WrapAction functionality
    #[test]
    fn test_wrap_action() {
        let items = vec![1, 2, 3];
        let action = WrapAction::new(items.into_iter());

        // Verify the action can be created
        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    // Test the SpawnWithLift functionality
    #[test]
    fn test_spawn_with_lift() {
        let iter = vec![TaskStatus::<i32, (), NoAction>::Ready(1)].into_iter();
        let action = SpawnWithLift::new(iter);

        // Verify the action can be created
        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    // Test the SpawnWithSchedule functionality
    #[test]
    fn test_spawn_with_schedule() {
        let counter = Arc::new(StdMutex::new(0));
        let counter_clone = counter.clone();

        let action = SpawnWithSchedule::new(move || {
            *counter_clone.lock().unwrap() += 1;
        });

        // Verify the action can be created
        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    // Test the SpawnWithBroadcast functionality
    #[test]
    fn test_spawn_with_broadcast() {
        let action = SpawnWithBroadcast::new(100, vec![]);

        // Verify the action can be created
        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    // Test the WrapTask functionality
    #[test]
    fn test_wrap_task() {
        let items = vec![1, 2, 3];
        let mut task = WrapTask::new(items.into_iter());

        // Verify the task can be created and returns expected values
        assert_eq!(task.next(), Some(TaskStatus::Ready(1)));
        assert_eq!(task.next(), Some(TaskStatus::Ready(2)));
        assert_eq!(task.next(), Some(TaskStatus::Ready(3)));
        assert_eq!(task.next(), None);
    }

    // Test the LiftTask functionality
    #[test]
    fn test_lift_task() {
        let iter = vec![TaskStatus::<i32, (), NoAction>::Ready(1)].into_iter();
        let mut task = LiftTask::new(iter);

        // Verify the task can be created and returns expected values
        assert_eq!(task.next(), Some(TaskStatus::Ready(1)));
        assert_eq!(task.next(), None);
    }

    // Test the ScheduleTask functionality
    #[test]
    fn test_schedule_task() {
        let executed = Arc::new(StdMutex::new(false));
        let executed_clone = executed.clone();

        let mut task = ScheduleTask::new(move || {
            *executed_clone.lock().unwrap() = true;
        });

        // Verify the task executes and returns expected values
        assert!(task.next().is_some());
        assert_eq!(*executed.lock().unwrap(), true);
        assert!(task.next().is_none());
    }

    // Test the BroadcastTask functionality
    #[test]
    fn test_broadcast_task() {
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

        // Verify the task broadcasts to all receivers
        assert!(task.next().is_some());
        assert_eq!(*receiver1.lock().unwrap(), Some(42));
        assert_eq!(*receiver2.lock().unwrap(), Some(42));
        assert_eq!(*receiver3.lock().unwrap(), Some(42));
        assert!(task.next().is_none());
    }

    // Test the UnifiedExecutor functionality
    #[test]
    fn test_unified_executor() {
        // Create a mock executor (this would be created in real implementation)
        let engine = Box::new(NoAction);
        let executor = UnifiedExecutor::new(engine);

        // Verify the executor can be created
        let _: Box<dyn ExecutionAction> = Box::new(executor);
    }

    // Test the StateMachine functionality
    #[test]
    fn test_state_machine() {
        let mut state_machine = StateMachine::new(TaskState::Ready);

        // Verify the state machine can be created and state can be checked
        assert_eq!(state_machine.get_state(), &TaskState::Ready);
        assert!(state_machine.is_ready());
        assert!(!state_machine.is_completed());
        assert!(!state_machine.is_failed());

        // Change the state and verify
        state_machine.set_state(TaskState::Completed);
        assert_eq!(state_machine.get_state(), &TaskState::Completed);
        assert!(state_machine.is_completed());
        assert!(!state_machine.is_ready());
    }
}