#![allow(clippy::type_complexity)]

use crate::valtron::Stream;
use crate::valtron::StreamIterator;
use concurrent_queue::ConcurrentQueue;
use derive_more::From;

use std::{
    any::Any,
    cell::{self, RefCell},
    marker::PhantomData,
    rc::{self, Rc},
    sync::Arc,
    time,
};

use crate::compati::Mutex;
use rand_chacha::ChaCha8Rng;

use crate::{
    synca::Entry,
    valtron::{AnyResult, GenericResult},
};

/// The type for a panic handling closure. Note that this same closure
/// may be invoked multiple times in parallel.
pub type PanicHandler = dyn Fn(Box<dyn Any + Send>) + Send + Sync;

pub type BoxedPanicHandler = Box<PanicHandler>;

/// completed and delivered from the iterator.
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

impl<D, P, S: ExecutionAction> From<TaskStatus<D, P, S>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P, S>) -> Self {
        match val {
            TaskStatus::Init => Stream::Init,
            TaskStatus::Spawn(_) => Stream::Ignore,
            TaskStatus::Ready(inner) => Stream::Next(inner),
            TaskStatus::Delayed(inner) => Stream::Delayed(inner),
            TaskStatus::Pending(inner) => Stream::Pending(inner),
        }
    }
}

impl<D: PartialEq, P: PartialEq, S: ExecutionAction> Eq for TaskStatus<D, P, S> {}

impl<D: PartialEq, P: PartialEq, S: ExecutionAction> PartialEq for TaskStatus<D, P, S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TaskStatus::Delayed(me), TaskStatus::Delayed(them)) => me == them,
            (TaskStatus::Pending(me), TaskStatus::Pending(them)) => me == them,
            (TaskStatus::Ready(me), TaskStatus::Ready(them)) => me == them,
            (TaskStatus::Spawn(_), TaskStatus::Spawn(_)) | (TaskStatus::Init, TaskStatus::Init) => {
                true
            }
            _ => false,
        }
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug, S: ExecutionAction> core::fmt::Display
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
            TaskStatus::Delayed(duration) => TStatus::Delayed(*duration),
            TaskStatus::Pending(inner) => TStatus::Pending(inner),
            TaskStatus::Ready(inner) => TStatus::Ready(inner),
            TaskStatus::Spawn(_) => TStatus::Spawn,
            TaskStatus::Init => TStatus::Init,
        };

        write!(f, "{debug_item:?}")
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
            TaskStatus::Delayed(duration) => TStatus::Delayed(*duration),
            TaskStatus::Pending(inner) => TStatus::Pending(inner),
            TaskStatus::Ready(inner) => TStatus::Ready(inner),
            TaskStatus::Spawn(_) => TStatus::Spawn,
            TaskStatus::Init => TStatus::Init,
        };

        write!(f, "{debug_item:?}")
    }
}

/// `AsTaskIterator` represents a type for an iterator with
/// the underlying output of the iterator to be `TaskStatus`
/// and it's relevant semantics.
pub type AsTaskIterator<D, P, S> = dyn Iterator<Item = TaskStatus<D, P, S>>;

/// `BoxedTaskIterator` provides a Boxed (heap-allocated) Iterator based on a `TaskIterator`.
pub type BoxedTaskIterator<D, P, S> = Box<dyn Iterator<Item = TaskStatus<D, P, S>>>;

