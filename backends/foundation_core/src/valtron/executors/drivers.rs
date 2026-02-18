//! Drivenr implementations that seamlessly wrap and making interacting with
//! valtron iterators seamless and unseen.

#![allow(clippy::type_complexity)]

use core::future::Future;

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::valtron::FutureTask;
#[cfg(any(feature = "std", feature = "alloc"))]
use crate::valtron::StreamTask;

use crate::{
    synca::mpp::{RecvIterator, Stream, StreamRecvIterator},
    valtron::{
        ExecutionAction, InlineAction, InlineActionBehaviour, InlineSendAction,
        InlineSendActionBehaviour, ProgressIndicator, State, TaskIterator, TaskStatus,
        TaskStatusMapper,
    },
};

// ===========================================
// Pool initializers
// ===========================================

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
            use crate::valtron::single;
            single::initialize_pool(seed_for_rng);
        }
    }
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
            State::SpawnFailed(_) | State::SpawnFinished(_) | State::Reschedule
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

/// [`run_until_receiver_has_value`] with a reciever object until the receiver object has a value(s)
/// to report or the.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
#[tracing::instrument(skip(stream, checker))]
pub fn run_until_receiver_has_value<T, S>(
    stream: RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
    checker: S,
) -> RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>
where
    S: Fn(ProgressIndicator) -> bool,
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;

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
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
#[tracing::instrument(skip(stream, checker))]
pub fn run_until_stream_has_value<T, S>(
    stream: StreamRecvIterator<T::Ready, T::Pending>,
    checker: S,
) -> StreamRecvIterator<T::Ready, T::Pending>
where
    S: Fn(ProgressIndicator) -> bool,
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "multi")))]
    {
        use crate::valtron::single;

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

/// [`run_until`] provides a unified method to attempt to execute the `run_until`.
/// in a non cfg way by encapsulating that call and configuration into this method.
///
/// This really only apply for single threaded situations (multi=off feature flag) and wasm context.
#[tracing::instrument(skip(checker))]
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
#[tracing::instrument()]
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
#[tracing::instrument()]
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

// ===========================================
// Future based iterators methods
// ===========================================

/// [`from_future`] creates a new [`DrivenSendTaskIterator<FutureTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with [`run_until`]
/// types of function calls and abstract out that portion.
///
/// It relies on the [`drive_iter`] method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(all(any(feature = "std", feature = "alloc"), target_arch = "wasm32"))]
pub fn drive_future<F>(future: F) -> DrivenNonSendTaskIterator<FutureTask<F>>
where
    F: Future + 'static,
    F::Output: 'static,
{
    drive_non_send_iterator(crate::valtron::from_future(future))
}

/// [`from_future`] creates a new [`DrivenSendTaskIterator<FutureTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with [`run_until`]
/// types of function calls and abstract out that portion.
///
/// It relies on the [`drive_iter`] method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
pub fn drive_future<F>(future: F) -> DrivenSendTaskIterator<FutureTask<F>>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    drive_iterator(crate::valtron::from_future(future))
}

/// [`from_future_stream`] creates a new [`DrivenSendTaskIterator<StreamTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with [`run_until`]
/// types of function calls and abstract out that portion.
///
/// It relies on the [`drive_iter`] method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(all(any(feature = "std", feature = "alloc"), not(target_arch = "wasm32")))]
pub fn drive_future_stream<S>(stream: S) -> DrivenSendTaskIterator<StreamTask<S>>
where
    S: futures_core::Stream + Send + 'static,
    S::Item: Send + 'static,
{
    drive_iterator(crate::valtron::from_stream(stream))
}

/// [`from_future`] creates a new [`DrivenNonSendTaskIterator<FutureTask<S>>`]
/// task iterator which will internally drive the state of the stream in single threaded
/// environments or rely on the multi-threaded executor in multi-threaded environments.
///
/// This makes it easy to hide away the need to litter your project codebase with [`run_until`]
/// types of function calls and abstract out that portion.
///
/// It relies on the [`drive_iter`] method to drive the state of the stream which internally
/// uses the [`run_until_next_state`] function.
#[cfg(all(any(feature = "std", feature = "alloc"), target_arch = "wasm32"))]
pub fn drive_future_stream<S>(stream: S) -> DrivenNonSendTaskIterator<StreamTask<S>>
where
    S: futures_core::Stream + 'static,
    S::Item: 'static,
{
    drive_non_send_iterator(crate::valtron::from_stream(future))
}

// ===========================================
// inline iterator creation methods
// ===========================================

/// [`inlined_non_send_mapped_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using [`BoxedTaskAction`]
///
/// You then are able to receive the output of that task from the returned
/// channel [`RecvIterator<TaskStatus<Done, Pending, Action>>`].
///
/// This is predominantly when you specifically do not want a Send action and receiver type.
pub fn inlined_non_send_mapped_task<Done, Pending, Action, Task, Mapper>(
    behaviour: InlineActionBehaviour,
    mappers: Vec<Mapper>,
    task: Task,
    wait_cycle: std::time::Duration,
) -> (
    InlineAction<Done, Pending, Action, Task, Mapper>,
    DrivenNonSendRecvIterator<Task>,
)
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    let (task_action, task_receiver) = InlineAction::new(behaviour, mappers, task, wait_cycle);
    (task_action, drive_non_send_receiver(task_receiver))
}

