use std::time;

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
    /// the relevant result.
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

pub type BoxedTaskReadyResolver<D, P> = Box<dyn TaskReadyResolver<D, P>>;

/// `TaskResolver` are types implementing this trait to
/// perform final resolution of a task when the task emits
/// the relevant `TaskStatus::Ready` enum state.
///
/// Unlike `TaskStatusMapper` these implementing types do
/// not care about the varying states of a `TaskIterator`
/// but about the final state of the task when it signals
/// it's readiness via the `TaskStatus::Ready` state.
pub trait TaskReadyResolver<D, P> {
    fn handle(&self, item: TaskStatus<D, P>);
}

impl<F, D, P> TaskReadyResolver<D, P> for F
where
    F: Fn(TaskStatus<D, P>),
{
    fn handle(&self, item: TaskStatus<D, P>) {
        (self)(item)
    }
}

pub mod resolvers {
    use std::cell;

    use super::*;

    pub struct FnMutReady<F>(cell::RefCell<F>);

    impl<F> FnMutReady<F> {
        pub fn new(f: F) -> Self {
            Self(cell::RefCell::new(f))
        }
    }

    impl<F, D, P> TaskReadyResolver<D, P> for FnMutReady<F>
    where
        F: FnMut(TaskStatus<D, P>),
    {
        fn handle(&self, item: TaskStatus<D, P>) {
            let mut mut_fn = self.0.borrow_mut();
            (mut_fn)(item)
        }
    }

    pub struct FnReady<F>(F);

    impl<F> FnReady<F> {
        pub fn new(f: F) -> Self {
            Self(f)
        }
    }

    impl<F, D, P> TaskReadyResolver<D, P> for FnReady<F>
    where
        F: Fn(TaskStatus<D, P>),
    {
        fn handle(&self, item: TaskStatus<D, P>) {
            self.0(item)
        }
    }

    /// `TaskStatusMapper` are types implementing this trait to
    /// perform unique operations on the underlying `TaskStatus`
    /// received, possibly generating a new `TaskStatus`.
    pub trait TaskStatusMapper<D, P> {
        fn map(&mut self, item: Option<TaskStatus<D, P>>) -> Option<TaskStatus<D, P>>;
    }

    pub struct FnMapper<F>(F);

    impl<F> FnMapper<F> {
        pub fn new(f: F) -> Self {
            Self(f)
        }
    }

    impl<F, D, P> TaskStatusMapper<D, P> for FnMapper<F>
    where
        F: FnMut(TaskStatus<D, P>) -> Option<TaskStatus<D, P>>,
    {
        fn map(&mut self, item: Option<TaskStatus<D, P>>) -> Option<TaskStatus<D, P>> {
            match item {
                None => None,
                Some(item) => self.0(item),
            }
        }
    }

    pub struct FnOptionMapper<F>(F);

    impl<F> FnOptionMapper<F> {
        pub fn new(f: F) -> Self {
            Self(f)
        }
    }

    impl<F, D, P> TaskStatusMapper<D, P> for FnOptionMapper<F>
    where
        F: FnMut(Option<TaskStatus<D, P>>) -> Option<TaskStatus<D, P>>,
    {
        fn map(&mut self, item: Option<TaskStatus<D, P>>) -> Option<TaskStatus<D, P>> {
            self.0(item)
        }
    }

    /// OnceCache implements a TaskStatus iterator that wraps
    /// a provided iterator and provides a onetime read semantic
    /// on the iterator, where it ends its operation once the first
    /// value the iterator is received and returns None from then on.
    ///
    /// It captures the value that allows you to retrieve it via it's
    /// [`OnceCache::take`] method.
    ///
    /// if you prefer an iterator that becomes re-usable again after the
    /// value is taking look at the [`UntilTake`] iterator.
    ///
    /// Usually yo use these types of iterator in instances where you control ownership
    /// of them and can retrieve them after whatever runs them (calling their next)
    /// consider it finished.
    pub struct OnceCache<D, P, T: Iterator<Item = TaskStatus<D, P>>> {
        iter: T,
        used: Option<()>,
        cache: Option<TaskStatus<D, P>>,
    }

