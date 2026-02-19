//! Custom builders for executors

use crate::valtron::{SpawnInfo, Stream};

use std::{any::Any, marker::PhantomData, sync::Arc, time};

use crate::{
    synca::mpp::{RecvIterator, StreamRecvIterator},
    valtron::StreamConsumingIter,
};
use concurrent_queue::ConcurrentQueue;

use crate::{synca::Entry, valtron::AnyResult};

use crate::valtron::{
    BoxedExecutionEngine, BoxedPanicHandler, BoxedSendExecutionIterator, ConsumingIter, DoNext,
    ExecutionAction, ExecutorError, FnMutReady, FnReady, OnNext, TaskIterator, TaskReadyResolver,
    TaskStatus, TaskStatusMapper,
};

pub struct ExecutionTaskIteratorBuilder<
    Done,
    Pending,
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
> {
    engine: BoxedExecutionEngine,
    task: Option<Task>,
    parent: Option<Entry>,
    resolver: Option<Resolver>,
    mappers: Option<Vec<Mapper>>,
    panic_handler: Option<BoxedPanicHandler>,
    _marker: PhantomData<(Done, Pending, Action)>,
}

impl<
        Done: 'static,
        Pending: 'static,
        Action: ExecutionAction + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
        Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
    > ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, Resolver, Task>
{
    #[must_use]
    pub fn new(engine: BoxedExecutionEngine) -> Self {
        Self {
            engine,
            task: None,
            parent: None,
            mappers: None,
            resolver: None,
            panic_handler: None,
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn with_mappers(mut self, mapper: Mapper) -> Self {
        if let Some(mappers) = &mut self.mappers {
            mappers.push(mapper);
        } else {
            self.mappers = Some(vec![mapper]);
        }
        self
    }

    #[must_use]
    pub fn with_panic_handler<T>(mut self, handler: T) -> Self
    where
        T: Fn(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panic_handler = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn maybe_parent(mut self, parent: Option<Entry>) -> Self {
        self.parent = parent;
        self
    }

    #[must_use]
    pub fn with_parent(mut self, parent: Entry) -> Self {
        self.parent = Some(parent);
        self
    }

    #[must_use]
    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    #[must_use]
    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// [`scheduled_stream_iter`] adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a [`StreamRecvIterator`] that implements the [`Iterator`] trait.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// This will schedule the task to the bottom of the queue by calling `schedule`
    /// method to deliver a task to the bottom of the thread-local execution queue.
    pub fn scheduled_stream_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<StreamRecvIterator<Done, Pending>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<Stream<Done, Pending>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => StreamConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => StreamConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .schedule(boxed_task.into())
            .map(|_| StreamRecvIterator::new(RecvIterator::from_chan(iter_chan, wait_cycle)))
    }

    /// [`schedule_iter`] adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a [`RecvIterator`] that implements the [`Iterator`] trait.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// Following our naming: [`schedule_iter`] calls the `schedule` method to deliver
    /// a task to the bottom of the thread-local execution queue.
    #[must_use]
    pub fn schedule_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<RecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .schedule(boxed_task.into())
            .map(|_| RecvIterator::from_chan(iter_chan, wait_cycle))
    }

    /// [`schedule`] delivers a task to the bottom of the thread-local execution queue.
    #[must_use]
    pub fn schedule(self) -> AnyResult<SpawnInfo, ExecutorError> {
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

    /// [`lift_iter`] adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a [`RecvIterator`] that implements the [`Iterator`] trait.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// Following our naming: [`lift_iter`] calls the `lift` method to deliver
    /// a task to the top of the thread-local execution queue.
    #[must_use]
    pub fn lift_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<RecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let parent = self.parent;
        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .lift(boxed_task.into(), parent)
            .map(|_| RecvIterator::from_chan(iter_chan, wait_cycle))
    }

    /// [`stream_lift_iter`] similar to [`lift_iter`] returns a [`StreamRecvIterator`]
    /// which returns a simplified representatiopn without the complexity of the [`Action`]
    /// type providing easier usage within logic blocks.
    #[must_use]
    pub fn stream_lift_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<StreamRecvIterator<Done, Pending>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<Stream<Done, Pending>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let parent = self.parent;
        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => StreamConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => StreamConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .lift(boxed_task.into(), parent)
            .map(|_| StreamRecvIterator::new(RecvIterator::from_chan(iter_chan, wait_cycle)))
    }

    /// [`lift`] delivers a task to the top of the thread-local execution queue.
    #[must_use]
    pub fn lift(self) -> AnyResult<SpawnInfo, ExecutorError> {
        let parent = self.parent;
        match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let task_iter = OnNext::new(task, resolver, mappers);
                    self.engine.lift(task_iter.into(), parent)
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

    /// [`sequenced_iter`] creates a iterator from a sequenced task allowing you
    /// to use that to get the result.
    #[must_use]
    pub fn sequenced_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<RecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let Some(parent) = self.parent else {
            return Err(ExecutorError::ParentMustBeSupplied);
        };

        let iter_chan: Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .sequenced(boxed_task.into(), parent)
            .map(|_| RecvIterator::from_chan(iter_chan, wait_cycle))
    }

    /// Creates a stream from a sequenced task, allowing iteration to retrieve results.
    /// Returns: The SpawnInfo for the result of the operation, or an error.
    #[must_use]
    pub fn stream_sequenced_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<StreamRecvIterator<Done, Pending>, ExecutorError> {
        let Some(parent) = self.parent else {
            return Err(ExecutorError::ParentMustBeSupplied);
        };

        let iter_chan: Arc<ConcurrentQueue<Stream<Done, Pending>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => StreamConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => StreamConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .sequenced(boxed_task.into(), parent)
            .map(|_| StreamRecvIterator::new(RecvIterator::from_chan(iter_chan, wait_cycle)))
    }

    /// Places a task at the head of the thread-local execution queue with the given parent.
    ///
    /// Returns: The SpawnInfo for the result of the operation, or an error.
    #[must_use]
    pub fn sequenced(self) -> AnyResult<SpawnInfo, ExecutorError> {
        let Some(parent) = self.parent else {
            return Err(ExecutorError::ParentMustBeSupplied);
        };

        match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => {
                    let task_iter = OnNext::new(task, resolver, mappers);
                    self.engine.sequenced(task_iter.into(), parent)
                }
                (Some(resolver), None) => {
                    let task_iter = OnNext::new(task, resolver, Vec::<Mapper>::new());
                    self.engine.sequenced(Box::new(task_iter), parent)
                }
                (None, None) => {
                    let task_iter = DoNext::new(task);
                    self.engine.sequenced(Box::new(task_iter), parent)
                }
                (None, Some(_)) => Err(ExecutorError::FailedToCreate),
            },
            None => Err(ExecutorError::TaskRequired),
        }
    }
}