/// [`inlined_non_send_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using boxed [`TaskStatusMapper`].
///
/// You then are able to receive the output of that task from the returned
/// channel [`RecvIterator<TaskStatus<Done, Pending, Action>>`].
///
/// This is predominantly when you specifically do not want a Send action and receiver type.
pub fn inlined_non_send_task<Done, Pending, Action, Task>(
    behaviour: InlineActionBehaviour,
    mappers: Vec<Box<dyn TaskStatusMapper<Done, Pending, Action> + 'static>>,
    task: Task,
    wait_cycle: std::time::Duration,
) -> (
    InlineAction<
        Done,
        Pending,
        Action,
        Task,
        Box<dyn TaskStatusMapper<Done, Pending, Action> + 'static>,
    >,
    DrivenNonSendRecvIterator<Task>,
)
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    let (task_action, task_receiver) =
        InlineAction::boxed_mapper(behaviour, mappers, task, wait_cycle);
    (task_action, drive_non_send_receiver(task_receiver))
}

/// [`inlined_mapped_task`] creates an inlined task you can use within another task that
/// lets you forward the task as a action your main task can send for execution
/// as part of it's process, allowing you to define the Spawner type for the parent
/// task in a specific type or using [`BoxedTaskAction`]
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

/// [`drive_non_send_iterator`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_non_send_iterator<T>(incoming: T) -> DrivenNonSendTaskIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    DrivenNonSendTaskIterator::new(incoming)
}

/// [`drive_iterator`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_iterator<T>(incoming: T) -> DrivenSendTaskIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    DrivenSendTaskIterator::new(incoming)
}

/// [`drive_non_send_receiver`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_non_send_receiver<T>(
    incoming: RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
) -> DrivenNonSendRecvIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    DrivenNonSendRecvIterator::new(incoming)
}

/// [`drive_receiver`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_receiver<T>(
    incoming: RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>,
) -> DrivenRecvIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    DrivenRecvIterator::new(incoming)
}

/// [`drive_non_send_stream`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_non_send_stream<T>(
    incoming: StreamRecvIterator<T::Ready, T::Pending>,
) -> DrivenNonSendStreamIterator<T>
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
    T::Spawner: ExecutionAction + 'static,
{
    DrivenNonSendStreamIterator::new(incoming)
}

/// [`drive_stream`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
#[tracing::instrument(skip(incoming))]
pub fn drive_stream<T>(
    incoming: StreamRecvIterator<T::Ready, T::Pending>,
) -> DrivenStreamIterator<T>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + 'static,
    T::Pending: Send + 'static,
    T::Spawner: ExecutionAction + Send + 'static,
{
    DrivenStreamIterator::new(incoming)
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
            tracing::debug!("Run: run_until_next_state");

            // execute the execution engine until the next state is ready.
            run_until_next_state();

            tracing::debug!("Get the next value from inner iterator");
            let next_value = task_iterator.next();
            tracing::debug!(
                "Gotten next value from inner iterator: is_some: {}",
                next_value.is_some()
            );

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
            tracing::debug!("Run: run_until_next_state");

            // execute the execution engine until the next state is ready.
            run_until_next_state();

            tracing::debug!("Get the next value from inner iterator");
            let next_value = task_iterator.next();
            tracing::debug!(
                "Gotten next value from inner iterator: is_some: {}",
                next_value.is_some()
            );

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
pub struct DrivenNonSendStreamIterator<T>(Option<StreamRecvIterator<T::Ready, T::Pending>>)
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
    pub fn new(task_iterator: StreamRecvIterator<T::Ready, T::Pending>) -> Self {
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
            tracing::debug!("Run: run_until_next_state");
            // execute the execution engine until the next state is ready.
            run_until_next_state();

            tracing::debug!("Get the next value from inner iterator");
            let next_value = task_iterator.next();
            tracing::debug!(
                "Gotten next value from inner iterator: is_some: {}",
                next_value.is_some()
            );

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

pub struct DrivenStreamIterator<T>(Option<StreamRecvIterator<T::Ready, T::Pending>>)
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
    pub fn new(task_iterator: StreamRecvIterator<T::Ready, T::Pending>) -> Self {
        Self(Some(task_iterator))
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
            tracing::debug!("Run: run_until_next_state");

            // execute the execution engine until the next state is ready.
            task_iterator = run_until_stream_has_value::<T, _>(task_iterator, |indicator| {
                if let ProgressIndicator::CanProgress(Some(state)) = indicator {
                    if matches!(
                        state,
                        State::SpawnFailed(_) | State::SpawnFinished(_) | State::Reschedule
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

            tracing::debug!("Get the next value from inner iterator");
            let next_value = task_iterator.next();
            tracing::debug!(
                "Gotten next value from inner iterator: is_some: {}",
                next_value.is_some()
            );

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
    Option<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>,
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
    pub fn new(task_iterator: RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>) -> Self {
        Self(Some(task_iterator))
    }
}

// This is safe to send since it contains a type [`RecvIterator`]
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
            tracing::debug!("Run: run_until_next_state");

            // execute the execution engine until the next state is ready.
            task_iterator = run_until_receiver_has_value::<T, _>(task_iterator, |indicator| {
                if let ProgressIndicator::CanProgress(Some(state)) = indicator {
                    if matches!(
                        state,
                        State::SpawnFailed(_) | State::SpawnFinished(_) | State::Reschedule
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

            tracing::debug!("Get the next value from inner iterator");
            let next_value = task_iterator.next();
            tracing::debug!(
                "Gotten next value from inner iterator: is_some: {}",
                next_value.is_some()
            );

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

pub struct DrivenNonSendRecvIterator<T>(
    Option<RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>>,
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
    pub fn new(task_iterator: RecvIterator<TaskStatus<T::Ready, T::Pending, T::Spawner>>) -> Self {
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
            tracing::debug!("Run: run_until_next_state");

            // execute the execution engine until the next state is ready.
            run_until_next_state();

            tracing::debug!("Get the next value from inner iterator");
            let next_value = task_iterator.next();
            tracing::debug!(
                "Gotten next value from inner iterator: is_some: {}",
                next_value.is_some()
            );

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