/// `TaskIterator` is an iterator engineered around the concept of asynchronouse
/// task that have 3 states: PENDING, INIT (Initializing) and READY.
///
/// This follows the Iterators in that when the iterator returns None then it's
/// finished, else we can assume its always producing results that might be in any of
/// those states both for either a singular result or multiple elements.
///
/// This means this can keep producing elements as and the caller simply takes the
/// [`TaskStatus::Done`] state results (if they only care about the final value)
/// else use the other state for whatever is sensible.
///
/// One thing to note is, the same restrictions apply with these iterators, you do
/// not want to block the thread where this is executed or other processes and its always
/// advised to finish early by moving long processes into other threads that allow you
/// notify once next is called to notify if the task is done or still pending.
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
    type Ready;

    /// Spawner provides us a way to declare a type that
    /// if returned is provided an handle to the executor
    /// to spawn sub-tasks that are related to this task.
    type Spawner: ExecutionAction;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;

    /// `into_stream_iter` consumes the implementation and wraps
    /// it in an iterator type that emits
    /// `TaskStatus<TaskIterator::Pending ,TaskIterator::Done>`
    /// match the behavior desired for an iterator.
    fn into_stream_iter(self) -> impl Iterator<Item = Stream<Self::Ready, Self::Pending>>
    where
        Self: Sized + 'static,
    {
        TaskAsStreamIterator(Box::new(self))
    }

    /// `into_iter` consumes the implementation and wraps
    /// it in an iterator type that emits
    /// `TaskStatus<TaskIterator::Pending ,TaskIterator::Done>`
    /// match the behavior desired for an iterator.
    fn into_iter(
        self,
    ) -> impl Iterator<Item = TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>
    where
        Self: Sized + 'static,
    {
        TaskAsIterator(Box::new(self))
    }

    /// [`into_asstream`] returns a [`AsStream`] instance which takes ownership
    /// of implementer of `TaskIterator` directly without boxing the underlying type
    /// within a [`Box`] essentially it is stack friendly but due to limitations for the
    /// iterator type, `self` is still boxed and wrapped with a [`TaskAsIterator`].
    ///
    /// Returns all values as [`Stream`].
    fn into_asstream(
        self,
    ) -> AsStream<
        Self::Ready,
        Self::Pending,
        Self::Spawner,
        TaskAsIterator<Self::Ready, Self::Pending, Self::Spawner>,
    >
    where
        Self: Sized + 'static,
    {
        AsStream::new(TaskAsIterator(Box::new(self)))
    }

    /// [`into_ready_values`] returns a [`ReadyValues`] iterator implementing instance
    /// which takes ownership of implementer of `TaskIterator` directly without boxing
    /// the underlying type within a [`Box`] essentially it is stack friendly but
    /// due to limitations for the iterator type, `self` is still
    /// boxed and wrapped with a [`TaskAsIterator`].
    ///
    /// Returns all values as [`Stream`].
    fn into_ready_values(
        self,
    ) -> ReadyValues<
        Self::Ready,
        Self::Pending,
        Self::Spawner,
        TaskAsIterator<Self::Ready, Self::Pending, Self::Spawner>,
    >
    where
        Self: Sized + 'static,
    {
        ReadyValues::new(TaskAsIterator(Box::new(self)))
    }
}

pub struct TaskAsStreamIterator<D, P, S>(
    Box<dyn TaskIterator<Ready = D, Pending = P, Spawner = S>>,
);

impl<D, P, S> TaskAsStreamIterator<D, P, S> {
    pub fn from_impl(t: impl TaskIterator<Ready = D, Pending = P, Spawner = S> + 'static) -> Self {
        Self(Box::new(t))
    }

    #[must_use]
    pub fn new(t: Box<dyn TaskIterator<Ready = D, Pending = P, Spawner = S>>) -> Self {
        Self(t)
    }
}

impl<D, P, S: ExecutionAction> StreamIterator<D, P> for TaskAsStreamIterator<D, P, S> {}

impl<D, P, S: ExecutionAction> Iterator for TaskAsStreamIterator<D, P, S> {
    type Item = crate::valtron::Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(std::convert::Into::into)
    }
}

pub struct TaskAsIterator<D, P, S>(Box<dyn TaskIterator<Ready = D, Pending = P, Spawner = S>>);

impl<D, P, S> TaskAsIterator<D, P, S> {
    pub fn from_impl(t: impl TaskIterator<Ready = D, Pending = P, Spawner = S> + 'static) -> Self {
        Self(Box::new(t))
    }

    #[must_use]
    pub fn new(t: Box<dyn TaskIterator<Ready = D, Pending = P, Spawner = S>>) -> Self {
        Self(t)
    }
}

