#![allow(clippy::type_complexity)]

use crate::valtron::Stream;
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SpawnType {
    Sequenced,
    Lifted,
    LiftedWithParent,
    Broadcasted,
    Scheduled,
    None,
}

/// [`SpawnInfo`] returns detailed information about
/// the request to spawn a given task with the
/// parent of said tasks the last entry if present.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SpawnInfo(SpawnType, Option<Entry>, Option<Entry>);

impl SpawnInfo {
    #[must_use]
    pub fn new(st: SpawnType, task: Option<Entry>, parent: Option<Entry>) -> Self {
        Self(st, task, parent)
    }

    #[must_use]
    pub fn spawn_type(&self) -> SpawnType {
        self.0
    }

    #[must_use]
    pub fn child(&self) -> Option<Entry> {
        self.1
    }

    #[must_use]
    pub fn parent(&self) -> Option<Entry> {
        self.2
    }

    #[must_use]
    pub fn has_parent_and_child(&self) -> bool {
        self.1.is_some() && self.2.is_some()
    }
}

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

    /// Ignore signals that this item should be skipped.
    /// Used by filtering combinators to indicate filtered-out items
    /// without blocking the iterator.
    Ignore,
}

impl<D, P, S: ExecutionAction> From<TaskStatus<D, P, S>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P, S>) -> Self {
        match val {
            TaskStatus::Init => Stream::Init,
            TaskStatus::Spawn(_) => Stream::Ignore,
            TaskStatus::Ready(inner) => Stream::Next(inner),
            TaskStatus::Delayed(inner) => Stream::Delayed(inner),
            TaskStatus::Pending(inner) => Stream::Pending(inner),
            TaskStatus::Ignore => Stream::Ignore,
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
            (TaskStatus::Spawn(_), TaskStatus::Spawn(_))
            | (TaskStatus::Init, TaskStatus::Init)
            | (TaskStatus::Ignore, TaskStatus::Ignore) => true,
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
            Ignore,
        }

        let debug_item = match self {
            TaskStatus::Delayed(duration) => TStatus::Delayed(*duration),
            TaskStatus::Pending(inner) => TStatus::Pending(inner),
            TaskStatus::Ready(inner) => TStatus::Ready(inner),
            TaskStatus::Spawn(_) => TStatus::Spawn,
            TaskStatus::Init => TStatus::Init,
            TaskStatus::Ignore => TStatus::Ignore,
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
            Ignore,
        }

        let debug_item = match self {
            TaskStatus::Delayed(duration) => TStatus::Delayed(*duration),
            TaskStatus::Pending(inner) => TStatus::Pending(inner),
            TaskStatus::Ready(inner) => TStatus::Ready(inner),
            TaskStatus::Spawn(_) => TStatus::Spawn,
            TaskStatus::Init => TStatus::Init,
            TaskStatus::Ignore => TStatus::Ignore,
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
/// `TaskStatus::Done` state results (if they only care about the final value)
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
    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>>;

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

    /// `into_asstream` returns a [`AsStream`] instance which takes ownership
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

    /// `into_ready_values` returns a [`ReadyValues`] iterator implementing instance
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

// TaskIterator implementations for wrapper types
//
impl<M, R, P, S> TaskIterator for M
where
    M: Iterator<Item = TaskStatus<R, P, S>> + ?Sized,
    R: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    type Ready = R;
    type Pending = P;
    type Spawner = S;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

// 1. Implementation for standard trait objects
impl<R, P, S> Iterator for Box<dyn TaskIterator<Ready = R, Pending = P, Spawner = S> + '_>
where
    R: 'static,
    P: 'static,
    S: ExecutionAction + 'static,
{
    type Item = TaskStatus<R, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        // Use as_mut() to access the trait method on the inner dyn object
        // to avoid the blanket Iterator -> TaskIterator loop
        self.as_mut().next_status()
    }
}

// 2. Implementation for Send trait objects (Crucial for your error message!)
//
// This was added to resolve below error:
//
//  But i got this error:error[E0277]:
// `dyn TaskIterator<Pending = gen_model_descriptors::FetchPending, Ready = Vec<ModelEntry>, Spawner = Box<(dyn ExecutionAction + Send + 'static)>> + Send` is not an iterator
//
// Why as_mut()?
// By calling self.as_mut().next_status(), you are explicitly telling Rust to use
// the next_status method defined on the dyn TaskIterator trait itself, rather than
// the one provided by the blanket Iterator implementation. This prevents the infinite loop.
//
// Why the Send block?
//
// In Rust, Box<dyn Trait> and Box<dyn Trait + Send> are different types. Since your
// error specifically mentioned a + Send trait object, you must provide the
// Iterator implementation for that specific variant.
//
// Does valtron::execute require the iterator to be Sync as well, or is Send
// enough for your current setup?
//
// You are on the right track by implementing Iterator for the Box, but you’ve
// actually hit a coherence and recursion loop because of how your blanket
// implementation works.
//
// Here is what is happening:
//
// You have a blanket impl: impl TaskIterator for M where M: Iterator.
// You implemented Iterator for Box<dyn TaskIterator>.
//
// Because of #2, the compiler sees that Box<dyn TaskIterator> is now an Iterator.
// Because of #1, the compiler says: "Oh, since it's an Iterator, I will automatically
// implement TaskIterator for it!"
//
// The Problem:
//
// Since the TaskIterator impl for the box is now "automatic" via the blanket impl,
// your Iterator::next call to self.next_status() calls the blanket next_status,
// which calls Iterator::next... resulting in infinite recursion.
//
// The Fix:
//
// To break the loop and satisfy the Send requirement from your error message,
// you need to explicitly implement Iterator for the Send version of the box and
// call the inner trait method directly.
//
impl<R, P, S> Iterator for Box<dyn TaskIterator<Ready = R, Pending = P, Spawner = S> + Send + '_>
where
    R: Send + 'static,
    P: Send + 'static,
    S: ExecutionAction + Send + 'static,
{
    type Item = TaskStatus<R, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        self.as_mut().next_status()
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

impl<D: 'static, P: 'static, S: ExecutionAction + 'static> Iterator
    for TaskAsStreamIterator<D, P, S>
{
    type Item = crate::valtron::Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_status().map(std::convert::Into::into)
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

impl<D: 'static, P: 'static, S: ExecutionAction + 'static> Iterator for TaskAsIterator<D, P, S> {
    type Item = TaskStatus<D, P, S>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next_status()
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
    SpawnFailed(Entry),

    /// The state indicating a spawn action succeeded.
    SpawnFinished(SpawnInfo),

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

    /// Done indicates that the iterator has finished can also be when it returns None
    /// but generally this means we should not in anyway process it further.
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
    /// based `ExecutorIterator`.
    TaskRequired,

    #[from(ignore)]
    FailedToSendThreadActivity,
    ParentMustBeSupplied,
}

impl std::error::Error for ExecutorError {}

impl core::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// `ExecutorEngine` is the backbone of the valtron execution model
/// they can be spawned within threads or be the singular owner
/// of a thread which the user/caller create to manage execution within the
/// thread.
pub trait ExecutionEngine {
    /// `sequenced` prioritizes an incoming task to the top of the local
    /// execution queue as a sequential operation that links both this task
    /// and parent into a execution loop where one execution of the task
    /// must lead to the execution of the parent as well.
    ///
    /// If `parent` is provided then a dependency connection is made
    /// with the relevant parent's identified by the `Entry` key.
    fn sequenced(
        &self,
        task: BoxedExecutionIterator,
        parent: Entry,
    ) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `lift` prioritizes an incoming task to the top of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the `ExecutionEngine`).
    ///
    /// If `parent` is provided then a dependency connection is made
    /// with the relevant parent's identified by the `Entry` key.
    /// We expect that such connection will have the task executed
    /// to completion before the parent is scheduled immediately
    /// to process the result or continue its opration.
    ///
    fn lift(
        &self,
        task: BoxedExecutionIterator,
        parent: Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `schedule` adds provided incoming task to the bottom of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the `ExecutionEngine`).
    fn schedule(&self, task: BoxedExecutionIterator) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `broadcast` allows you to deliver a task to the global execution queue
    /// which then lets the giving task to be sent of to the same or another
    /// executor in another thread for processing, which requires the type to be
    /// `Send` safe.
    fn broadcast(&self, task: BoxedSendExecutionIterator) -> AnyResult<SpawnInfo, ExecutorError>;

    /// `boxed_engine` returns a instance of the engine as a [`BoxedExecutionEngine`].
    fn boxed_engine(&self) -> BoxedExecutionEngine;

    /// `shared_queue` returns access to the global queue.
    fn shared_queue(&self) -> SharedTaskQueue;

    /// rng returns a shared thread-safe `ChaCha8Rng` random generation
    /// managed by the executor.
    fn rng(&self) -> rc::Rc<cell::RefCell<ChaCha8Rng>>;
}

pub type BoxedExecutionAction = Box<dyn ExecutionAction>;
pub type BoxedSendExecutionAction = Box<dyn ExecutionAction + Send + 'static>;

/// [`ExecutionAction`] represents a underlying action definition that can
/// spawn some other task by using the provided execution engine and parent key.
///
/// Returns:
///     A result indicating success or failure with the error that caused said failure.
pub trait ExecutionAction {
    fn apply(
        &mut self,
        key: Option<Entry>,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo>;
}

impl<M> ExecutionAction for Box<M>
where
    M: ExecutionAction + ?Sized,
{
    fn apply(
        &mut self,
        key: Option<Entry>,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        (**self).apply(key, engine)
    }
}

pub trait IntoBoxedSendExecutionAction {
    fn into_box_send_execution_action(self) -> Box<dyn ExecutionAction + Send + 'static>;
}

impl<F> IntoBoxedSendExecutionAction for F
where
    F: ExecutionAction + Send + 'static,
{
    fn into_box_send_execution_action(self) -> Box<dyn ExecutionAction + Send + 'static> {
        Box::new(self)
    }
}

pub trait IntoBoxedExecutionAction {
    fn into_box_execution_action(self) -> Box<dyn ExecutionAction>;
}

impl<F> IntoBoxedExecutionAction for F
where
    F: ExecutionAction + 'static,
{
    fn into_box_execution_action(self) -> Box<dyn ExecutionAction> {
        Box::new(self)
    }
}

pub type NoSpawner = NoAction;

#[derive(Default, Clone, Debug)]
pub struct NoAction;

impl ExecutionAction for NoAction {
    fn apply(
        &mut self,
        _entry: Option<Entry>,
        _engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        // do nothing
        Ok(SpawnInfo::new(SpawnType::None, None, None))
    }
}

pub type BoxedTaskReadyResolver<S, D, P> = Box<dyn TaskReadyResolver<S, D, P>>;

pub type BoxedSendTaskReadyResolver<S, D, P> = Box<dyn TaskReadyResolver<S, D, P> + Send + 'static>;

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

pub trait IntoBoxedSendTaskReadyResolver<S: ExecutionAction, D, P> {
    fn into_box_send_task_ready_resolver(
        self,
    ) -> Box<dyn TaskReadyResolver<S, D, P> + Send + 'static>;
}

impl<F, S, D, P> IntoBoxedSendTaskReadyResolver<S, D, P> for F
where
    S: ExecutionAction,
    F: TaskReadyResolver<S, D, P> + Send + 'static,
{
    fn into_box_send_task_ready_resolver(
        self,
    ) -> Box<dyn TaskReadyResolver<S, D, P> + Send + 'static> {
        Box::new(self)
    }
}

