use std::{cell, marker::PhantomData, time};

use derive_more::derive::From;

use crate::{
    synca::Entry,
    valtron::{AnyResult, GenericResult},
};

use super::{task::TaskStatus, DoNext, LocalExecutorEngine, OnNext, TaskIterator};

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

    /// The state is sent out when there was an attempt to spawn
    /// a task from another and that failed which is not a desired
    /// or wanted state to be in ever.
    SpawnFailed,

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

    fn next(&mut self, entry: Entry, executor: Self::Executor) -> Option<State>;
}

pub trait IntoBoxedExecutionIterator<Executor> {
    fn into_box_execution_iterator(self) -> Box<dyn ExecutionIterator<Executor = Executor>>;
}

impl<F, M> IntoBoxedExecutionIterator<M> for F
where
    M: ExecutionEngine,
    F: ExecutionIterator<Executor = M> + 'static,
{
    fn into_box_execution_iterator(self) -> Box<dyn ExecutionIterator<Executor = M>> {
        Box::new(self)
    }
}

pub type BoxedLocalExecutionIterator = Box<dyn ExecutionIterator<Executor = LocalExecutorEngine>>;

pub type BoxedCloneSendLocalExecutorIterator =
    Box<dyn ClonableExecutionIterator<Executor = LocalExecutorEngine> + Send>;

pub type BoxedCloneLocalExecutorIterator =
    Box<dyn ClonableExecutionIterator<Executor = LocalExecutorEngine>>;

pub trait ClonableExecutionIterator: ExecutionIterator {
    fn clone_execution_iterator(
        &self,
    ) -> Box<dyn ClonableExecutionIterator<Executor = Self::Executor>>;
}

pub trait IntoRawExecutionIterator: ClonableExecutionIterator {
    fn into_execution_iterator(self) -> Box<dyn ExecutionIterator<Executor = Self::Executor>>;
}

impl<M, Executor> IntoRawExecutionIterator for M
where
    Executor: ExecutionEngine,
    M: ClonableExecutionIterator<Executor = Executor> + 'static,
{
    fn into_execution_iterator(self) -> Box<dyn ExecutionIterator<Executor = Self::Executor>> {
        Box::new(self)
    }
}

impl<M, Executor> ClonableExecutionIterator for M
where
    Executor: ExecutionEngine,
    M: ExecutionIterator<Executor = Executor> + Clone + 'static,
{
    fn clone_execution_iterator(
        &self,
    ) -> Box<dyn ClonableExecutionIterator<Executor = Self::Executor>> {
        Box::new(self.clone())
    }
}

pub struct CanCloneExecutionIterator<E: ExecutionEngine>(
    Box<dyn ClonableExecutionIterator<Executor = E>>,
);

impl CanCloneExecutionIterator<LocalExecutorEngine> {
    pub fn new(elem: Box<dyn ClonableExecutionIterator<Executor = LocalExecutorEngine>>) -> Self {
        Self(elem)
    }
}

impl<E: ExecutionEngine> Clone for CanCloneExecutionIterator<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_execution_iterator())
    }
}

impl<E: ExecutionEngine> ExecutionIterator for CanCloneExecutionIterator<E> {
    type Executor = E;

    fn next(&mut self, entry: Entry, executor: Self::Executor) -> Option<State> {
        self.0.next(entry, executor)
    }
}

impl<'a, M, Executor> ExecutionIterator for &'a mut M
where
    Executor: ExecutionEngine,
    M: ExecutionIterator<Executor = Executor>,
{
    type Executor = M::Executor;

    fn next(&mut self, entry: Entry, executor: Self::Executor) -> Option<State> {
        (**self).next(entry, executor)
    }
}

impl<M, Executor> ExecutionIterator for Box<M>
where
    Executor: ExecutionEngine,
    M: ExecutionIterator<Executor = Executor> + ?Sized,
{
    type Executor = M::Executor;

    fn next(&mut self, entry: Entry, executor: Self::Executor) -> Option<State> {
        (**self).next(entry, executor)
    }
}

#[derive(Clone, Debug, From)]
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

    /// A given executor has no provided task in the case of a TaskIterator
    /// based ExecutorIterator.
    TaskRequired,
}

impl std::error::Error for ExecutorError {}

impl core::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct ExecutionTaskIteratorBuilder<
    Done,
    Pending,
    Engine: ExecutionEngine,
    Action: ExecutionAction<Executor = Engine>,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Engine, Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
> {
    engine: Engine,
    task: Option<Task>,
    parent: Option<Entry>,
    resolver: Option<Resolver>,
    mappers: Option<Vec<Mapper>>,
    _marker: PhantomData<(Done, Pending, Action)>,
}