impl<D, P, S: ExecutionAction> Iterator for TaskAsIterator<D, P, S> {
    type Item = TaskStatus<D, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// Pending indicates the underlying process to be
    /// still waiting progress to it's next state with
    /// a communicated indicator of how long possibly that
    /// state might be. Its an optional value that the
    /// underlying process could communicate to the executor
    /// that allows the executor to be smarter about how it
    /// polls for progress.
    Pending(Option<time::Duration>),

    /// The state indicating a underlying task Panicked.
    Panicked,

    /// The state is sent out when there was an attempt to spawn
    /// a task from another and that failed which is not a desired
    /// or wanted state to be in ever.
    SpawnFailed,

    /// The state indicating a spawn action succeeded.
    SpawnFinished,

    /// Reschedule indicates we want to reschedule the underlying
    /// task leaving the performance of that to the underlying
    /// process that receives this.
    Reschedule,

    /// Progressed simply indicates the underlying iterator
    /// has progressed in it's state. This lets the executor
    /// perform whatever tracking/progress logic it needs to do
    /// in relation to this.
    Progressed,

    /// Indicate that the state saw a ready value from one of the
    /// task executors, and provides the entry id for the task.
    ReadyValue(Entry),

    /// Done indicates that the iterator has finished (when it returns None)
    /// and no further execution is required for giving iterator.
    Done,
}

pub type BoxedStateIterator = Box<dyn Iterator<Item = State>>;
pub type BoxedSendStateIterator = Box<dyn Iterator<Item = State> + Send>;
pub type BoxedSendSyncStateIterator = Box<dyn Iterator<Item = State> + Send + Sync + 'static>;
pub type SharedTaskQueue = Arc<ConcurrentQueue<BoxedSendExecutionIterator>>;

pub type BoxedExecutionEngine = Box<dyn ExecutionEngine>;

/// [`ExecutionIterator`] is a type of Iterator that
/// uniquely always just returns the State of
/// it's internal processes and never
/// an actual value of the internal calculation
/// it performs.
///
/// It provides a clean way for an execution engine to
/// progressively generate progress for task only based on
/// the underlying state information it returns.
pub trait ExecutionIterator {
    fn next(&mut self, parent_id: Entry, engine: BoxedExecutionEngine) -> Option<State>;
}

pub trait IntoBoxedExecutionIterator {
    fn into_box_execution_iterator(self) -> Box<dyn ExecutionIterator>;
}

pub trait IntoBoxedSendExecutionIterator {
    fn into_box_send_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static>;
}

impl<F> IntoBoxedSendExecutionIterator for F
where
    F: ExecutionIterator + Send + 'static,
{
    fn into_box_send_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static> {
        Box::new(self)
    }
}

impl<F> IntoBoxedExecutionIterator for F
where
    F: ExecutionIterator + 'static,
{
    fn into_box_execution_iterator(self) -> Box<dyn ExecutionIterator> {
        Box::new(self)
    }
}

/// [`BoxedExecutionIterator`] defines a [`Box`] version of an [`ExecutionIterator`]
/// which allows us allocate an [`ExecutionIterator`] on the heap.
pub type BoxedExecutionIterator = Box<dyn ExecutionIterator + 'static>;

/// [`BoxedSendExecutionIterator`] defines a [`Box`] version of an [`ExecutionIterator`]
/// which allows us allocate an [`ExecutionIterator`] on the heap that is [`Send`].
pub type BoxedSendExecutionIterator = Box<dyn ExecutionIterator + Send + 'static>;

pub trait CloneableExecutionIterator: ExecutionIterator {
    fn clone_execution_iterator(&self) -> Box<dyn CloneableExecutionIterator>;
}

pub trait IntoRawExecutionIterator: CloneableExecutionIterator {
    fn into_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static>;
}

impl<M> IntoRawExecutionIterator for M
where
    M: CloneableExecutionIterator + Send + 'static,
{
    fn into_execution_iterator(self) -> Box<dyn ExecutionIterator + Send + 'static> {
        Box::new(self)
    }
}