pub trait IntoBoxedTaskReadyResolver<S: ExecutionAction, D, P> {
    fn into_box_task_ready_resolver(self) -> Box<dyn TaskReadyResolver<S, D, P>>;
}

impl<F, S, D, P> IntoBoxedTaskReadyResolver<S, D, P> for F
where
    S: ExecutionAction,
    F: TaskReadyResolver<S, D, P> + Send + 'static,
{
    fn into_box_task_ready_resolver(self) -> Box<dyn TaskReadyResolver<S, D, P>> {
        Box::new(self)
    }
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

pub type BoxedTaskStatusMapper<Done, Pending, Action> =
    Box<dyn TaskStatusMapper<Done, Pending, Action>>;

pub type BoxedSendTaskStatusMapper<Done, Pending, Action> =
    Box<dyn TaskStatusMapper<Done, Pending, Action> + Send + 'static>;

/// [`TaskStatusMapper`] are types implementing this trait to
/// perform unique operations on the underlying `TaskStatus`
/// received, possibly generating a new `TaskStatus`.
pub trait TaskStatusMapper<D, P, S: ExecutionAction> {
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>>;
}

pub trait IntoBoxedSendTaskStatusMapper<D, P, S: ExecutionAction> {
    fn into_box_send_task_mapper(self) -> Box<dyn TaskStatusMapper<D, P, S> + Send + 'static>;
}

impl<F, S, D, P> IntoBoxedSendTaskStatusMapper<D, P, S> for F
where
    S: ExecutionAction,
    F: TaskStatusMapper<D, P, S> + Send + 'static,
{
    fn into_box_send_task_mapper(self) -> Box<dyn TaskStatusMapper<D, P, S> + Send + 'static> {
        Box::new(self)
    }
}

pub trait IntoBoxedTaskStatusMapper<D, P, S: ExecutionAction> {
    fn into_box_task_mapper(self) -> Box<dyn TaskStatusMapper<D, P, S>>;
}

impl<F, D, P, S> IntoBoxedTaskStatusMapper<D, P, S> for F
where
    S: ExecutionAction,
    F: TaskStatusMapper<D, P, S> + Send + 'static,
{
    fn into_box_task_mapper(self) -> Box<dyn TaskStatusMapper<D, P, S>> {
        Box::new(self)
    }
}

#[derive(Default, Clone)]
pub struct ZeroMapping<D, P, S: ExecutionAction>(PhantomData<(D, P, S)>);

impl<D, P, S: ExecutionAction> TaskStatusMapper<D, P, S> for ZeroMapping<D, P, S> {
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>> {
        item
    }
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

/// [`OnceCache`] implements a `TaskStatus` iterator that wraps
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
                TaskStatus::Ignore => Some(TaskStatus::Ignore),
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
                TaskStatus::Ignore => Some(Stream::Ignore),
            },
            None => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
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
                | TaskStatus::Pending(_)
                | TaskStatus::Ignore => Some(ReadyValue::Skip),
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
                TaskStatus::Ignore => Some(TaskStatus::Ignore),
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
            TaskStatus::Ignore => TaskStatus::Ignore,
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

// ============================================================================
// TaskIterator Extension Trait and Combinators
// ============================================================================

use crate::valtron::branches::CollectionState;

/// Control enum for the `map_circuit` combinator on `TaskIterator`.
///
/// Allows short-circuiting task iterator chains with three possible outcomes:
/// - `Continue(item)` - Continue iteration with the transformed item
/// - `ReturnAndStop(item)` - Return this item and then stop iteration permanently
/// - `Stop` - Stop iteration without returning anything
///
/// This is useful for error handling patterns where you want to:
/// - Return an error value and immediately stop when an error is encountered
/// - Stop silently without returning anything in certain conditions
/// - Continue normal iteration otherwise
///
/// ## Type Parameters
///
/// - `D`: The Ready/done value type
/// - `P`: The Pending value type
/// - `S`: The ExecutionAction/Spawn type (must implement `ExecutionAction`)
///
/// ## Example
///
/// ```ignore
/// let task = my_task
///     .map_circuit(|status| {
///         match status {
///             TaskStatus::Ready(err) if err.is_error() => {
///                 TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(err))
///             }
///             TaskStatus::Pending(_) | TaskStatus::Ready(_) => {
///                 TaskShortCircuit::Continue(status)
///             }
///             _ => TaskShortCircuit::Stop,
///         }
///     });
/// ```
pub enum TaskShortCircuit<D, P, S: ExecutionAction> {
    /// Continue iteration with the wrapped item
    Continue(TaskStatus<D, P, S>),
    /// Return this item and then stop iteration permanently
    ReturnAndStop(TaskStatus<D, P, S>),
    /// Stop iteration without returning anything
    Stop,
}

/// Extension trait providing builder-style combinator methods for any `TaskIterator`.
///
/// This trait is automatically implemented for any type that implements `TaskIterator`
/// with the appropriate bounds. This includes:
/// - Raw task iterators implementing `TaskIterator`
/// - Driven iterators like `DrivenRecvIterator` and `DrivenSendTaskIterator`
///
/// ## Combinators
///
/// - [`map_ready`](TaskIteratorExt::map_ready) - Transform Ready values
/// - [`map_pending`](TaskIteratorExt::map_pending) - Transform Pending values
/// - [`filter_ready`](TaskIteratorExt::filter_ready) - Filter Ready values
/// - [`stream_collect`](TaskIteratorExt::stream_collect) - Collect all Ready values
/// - [`split_collector`](TaskIteratorExt::split_collector) - Split into observer + continuation
/// - [`split_collect_one`](TaskIteratorExt::split_collect_one) - Split for first match
///
/// ## Example
///
/// ```ignore
/// let task = MyTask::new()
///     .map_ready(|v| v * 2)
///     .map_pending(|p| format!("Still waiting: {:?}", p))
///     .filter_ready(|v| v > 10);
/// ```
pub trait TaskIteratorExt: TaskIterator + Sized {
    /// Transform Ready values using the provided function.
    ///
    /// Pending, Delayed, Init, and Spawn states pass through unchanged.
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Ready, Delayed, Init, and Spawn states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, R>
    where
        F: Fn(Self::Pending) -> R + Send + 'static,
        R: Send + 'static;