impl<
        Done: 'static,
        Pending: 'static,
        Engine: ExecutionEngine<Executor = Engine> + 'static,
        Action: ExecutionAction<Executor = Engine> + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
        Resolver: TaskReadyResolver<Engine, Action, Done, Pending> + 'static,
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
    > ExecutionTaskIteratorBuilder<Done, Pending, Engine, Action, Mapper, Resolver, Task>
{
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            task: None,
            parent: None,
            mappers: None,
            resolver: None,
            _marker: PhantomData::default(),
        }
    }

    pub fn with_mappers(mut self, mapper: Mapper) -> Self {
        let mut mappers = if self.mappers.is_some() {
            self.mappers.take().unwrap()
        } else {
            Vec::new()
        };

        mappers.push(mapper);
        self.mappers = Some(mappers);
        self
    }

    pub fn with_parent(mut self, parent: Entry) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    pub fn lift(self) -> AnyResult<(), ExecutorError> {
        let parent = self.parent;
        match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let task_iter = OnNext::new(task, resolver, mappers);
                    self.engine.lift(Box::new(task_iter), parent)
                }
                (Some(resolver), None) => {
                    let task_iter = OnNext::new(task, resolver, Vec::<Mapper>::new());
                    self.engine.lift(Box::new(task_iter), parent)
                }
                (None, None) => {
                    let task_iter = DoNext::new(task);
                    self.engine.lift(Box::new(task_iter), parent)
                }
                (None, Some(_)) => Err(ExecutorError::FailedToCreate),
            },
            None => Err(ExecutorError::TaskRequired),
        }
    }

    pub fn schedule(self) -> AnyResult<(), ExecutorError> {
        match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let task_iter = OnNext::new(task, resolver, mappers);
                    self.engine.schedule(Box::new(task_iter))
                }
                (Some(resolver), None) => {
                    let task_iter = OnNext::new(task, resolver, Vec::<Mapper>::new());
                    self.engine.schedule(Box::new(task_iter))
                }
                (None, None) => {
                    let task_iter = DoNext::new(task);
                    self.engine.schedule(Box::new(task_iter))
                }
                (None, Some(_)) => Err(ExecutorError::FailedToCreate),
            },
            None => Err(ExecutorError::TaskRequired),
        }
    }

    pub fn broadcast(self) -> AnyResult<(), ExecutorError> {
        match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let task_iter = OnNext::new(task, resolver, mappers);
                    self.engine.broadcast(Box::new(task_iter))
                }
                (Some(resolver), None) => {
                    let task_iter = OnNext::new(task, resolver, Vec::<Mapper>::new());
                    self.engine.broadcast(Box::new(task_iter))
                }
                (None, None) => {
                    let task_iter = DoNext::new(task);
                    self.engine.broadcast(Box::new(task_iter))
                }
                (None, Some(_)) => Err(ExecutorError::FailedToCreate),
            },
            None => Err(ExecutorError::TaskRequired),
        }
    }
}

impl<
        F,
        Done: 'static,
        Pending: 'static,
        Engine: ExecutionEngine<Executor = Engine> + 'static,
        Action: ExecutionAction<Executor = Engine> + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
    >
    ExecutionTaskIteratorBuilder<
        Done,
        Pending,
        Engine,
        Action,
        Mapper,
        FnMutReady<F, Engine, Action>,
        Task,
    >
where
    F: FnMut(TaskStatus<Done, Pending, Action>, Engine) + 'static,
{
    pub fn on_next_mut(self, action: F) -> Self
    where
        F: Fn(TaskStatus<Done, Pending, Action>, Engine) + 'static,
    {
        self.with_resolver(FnMutReady::new(action))
    }
}

impl<
        F,
        Done: 'static,
        Pending: 'static,
        Engine: ExecutionEngine<Executor = Engine> + 'static,
        Action: ExecutionAction<Executor = Engine> + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
        Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
    >
    ExecutionTaskIteratorBuilder<
        Done,
        Pending,
        Engine,
        Action,
        Mapper,
        FnReady<F, Engine, Action>,
        Task,
    >
where
    F: Fn(TaskStatus<Done, Pending, Action>, Engine) + 'static,
{
    pub fn on_next(self, action: F) -> Self
    where
        F: Fn(TaskStatus<Done, Pending, Action>, Engine) + 'static,
    {
        self.with_resolver(FnReady::new(action))
    }
}

/// ExecutorEngine is the backbone of the valtron execution model
/// they can be spawned within threads or be the singular owner
/// of a thread which the user/caller create to manage execution within the
/// thread.
pub trait ExecutionEngine {
    type Executor;

    /// lift prioritizes an incoming task to the top of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the ExecutionEngine).
    ///
    /// If `parent` is provided then a dependency connection is made
    /// with the relevant parent's identified by the `Entry` key.
    fn lift(
        &self,
        task: Box<dyn ExecutionIterator<Executor = Self::Executor>>,
        parent: Option<Entry>,
    ) -> AnyResult<(), ExecutorError>;