impl<M> CloneableExecutionIterator for M
where
    M: ExecutionIterator + Clone + 'static,
{
    fn clone_execution_iterator(&self) -> Box<dyn CloneableExecutionIterator> {
        Box::new(self.clone())
    }
}

pub struct CanCloneExecutionIterator(Box<dyn CloneableExecutionIterator>);

impl CanCloneExecutionIterator {
    #[must_use]
    pub fn new(elem: Box<dyn CloneableExecutionIterator>) -> Self {
        Self(elem)
    }
}

impl Clone for CanCloneExecutionIterator {
    fn clone(&self) -> Self {
        Self(self.0.clone_execution_iterator())
    }
}

impl ExecutionIterator for CanCloneExecutionIterator {
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        self.0.next(entry, engine)
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a, M> ExecutionIterator for &'a mut M
where
    M: ExecutionIterator,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        (**self).next(entry, engine)
    }
}

impl<M> ExecutionIterator for RefCell<Box<M>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        self.get_mut().next(entry, engine)
    }
}

impl<M> ExecutionIterator for Rc<RefCell<Box<M>>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        self.borrow_mut().next(entry, engine)
    }
}

impl<M> ExecutionIterator for Mutex<Box<M>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        self.get_mut().unwrap().next(entry, engine)
    }
}

impl<M> ExecutionIterator for Arc<Mutex<Box<M>>>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        self.lock().unwrap().next(entry, engine)
    }
}

impl<M> ExecutionIterator for Box<M>
where
    M: ExecutionIterator + ?Sized,
{
    fn next(&mut self, entry: Entry, engine: BoxedExecutionEngine) -> Option<State> {
        (**self).next(entry, engine)
    }
}

#[derive(Debug, From)]
pub enum ExecutorError {
    /// Executor failed to lift a new task as priority.
    FailedToLift,

    /// Executor cant schedule new task as provided.
    FailedToSchedule,

    /// Indicates a task has already lifted another
    /// task as priority and cant lift another since by
    /// definition it should not at all be able to do so
    /// since its not being executed at such periods of time.
    AlreadyLiftedTask,

    /// Is issued when a task not currently being processed by the executor
    /// attempts to lift a task as priority, only currently executing tasks
    /// can do that.
    ParentMustBeExecutingToLift,

    /// Issued when queue is closed and we are unable to deliver tasks anymore.
    QueueClosed,

    /// Issued when the queue is a bounded queue and execution of new tasks is
    /// impossible.
    QueueFull,

    /// We failed to create the relevant executor task.
    FailedToCreate,

    /// We failed to create the relevant executor task due to configuration not
    /// being supported.
    NotSupported,

    /// A given executor has no provided task in the case of a [`TaskIterator`]
    /// based [`ExecutorIterator`].
    TaskRequired,

    #[from(ignore)]
    FailedToSendThreadActivity,
}

impl std::error::Error for ExecutorError {}

impl core::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// [`ExecutorEngine`] is the backbone of the valtron execution model
/// they can be spawned within threads or be the singular owner
/// of a thread which the user/caller create to manage execution within the
/// thread.
pub trait ExecutionEngine {
    /// [`lift`] prioritizes an incoming task to the top of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the `ExecutionEngine`).
    ///
    /// If `parent` is provided then a dependency connection is made
    /// with the relevant parent's identified by the `Entry` key.
    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<(), ExecutorError>;

    /// [`schedule`] adds provided incoming task to the bottom of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the `ExecutionEngine`).
    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<(), ExecutorError>;

    /// [`broadcast`] allows you to deliver a task to the global execution queue
    /// which then lets the giving task to be sent of to the same or another
    /// executor in another thread for processing, which requires the type to be
    /// `Send` safe.
    fn broadcast(&self, task: BoxedSendExecutionIterator) -> AnyResult<(), ExecutorError>;

    /// [`shared_queue`] returns access to the global queue.
    fn shared_queue(&self) -> SharedTaskQueue;

