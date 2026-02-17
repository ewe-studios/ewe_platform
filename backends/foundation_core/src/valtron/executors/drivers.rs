//! Drivenr implementations that seamlessly wrap and making interacting with
//! valtron iterators seamless and unseen.

#![allow(clippy::type_complexity)]

use crate::{
    synca::mpp::{RecvIterator, Stream, StreamRecvIterator},
    valtron::{ExecutionAction, ProgressIndicator, State, TaskIterator, TaskStatus},
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
// Iterator execution methods
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
