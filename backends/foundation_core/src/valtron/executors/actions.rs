//! Reusable ExecutionAction types for common task patterns.
//!
//! This module provides pre-built `ExecutionAction` implementations that can be
//! composed and reused across different task types. These actions encapsulate
//! common patterns like wrapping iterators, scheduling callbacks, and
//! broadcasting work.
//!
//! ## Action Types and Their Engine Methods
//!
//! Each action type calls a specific `ExecutionEngine` method:
//!
//! - **WrapAction**: Calls `engine.schedule()` - adds to local queue
//! - **SpawnWithLift**: Calls `engine.lift(task, parent)` - schedules with parent linkage
//! - **SpawnWithSchedule**: Calls `engine.schedule()` - adds to local queue
//! - **SpawnWithBroadcast**: Calls `engine.broadcast()` - sends to global queue for any thread
//!
//! This enables different execution strategies through the Spawner type pattern.

use crate::valtron::{
    spawn_broadcaster, spawn_builder, BoxedExecutionEngine, ExecutionAction, GenericResult,
    NoAction, TaskIterator, TaskStatus,
};
use std::marker::PhantomData;

// ============================================================================
// WrapTask - Wrap plain values in TaskStatus::Ready
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

// ============================================================================
// LiftTask - Pass through TaskStatus from iterator
// ============================================================================

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

// ============================================================================
// WrapAction - Action for wrapping plain value iterators
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
                .with_parent(key.clone())
                .with_task(task)
                .schedule()?;
        }
        Ok(())
    }
}

// ============================================================================
// SpawnWithLift - Action for TaskStatus iterators
// ============================================================================

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

// ============================================================================
// SpawnWithSchedule - Execute closures as tasks
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

// ============================================================================
// SpawnWithBroadcast - Send values to multiple receivers
// ============================================================================

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
    T: Clone + 'static,
{
    value: Option<T>,
    callbacks: Vec<Box<dyn FnOnce(T) + Send>>,
}

impl<T> SpawnWithBroadcast<T>
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
// SpawnStrategy - Enum combining action types
// ============================================================================

/// Enum that combines all action types plus a custom action slot.
///
/// WHY: Allows using different action types through a single enum type
/// WHAT: Enum with variants for each action type, delegates to inner action
pub enum SpawnStrategy<IW, TW, IL, DL, PL, SL, F, V, C>
where
    IW: Iterator<Item = TW> + 'static,
    TW: 'static,
    IL: Iterator<Item = TaskStatus<DL, PL, SL>> + 'static,
    DL: 'static,
    PL: 'static,
    SL: ExecutionAction + 'static,
    F: FnOnce() + 'static,
    V: Clone + 'static,
    C: ExecutionAction,
{
    None,
    Wrap(WrapAction<IW, TW>),
    Lift(SpawnWithLift<IL, DL, PL, SL>),
    Schedule(SpawnWithSchedule<F>),
    Broadcast(SpawnWithBroadcast<V>),
    Custom(C),
}

impl<IW, TW, IL, DL, PL, SL, F, V, C> ExecutionAction
    for SpawnStrategy<IW, TW, IL, DL, PL, SL, F, V, C>