    impl<D, P, T> OnceCache<D, P, T>
    where
        T: Iterator<Item = TaskStatus<D, P>>,
    {
        pub fn new(item: T) -> Self {
            Self {
                iter: item,
                cache: None,
                used: None,
            }
        }

        pub fn take(&mut self) -> Option<TaskStatus<D, P>> {
            self.cache.take()
        }
    }

    impl<D, P, T> Iterator for OnceCache<D, P, T>
    where
        T: Iterator<Item = TaskStatus<D, P>>,
    {
        type Item = TaskStatus<D, P>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.used.is_some() {
                return None;
            }

            match self.iter.next() {
                Some(elem) => match elem {
                    TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                    TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                    TaskStatus::Init => Some(TaskStatus::Init),
                    TaskStatus::Ready(item) => {
                        self.cache = Some(TaskStatus::Ready(item));
                        self.used = Some(());
                        return None;
                    }
                },
                None => None,
            }
        }
    }

    /// UntilTake implements an iterator that becomes temporarily finished/done
    /// by always returning `None` until it's cached value is taken.
    ///
    /// This allows you to allocate the iterator for only one cycle, get it
    /// back and re-add it for another cycle later.
    ///
    /// To be clear, the iterator never returns the actual value in next
    /// you can use it to cache said value and only have a call to `UntilTake::take`
    /// will it ever allow progress.
    ///
    /// Usually yo use these types of iterator in instances where you control ownership
    /// of them and can retrieve them after whatever runs them (calling their next)
    /// consider it finished for one that inverts this behaviour i.e yielding the
    /// next value then being unusable till it's reset for reuse, see `UntilReset`.
    pub struct UntilTake<D, P, T: Iterator<Item = TaskStatus<D, P>>> {
        iter: T,
        next: Option<TaskStatus<D, P>>,
    }

    impl<D, P, T> UntilTake<D, P, T>
    where
        T: Iterator<Item = TaskStatus<D, P>>,
    {
        pub fn new(item: T) -> Self {
            Self {
                iter: item,
                next: None,
            }
        }

        pub fn take(&mut self) -> Option<TaskStatus<D, P>> {
            self.next.take()
        }
    }

    impl<D, P, T> Iterator for UntilTake<D, P, T>
    where
        T: Iterator<Item = TaskStatus<D, P>>,
    {
        type Item = TaskStatus<D, P>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.next.is_some() {
                return None;
            }

            match self.iter.next() {
                Some(elem) => match elem {
                    TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                    TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                    TaskStatus::Init => Some(TaskStatus::Init),
                    TaskStatus::Ready(item) => {
                        self.next = Some(TaskStatus::Ready(item));
                        return None;
                    }
                },
                None => None,
            }
        }
    }

    /// UntilUnblocked implements an iterator that yields the first received
    /// value from a owned iterator after which it becomes blocked until
    /// you call `UntilUnblocked::reset` method to be reusable again.
    pub struct UntilUnblocked<D, P, T: Iterator<Item = TaskStatus<D, P>>> {
        iter: T,
        blocked: Option<()>,
    }

    impl<D, P, T> UntilUnblocked<D, P, T>
    where
        T: Iterator<Item = TaskStatus<D, P>>,
    {
        pub fn new(item: T) -> Self {
            Self {
                iter: item,
                blocked: None,
            }
        }

        pub fn reset(&mut self) {
            self.blocked.take();
        }
    }

    impl<D, P, T> Iterator for UntilUnblocked<D, P, T>
    where
        T: Iterator<Item = TaskStatus<D, P>>,
    {
        type Item = TaskStatus<D, P>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.blocked.is_some() {
                return None;
            }

            match self.iter.next() {
                Some(elem) => match elem {
                    TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                    TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                    TaskStatus::Init => Some(TaskStatus::Init),
                    TaskStatus::Ready(item) => {
                        self.blocked = Some(());
                        return Some(TaskStatus::Ready(item));
                    }
                },
                None => None,
            }
        }
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