    /// Filter Ready values using the provided predicate.
    ///
    /// Non-Ready states pass through unchanged. Ready values that don't
    /// satisfy the predicate are returned as `TaskStatus::Ignore`.
    fn filter_ready<F>(self, f: F) -> TFilterReady<Self>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Collect all Ready values into a Vec.
    ///
    /// Unlike `std::Iterator::collect()`, this does NOT block waiting for all items.
    /// It passes through Pending, Delayed, Init, Spawn states unchanged,
    /// replaces Ready values with `TaskStatus::Ignore`, and only yields the
    /// collected `Vec<Ready>` when the inner iterator completes.
    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: Clone + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch.
    ///
    /// The observer receives a copy of items matching the predicate,
    /// while the continuation continues the chain for further combinators.
    ///
    /// ## Type Requirements
    ///
    /// - `Ready` must be `Clone` (observer gets a copy)
    /// - `Pending` must be `Clone` (observer gets a copy)
    ///
    /// ## Arguments
    ///
    /// * `predicate` - Function determining which items to send to observer
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `CollectorStreamIterator` - Observer that receives matched items
    /// - `SplitCollectorContinuation` - Continuation that continues the chain
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let (observer, continuation) = send_request_task
    ///     .split_collector(
    ///         |item| matches!(item, RequestIntro::Success { .. }),
    ///         1  // Queue size 1 for immediate delivery
    ///     );
    /// ```
    fn split_collector<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Convenience method: `split_collector` with `queue_size` = 1.
    ///
    /// Sends the first matching item to the observer, then continues.
    /// Perfect for "get intro first, then body" patterns.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let (observer, continuation) = send_request_task
    ///     .split_collect_one(|item| matches!(item, RequestIntro::Success { .. }));
    /// ```
    fn split_collect_one<P>(
        self,
        predicate: P,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch,
    /// closing the observer when the predicate returns a close signal.
    ///
    /// The observer receives a copy of items based on the predicate's returned
    /// `CollectionState`. The predicate can decide to skip items, collect them,
    /// or close the observer (with or without collecting the final item).
    ///
    /// ## Type Requirements
    ///
    /// - `Ready` must be `Clone` (observer gets a copy)
    /// - `Pending` must be `Clone` (observer gets a copy)
    ///
    /// ## Arguments
    ///
    /// * `predicate` - Function returning `CollectionState` to control collection behavior
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `SplitUntilObserver` - Observer that receives items based on `CollectionState`
    /// - `SplitUntilContinuation` - Continuation that continues the chain
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Observer gets items until first Success, then closes
    /// let (observer, continuation) = send_request_task
    ///     .split_collect_until(
    ///         |item| match item {
    ///             RequestIntro::Success { .. } => CollectionState::Close(true),
    ///             _ => CollectionState::Collect,
    ///         },
    ///         1  // Queue size 1 for immediate delivery
    ///     );
    /// ```
    fn split_collect_until<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        SplitUntilObserver<Self::Ready, Self::Pending>,
        SplitUntilContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> CollectionState + Send + 'static;

    /// Split the iterator with transformation for observer.
    ///
    /// Like `split_collect_until` but observer receives transformed data D (which must be Clone),
    /// while continuation receives original Ready values. Useful when Ready is not Clone
    /// but extractable subset is.
    ///
    /// The single closure returns `(CollectionState, Option<D>)`:
    /// - `CollectionState` controls whether to skip, collect, or close the observer
    /// - `Option<D>` provides the transformed value (`None` skips sending to observer)
    ///
    /// ## Type Requirements
    ///
    /// - `D` must be `Clone` (observer gets transformed copy)
    /// - `Pending` must be `Clone` (observer gets transformed copy)
    ///
    /// ## Arguments
    ///
    /// * `transform` - Function returning `(CollectionState, Option<D>)` to control collection and transform
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `SplitUntilObserverMap` - Observer that receives transformed D values
    /// - `SplitUntilContinuationMap` - Continuation that continues with original Ready
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Observer gets RequestIntroData (cloneable), continuation gets full RequestIntro
    /// let (observer, continuation) = send_request_task
    ///     .split_collect_until_map(
    ///         |item| match item {
    ///             RequestIntro::Success { .. } => (CollectionState::Close(true), item.to_cloneable_data()),
    ///             _ => (CollectionState::Collect, item.to_cloneable_data()),
    ///         },
    ///         1
    ///     );
    /// ```
    fn split_collect_until_map<F, D>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitUntilObserverMap<D, Self::Pending>,
        SplitUntilContinuationMap<Self, D>,
    )
    where
        Self: Sized,
        D: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (CollectionState, Option<D>) + Send + 'static;

    /// Split the iterator into an observer branch and a continuation branch,
    /// mapping matched Ready values to a different type before sending to the observer.
    ///
    /// Like `split_collector` but the observer receives transformed values of type `M`
    /// instead of cloned `Ready` values. The continuation continues with original `Ready`.
    /// Useful when `Ready` is not `Clone` but a subset can be extracted.
    ///
    /// The single closure returns `(bool, Option<M>)`:
    /// - `bool` - whether this item matched (true = matched, false = skip)
    /// - `Option<M>` - the transformed value (`None` skips sending to observer)
    ///
    /// ## Type Requirements
    ///
    /// - `M` must be `Clone + Send + 'static` (observer gets transformed copy)
    /// - `Pending` must be `Clone` (observer gets a copy)
    ///
    /// ## Arguments
    ///
    /// * `transform` - Function returning `(bool, Option<M>)` to control matching and transform
    /// * `queue_size` - Size of the `ConcurrentQueue` between branches
    ///
    /// ## Returns
    ///
    /// Tuple of:
    /// - `SplitCollectorMapObserver` - Observer that receives transformed M values
    /// - `SplitCollectorMapContinuation` - Continuation that continues with original Ready
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Observer gets u64 ids, continuation gets full Item
    /// let (observer, continuation) = task
    ///     .split_collector_map(
    ///         |item| (item.is_important(), Some(item.id())),
    ///         10
    ///     );
    /// ```
    fn split_collector_map<F, M>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static;

    /// Convenience method: `split_collector_map` with `queue_size` = 1.
    ///
    /// Sends the first matching transformed item to the observer, then continues.
    fn split_collect_one_map<F, M>(
        self,
        transform: F,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }

    /// Short-circuit the iterator based on a circuit function.
    ///
    /// The circuit function receives each item and returns a `TaskShortCircuit` enum
    /// that determines whether to:
    /// - `Continue(item)` - Continue iteration with the transformed item
    /// - `ReturnAndStop(item)` - Return this item and then stop permanently
    /// - `Stop` - Stop iteration without returning anything
    ///
    /// This is useful for error handling patterns where you want to return
    /// an error value and immediately stop when an error is encountered.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// let task = my_task
    ///     .map_circuit(|status| {
    ///         match status {
    ///             TaskStatus::Ready(err) if err.is_error() => {
    ///                 TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(err))
    ///             }
    ///             TaskStatus::Pending(_) | TaskStatus::Ready(_) => {
    ///                 TaskShortCircuit::Continue(status)
    ///             }
    ///             _ => TaskShortCircuit::Stop,
    ///         }
    ///     });
    /// ```
    fn map_circuit<F>(self, f: F) -> TMapCircuit<Self>
    where
        Self: Sized,
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskShortCircuit<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static;