    /// lift adds provided incoming task to the bottom of the local
    /// execution queue which pauses all processing task till that
    /// point till the new task is done or goes to sleep (dependent on
    /// the internals of the ExecutionEngine).
    fn schedule(
        &self,
        task: Box<dyn ExecutionIterator<Executor = Self::Executor>>,
    ) -> AnyResult<(), ExecutorError>;

    /// broadcast allows you to deliver a task to the global execution queue
    /// which then lets the giving task to be sent of to the same or another
    /// executor in another thread for processing, which requires the type to be
    /// `Send` safe.
    fn broadcast(
        &self,
        task: Box<dyn ExecutionIterator<Executor = Self::Executor>>,
    ) -> AnyResult<(), ExecutorError>;
}

pub type BoxedExecutionIterator<M> = Box<dyn ExecutionIterator<Executor = M>>;

/// TaskSpawner represents a underlying type that can
/// spawn some other task by using the provided executor.
pub trait ExecutionAction {
    type Executor: ExecutionEngine;

    fn apply(self, key: Entry, executor: Self::Executor) -> GenericResult<()>;
}

pub type NoSpawner = NoAction<LocalExecutorEngine>;

#[derive(Default, Clone, Debug)]
pub struct NoAction<E: ExecutionEngine + Clone>(PhantomData<E>);

impl<E: ExecutionEngine + Clone> ExecutionAction for NoAction<E> {
    type Executor = E;

    fn apply(self, _entry: Entry, _engine: Self::Executor) -> GenericResult<()> {
        // do nothing
        Ok(())
    }
}

pub type BoxedTaskReadyResolver<E, S, D, P> = Box<dyn TaskReadyResolver<E, S, D, P>>;

/// `TaskResolver` are types implementing this trait to
/// perform final resolution of a task when the task emits
/// the relevant `TaskStatus::Ready` enum state.
///
/// Unlike `TaskStatusMapper` these implementing types do
/// not care about the varying states of a `TaskIterator`
/// but about the final state of the task when it signals
/// it's readiness via the `TaskStatus::Ready` state.
pub trait TaskReadyResolver<E, S: ExecutionAction, D, P> {
    fn handle(&self, item: TaskStatus<D, P, S>, engine: E);
}

impl<'a, F, E, S, D, P> TaskReadyResolver<E, S, D, P> for &'a mut F
where
    E: ExecutionEngine,
    S: ExecutionAction,
    F: TaskReadyResolver<E, S, D, P>,
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: E) {
        (**self).handle(item, engine)
    }
}

impl<F, E, S, D, P> TaskReadyResolver<E, S, D, P> for Box<F>
where
    E: ExecutionEngine,
    S: ExecutionAction,
    F: TaskReadyResolver<E, S, D, P> + ?Sized,
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: E) {
        (**self).handle(item, engine)
    }
}

pub struct FnMutReady<F, E, S>(cell::RefCell<F>, PhantomData<(E, S)>);

impl<F, E, S> FnMutReady<F, E, S> {
    pub fn new(f: F) -> Self {
        Self(cell::RefCell::new(f), PhantomData::default())
    }
}

impl<F, S, E, D, P> TaskReadyResolver<E, S, D, P> for FnMutReady<F, E, S>
where
    S: ExecutionAction,
    F: FnMut(TaskStatus<D, P, S>, E),
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: E) {
        let mut mut_fn = self.0.borrow_mut();
        (mut_fn)(item, engine)
    }
}

pub struct FnReady<F, E, S>(F, PhantomData<(E, S)>);

impl<F, E, S> FnReady<F, E, S> {
    pub fn new(f: F) -> Self {
        Self(f, PhantomData::default())
    }
}

impl<F, S, E, D, P> TaskReadyResolver<E, S, D, P> for FnReady<F, E, S>
where
    S: ExecutionAction,
    F: Fn(TaskStatus<D, P, S>, E),
{
    fn handle(&self, item: TaskStatus<D, P, S>, engine: E) {
        self.0(item, engine)
    }
}

/// `TaskStatusMapper` are types implementing this trait to
/// perform unique operations on the underlying `TaskStatus`
/// received, possibly generating a new `TaskStatus`.
pub trait TaskStatusMapper<D, P, S: ExecutionAction> {
    fn map(&mut self, item: Option<TaskStatus<D, P, S>>) -> Option<TaskStatus<D, P, S>>;
}

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

    use super::{FnMapper, NoSpawner, TaskStatusMapper};

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

        match self.iter.next() {
            Some(elem) => match elem {
                TaskStatus::Delayed(dur) => Some(TaskStatus::Delayed(dur)),
                TaskStatus::Spawn(inner) => Some(TaskStatus::Spawn(inner)),
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