    /// rng returns a shared thread-safe `ChaCha8Rng` random generation
    /// managed by the executor.
    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>>;
}

/// [`ExecutionAction`] represents a underlying action definition that can
/// spawn some other task by using the provided execution engine and parent key.
pub trait ExecutionAction {
    fn apply(&mut self, key: Entry, engine: BoxedExecutionEngine) -> GenericResult<()>;
}

pub type NoSpawner = NoAction;

#[derive(Default, Clone, Debug)]
pub struct NoAction;

impl ExecutionAction for NoAction {
    fn apply(&mut self, _entry: Entry, _engine: BoxedExecutionEngine) -> GenericResult<()> {
        // do nothing
        Ok(())
    }
}

pub type BoxedTaskReadyResolver<S, D, P> = Box<dyn TaskReadyResolver<S, D, P>>;

/// `TaskResolver` are types implementing this trait to
/// perform final resolution of a task when the task emits
/// the relevant `TaskStatus::Ready` enum state.
///
/// Unlike `TaskStatusMapper` these implementing types do
/// not care about the varying states of a `TaskIterator`
/// but about the final state of the task when it signals
/// it's readiness via the `TaskStatus::Ready` state.
pub trait TaskReadyResolver<S: ExecutionAction, D, P> {
    fn handle(&self, item: TaskStatus<D, P, S>, engine: BoxedExecutionEngine);
}

pub type NoResolver<Action, Done, Pending> = NoResolving<Action, Done, Pending>;

pub type NoResolverAndSpawner<Done, Pending> = NoResolving<NoSpawner, Done, Pending>;

pub struct NoResolving<Action: ExecutionAction, Done, Pending>(
    PhantomData<(Action, Done, Pending)>,
);

impl<Action, Done, Pending> NoResolving<Action, Done, Pending>
where
    Action: ExecutionAction,
{
    #[must_use]
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Action, Done, Pending> Default for NoResolving<Action, Done, Pending>
where
    Action: ExecutionAction,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Action, Done, Pending> TaskReadyResolver<Action, Done, Pending>
    for NoResolving<Action, Done, Pending>
where
    Action: ExecutionAction,
{
    fn handle(&self, _item: TaskStatus<Done, Pending, Action>, _engine: BoxedExecutionEngine) {
        // do nothing
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a, F, S, D, P> TaskReadyResolver<S, D, P> for &'a mut F
where
    S: ExecutionAction,
    F: TaskReadyResolver<S, D, P>,
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: BoxedExecutionEngine) {
        (**self).handle(item, engine);
    }
}

impl<F, S, D, P> TaskReadyResolver<S, D, P> for Box<F>
where
    S: ExecutionAction,
    F: TaskReadyResolver<S, D, P> + ?Sized,
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: BoxedExecutionEngine) {
        (**self).handle(item, engine);
    }
}

pub struct FnMutReady<F, S>(cell::RefCell<F>, PhantomData<S>);

impl<F, S> FnMutReady<F, S> {
    pub fn new(f: F) -> Self {
        Self(cell::RefCell::new(f), PhantomData)
    }
}

impl<F, S, D, P> TaskReadyResolver<S, D, P> for FnMutReady<F, S>
where
    S: ExecutionAction,
    F: FnMut(TaskStatus<D, P, S>, BoxedExecutionEngine),
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: BoxedExecutionEngine) {
        let mut mut_fn = self.0.borrow_mut();
        (mut_fn)(item, engine);
    }
}

pub struct FnReady<F, S>(F, PhantomData<S>);

impl<F, S> FnReady<F, S> {
    pub fn new(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<F, S, D, P> TaskReadyResolver<S, D, P> for FnReady<F, S>
where
    S: ExecutionAction,
    F: Fn(TaskStatus<D, P, S>, BoxedExecutionEngine),
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: BoxedExecutionEngine) {
        self.0(item, engine);
    }
}

/// [`TaskStatusMapper`] are types implementing this trait to
/// perform unique operations on the underlying `TaskStatus`
/// received, possibly generating a new `TaskStatus`.
pub trait TaskStatusMapper<D, P, S: ExecutionAction> {
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>>;
}