    /// Returns the first `Ready` value from the iterator, short-circuiting after finding it.
    ///
    /// Uses `map_circuit` internally to stop iteration after the first `Ready` value.
    /// Returns `None` if no `Ready` value is found before the iterator is exhausted.
    fn first_ready(self) -> Option<Self::Ready>
    where
        Self: Sized + Send + 'static,
        Self::Ready: Send + 'static,
        Self::Pending: Send + 'static,
        Self::Spawner: ExecutionAction + Send + 'static,
    {
        self.map_circuit(|status| match status {
            TaskStatus::Ready(value) => TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(value)),
            _ => TaskShortCircuit::Continue(status),
        })
        .next()
        .and_then(|status| match status {
            TaskStatus::Ready(value) => Some(value),
            _ => None,
        })
    }

    /// Returns the first `Pending` value from the iterator, short-circuiting after finding it.
    ///
    /// Uses `map_circuit` internally to stop iteration after the first `Pending` value.
    /// Returns `None` if no `Pending` value is found before the iterator is exhausted.
    fn first_pending(self) -> Option<Self::Pending>
    where
        Self: Sized + Send + 'static,
        Self::Ready: Send + 'static,
        Self::Pending: Send + 'static,
        Self::Spawner: ExecutionAction + Send + 'static,
    {
        self.map_circuit(|status| match status {
            TaskStatus::Pending(value) => {
                TaskShortCircuit::ReturnAndStop(TaskStatus::Pending(value))
            }
            _ => TaskShortCircuit::Continue(status),
        })
        .next()
        .and_then(|status| match status {
            TaskStatus::Pending(value) => Some(value),
            _ => None,
        })
    }

    /// Flatten nested iterator patterns where outer yields inner iterators.
    ///
    /// The mapper function transforms each `Ready` value into an inner iterator.
    /// The returned `TMapIter` drains each inner iterator until `None`, then polls
    /// the outer for the next item.
    ///
    /// Non-Ready states (Pending, Spawn) from the outer iterator pass through via `Into`,
    /// so `Self::Pending` must be convertible to `InnerP` and `Self::Spawner` to `InnerS`.
    ///
    /// ## Example
    ///
    /// ```ignore
    /// // Outer yields Ready(Vec<u8>), mapper returns vec.into_iter()
    /// let flattened = task.map_iter(|vec| vec.into_iter());
    /// ```
    fn map_iter<F, InnerIter, InnerR, InnerP, InnerS>(
        self,
        mapper: F,
    ) -> TMapIter<
        Self,
        F,
        InnerIter,
        InnerR,
        InnerP,
        InnerS,
        Self::Ready,
        Self::Pending,
        Self::Spawner,
    >
    where
        Self: Sized,
        F: Fn(Self::Ready) -> InnerIter + Send + 'static,
        InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>> + Send + 'static,
        InnerR: Send + 'static,
        InnerP: Send + 'static,
        InnerS: ExecutionAction + Send + 'static,
        Self::Pending: Into<InnerP> + Send + 'static,
        Self::Spawner: Into<InnerS> + Send + 'static;

    /// Flatten Ready values that implement `IntoIterator`.
    ///
    /// Input:  `TaskIterator`<Ready = `Vec<M>`, Pending = P, Spawner = S>
    /// Output: `TaskIterator`<Ready = M, Pending = P, Spawner = S>
    fn flatten_ready(self) -> TFlattenReady<Self>
    where
        Self: Sized,
        Self::Ready: IntoIterator,
        <Self::Ready as IntoIterator>::Item: Send + 'static;

    /// Flatten Pending values that implement `IntoIterator`.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = `Vec<M>`, Spawner = S>
    /// Output: `TaskIterator`<Ready = D, Pending = M, Spawner = S>
    fn flatten_pending(self) -> TFlattenPending<Self>
    where
        Self: Sized,
        Self::Pending: IntoIterator,
        <Self::Pending as IntoIterator>::Item: Send + 'static;

    /// Flat map Ready values - transform and flatten in one operation.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = P, Spawner = S>
    /// Output: `TaskIterator`<Ready = `U::Item`, Pending = P, Spawner = S>
    fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    /// Flat map Pending values - transform and flatten in one operation.
    ///
    /// Input:  `TaskIterator`<Ready = D, Pending = P, Spawner = S>
    /// Output: `TaskIterator`<Ready = D, Pending = `U::Item`, Spawner = S>
    fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Pending) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static;

    // ===== Feature 08: Iterator Extension Completion =====

    /// Transform any `TaskStatus` with full state access.
    fn map_state<F>(self, f: F) -> TMapState<Self, F>
    where
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskStatus<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static;

    /// Side-effect on any `TaskStatus`.
    fn inspect_state<F>(self, f: F) -> TInspectState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) + Send + 'static;

    /// Filter based on full `TaskStatus`. Non-matching items return `TaskStatus::Ignore`.
    fn filter_state<F>(self, f: F) -> TFilterState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Take items while state predicate returns true.
    fn take_while_state<F>(self, predicate: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Skip items while state predicate returns true.
    fn skip_while_state<F>(self, predicate: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Take at most n items matching state predicate.
    fn take_state<F>(self, n: usize, state_predicate: F) -> TTakeState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Skip first n items matching state predicate.
    fn skip_state<F>(self, n: usize, state_predicate: F) -> TSkipState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static;

    /// Take while predicate true on Ready values, pass through non-Ready.
    fn take<F>(
        self,
        n: usize,
    ) -> TTakeState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    where
        Self::Ready: Send + 'static,
    {
        self.take_state(n, |s| matches!(s, TaskStatus::Ready(_)))
    }

    /// Take at most n items of any state.
    fn take_all(
        self,
        n: usize,
    ) -> TTakeState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    {
        self.take_state(n, |_| true)
    }

    /// Skip first n Ready items, return all others unchanged.
    fn skip(
        self,
        n: usize,
    ) -> TSkipState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    where
        Self::Ready: Send + 'static,
    {
        self.skip_state(n, |s| matches!(s, TaskStatus::Ready(_)))
    }

    /// Skip first n items of any state.
    fn skip_all(
        self,
        n: usize,
    ) -> TSkipState<Self, impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool>
    {
        self.skip_state(n, |_| true)
    }

    /// Take while predicate true on Ready values, pass through non-Ready.
    fn take_while<F>(
        self,
        f: F,
    ) -> TTakeWhileState<
        Self,
        impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool,
    >
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        self.take_while_state(move |s| match s {
            TaskStatus::Ready(v) => f(v),
            _ => true,
        })
    }

    /// Take while predicate true on ANY state.
    fn take_while_any<F>(self, f: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        self.take_while_state(f)
    }

    /// Skip while predicate true on Ready values, return all others.
    fn skip_while<F>(
        self,
        f: F,
    ) -> TSkipWhileState<
        Self,
        impl Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool,
    >
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        self.skip_while_state(move |s| match s {
            TaskStatus::Ready(v) => f(v),
            _ => false,
        })
    }

    /// Skip while predicate true on ANY state.
    fn skip_while_any<F>(self, f: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        self.skip_while_state(f)
    }

    /// Add index to Ready items, changing Ready type from D to (usize, D).
    fn enumerate(self) -> TEnumerate<Self> {
        TEnumerate {
            inner: self,
            count: 0,
        }
    }

    /// Find first item matching predicate.
    fn find<F>(self, predicate: F) -> TFind<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Find first item mapping to Some value.
    fn find_map<F, R>(self, f: F) -> TFindMap<Self, F, R>
    where
        F: Fn(Self::Ready) -> Option<R> + Send + 'static,
        R: Send + 'static;

    /// Fold/accumulate values. Returns final accumulator when done.
    fn fold<F, R>(self, init: R, f: F) -> TFold<Self, F, R>
    where
        F: Fn(R, Self::Ready) -> R + Send + 'static,
        R: Send + 'static;

    /// Check if all Ready items satisfy predicate.
    fn all<F>(self, f: F) -> TAll<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static;

    /// Check if any Ready item satisfies predicate.
    fn any<F>(self, f: F) -> TAny<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static;

    /// Count Ready items.
    fn count(self) -> TCount<Self>;

    /// Count all items (any state).
    fn count_all(self) -> TCountAll<Self>;
}

