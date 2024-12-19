/// TaskStatus provides a clear way for an underlying task to indicate
/// the state of the task whether its still pending or done.
/// In similar version to the way futures work.
pub enum TaskStatus<P, D> {
    Pending(P),
    Ready(D),
}

/// AsyncIterator is an iterator whoes underlying type always returns a TaskStatus
/// wrapped type that allows indication that a type is still pending.
///
/// This allows us to further allow an iterator to keep representing internal
/// status to the outside world where by it will not block on the call to `AsyncIterator::next()`
/// but always return a wrapped `TaskStatus` which you can use to indicate the readiness
/// or pending state with some value status you wish to pass along to the
/// caller.
pub trait AsyncIterator<P, D>: Iterator<Item = TaskStatus<P, D>> {}

/// Notifable is a type that can be notified when the AsyncIterator is ready.
pub trait Notifiable<T> {
    fn notify(&self, t: T);
}

/// AsyncIteratorWithNotification allows a variant of an AsyncIterator
/// that allows the signal to be sent from the iterator when data is ready.
///
/// As the original AsyncIterator leaves it to the caller to determine how, when
/// and what frequency they checks for when the TaskStatus has shifted which might have
/// runtimes running wasted CPU cycles, there exists this variant that builds of the async iterator
/// but lets you specify a type implementing the Notifable to be notied when an async iterator should
/// be called again.
pub trait AsyncIteratorWithNotification<P, D>: AsyncIterator<P, D> {
    fn register<T: Notifiable<Self>>(&self, t: T)
    where
        Self: Sized;
}

pub type BoxedAsyncIterator<P, D> = Box<dyn Iterator<Item = TaskStatus<P, D>>>;

/// ClonableAsyncIterator represents a Iterator whoes specifity must always
/// returned values wrapped by `TaskStatus` indicative if the type is finished
/// or not.
pub trait ClonableAsyncIterator<P, D>: AsyncIterator<P, D> {
    fn clone_box(&self) -> Box<dyn ClonableAsyncIterator<P, D, Item = Self::Item>>;
}

impl<T, P, D> ClonableAsyncIterator<P, D> for T
where
    T: AsyncIterator<P, D> + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ClonableAsyncIterator<P, D, Item = TaskStatus<P, D>>> {
        Box::new(self.clone())
    }
}

pub struct CanCloneAsyncIterator<P, D>(
    Box<dyn ClonableAsyncIterator<P, D, Item = TaskStatus<P, D>>>,
);

impl<P, D> CanCloneAsyncIterator<P, D> {
    pub fn new(elem: Box<dyn ClonableAsyncIterator<P, D, Item = TaskStatus<P, D>>>) -> Self {
        Self(elem)
    }
}

impl<P, D> Clone for CanCloneAsyncIterator<P, D> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<P, D> Iterator for CanCloneAsyncIterator<P, D> {
    type Item = TaskStatus<P, D>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