#[allow(clippy::extra_unused_lifetimes)]
#[allow(clippy::needless_lifetimes)]
impl<'a, F, S, D, P> TaskStatusMapper<D, P, S> for &'a mut F
where
    S: ExecutionAction,
    F: TaskStatusMapper<D, P, S>,
{
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>> {
        (**self).map(item)
    }
}

impl<F, S, D, P> TaskStatusMapper<D, P, S> for Box<F>
where
    S: ExecutionAction,
    F: TaskStatusMapper<D, P, S> + ?Sized,
{
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>> {
        (**self).map(item)
    }
}

pub struct FnMapper<D, P, S: ExecutionAction>(
    Box<dyn FnMut(TaskStatus<D, P, S>) -> Option<TaskStatus<D, P, S>>>,
);

impl<D, P, S: ExecutionAction> FnMapper<D, P, S> {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(TaskStatus<D, P, S>) -> Option<TaskStatus<D, P, S>> + 'static,
    {
        Self(Box::new(f))
    }
}

impl<D, P, S: ExecutionAction> TaskStatusMapper<D, P, S> for FnMapper<D, P, S> {
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>> {
        match item {
            None => None,
            Some(item) => self.0(item),
        }
    }
}

pub struct FnOptionMapper<D, P, S: ExecutionAction>(
    Box<dyn FnMut(Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>>>,
);

impl<D, P, S: ExecutionAction> FnOptionMapper<D, P, S> {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>> + 'static,
    {
        Self(Box::new(f))
    }
}

impl<D, P, S: ExecutionAction> TaskStatusMapper<D, P, S> for FnOptionMapper<D, P, S> {
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>> {
        self.0(item)
    }
}

#[cfg(test)]
mod test_fn_mapper {
    use crate::valtron::TaskStatus;

    use crate::valtron::{FnMapper, NoSpawner, TaskStatusMapper};

    #[test]
    fn test_can_create_fn_mapper_for_trait() {
        let mut mapper = FnMapper::new(Box::new(|item: TaskStatus<usize, usize, NoSpawner>| {
            Some(item)
        }));

        let instance = TaskStatus::Pending(1);
        assert_eq!(mapper.map(Some(instance.clone())), Some(instance));

        // validate we can meet expected trait type
        let _: Box<dyn TaskStatusMapper<usize, usize, NoSpawner>> = Box::new(mapper);
    }
}

/// `OnceCache` implements a `TaskStatus` iterator that wraps
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
pub struct OnceCache<D, P, S: ExecutionAction, T: Iterator<Item = TaskStatus<D, P, S>>> {
    iter: T,
    used: Option<()>,
    cache: Option<TaskStatus<D, P, S>>,
}

impl<D, P, S, T> OnceCache<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    pub fn new(item: T) -> Self {
        Self {
            iter: item,
            cache: None,
            used: None,
        }
    }

    pub fn take(&mut self) -> Option<TaskStatus<D, P, S>> {
        self.cache.take()
    }
}

impl<D, P, S, T> Iterator for OnceCache<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    type Item = TaskStatus<D, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.used.is_some() {
            return None;
        }

        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Spawn(inner) => Some(TaskStatus::Spawn(inner)),
                TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                TaskStatus::Init => Some(TaskStatus::Init),
                TaskStatus::Ready(item) => {
                    self.cache = Some(TaskStatus::Ready(item));
                    self.used = Some(());
                    None
                }
            },
            None => None,
        }
    }
}

pub struct AsStream<D, P, S: ExecutionAction, T: Iterator<Item = TaskStatus<D, P, S>>> {
    iter: T,
}

impl<D, P, S, T> AsStream<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    pub fn new(item: T) -> Self {
        Self { iter: item }
    }
}