// Blanket implementation: anything implementing TaskIterator gets TaskIteratorExt
impl<I> TaskIteratorExt for I
where
    I: TaskIterator + Send + 'static,
    I::Ready: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: ExecutionAction + Send + 'static,
{
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, R>
    where
        F: Fn(Self::Ready) -> R + Send + 'static,
        R: Send + 'static,
    {
        TMapReady {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, R>
    where
        F: Fn(Self::Pending) -> R + Send + 'static,
        R: Send + 'static,
    {
        TMapPending {
            inner: self,
            mapper: Box::new(f),
        }
    }

    fn filter_ready<F>(self, f: F) -> TFilterReady<Self>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        TFilterReady {
            inner: self,
            predicate: Box::new(f),
        }
    }

    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: Send + 'static,
    {
        TStreamCollect {
            inner: self,
            collected: Vec::new(),
            done: false,
        }
    }

    fn split_collector<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collector: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = CollectorStreamIterator {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitCollectorContinuation {
            inner: self,
            queue,
            predicate: Box::new(predicate),
        };

        (observer, continuation)
    }

    fn split_collect_one<P>(
        self,
        predicate: P,
    ) -> (
        CollectorStreamIterator<Self::Ready, Self::Pending>,
        SplitCollectorContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        self.split_collector(predicate, 1)
    }

    fn split_collect_until<P>(
        self,
        predicate: P,
        queue_size: usize,
    ) -> (
        SplitUntilObserver<Self::Ready, Self::Pending>,
        SplitUntilContinuation<Self>,
    )
    where
        Self: Sized,
        Self::Ready: Clone,
        Self::Pending: Clone,
        P: Fn(&Self::Ready) -> CollectionState + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collect_until: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitUntilObserver {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitUntilContinuation {
            inner: self,
            queue,
            predicate: Box::new(predicate),
        };

        (observer, continuation)
    }

    fn split_collect_until_map<F, D>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitUntilObserverMap<D, Self::Pending>,
        SplitUntilContinuationMap<Self, D>,
    )
    where
        Self: Sized,
        D: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (CollectionState, Option<D>) + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collect_until_map: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitUntilObserverMap {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitUntilContinuationMap {
            inner: self,
            queue,
            transform: Box::new(transform),
        };

        (observer, continuation)
    }

    fn split_collector_map<F, M>(
        self,
        transform: F,
        queue_size: usize,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static,
    {
        let queue = Arc::new(ConcurrentQueue::bounded(queue_size));
        tracing::debug!(
            "split_collector_map: creating observer and continuation with queue_size={}",
            queue_size
        );

        let observer = SplitCollectorMapObserver {
            queue: Arc::clone(&queue),
        };

        let continuation = SplitCollectorMapContinuation {
            inner: self,
            queue,
            transform: Box::new(transform),
        };

        (observer, continuation)
    }

    fn split_collect_one_map<F, M>(
        self,
        transform: F,
    ) -> (
        SplitCollectorMapObserver<M, Self::Pending>,
        SplitCollectorMapContinuation<Self, M>,
    )
    where
        Self: Sized,
        M: Clone + Send + 'static,
        Self::Pending: Clone,
        F: Fn(&Self::Ready) -> (bool, Option<M>) + Send + 'static,
    {
        self.split_collector_map(transform, 1)
    }

    fn map_circuit<F>(self, f: F) -> TMapCircuit<Self>
    where
        Self: Sized,
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskShortCircuit<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static,
    {
        TMapCircuit {
            inner: self,
            circuit: Box::new(f),
            stopped: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_iter<F, InnerIter, InnerR, InnerP, InnerS>(
        self,
        mapper: F,
    ) -> TMapIter<
        Self,
        F,
        InnerIter,
        InnerR,
        InnerP,
        InnerS,
        Self::Ready,
        Self::Pending,
        Self::Spawner,
    >
    where
        Self: Sized,
        F: Fn(Self::Ready) -> InnerIter + Send + 'static,
        InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>> + Send + 'static,
        InnerR: Send + 'static,
        InnerP: Send + 'static,
        InnerS: ExecutionAction + Send + 'static,
        Self::Pending: Into<InnerP> + Send + 'static,
        Self::Spawner: Into<InnerS> + Send + 'static,
    {
        TMapIter {
            outer: self,
            mapper,
            current_inner: None,
            _phantom: std::marker::PhantomData,
        }
    }

    fn flatten_ready(self) -> TFlattenReady<Self>
    where
        Self: Sized,
        Self::Ready: IntoIterator,
        <Self::Ready as IntoIterator>::Item: Send + 'static,
    {
        TFlattenReady {
            inner: self,
            current_inner: None,
        }
    }

    fn flatten_pending(self) -> TFlattenPending<Self>
    where
        Self: Sized,
        Self::Pending: IntoIterator,
        <Self::Pending as IntoIterator>::Item: Send + 'static,
    {
        TFlattenPending {
            inner: self,
            current_inner: None,
        }
    }

    fn flat_map_ready<F, U>(self, f: F) -> TFlatMapReady<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Ready) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static,
    {
        TFlatMapReady {
            inner: self,
            mapper: f,
            current_inner: None,
        }
    }

    fn flat_map_pending<F, U>(self, f: F) -> TFlatMapPending<Self, F, U>
    where
        Self: Sized,
        F: Fn(Self::Pending) -> U + Send + 'static,
        U: IntoIterator,
        U::Item: Send + 'static,
    {
        TFlatMapPending {
            inner: self,
            mapper: f,
            current_inner: None,
        }
    }

    // ===== Feature 08 implementations =====

    fn map_state<F>(self, f: F) -> TMapState<Self, F>
    where
        F: Fn(
                TaskStatus<Self::Ready, Self::Pending, Self::Spawner>,
            ) -> TaskStatus<Self::Ready, Self::Pending, Self::Spawner>
            + Send
            + 'static,
    {
        TMapState {
            inner: self,
            mapper: f,
        }
    }

    fn inspect_state<F>(self, f: F) -> TInspectState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) + Send + 'static,
    {
        TInspectState {
            inner: self,
            inspector: f,
        }
    }

    fn filter_state<F>(self, f: F) -> TFilterState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TFilterState {
            inner: self,
            predicate: f,
        }
    }

    fn take_while_state<F>(self, predicate: F) -> TTakeWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TTakeWhileState {
            inner: self,
            predicate,
            done: false,
        }
    }

    fn skip_while_state<F>(self, predicate: F) -> TSkipWhileState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TSkipWhileState {
            inner: self,
            predicate,
            done_skipping: false,
        }
    }

    fn take_state<F>(self, n: usize, state_predicate: F) -> TTakeState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TTakeState {
            inner: self,
            remaining: n,
            state_predicate,
        }
    }

    fn skip_state<F>(self, n: usize, state_predicate: F) -> TSkipState<Self, F>
    where
        F: Fn(&TaskStatus<Self::Ready, Self::Pending, Self::Spawner>) -> bool + Send + 'static,
    {
        TSkipState {
            inner: self,
            to_skip: n,
            state_predicate,
        }
    }

    fn find<F>(self, predicate: F) -> TFind<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        TFind {
            inner: self,
            predicate,
            found: false,
        }
    }

    fn find_map<F, R>(self, f: F) -> TFindMap<Self, F, R>
    where
        F: Fn(Self::Ready) -> Option<R> + Send + 'static,
        R: Send + 'static,
    {
        TFindMap {
            inner: self,
            mapper: f,
            found: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn fold<F, R>(self, init: R, f: F) -> TFold<Self, F, R>
    where
        F: Fn(R, Self::Ready) -> R + Send + 'static,
        R: Send + 'static,
    {
        TFold {
            inner: self,
            acc: Some(init),
            folder: f,
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }

    fn all<F>(self, f: F) -> TAll<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static,
    {
        TAll {
            inner: self,
            predicate: f,
            all_true: true,
            done: false,
        }
    }

    fn any<F>(self, f: F) -> TAny<Self, F>
    where
        F: Fn(Self::Ready) -> bool + Send + 'static,
    {
        TAny {
            inner: self,
            predicate: f,
            any_true: false,
            done: false,
        }
    }

    fn count(self) -> TCount<Self> {
        TCount {
            inner: self,
            count: 0,
        }
    }

    fn count_all(self) -> TCountAll<Self> {
        TCountAll {
            inner: self,
            count: 0,
            done: false,
        }
    }
}

/// Wrapper type that transforms Ready values.
pub struct TMapReady<I: TaskIterator, R> {
    inner: I,
    mapper: Box<dyn Fn(I::Ready) -> R + Send>,
}

impl<I, R> Iterator for TMapReady<I, R>
where
    I: TaskIterator,
    R: Send + 'static,
{
    type Item = TaskStatus<R, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_status().map(|status| match status {
            TaskStatus::Ready(v) => TaskStatus::Ready((self.mapper)(v)),
            TaskStatus::Pending(v) => TaskStatus::Pending(v),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Spawn(s) => TaskStatus::Spawn(s),
        })
    }
}

/// Wrapper type that transforms Pending values.
pub struct TMapPending<I: TaskIterator, R> {
    inner: I,
    mapper: Box<dyn Fn(I::Pending) -> R + Send>,
}

impl<I, R> Iterator for TMapPending<I, R>
where
    I: TaskIterator,
    R: Send + 'static,
{
    type Item = TaskStatus<I::Ready, R, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_status().map(|status| match status {
            TaskStatus::Ready(v) => TaskStatus::Ready(v),
            TaskStatus::Pending(v) => TaskStatus::Pending((self.mapper)(v)),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Spawn(s) => TaskStatus::Spawn(s),
        })
    }
}

/// Wrapper type that filters Ready values.
///
/// Filtered-out Ready values are returned as `TaskStatus::Ignore` to avoid blocking.
pub struct TFilterReady<I: TaskIterator> {
    inner: I,
    predicate: Box<dyn Fn(&I::Ready) -> bool + Send>,
}

impl<I> Iterator for TFilterReady<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        match &status {
            TaskStatus::Ready(v) => {
                if (self.predicate)(v) {
                    Some(status)
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            _ => Some(status), // Pass through non-Ready states
        }
    }
}

/// Wrapper type for `map_iter` combinator that flattens nested iterator patterns.
///
/// The outer iterator yields `TaskStatus::Ready(inner_iter)` values.
/// The mapper function transforms each `Ready` value into an inner iterator.
/// `TMapIter` drains each inner iterator until `None`, then polls the outer
/// for the next item to continue flattening.
///
/// Non-Ready states (Pending, Spawn) from the outer iterator pass through via `Into`,
/// so `P` must be convertible to `InnerP` and `S` to `InnerS`.
pub struct TMapIter<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S>
where
    I: TaskIterator<Ready = R, Pending = P, Spawner = S>,
    F: Fn(R) -> InnerIter,
    InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>>,
    InnerS: ExecutionAction,
    S: ExecutionAction,
{
    outer: I,
    mapper: F,
    current_inner: Option<InnerIter>,
    _phantom: std::marker::PhantomData<(InnerR, InnerP, InnerS, R, P, S)>,
}

impl<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S> Iterator
    for TMapIter<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S>
