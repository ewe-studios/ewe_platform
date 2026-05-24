//! Drivenr implementations that seamlessly wrap and making interacting with
//! valtron iterators seamless and unseen.

#![allow(clippy::type_complexity)]

use crate::valtron::{
    ExecutionAction, NotificationItem, NotifyQueueStreamIterator, NotifyRecvIterator,
    ProgressIndicator, State, Stream, TaskIterator, TaskStatus,
};

// ===========================================
// Pool initializers
// ===========================================

/// [`initialize_pool`] provides a unified method to initialize the underlying
/// thread pool for both the single and multi-threaded instances.
///
/// Returns a `PoolGuard` that must be kept alive for the pool to remain functional.
/// When the guard is dropped, all threads are shut down.
#[cfg(not(feature = "multi"))]
#[must_use]
pub fn initialize_pool(
    seed_for_rng: u64,
    _user_thread_num: Option<usize>,
) -> super::single::PoolGuard {
    #[cfg(target_arch = "wasm32")]
    {
        use super::single;
        tracing::debug!("Starting under wasm");
        single::initialize_pool(seed_for_rng);
        return single::PoolGuard::default();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use super::single;
        tracing::debug!("Starting under not(feature=multi), so single threaded");
        single::initialize_pool(seed_for_rng);
        single::PoolGuard::default()
    }
}

#[cfg(feature = "multi")]
#[must_use]
pub fn initialize_pool(
    seed_for_rng: u64,
    user_thread_num: Option<usize>,
) -> crate::valtron::multi::PoolGuard {
    use crate::valtron::multi;
    tracing::debug!("Starting under feature=multi");
    multi::initialize_pool(seed_for_rng, user_thread_num)
}

// ===========================================
// Runner Methods
// ===========================================

/// [`run_until_next_state`] this is a no-op in multi-threaded situations (i.e multi feature flag is on), but
/// under multi=off or wasm execution, this will  executing the execution engine until the next valid
/// state is seen to indicate task has reported progress as [`State`].
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
#[tracing::instrument()]
pub fn run_until_next_state() {
    run_until_next_acceptable_state(|candidate| {
        !matches!(
            candidate,
            State::SpawnFailed(_)
                | State::SpawnFinished(_)
                | State::Reschedule
                | State::Done
                | State::Wait
        )
    });
}

/// [`run_until_next_acceptable_state`] this is a no-op in multi-threaded situations (i.e multi feature flag is on), but
/// under multi=off or wasm execution, this will  executing the execution engine until the next valid
/// state within the provided `checker` function is seen to indicate task has reported progress as [`State`].
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
pub fn run_until_next_acceptable_state<T>(checker: T)
where
    T: Fn(&State) -> bool,
{
    #[cfg_attr(not(feature = "multi"), allow(unused_variables))]
    run_until(|indicator: ProgressIndicator| {
        if let ProgressIndicator::CanProgress(Some(state)) = indicator {
            if checker(&state) {
                tracing::debug!("Valtron: checker returned true for state={state:?}");
                true
            } else {
                false
            }
        } else {
            false
        }
    });
}