impl<D, P, S, T> Iterator for AsStream<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Init => Some(Stream::Init),
                TaskStatus::Spawn(_) => Some(Stream::Ignore),
                TaskStatus::Delayed(inner) => Some(Stream::Delayed(inner)),
                TaskStatus::Pending(inner) => Some(Stream::Pending(inner)),
                TaskStatus::Ready(item) => Some(Stream::Next(item)),
            },
            None => None,
        }
    }
}

#[derive(Debug)]
pub enum ReadyValue<P> {
    Skip,
    Inner(P),
}

impl<P> ReadyValue<P> {
    pub fn inner(self) -> Option<P> {
        match self {
            Self::Skip => None,
            Self::Inner(inner) => Some(inner),
        }
    }
}

pub struct ReadyValues<D, P, S: ExecutionAction, T: Iterator<Item = TaskStatus<D, P, S>>> {
    iter: T,
}

impl<D, P, S, T> ReadyValues<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    pub fn new(item: T) -> Self {
        Self { iter: item }
    }
}

impl<D, P, S, T> Iterator for ReadyValues<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    type Item = ReadyValue<D>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Init
                | TaskStatus::Spawn(_)
                | TaskStatus::Delayed(_)
                | TaskStatus::Pending(_) => Some(ReadyValue::Skip),
                TaskStatus::Ready(item) => Some(ReadyValue::Inner(item)),
            },
            None => None,
        }
    }
}

/// `UntilTake` implements an iterator that becomes temporarily finished/done
/// by always returning `None` until it's cached value is taken.
///
/// This allows you to allocate the iterator for only one cycle, get it
/// back and re-add it for another cycle later.
///
/// To be clear, the iterator never returns the actual value in next
/// you can use it to cache said value and only have a call to `UntilTake::take`
/// will it ever allow progress.
///
/// Usually you use these types of iterator in instances where you control ownership
/// of them and can retrieve them after whatever runs them (calling their next)
/// consider it finished for one that inverts this behaviour i.e yielding the
/// next value then being unusable till it's reset for reuse, see `UntilReset`.
pub struct UntilTake<D, P, S: ExecutionAction, T: Iterator<Item = TaskStatus<D, P, S>>> {
    iter: T,
    next: Option<TaskStatus<D, P, S>>,
}

impl<D, P, S, T> UntilTake<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    pub fn new(item: T) -> Self {
        Self {
            iter: item,
            next: None,
        }
    }

    pub fn take(&mut self) -> Option<TaskStatus<D, P, S>> {
        self.next.take()
    }
}

impl<D, P, S, T> Iterator for UntilTake<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    type Item = TaskStatus<D, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next.is_some() {
            return None;
        }

        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                TaskStatus::Spawn(inner) => Some(TaskStatus::Spawn(inner)),
                TaskStatus::Pending(dur) => Some(TaskStatus::Pending(dur)),
                TaskStatus::Init => Some(TaskStatus::Init),
                TaskStatus::Ready(item) => {
                    self.next = Some(TaskStatus::Ready(item));
                    None
                }
            },
            None => None,
        }
    }
}

/// `UntilUnblocked` implements an iterator that yields the first received
/// value from a owned iterator after which it becomes blocked until
/// you call `UntilUnblocked::reset` method to be reusable again.
pub struct UntilUnblocked<D, P, S: ExecutionAction, T: Iterator<Item = TaskStatus<D, P, S>>> {
    iter: T,
    blocked: Option<()>,
}

impl<D, P, S, T> UntilUnblocked<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
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

impl<D, P, S, T> Iterator for UntilUnblocked<D, P, S, T>
where
    S: ExecutionAction,
    T: Iterator<Item = TaskStatus<D, P, S>>,
{
    type Item = TaskStatus<D, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.blocked.is_some() {
            return None;
        }

        self.iter.next().map(|elem| match elem {
            TaskStatus::Delayed(dur) => TaskStatus::Delayed(dur),
            TaskStatus::Spawn(inner) => TaskStatus::Spawn(inner),
            TaskStatus::Pending(dur) => TaskStatus::Pending(dur),
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Ready(item) => {
                self.blocked = Some(());
                TaskStatus::Ready(item)
            }
        })
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
