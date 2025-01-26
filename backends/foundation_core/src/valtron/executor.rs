use std::{cell, time};

use super::task_iterator::TaskStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Pending indicates the the underlying process to be
    /// still waiting progress to it's next state with
    /// a comunicated indicator of how long possibly that
    /// state might be. Its an optional value that the
    /// underlying process could communicate to the executor
    /// that allows the executor to be smarter about how it
    /// polls for progress.
    Pending(Option<time::Duration>),

    /// Reschedule indicates we want to rechedule the underlying
    /// task leaving the performance of that to the underlying
    /// process that receives this.
    Reschedule,

    /// Progressed simply indicates the underlying iterator
    /// has progressed in it's state. This lets the executor
    /// perform whatever tracking/progress logic it needs to do
    /// in relation to this.
    Progressed,

    /// Done indicates that the iterator has finished (when it returns None)
    /// and no further execution is required for giving iterator.
    Done,
}

pub type BoxedStateIterator = Box<dyn Iterator<Item = State>>;
pub type BoxedSendStateIterator = Box<dyn Iterator<Item = State> + Send>;

/// ExecutorEngine is the backbone of the valtron execution model
/// they can be spawned within threads or be the singular owner
/// of a thread which the user/caller create to manage execution within the
/// thread.
pub trait ExecutionEngine {
    /// lift prioritizes an incoming task to the top of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the ExecutionEngine).
    fn lift(&self, task: impl ExecutionIterator<Executor = Self>)
    where
        Self: Sized;

    /// lift adds provided incoming task to the bottom of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the ExecutionEngine).
    fn schedule(&self, task: impl ExecutionIterator<Executor = Self>)
    where
        Self: Sized;

    /// broadcast allows you to deliver a task to the global execution queue
    /// which then lets the giving task to be sent of to the same or another
    /// executor in another thread for processing, which requires the type to be
    /// `Send` safe.
    fn broadcast(&self, task: impl ExecutionIterator<Executor = Self>)
    where
        Self: Sized;
}

pub type BoxedExecutionIterator<M> = Box<dyn ExecutionIterator<Executor = M>>;

/// ExecutionIterator is a type of Iterator that
/// uniquely always just returns the State of
/// it's internal procecesses and never
/// an actual value of the internal calculation
/// it performs.
///
/// It provides a clean way for an execution engine to
/// progressively generate progress for task only based on
/// the underlying state information it returns.
pub trait ExecutionIterator {
    type Executor: ExecutionEngine;

    fn next(&mut self, executor: Self::Executor) -> Option<State>;
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
    fn handle(&self, item: TaskStatus<D, P>, engine: Box<dyn ExecutionEngine>);
}

pub struct FnMutReady<F>(cell::RefCell<F>);

impl<F> FnMutReady<F> {
    pub fn new(f: F) -> Self {
        Self(cell::RefCell::new(f))
    }
}

impl<F, D, P> TaskReadyResolver<D, P> for FnMutReady<F>
where
    F: FnMut(TaskStatus<D, P>, Box<dyn ExecutionEngine>),
{
    fn handle(&self, item: TaskStatus<D, P>, engine: Box<dyn ExecutionEngine>) {
        let mut mut_fn = self.0.borrow_mut();
        (mut_fn)(item, engine)
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
    F: Fn(TaskStatus<D, P>, Box<dyn ExecutionEngine>),
{
    fn handle(&self, item: TaskStatus<D, P>, engine: Box<dyn ExecutionEngine>) {
        self.0(item, engine)
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