impl<
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, Resolver, Task>
{
    /// [`broadcast`] delivers a task to the bottom of the global execution queue.
    #[must_use]
    pub fn broadcast(self) -> AnyResult<SpawnInfo, ExecutorError> {
        let task: AnyResult<BoxedSendExecutionIterator, ExecutorError> = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (Some(resolver), Some(mappers)) => Ok(OnNext::new(task, resolver, mappers).into()),
                (Some(resolver), None) => {
                    Ok(OnNext::new(task, resolver, Vec::<Mapper>::new()).into())
                }
                (None, None) => Ok(DoNext::new(task).into()),
                (None, Some(_)) => Err(ExecutorError::FailedToCreate),
            },
            None => Err(ExecutorError::TaskRequired),
        };

        self.engine.broadcast(task?)
    }

    /// [`stream_broadcast_iter`] similar to [`broasdcast_iter`] returns a [`StreamRecvIterator`]
    /// which returns a simplified representatiopn without the complexity of the [`Action`]
    /// type providing easier usage within logic blocks.
    #[must_use]
    pub fn stream_broadcast_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<StreamRecvIterator<Done, Pending>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<Stream<Done, Pending>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => StreamConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => StreamConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .broadcast(boxed_task.into())
            .map(|_| StreamRecvIterator::new(RecvIterator::from_chan(iter_chan, wait_cycle)))
    }

    /// [`broadcast_iter`] adds a task into execution queue but instead of depending
    /// on a [`TaskReadyResolver`] to process the final state instead allows you
    /// to get back a wrapper iterator that allows you synchronously receive those
    /// values from a [`RecvIterator`] that implements the [`Iterator`] trait.
    ///
    /// This makes it possible to build synchronous experiences in a async world.
    ///
    /// Following our naming: [`broadcast_iter`] calls the `broadcast` method to deliver
    /// a task to the bottom of the global execution queue.
    #[must_use]
    pub fn broadcast_iter(
        self,
        wait_cycle: time::Duration,
    ) -> AnyResult<RecvIterator<TaskStatus<Done, Pending, Action>>, ExecutorError> {
        let iter_chan: Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>> =
            Arc::new(ConcurrentQueue::unbounded());

        let boxed_task = match self.task {
            Some(task) => match (self.resolver, self.mappers) {
                (None, Some(mappers)) => ConsumingIter::new(task, mappers, iter_chan.clone()),
                (None, None) => ConsumingIter::new(task, Vec::new(), iter_chan.clone()),
                (_, _) => return Err(ExecutorError::NotSupported),
            },
            None => return Err(ExecutorError::TaskRequired),
        };

        self.engine
            .broadcast(boxed_task.into())
            .map(|_| RecvIterator::from_chan(iter_chan, wait_cycle))
    }
}