/// [`run_until_next_state_within`] this is a no-op in multi-threaded situations (i.e multi feature flag is on), but
/// under multi=off or wasm execution, this will  executing the execution engine until the next valid
/// state within the provided `target_states` is seen to indicate task has reported progress as [`State`].
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
pub fn run_until_next_state_within(target_states: &[State]) {
    run_until(|indicator: ProgressIndicator| {
        if let ProgressIndicator::CanProgress(Some(state)) = indicator {
            if target_states.contains(&state) {
                tracing::debug!("Valtron: Seen new state value from state={state:?}");
                true
            } else {
                false
            }
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
#[tracing::instrument()]
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

/// [`run_until_receiver_has_value`] with a receiver object until the receiver object has a value(s)
/// to report or the.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
#[tracing::instrument(skip(stream, checker))]
pub fn run_until_receiver_has_value<T, S>(
    stream: NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
    #[allow(unused_variables)] checker: S,
) -> NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>
where
    S: Fn(ProgressIndicator) -> bool,
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use super::single;

        tracing::debug!("Executing as a single-threaded stream in no-wasm");
        while stream.is_empty() {
            single::run_until(&checker);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single stream in wasm");
        while stream.is_empty() {
            single::run_until(&checker);
        }
    }

    stream
}

/// [`run_until_stream_has_value`] with a stream object until the stream object has a value(s)
/// to report or the.
///
/// In single-threaded mode, this drives the executor to make progress.
/// In multi-threaded mode, this waits for the concurrent queue to receive items.
#[tracing::instrument(skip(stream, checker))]
pub fn run_until_stream_has_value<T, S>(
    stream: NotifyQueueStreamIterator<T::Ready, T::Pending>,
    #[allow(unused_variables)] checker: S,
) -> NotifyQueueStreamIterator<T::Ready, T::Pending>
where
    S: Fn(ProgressIndicator) -> bool,
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use super::single;

        tracing::debug!("Executing as a single-threaded stream in no-wasm");
        while stream.is_empty() && !stream.is_closed() {
            single::run_until(&checker);
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;

        tracing::debug!("Executing as a single stream in wasm");
        while stream.is_empty() && !stream.is_closed() {
            single::run_until(&checker);
        }
    }

    stream
}

/// `run_until` provides a unified method to attempt to execute the `run_until`.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
#[tracing::instrument(skip(checker))]
pub fn run_until<T>(#[allow(unused_variables)] checker: T)
where
    T: Fn(ProgressIndicator) -> bool,
{
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use super::single;

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
#[tracing::instrument()]
pub fn run_until_complete() {
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use super::single;

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

/// `run_once` provides a unified method to attempt to execute the `run_once`.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
#[tracing::instrument()]
pub fn run_once() {
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use super::single;

        tracing::trace!("Executing as a single-threaded stream in no-wasm");
        let _ = single::run_once();
    }

    #[cfg(target_arch = "wasm32")]
    {
        use crate::valtron::single;

        tracing::trace!("Executing as a single stream in wasm");
        let _ = single::run_once();
    }
}

// ===========================================
// Iterator Execution Drivenrs: Making seamless call to next without
// bothering about driving execution directly in your business logic.
// ===========================================

pub struct DrivenNonSendTaskIterator<T>(Option<T>)
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static;

impl<T> DrivenNonSendTaskIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    pub fn new(task_iterator: T) -> Self {
        Self(Some(task_iterator))
    }
}

impl<T> Iterator for DrivenNonSendTaskIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    type Item = TaskStatus<T::Ready, T::Pending, T::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            // execute the execution engine until the next state is ready.
            run_until_next_state();

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

///[`DrivenSendTaskIterator`] is a wrapper around a [`TaskIterator`] that drives the state of the stream.
/// This is good for send-safe types you want to be send-safe.
///
/// It internally uses the [`run_until_next_state`] function.
pub struct DrivenSendTaskIterator<T>(Option<T>)
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

impl<T> DrivenSendTaskIterator<T>
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

unsafe impl<T> Send for DrivenSendTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
}

impl<T> Iterator for DrivenSendTaskIterator<T>
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

            // execute the execution engine until the next state is ready.
            run_until_next_state();

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

///[`DrivenSendTaskIterator`] is a wrapper around a [`TaskIterator`] that drives the state of the stream.
/// This is good for non send-safe types you want to use in non-send contexts.
///
/// It internally uses the [`run_until_next_state`] function.
pub struct DrivenNonSendStreamIterator<T>(Option<NotifyQueueStreamIterator<T::Ready, T::Pending>>)
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static;

impl<T> DrivenNonSendStreamIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    #[must_use]
    pub fn new(task_iterator: NotifyQueueStreamIterator<T::Ready, T::Pending>) -> Self {
        Self(Some(task_iterator))
    }
}

impl<T> Iterator for DrivenNonSendStreamIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    type Item = Stream<T::Ready, T::Pending>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            tracing::trace!("Run: run_until_next_state");
            // execute the execution engine until the next state is ready.
            run_until_next_state();

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

pub struct DrivenSendStreamIterator<T>(Option<NotifyQueueStreamIterator<T::Ready, T::Pending>>)
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

impl<T> DrivenSendStreamIterator<T>
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

unsafe impl<T> Send for DrivenSendStreamIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
}

impl<T> Iterator for DrivenSendStreamIterator<T>
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

            // execute the execution engine until the next state is ready.
            task_iterator = run_until_stream_has_value::<T, _>(task_iterator, |indicator| {
                if let ProgressIndicator::CanProgress(Some(state)) = indicator {
                    if matches!(
                        state,
                        State::SpawnFailed(_)
                            | State::SpawnFinished(_)
                            | State::Reschedule
                            | State::Done
                            | State::Wait
                    ) {
                        false
                    } else {
                        tracing::debug!("Valtron: checker returned true for state={state:?}");
                        true
                    }
                } else {
                    false
                }
            });

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

pub struct DrivenSendRecvIterator<T>(
    Option<NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>,
)
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static;

impl<T> DrivenSendRecvIterator<T>
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
unsafe impl<T> Send for DrivenSendRecvIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
}

impl<T> Iterator for DrivenSendRecvIterator<T>
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

            // execute the execution engine until the next state is ready.
            task_iterator = run_until_receiver_has_value::<T, _>(task_iterator, |indicator| {
                if let ProgressIndicator::CanProgress(Some(state)) = indicator {
                    if matches!(
                        state,
                        State::SpawnFailed(_)
                            | State::SpawnFinished(_)
                            | State::Reschedule
                            | State::Wait
                    ) {
                        false
                    } else {
                        tracing::debug!("Valtron: checker returned true for state={state:?}");
                        true
                    }
                } else {
                    false
                }
            });

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

pub struct DrivenNonSendRecvIterator<T>(
    Option<NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>,
)
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static;

impl<T> DrivenNonSendRecvIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    #[must_use]
    pub fn new(
        task_iterator: NotifyRecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
    ) -> Self {
        Self(Some(task_iterator))
    }
}

impl<T> Iterator for DrivenNonSendRecvIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    type Item = TaskStatus<T::Ready, T::Pending, T::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut task_iterator) = self.0.take() {
            tracing::trace!("Run: run_until_next_state");

            // execute the execution engine until the next state is ready.
            run_until_next_state();

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

// ===========================================
// Type aliases that resolve to Send or non-Send variants based on feature flag.
// This lets structs store `DrivenStreamIterator<T>` without forcing `Send` bounds
// at the struct level — the function that constructs them decides.
// ===========================================

/// Under `multi=off`, resolves to the non-Send stream iterator.
/// Under `multi=on`, resolves to the Send stream iterator.
#[cfg(not(feature = "multi"))]
pub type DrivenStreamIterator<T> = DrivenNonSendStreamIterator<T>;

#[cfg(feature = "multi")]
pub type DrivenStreamIterator<T> = DrivenSendStreamIterator<T>;

/// Under `multi=off`, resolves to the non-Send recv iterator.
/// Under `multi=on`, resolves to the Send recv iterator.
#[cfg(not(feature = "multi"))]
pub type DrivenRecvIterator<T> = DrivenNonSendRecvIterator<T>;

#[cfg(feature = "multi")]
pub type DrivenRecvIterator<T> = DrivenSendRecvIterator<T>;
