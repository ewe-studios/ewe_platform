use std::time;

use super::ExecutionIterator;

/// TaskStatus represents the current state of a computation to be
/// completed and deliverd from the iterator.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TaskStatus<D, P = ()> {
    /// Allows a task status to communicate a delay
    /// to continued operation.
    Delayed(time::Duration),

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

/// AsTaskIterator represents a type for an iterator with
/// the underlying output of the iterator to be `TaskStatus`
/// and it's relevant semantics.
pub type AsTaskIterator<D, P> = dyn Iterator<Item = TaskStatus<D, P>>;
pub type BoxedTaskIterator<D, P> = Box<dyn Iterator<Item = TaskStatus<D, P>>>;

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
    type Pending;
    type Done;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<TaskStatus<Self::Done, Self::Pending>>;

    /// into_iter consumes the implementation and wraps
    /// it in an iterator type that emits
    /// `TaskStatus<TaskIterator::Pending ,TaskIterator::Done>`
    /// match the behavior desired for an iterator.
    fn into_iter(self) -> impl Iterator<Item = TaskStatus<Self::Done, Self::Pending>>
    where
        Self: Sized + 'static,
    {
        TaskAsIterator(Box::new(self))
    }
}

pub struct TaskAsIterator<D, P>(Box<dyn TaskIterator<Done = D, Pending = P>>);

impl<D, P> TaskAsIterator<D, P> {
    pub fn from_impl(t: impl TaskIterator<Done = D, Pending = P> + 'static) -> Self {
        Self(Box::new(t))
    }

    pub fn new(t: Box<dyn TaskIterator<Done = D, Pending = P>>) -> Self {
        Self(t)
    }
}

impl<D, P> Iterator for TaskAsIterator<D, P> {
    type Item = TaskStatus<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[cfg(test)]
mod test_task_iterator {
    use tokio::time;

    use super::*;

    #[test]
    fn vec_iterator_is_task_iterator() {
        let tasks: Vec<TaskStatus<(), ()>> = vec![
            TaskStatus::Delayed(time::Duration::from_secs(1)),
            TaskStatus::Pending(()),
            TaskStatus::Init,
            TaskStatus::Ready(()),
        ];

        let task_iterator = Box::new(tasks.into_iter());

        let collected_tasks: Vec<TaskStatus<(), ()>> = task_iterator.collect();
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
