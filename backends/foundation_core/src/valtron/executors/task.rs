use std::{any::Any, time};

use super::ExecutionAction;

/// The type for a panic handling closure. Note that this same closure
/// may be invoked multiple times in parallel.
pub type PanicHandler = dyn Fn(Box<dyn Any + Send>) + Send + Sync;

pub type BoxedPanicHandler = Box<PanicHandler>;

/// completed and deliverd from the iterator.
#[derive(Clone)]
pub enum TaskStatus<D, P, S: ExecutionAction> {
    /// Allows a task status to communicate a delay
    /// to continued operation.
    Delayed(time::Duration),

    /// Allows you from within an executor
    /// spawn a new task that either will take
    /// priority on the same thread or be handled
    /// by an open thread from the global task queue
    /// when there is any available to process work
    Spawn(S),

    /// Pending is a state indicative of the status
    /// of still awaiting the readiness of some operations
    /// this can be the an underlying process waiting for
    /// some timeout to expire or a response to be received
    /// over IO or the network.
    ///
    /// Generally you send this to indicate the task as still
    /// being in a state of processing.
    Pending(P),

    /// Init represents a middle point state where the process
    /// may not immediately move into a ready state e.g reconnect
    /// to some remote endpoint or to trigger some actual underlying
    /// processes that get us into a ready state with
    /// the relevant result .
    Init,

    /// Ready is the final state where we consider the task
    /// has finished/ended with relevant result.
    Ready(D),
}

impl<D: PartialEq, P: PartialEq, S: ExecutionAction> Eq for TaskStatus<D, P, S> {}

impl<D: PartialEq, P: PartialEq, S: ExecutionAction> PartialEq for TaskStatus<D, P, S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TaskStatus::Delayed(me), TaskStatus::Delayed(them)) => me == them,
            (TaskStatus::Pending(me), TaskStatus::Pending(them)) => me == them,
            (TaskStatus::Ready(me), TaskStatus::Ready(them)) => me == them,
            (TaskStatus::Spawn(_), TaskStatus::Spawn(_)) => true,
            (TaskStatus::Init, TaskStatus::Init) => true,
            _ => false,
        }
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug, S: ExecutionAction> core::fmt::Debug
    for TaskStatus<D, P, S>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(unused)]
        #[derive(Debug)]
        enum TStatus<D, P> {
            Delayed(time::Duration),
            Pending(P),
            Ready(D),
            Init,
            Spawn,
        }

        let debug_item = match self {
            TaskStatus::Delayed(duration) => TStatus::Delayed(duration.clone()),
            TaskStatus::Pending(inner) => TStatus::Pending(inner),
            TaskStatus::Ready(inner) => TStatus::Ready(inner),
            TaskStatus::Spawn(_) => TStatus::Spawn,
            TaskStatus::Init => TStatus::Init,
        };

        write!(f, "{:?}", debug_item)
    }
}

/// AsTaskIterator represents a type for an iterator with
/// the underlying output of the iterator to be `TaskStatus`
/// and it's relevant semantics.
pub type AsTaskIterator<D, P, S> = dyn Iterator<Item = TaskStatus<D, P, S>>;

/// BoxedTaskIterator provides a Boxed (heap-allocated) Iterator based on a TaskIterator.
pub type BoxedTaskIterator<D, P, S> = Box<dyn Iterator<Item = TaskStatus<D, P, S>>>;

/// TaskIterator is an iterator engineered around the concept of asynchronouse
/// task that have 3 states: PENDING, INIT (Initializing) and READY.
///
/// This follows the Iterators in that when the iterator returns None then it's
/// finished, else we can assume its always producing results that might be in any of
/// those states both for either a singular result or multiple elements.
///
/// This means this can keep producing elements as and the caller simply takes the
/// `TaskStatus::Done` state results (if they only care about the final value)
/// else use the other state for whatever is sensible.
///
/// One thing to note is, the same restrictions apply with these iterators, you do
/// not want to block the thread where this is executed or other processes and its always
/// adviced to finish early by moving long processes into other threads that allow you
/// noify once next is called to notify if the task is done or still pending.
pub trait TaskIterator {
    /// The type to indicate pending operation, either can be a `time::Duration`
    /// to communicate time to completion or some other type you wish
    /// to allow communication of the state of things before the `Done` signal
    /// is received.
    type Pending;

    /// The type that indicates when a single or partial sequence
    /// is what can be considered a intermediate and final result is.
    ///
    /// In the case of a stream this is the type you wish to produce every single
    /// time that is actual data.
    type Done;

    /// The type responsible for orchestrating spawn actions
    /// against the local execution engine.
    type Spawner: ExecutionAction;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<TaskStatus<Self::Done, Self::Pending, Self::Spawner>>;

    /// into_iter consumes the implementation and wraps
    /// it in an iterator type that emits
    /// `TaskStatus<TaskIterator::Pending ,TaskIterator::Done>`
    /// match the behavior desired for an iterator.
    fn into_iter(self) -> impl Iterator<Item = TaskStatus<Self::Done, Self::Pending, Self::Spawner>>
    where
        Self: Sized + 'static,
    {
        TaskAsIterator(Box::new(self))
    }
}

pub struct TaskAsIterator<D, P, S>(Box<dyn TaskIterator<Done = D, Pending = P, Spawner = S>>);

impl<D, P, S> TaskAsIterator<D, P, S> {
    pub fn from_impl(t: impl TaskIterator<Done = D, Pending = P, Spawner = S> + 'static) -> Self {
        Self(Box::new(t))
    }

    pub fn new(t: Box<dyn TaskIterator<Done = D, Pending = P, Spawner = S>>) -> Self {
        Self(t)
    }
}

impl<D, P, S: ExecutionAction> Iterator for TaskAsIterator<D, P, S> {
    type Item = TaskStatus<D, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[cfg(test)]
mod test_task_iterator {
    use std::time;

    use crate::valtron::NoAction;

    use super::*;

    #[test]
    fn vec_iterator_is_task_iterator() {
        let tasks: Vec<TaskStatus<(), (), NoAction>> = vec![
            TaskStatus::Delayed(time::Duration::from_secs(1)),
            TaskStatus::Pending(()),
            TaskStatus::Init,
            TaskStatus::Ready(()),
        ];

        let task_iterator = Box::new(tasks.into_iter());

        let collected_tasks: Vec<TaskStatus<(), (), NoAction>> = task_iterator.collect();
        assert_eq!(
            collected_tasks,
            vec![
                TaskStatus::Delayed(time::Duration::from_secs(1)),
                TaskStatus::Pending(()),
                TaskStatus::Init,
                TaskStatus::Ready(()),
            ]
        );
    }
}