where
    IW: Iterator<Item = TW> + 'static,
    TW: 'static,
    IL: Iterator<Item = TaskStatus<DL, PL, SL>> + 'static,
    DL: 'static,
    PL: 'static,
    SL: ExecutionAction + 'static,
    F: FnOnce() + 'static,
    V: Clone + Send + 'static,
    C: ExecutionAction,
{
    fn apply(
        &mut self,
        key: crate::synca::Entry,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<()> {
        match self {
            Self::None => Ok(()),
            Self::Wrap(action) => action.apply(key, engine),
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
    use std::time::Duration;

    // ============================================================================
    // PHASE 1: WrapTask Tests (formerly LiftTask)
    // ============================================================================

    /// WHY: WrapTask must convert plain values to TaskStatus::Ready
    /// WHAT: Construct WrapTask from vector iterator of plain integers
    #[test]
    fn test_wrap_task_from_vec_iterator() {
        let items = vec![1, 2, 3];
        let mut task = WrapTask::new(items.into_iter());

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

    /// WHY: WrapAction must wrap plain iterator in ExecutionAction for spawning
    /// WHAT: Create action that spawns a wrapped iterator task
    #[test]
    fn test_wrap_action_creates_spawnable_task() {
        let items = vec![10, 20, 30];
        let action = WrapAction::new(items.into_iter());

        // Action should be an ExecutionAction
        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    /// WHY: WrapTask should work with different iterator types
    /// WHAT: Test with range iterator
    #[test]
    fn test_wrap_task_with_range() {
        let mut task = WrapTask::new(0..3);

        assert_eq!(task.next(), Some(TaskStatus::Ready(0)));
        assert_eq!(task.next(), Some(TaskStatus::Ready(1)));
        assert_eq!(task.next(), Some(TaskStatus::Ready(2)));
        assert_eq!(task.next(), None);
    }

    // ============================================================================
    // PHASE 2: LiftTask Tests (NEW - preserves TaskStatus)
    // ============================================================================

    /// WHY: LiftTask must preserve Pending states without wrapping
    /// WHAT: TaskStatus::Pending should pass through as-is
    #[test]
    fn test_lift_task_preserves_pending_state() {
        let iter = vec![TaskStatus::<i32, (), NoAction>::Pending(())].into_iter();
        let mut task = LiftTask::new(iter);

        match task.next() {
            Some(TaskStatus::Pending(())) => {} // Correct! Not Ready(Pending(()))
            other => panic!("Expected Pending(()), got {:?}", other),
        }
    }

    /// WHY: LiftTask must preserve Delayed states without wrapping
    /// WHAT: TaskStatus::Delayed should pass through as-is
    #[test]
    fn test_lift_task_preserves_delayed_state() {
        let dur = Duration::from_secs(1);
        let iter = vec![TaskStatus::<i32, (), NoAction>::Delayed(dur)].into_iter();
        let mut task = LiftTask::new(iter);

        match task.next() {
            Some(TaskStatus::Delayed(d)) if d == dur => {} // Correct!
            other => panic!("Expected Delayed({:?}), got {:?}", dur, other),
        }
    }

    /// WHY: LiftTask must preserve Ready states as-is
    /// WHAT: TaskStatus::Ready should pass through without double-wrapping
    #[test]
    fn test_lift_task_preserves_ready_state() {
        let iter = vec![TaskStatus::<i32, (), NoAction>::Ready(42)].into_iter();
        let mut task = LiftTask::new(iter);

        match task.next() {
            Some(TaskStatus::Ready(42)) => {} // Correct! Not Ready(Ready(42))
            other => panic!("Expected Ready(42), got {:?}", other),
        }
    }

    /// WHY: LiftTask must handle mixed TaskStatus sequences correctly
    /// WHAT: Test sequence of different TaskStatus variants
    #[test]
    fn test_lift_task_with_mixed_statuses() {
        let dur = Duration::from_millis(500);
        let iter = vec![
            TaskStatus::<i32, (), NoAction>::Pending(()),
            TaskStatus::<i32, (), NoAction>::Delayed(dur),
            TaskStatus::<i32, (), NoAction>::Ready(100),
            TaskStatus::<i32, (), NoAction>::Pending(()),
        ]
        .into_iter();

        let mut task = LiftTask::new(iter);

        assert!(matches!(task.next(), Some(TaskStatus::Pending(()))));
        assert!(matches!(task.next(), Some(TaskStatus::Delayed(d)) if d == dur));
        assert!(matches!(task.next(), Some(TaskStatus::Ready(100))));
        assert!(matches!(task.next(), Some(TaskStatus::Pending(()))));
        assert!(task.next().is_none());
    }

    /// WHY: SpawnWithLift must be spawnable with TaskStatus iterators
    /// WHAT: Create action that spawns a LiftTask
    #[test]
    fn test_lift_action_is_execution_action() {
        let iter = vec![TaskStatus::<i32, (), NoAction>::Ready(1)].into_iter();
        let action = SpawnWithLift::new(iter);

        let _: Box<dyn ExecutionAction> = Box::new(action);
    }

    // ============================================================================
    // PHASE 2.5: Composition Tests - The Core Pattern
    // ============================================================================

    /// WHY: The CORE INSIGHT - Plain iterator -> WrapTask wraps values in Ready
    /// WHAT: WrapTask is the entry point for plain values into TaskIterator world
    #[test]
    fn test_wrap_task_is_entry_point() {
        // Start with plain values
        let plain = vec![1, 2, 3].into_iter();

        // WrapTask wraps them in Ready
        let mut wrapped = WrapTask::new(plain);

        assert_eq!(wrapped.next(), Some(TaskStatus::Ready(1)));
        assert_eq!(wrapped.next(), Some(TaskStatus::Ready(2)));
        assert_eq!(wrapped.next(), Some(TaskStatus::Ready(3)));
        assert_eq!(wrapped.next(), None);
    }

    /// WHY: LiftTask accepts iterators that ALREADY produce TaskStatus
    /// WHAT: LiftTask forwards the TaskStatus without nesting
    #[test]
    fn test_lift_task_accepts_status_iterator() {
        // Iterator that produces TaskStatus (NOT a TaskIterator)
        let status_iter = vec![
            TaskStatus::<i32, (), NoAction>::Ready(1),
            TaskStatus::<i32, (), NoAction>::Pending(()),
            TaskStatus::<i32, (), NoAction>::Ready(2),
        ]
        .into_iter();

        // LiftTask makes it a TaskIterator, forwarding states
        let mut lifted = LiftTask::new(status_iter);

        assert_eq!(lifted.next(), Some(TaskStatus::Ready(1)));
        assert_eq!(lifted.next(), Some(TaskStatus::Pending(())));
        assert_eq!(lifted.next(), Some(TaskStatus::Ready(2)));
        assert_eq!(lifted.next(), None);
    }

    /// WHY: Composition with wrappers (like TimeoutTask) should preserve states
    /// WHAT: Timeout wraps WrapTask (a TaskIterator) and forwards Ready states
    #[cfg(feature = "std")]
    #[test]
    fn test_composition_wrap_then_timeout() {
        use super::super::wrappers::TimeoutTask;

        // Plain values -> WrapTask -> TimeoutTask
        let plain = vec![1, 2, 3].into_iter();
        let wrapped = WrapTask::new(plain);
        let mut timed = TimeoutTask::new(wrapped, Duration::from_secs(10));

        // Should get Ready states through the composition
        assert_eq!(timed.next(), Some(TaskStatus::Ready(1.into())));
        assert_eq!(timed.next(), Some(TaskStatus::Ready(2.into())));
        assert_eq!(timed.next(), Some(TaskStatus::Ready(3.into())));
        assert!(timed.next().is_none());
    }

    /// WHY: The forwarding pattern enables multi-layer composition
    /// WHAT: TaskStatus -> LiftTask -> TimeoutTask preserves all states
    #[cfg(feature = "std")]
    #[test]
    fn test_composition_lift_then_timeout_preserves_pending() {
        use super::super::wrappers::TimeoutTask;

        // Create iterator with mixed states
        let statuses = vec![
            TaskStatus::<i32, (), NoAction>::Pending(()),
            TaskStatus::<i32, (), NoAction>::Ready(42),
        ]
        .into_iter();

        // LiftTask forwards states
        let lifted = LiftTask::new(statuses);

        // TimeoutTask wraps Pending but forwards Ready
        let mut timed = TimeoutTask::new(lifted, Duration::from_secs(10));

        // First should be Pending (wrapped in TimeoutState)
        match timed.next() {
            Some(TaskStatus::Pending(_)) => {} // Correct! TimeoutTask wraps pending
            other => panic!("Expected Pending, got {:?}", other),
        }

        // Second should be Ready (forwarded as-is)
        assert_eq!(timed.next(), Some(TaskStatus::Ready(42.into())));
        assert!(timed.next().is_none());
    }

    // ============================================================================
    // PHASE 3: SpawnWithSchedule Tests
    // ============================================================================

    /// WHY: SpawnWithSchedule must allow scheduling closures as one-shot tasks
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

    /// WHY: SpawnWithSchedule must create spawnable one-shot tasks
    /// WHAT: Wrap closure in ExecutionAction
    #[test]
    fn test_schedule_action_is_execution_action() {
        let counter = Arc::new(StdMutex::new(0));
        let counter_clone = counter.clone();

        let action = SpawnWithSchedule::new(move || {
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

    // ============================================================================
    // PHASE 4: SpawnWithBroadcast Tests
    // ============================================================================

    /// WHY: SpawnWithBroadcast must send values to multiple receivers
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

    /// WHY: SpawnWithBroadcast must be spawnable
    /// WHAT: Wrap broadcast task in ExecutionAction
    #[test]
    fn test_broadcast_action_is_execution_action() {
        let action = SpawnWithBroadcast::new(100, vec![]);
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

    // ============================================================================
    // PHASE 5: SpawnStrategy Tests
    // ============================================================================

    /// WHY: SpawnStrategy must support different action types via enum
    /// WHAT: SpawnStrategy::None variant compiles and is an ExecutionAction
    #[test]
    fn test_composite_action_none_variant() {
        type TestComposite = SpawnStrategy<
            std::ops::Range<i32>,
            i32,
            std::vec::IntoIter<TaskStatus<i32, (), NoAction>>,
            i32,
            (),
            NoAction,
            fn(),
            i32,
            NoAction,
        >;

        let composite: TestComposite = SpawnStrategy::None;
        let _: Box<dyn ExecutionAction> = Box::new(composite);
    }

    /// WHY: SpawnStrategy should work with Wrap variant
    /// WHAT: Wraps WrapAction in enum
    #[test]
    fn test_composite_action_with_wrap() {
        type TestComposite = SpawnStrategy<
            std::vec::IntoIter<i32>,
            i32,
            std::vec::IntoIter<TaskStatus<i32, (), NoAction>>,
            i32,
            (),
            NoAction,
            fn(),
            i32,
            NoAction,
        >;

        let wrap = WrapAction::new(vec![1, 2, 3].into_iter());
        let composite: TestComposite = SpawnStrategy::Wrap(wrap);
        let _: Box<dyn ExecutionAction> = Box::new(composite);
    }

    /// WHY: SpawnStrategy should work with Lift variant
    /// WHAT: Wraps SpawnWithLift in enum
    #[test]
    fn test_composite_action_with_lift() {
        type TestComposite = SpawnStrategy<
            std::ops::Range<i32>,
            i32,
            std::vec::IntoIter<TaskStatus<i32, (), NoAction>>,
            i32,
            (),
            NoAction,
            fn(),
            i32,
            NoAction,
        >;

        let iter = vec![TaskStatus::<i32, (), NoAction>::Ready(1)].into_iter();
        let lift = SpawnWithLift::new(iter);
        let composite: TestComposite = SpawnStrategy::Lift(lift);
        let _: Box<dyn ExecutionAction> = Box::new(composite);
    }

    /// WHY: SpawnStrategy should work with Schedule variant
    /// WHAT: Wraps SpawnWithSchedule in enum
    #[test]
    fn test_composite_action_with_schedule() {
        fn dummy_fn() {}

        type TestComposite = SpawnStrategy<
            std::ops::Range<i32>,
            i32,
            std::vec::IntoIter<TaskStatus<i32, (), NoAction>>,
            i32,
            (),
            NoAction,
            fn(),
            i32,
            NoAction,
        >;

        let schedule = SpawnWithSchedule::new(dummy_fn as fn());
        let composite: TestComposite = SpawnStrategy::Schedule(schedule);
        let _: Box<dyn ExecutionAction> = Box::new(composite);
    }
}
