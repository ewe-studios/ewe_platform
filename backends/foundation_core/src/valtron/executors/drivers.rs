//! Drivenr implementations that seamlessly wrap and making interacting with
//! valtron iterators seamless and unseen.

#![allow(clippy::type_complexity)]

use crate::{
    synca::mpp::{RecvIterator, Stream, StreamRecvIterator},
    valtron::{executors::unified, ExecutionAction, TaskIterator, TaskStatus},
};

// ===========================================
// Iterator execution methods
// ===========================================

/// [`drive_non_send_iterator`] provides a convenient function to
/// provide a wrapped stream that internally automatically
/// calls the execution methods in the situations of single threaded
/// or wasm context will auto-drive the execution engine.
#[must_use]
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
            unified::run_until_next_state();

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
            unified::run_until_next_state();

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
            unified::run_until_next_state();

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
            unified::run_until_next_state();

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
            unified::run_until_next_state();

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
            unified::run_until_next_state();

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