where
    I: TaskIterator<Ready = R, Pending = P, Spawner = S>,
    F: Fn(R) -> InnerIter,
    InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>>,
    InnerR: Send + 'static,
    InnerP: Send + 'static,
    InnerS: ExecutionAction + Send + 'static,
    P: Into<InnerP> + Send + 'static,
    S: Into<InnerS> + ExecutionAction + Send + 'static,
{
    type Item = TaskStatus<InnerR, InnerP, InnerS>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // First, try to drain the current inner iterator
            if let Some(ref mut inner) = self.current_inner {
                if let Some(item) = inner.next() {
                    return Some(item);
                }
                // Inner exhausted, clear it and continue to poll outer
                self.current_inner = None;
            }

            // Poll outer for next item
            match self.outer.next_status() {
                Some(TaskStatus::Ready(d)) => {
                    // Mapper returns a new inner iterator, start draining it
                    let new_inner = (self.mapper)(d);
                    self.current_inner = Some(new_inner);
                }
                Some(TaskStatus::Pending(p)) => return Some(TaskStatus::Pending(p.into())),
                Some(TaskStatus::Delayed(d)) => return Some(TaskStatus::Delayed(d)),
                Some(TaskStatus::Init) => return Some(TaskStatus::Init),
                Some(TaskStatus::Ignore) => return Some(TaskStatus::Ignore),
                Some(TaskStatus::Spawn(s)) => return Some(TaskStatus::Spawn(s.into())),
                None => return None, // Outer exhausted
            }
        }
    }
}

// impl<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S> TaskIterator
//     for TMapIter<I, F, InnerIter, InnerR, InnerP, InnerS, R, P, S>
// where
//     I: TaskIterator<Ready = R, Pending = P, Spawner = S>,
//     F: Fn(R) -> InnerIter,
//     InnerIter: Iterator<Item = TaskStatus<InnerR, InnerP, InnerS>>,
//     InnerR: Send + 'static,
//     InnerP: Send + 'static,
//     InnerS: ExecutionAction + Send + 'static,
//     P: Into<InnerP> + Send + 'static,
//     S: Into<InnerS> + ExecutionAction + Send + 'static,
// {
//     type Ready = InnerR;
//     type Pending = InnerP;
//     type Spawner = InnerS;

//     fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
//         Iterator::next(self)
//     }
// }

// ============================================================================
// Flatten Ready / Pending
// ============================================================================

/// Wrapper type that flattens Ready values that implement `IntoIterator`.
///
/// The Ready type must implement `IntoIterator`. We store the inner iterator
/// and drain it over multiple `next()` calls. When exhausted, poll outer again.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlattenReady<I: TaskIterator>
where
    I::Ready: IntoIterator,
{
    inner: I,
    current_inner: Option<<I::Ready as IntoIterator>::IntoIter>,
}

impl<I> Iterator for TFlattenReady<I>
where
    I: TaskIterator,
    I::Ready: IntoIterator,
    <I::Ready as IntoIterator>::Item: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<<I::Ready as IntoIterator>::Item, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            // Inner exhausted - clear and fall through to poll outer
            self.current_inner = None;
        }

        // Get next from outer iterator
        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Ready(iterable) => {
                // Store iterator, drain on NEXT call
                self.current_inner = Some(iterable.into_iter());
                // Return Ignore to signal "still working, no Ready value yet"
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Ready states unchanged (Pending/Spawner type unchanged)
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

// ============================================================================
// Flatten Pending
// ============================================================================

/// Wrapper type that flattens Pending values that implement `IntoIterator`.
///
/// The Pending type must implement `IntoIterator`. We store the inner iterator
/// and drain it over multiple `next()` calls. When exhausted, poll outer again.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlattenPending<I: TaskIterator>
where
    I::Pending: IntoIterator,
{
    inner: I,
    current_inner: Option<<I::Pending as IntoIterator>::IntoIter>,
}

impl<I> Iterator for TFlattenPending<I>
where
    I: TaskIterator,
    I::Pending: IntoIterator,
    <I::Pending as IntoIterator>::Item: Send + 'static,
    I::Ready: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<I::Ready, <I::Pending as IntoIterator>::Item, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Pending(iterable) => {
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Pending states unchanged
            TaskStatus::Ready(r) => Some(TaskStatus::Ready(r)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

// ============================================================================
// Flat Map Ready
// ============================================================================

/// Wrapper type that maps Ready to `IntoIterator`, then flattens.
///
/// User provides function `Ready -> IntoIterator`. We store the returned iterator
/// and drain it over multiple `next()` calls.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlatMapReady<I: TaskIterator, F, U: IntoIterator> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for TFlatMapReady<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> U + Send + 'static,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<U::Item, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Ready(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Ready(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Ready states (Pending/Spawner type unchanged)
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

// ============================================================================
// Flat Map Pending
// ============================================================================

/// Wrapper type that maps Pending to `IntoIterator`, then flattens.
///
/// User provides function `Pending -> IntoIterator`. We store the returned iterator
/// and drain it over multiple `next()` calls.
///
/// **Important**: Returns `TaskStatus::Ignore` when waiting for inner iterator,
/// never blocks in a loop.
pub struct TFlatMapPending<I: TaskIterator, F, U: IntoIterator> {
    inner: I,
    mapper: F,
    current_inner: Option<U::IntoIter>,
}

impl<I, F, U> Iterator for TFlatMapPending<I, F, U>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> U + Send + 'static,
    U: IntoIterator,
    U::Item: Send + 'static,
    I::Ready: Send + 'static,
    I::Spawner: Send + 'static,
{
    type Item = TaskStatus<I::Ready, U::Item, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // First drain current inner iterator
        if let Some(ref mut inner) = self.current_inner {
            if let Some(item) = inner.next() {
                return Some(TaskStatus::Pending(item));
            }
            self.current_inner = None;
        }

        let status = self.inner.next_status()?;

        match status {
            TaskStatus::Pending(v) => {
                let iterable = (self.mapper)(v);
                self.current_inner = Some(iterable.into_iter());
                Some(TaskStatus::Ignore)
            }
            // Pass through non-Pending states (Ready/Spawner type unchanged)
            TaskStatus::Ready(r) => Some(TaskStatus::Ready(r)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
        }
    }
}

/// Wrapper type that collects all Ready values into a Vec.
///
/// Passes through Pending, Delayed, Init, Spawn states unchanged.
/// Ready values are collected and replaced with `TaskStatus::Ignore`.
/// Only yields the collected Vec when the inner iterator completes.
/// Does NOT require Ready to implement Clone.
pub struct TStreamCollect<I: TaskIterator> {
    inner: I,
    collected: Vec<I::Ready>,
    done: bool,
}

impl<I> Iterator for TStreamCollect<I>
where
    I: TaskIterator,
    I::Ready: Send + 'static,
{
    type Item = TaskStatus<Vec<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already yielded the collected result, we're done
        if self.done {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(value)) => {
                self.collected.push(value);
                // Keep collecting, return Ignore to signal collected but continue
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            None => {
                // Inner iterator is done, yield the collected result
                self.done = true;
                let collected = std::mem::take(&mut self.collected);
                Some(TaskStatus::Ready(collected))
            }
        }
    }
}

// ============================================================================
// Split Collector Combinators (Feature 07)
// ============================================================================

/// Observer branch from `split_collector()`.
///
/// Receives copies of items matching the predicate via a `ConcurrentQueue`.
/// Yields `Stream::Next` for matched items, forwards Pending/Delayed from source.
pub struct CollectorStreamIterator<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for CollectorStreamIterator<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // Try to get item from queue
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("CollectorStreamIterator: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                // Queue is empty, check if source is done (queue closed)
                if self.queue.is_closed() {
                    None
                } else {
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => None,
        }
    }
}

/// Continuation branch from `split_collector()`.
///
/// Wraps the original iterator, copying matched items to the observer queue
/// while continuing the chain for further combinators.
pub struct SplitCollectorContinuation<I: TaskIterator> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<I::Ready, I::Pending>>>,
    /// Predicate to determine which items to copy
    predicate: Box<dyn Fn(&I::Ready) -> bool + Send>,
}

impl<I> Iterator for SplitCollectorContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitCollectorContinuation: source exhausted, queue closed");
            return None;
        };

        // Copy matched items to observer queue
        if let TaskStatus::Ready(value) = &item {
            if (self.predicate)(value) {
                let stream_item = Stream::Next(value.clone());
                if let Err(e) = self.queue.force_push(stream_item) {
                    tracing::error!("SplitCollectorContinuation: failed to push to queue: {}", e);
                } else {
                    tracing::trace!(
                        "SplitCollectorContinuation: copied matched item to observer queue"
                    );
                }
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I> Drop for SplitCollectorContinuation<I>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        // Close the queue to signal that the source is done
        self.queue.close();
        tracing::debug!("SplitCollectorContinuation: dropped, queue closed");
    }
}

// ============================================================================
// Split Collect Until Combinator
// ============================================================================

/// Observer branch from `split_collect_until()`.
///
/// Receives copies of items until the predicate is met, then the queue
/// is closed and the observer completes.
pub struct SplitUntilObserver<D, P> {
    /// Shared queue receiving copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for SplitUntilObserver<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SplitUntilObserver: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    None
                } else {
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SplitUntilObserver: queue closed, returning None");
                None
            }
        }
    }
}