impl<
        F,
        Done: 'static,
        Pending: 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
    > ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, FnMutReady<F, Action>, Task>
where
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
{
    #[must_use]
    pub fn on_next_mut(self, action: F) -> Self
    where
        F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
    {
        self.with_resolver(FnMutReady::new(action))
    }
}

impl<
        F,
        Done: 'static,
        Pending: 'static,
        Action: ExecutionAction + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
    > ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, FnReady<F, Action>, Task>
where
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
{
    #[must_use]
    pub fn on_next(self, action: F) -> Self {
        self.with_resolver(FnReady::new(action))
    }
}

impl<
        F,
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, FnReady<F, Action>, Task>
where
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    #[must_use]
    pub fn on_send_next(self, action: F) -> Self {
        self.with_resolver(FnReady::new(action))
    }
}

impl<
        F,
        Done: Send + 'static,
        Pending: Send + 'static,
        Action: ExecutionAction + Send + 'static,
        Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
        Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    > ExecutionTaskIteratorBuilder<Done, Pending, Action, Mapper, FnMutReady<F, Action>, Task>
where
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    #[must_use]
    pub fn on_send_next_mut(self, action: F) -> Self
    where
        F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
    {
        self.with_resolver(FnMutReady::new(action))
    }
}

/// [`spawn_builder`] returns a new instance of the [`ExecutionTaskIteratorBuilder`]
/// which maps the task mapper and resolver to boxed versions which will be heap allocated
/// for easier usage.
#[must_use]
#[allow(clippy::type_complexity)]
pub fn spawn_builder<Task, Action>(
    engine: BoxedExecutionEngine,
) -> ExecutionTaskIteratorBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + 'static>,
    Task,
>
where
    Task::Ready: 'static,
    Task::Pending: 'static,
    Task: TaskIterator<Spawner = Action> + 'static,
    Action: ExecutionAction + 'static,
{
    ExecutionTaskIteratorBuilder::new(engine)
}

/// [`spawn_broadcaster`] returns a new instance of the [`ExecutionTaskIteratorBuilder`]
/// which maps the task mapper and resolver to boxed versions which will be heap allocated
/// for easier usage.
#[must_use]
#[allow(clippy::type_complexity)]
pub fn spawn_broadcaster<Task, Action>(
    engine: BoxedExecutionEngine,
) -> ExecutionTaskIteratorBuilder<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + Send + 'static>,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
{
    ExecutionTaskIteratorBuilder::new(engine)
}