/// Continuation branch from `split_collect_until()`.
///
/// Wraps the original iterator, copying items to the observer queue
/// until the predicate is met. When predicate returns true, that item
/// is sent and the queue is closed (observer completes).
pub struct SplitUntilContinuation<I: TaskIterator> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send copied items to observer
    queue: Arc<ConcurrentQueue<Stream<I::Ready, I::Pending>>>,
    /// Predicate to determine when to close observer
    predicate: Box<dyn Fn(&I::Ready) -> CollectionState + Send>,
}

impl<I> Iterator for SplitUntilContinuation<I>
where
    I: TaskIterator,
    I::Ready: Clone,
    I::Pending: Clone,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitUntilContinuation: source exhausted, queue closed");
            return None;
        };

        // Handle items based on CollectionState from predicate
        if let TaskStatus::Ready(value) = &item {
            match (self.predicate)(value) {
                CollectionState::Skip => {
                    // Skip this item - don't send to observer
                    tracing::trace!(
                        "SplitUntilContinuation: skipping item (CollectionState::Skip)"
                    );
                }
                CollectionState::Collect => {
                    // Collect this item for the observer
                    let stream_item = Stream::Next(value.clone());
                    if let Err(e) = self.queue.force_push(stream_item) {
                        tracing::error!("SplitUntilContinuation: failed to push to queue: {}", e);
                    } else {
                        tracing::trace!("SplitUntilContinuation: collected item for observer");
                    }
                }
                CollectionState::Close(collect_this) => {
                    // Close the observer after optionally collecting this item
                    if collect_this {
                        let stream_item = Stream::Next(value.clone());
                        if let Err(e) = self.queue.force_push(stream_item) {
                            tracing::error!(
                                "SplitUntilContinuation: failed to push to queue: {}",
                                e
                            );
                        } else {
                            tracing::trace!("SplitUntilContinuation: collecting final item and closing observer queue");
                        }
                    } else {
                        tracing::trace!("SplitUntilContinuation: closing observer queue without collecting final item");
                    }
                    self.queue.close();
                    tracing::debug!("SplitUntilContinuation: observer queue closed after CollectionState::Close");
                }
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I> Drop for SplitUntilContinuation<I>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        // Close the queue as backup if not already closed
        if !self.queue.is_closed() {
            tracing::debug!("SplitUntilContinuation: dropped before completion, closing queue");
            self.queue.close();
        }
    }
}

// ============================================================================
// Split Collect Until Map Combinator
// ============================================================================

/// Observer branch from `split_collect_until_map()`.
///
/// Receives transformed copies of items until the predicate is met, then the queue
/// is closed and the observer completes.
pub struct SplitUntilObserverMap<D, P> {
    /// Shared queue receiving transformed copied items from the splitter
    queue: Arc<ConcurrentQueue<Stream<D, P>>>,
}

impl<D, P> Iterator for SplitUntilObserverMap<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => {
                tracing::trace!("SplitUntilObserverMap: received item from queue");
                Some(item)
            }
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    None
                } else {
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => {
                tracing::debug!("SplitUntilObserverMap: queue closed, returning None");
                None
            }
        }
    }
}

/// Continuation branch from `split_collect_until_map()`.
///
/// Wraps the original iterator. The transform function returns
/// `(CollectionState, Option<D>)` to control both collection behavior and transformation.
/// The continuation continues with original Ready values unchanged.
pub struct SplitUntilContinuationMap<I: TaskIterator, D> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send transformed copied items to observer
    queue: Arc<ConcurrentQueue<Stream<D, I::Pending>>>,
    /// Combined predicate + transform function
    transform: Box<dyn Fn(&I::Ready) -> (CollectionState, Option<D>) + Send>,
}

impl<I, D> Iterator for SplitUntilContinuationMap<I, D>
where
    I: TaskIterator,
    I::Pending: Clone,
    D: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            // Source iterator is naturally exhausted, close the queue
            self.queue.close();
            tracing::debug!("SplitUntilContinuationMap: source exhausted, queue closed");
            return None;
        };

        // Handle items based on (CollectionState, Option<D>) from transform
        if let TaskStatus::Ready(value) = &item {
            let (state, transformed) = (self.transform)(value);
            match state {
                CollectionState::Skip => {
                    // Skip this item - don't send to observer
                    tracing::trace!(
                        "SplitUntilContinuationMap: skipping item (CollectionState::Skip)"
                    );
                }
                CollectionState::Collect => {
                    // Collect this item for the observer (with transformation)
                    if let Some(transformed) = transformed {
                        let stream_item = Stream::Next(transformed);
                        if let Err(e) = self.queue.force_push(stream_item) {
                            tracing::error!(
                                "SplitUntilContinuationMap: failed to push to queue: {}",
                                e
                            );
                        } else {
                            tracing::trace!("SplitUntilContinuationMap: collected transformed item for observer");
                        }
                    }
                }
                CollectionState::Close(collect_this) => {
                    // Close the observer after optionally collecting this item
                    if collect_this {
                        if let Some(transformed) = transformed {
                            let stream_item = Stream::Next(transformed);
                            if let Err(e) = self.queue.force_push(stream_item) {
                                tracing::error!(
                                    "SplitUntilContinuationMap: failed to push to queue: {}",
                                    e
                                );
                            } else {
                                tracing::trace!("SplitUntilContinuationMap: collecting final transformed item and closing observer queue");
                            }
                        }
                    } else {
                        tracing::trace!("SplitUntilContinuationMap: closing observer queue without collecting final item");
                    }
                    self.queue.close();
                    tracing::debug!("SplitUntilContinuationMap: observer queue closed after CollectionState::Close");
                }
            }
        }

        // Always forward to continuation
        Some(item)
    }
}

impl<I, D> Drop for SplitUntilContinuationMap<I, D>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        // Close the queue as backup if not already closed
        if !self.queue.is_closed() {
            tracing::debug!("SplitUntilContinuationMap: dropped before completion, closing queue");
            self.queue.close();
        }
    }
}

// ============================================================================
// Split Collector Map Combinator
// ============================================================================

/// Observer branch from `split_collector_map()`.
///
/// Receives transformed copies of matched Ready items via a `ConcurrentQueue`.
/// The observer yields `Stream<M, P>` where M is the mapped type from the transform function.
pub struct SplitCollectorMapObserver<M, P> {
    /// Shared queue receiving transformed items from the continuation
    queue: Arc<ConcurrentQueue<Stream<M, P>>>,
}

impl<M, P> Iterator for SplitCollectorMapObserver<M, P>
where
    M: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<M, P>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Ok(item) => Some(item),
            Err(concurrent_queue::PopError::Empty) => {
                if self.queue.is_closed() {
                    None
                } else {
                    Some(Stream::Ignore)
                }
            }
            Err(concurrent_queue::PopError::Closed) => None,
        }
    }
}

/// Continuation branch from `split_collector_map()`.
///
/// Wraps the original iterator. The transform function returns `(bool, Option<M>)`:
/// - `true` + `Some(m)` sends `m` to the observer queue
/// - `false` or `None` skips sending to observer
///
/// The continuation continues with original Ready values unchanged.
pub struct SplitCollectorMapContinuation<I: TaskIterator, M> {
    /// The wrapped iterator
    inner: I,
    /// Queue to send transformed items to observer
    queue: Arc<ConcurrentQueue<Stream<M, I::Pending>>>,
    /// Combined predicate + transform function
    transform: Box<dyn Fn(&I::Ready) -> (bool, Option<M>) + Send>,
}

impl<I, M> Iterator for SplitCollectorMapContinuation<I, M>
where
    I: TaskIterator,
    I::Pending: Clone,
    M: Clone + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = if let Some(item) = self.inner.next_status() {
            item
        } else {
            self.queue.close();
            tracing::debug!("SplitCollectorMapContinuation: source exhausted, queue closed");
            return None;
        };

        if let TaskStatus::Ready(value) = &item {
            let (matched, transformed) = (self.transform)(value);
            if matched {
                if let Some(transformed) = transformed {
                    let stream_item = Stream::Next(transformed);
                    if let Err(e) = self.queue.force_push(stream_item) {
                        tracing::error!(
                            "SplitCollectorMapContinuation: failed to push to queue: {}",
                            e
                        );
                    } else {
                        tracing::trace!(
                            "SplitCollectorMapContinuation: copied transformed item to observer queue"
                        );
                    }
                }
            }
        }

        Some(item)
    }
}

impl<I, M> Drop for SplitCollectorMapContinuation<I, M>
where
    I: TaskIterator,
{
    fn drop(&mut self) {
        if !self.queue.is_closed() {
            self.queue.close();
        }
    }
}

// ===== Feature 08: Iterator Extension Completion Wrapper Structs =====

/// Wrapper for `map_state()` - transforms any `TaskStatus`
pub struct TMapState<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F> Iterator for TMapState<I, F>
where
    I: TaskIterator,
    F: Fn(
            TaskStatus<I::Ready, I::Pending, I::Spawner>,
        ) -> TaskStatus<I::Ready, I::Pending, I::Spawner>
        + Send
        + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_status().map(&self.mapper)
    }
}

/// Wrapper for `inspect_state()` - side-effect on any `TaskStatus`
pub struct TInspectState<I, F> {
    inner: I,
    inspector: F,
}

impl<I, F> Iterator for TInspectState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        (self.inspector)(&status);
        Some(status)
    }
}

/// Wrapper for `filter_state()` - filter based on full `TaskStatus`
pub struct TFilterState<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> Iterator for TFilterState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        if (self.predicate)(&status) {
            Some(status)
        } else {
            Some(TaskStatus::Ignore)
        }
    }
}

/// Wrapper for `take_while_state()` - take while state predicate true
pub struct TTakeWhileState<I, F> {
    inner: I,
    predicate: F,
    done: bool,
}

impl<I, F> Iterator for TTakeWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let status = self.inner.next_status()?;
        if (self.predicate)(&status) {
            Some(status)
        } else {
            self.done = true;
            None
        }
    }
}

/// Wrapper for `skip_while_state()` - skip while state predicate true
pub struct TSkipWhileState<I, F> {
    inner: I,
    predicate: F,
    done_skipping: bool,
}

impl<I, F> Iterator for TSkipWhileState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        if !self.done_skipping && (self.predicate)(&status) {
            return Some(TaskStatus::Ignore);
        }
        self.done_skipping = true;
        Some(status)
    }
}

/// Wrapper for `take_state()` - take at most n items matching predicate
pub struct TTakeState<I, F> {
    inner: I,
    remaining: usize,
    state_predicate: F,
}

impl<I, F> Iterator for TTakeState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let status = self.inner.next_status()?;
        if (self.state_predicate)(&status) {
            self.remaining -= 1;
        }
        Some(status)
    }
}

/// Wrapper for `skip_state()` - skip first n items matching predicate
pub struct TSkipState<I, F> {
    inner: I,
    to_skip: usize,
    state_predicate: F,
}

impl<I, F> Iterator for TSkipState<I, F>
where
    I: TaskIterator,
    F: Fn(&TaskStatus<I::Ready, I::Pending, I::Spawner>) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        if self.to_skip > 0 && (self.state_predicate)(&status) {
            self.to_skip -= 1;
            return Some(TaskStatus::Ignore);
        }
        Some(status)
    }
}

/// Wrapper for `enumerate()` - adds index to Ready items
pub struct TEnumerate<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for TEnumerate<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<(usize, I::Ready), I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next_status()?;
        match status {
            TaskStatus::Ready(v) => {
                let item = TaskStatus::Ready((self.count, v));
                self.count += 1;
                Some(item)
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
        }
    }
}

/// Wrapper for `find()` - find first Ready matching predicate
pub struct TFind<I, F> {
    inner: I,
    predicate: F,
    found: bool,
}

impl<I, F> Iterator for TFind<I, F>
where
    I: TaskIterator,
    F: Fn(&I::Ready) -> bool + Send + 'static,
{
    type Item = TaskStatus<Option<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.found {
            return None;
        }

        let status = self.inner.next_status()?;
        match status {
            TaskStatus::Ready(v) => {
                if (self.predicate)(&v) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(v)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
        }
    }
}

/// Wrapper for `find_map()` - find first Ready that maps to Some
pub struct TFindMap<I, F, R> {
    inner: I,
    mapper: F,
    found: bool,
    _phantom: std::marker::PhantomData<R>,
}

impl<I, F, R> Iterator for TFindMap<I, F, R>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> Option<R> + Send + 'static,
    R: Send + 'static,
{
    type Item = TaskStatus<Option<R>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.found {
            return None;
        }

        let status = self.inner.next_status()?;
        match status {
            TaskStatus::Ready(v) => {
                if let Some(r) = (self.mapper)(v) {
                    self.found = true;
                    Some(TaskStatus::Ready(Some(r)))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            TaskStatus::Pending(p) => Some(TaskStatus::Pending(p)),
            TaskStatus::Delayed(d) => Some(TaskStatus::Delayed(d)),
            TaskStatus::Init => Some(TaskStatus::Init),
            TaskStatus::Spawn(s) => Some(TaskStatus::Spawn(s)),
            TaskStatus::Ignore => Some(TaskStatus::Ignore),
        }
    }
}

/// Wrapper for `fold()` - accumulate values
pub struct TFold<I, F, R> {
    inner: I,
    acc: Option<R>,
    folder: F,
    done: bool,
    _phantom: std::marker::PhantomData<(I, R)>,
}

impl<I, F, R> Iterator for TFold<I, F, R>
where
    I: TaskIterator,
    F: Fn(R, I::Ready) -> R + Send + 'static,
    R: Send + 'static,
{
    type Item = TaskStatus<R, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => {
                if let Some(acc) = self.acc.take() {
                    self.acc = Some((self.folder)(acc, v));
                }
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            None => {
                // Inner exhausted, yield final accumulated value
                self.done = true;
                self.acc.take().map(TaskStatus::Ready)
            }
        }
    }
}

impl<I, F, R> Drop for TFold<I, F, R> {
    fn drop(&mut self) {
        // Iterator was not fully consumed
    }
}

/// Wrapper for `all()` - check if all Ready satisfy predicate
pub struct TAll<I, F> {
    inner: I,
    predicate: F,
    all_true: bool,
    done: bool,
}

impl<I, F> Iterator for TAll<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> bool + Send + 'static,
{
    type Item = TaskStatus<bool, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if !self.all_true {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => {
                if !(self.predicate)(v) {
                    self.all_true = false;
                    self.done = true;
                    Some(TaskStatus::Ready(false))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            None => {
                self.done = true;
                Some(TaskStatus::Ready(true))
            }
        }
    }
}

/// Wrapper for `any()` - check if any Ready satisfies predicate
pub struct TAny<I, F> {
    inner: I,
    predicate: F,
    any_true: bool,
    done: bool,
}

impl<I, F> Iterator for TAny<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> bool + Send + 'static,
{
    type Item = TaskStatus<bool, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if self.any_true {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ready(v)) => {
                if (self.predicate)(v) {
                    self.any_true = true;
                    self.done = true;
                    Some(TaskStatus::Ready(true))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            None => {
                self.done = true;
                Some(TaskStatus::Ready(false))
            }
        }
    }
}

/// Wrapper for `count()` - count Ready items
pub struct TCount<I> {
    inner: I,
    count: usize,
}

impl<I> Iterator for TCount<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<usize, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next_status() {
            Some(TaskStatus::Ready(_)) => {
                self.count += 1;
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            None => Some(TaskStatus::Ready(self.count)),
        }
    }
}

/// Wrapper for `count_all()` - count all items
pub struct TCountAll<I> {
    inner: I,
    count: usize,
    done: bool,
}

impl<I> Iterator for TCountAll<I>
where
    I: TaskIterator,
{
    type Item = TaskStatus<usize, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.inner.next_status() {
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            Some(TaskStatus::Ready(_))
            | Some(TaskStatus::Pending(_))
            | Some(TaskStatus::Delayed(_))
            | Some(TaskStatus::Init)
            | Some(TaskStatus::Spawn(_)) => {
                self.count += 1;
                Some(TaskStatus::Ignore)
            }
            None => {
                self.done = true;
                Some(TaskStatus::Ready(self.count))
            }
        }
    }
}

// ============================================================================
// TaskIterator implementations for wrapped types
// ============================================================================

// Note: Implementations for &, &mut, Box, Rc, RefCell, Arc, Mutex are in task.rs
// ===== map_circuit combinator =====

/// Wrapper for `map_circuit()` - applies a circuit function to each `TaskStatus`.
///
/// The circuit function determines whether to continue iteration,
/// return a value and stop, or just stop.
pub struct TMapCircuit<I: TaskIterator> {
    inner: I,
    circuit: Box<
        dyn Fn(
                TaskStatus<I::Ready, I::Pending, I::Spawner>,
            ) -> TaskShortCircuit<I::Ready, I::Pending, I::Spawner>
            + Send,
    >,
    stopped: bool,
    _phantom: std::marker::PhantomData<I>,
}

impl<I> Iterator for TMapCircuit<I>
where
    I: TaskIterator + Send + 'static,
    I::Ready: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: ExecutionAction + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've stopped, return None permanently
        if self.stopped {
            return None;
        }

        // Get the next item from the inner iterator
        let status = self.inner.next_status()?;

        // Apply the circuit function
        match (self.circuit)(status) {
            TaskShortCircuit::Continue(item) => Some(item),
            TaskShortCircuit::ReturnAndStop(item) => {
                // Mark as stopped so future calls return None
                self.stopped = true;
                Some(item)
            }
            TaskShortCircuit::Stop => {
                // Mark as stopped and return None
                self.stopped = true;
                None
            }
        }
    }
}
