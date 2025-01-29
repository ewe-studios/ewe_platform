// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::marker::PhantomData;

use super::{
    BoxedExecutionIterator, ExecutionAction, ExecutionEngine, ExecutionIterator, FnMutReady,
    FnReady, State, TaskIterator, TaskReadyResolver, TaskStatus, TaskStatusMapper,
};
use crate::synca::Entry;

pub struct OnNext<Engine, Action, Resolver, Mapper, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Engine, Action, Done, Pending>,
    Task: TaskIterator,
{
    pub task: Task,
    pub resolver: Resolver,
    pub local_mappers: Vec<Mapper>,
    _engine: PhantomData<(Engine, Action, Pending, Done)>,
}

impl<Engine, Action, Resolver, Mapper, Task, Done, Pending>
    OnNext<Engine, Action, Resolver, Mapper, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Engine, Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    pub fn new(iter: Task, resolver: Resolver, mappers: Vec<Mapper>) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
            _engine: PhantomData::default(),
        }
    }

    pub fn add_mapper(mut self, mapper: Mapper) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

impl<F, Engine, Action, Task, Done, Pending>
    OnNext<
        Engine,
        Action,
        FnReady<F, Engine, Action>,
        Box<dyn TaskStatusMapper<Done, Pending, Action>>,
        Task,
        Done,
        Pending,
    >
where
    Engine: ExecutionEngine,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
    F: Fn(TaskStatus<Done, Pending, Action>, Engine) + 'static,
{
    pub fn on_next(
        t: Task,
        f: F,
        mappers: Option<Vec<Box<dyn TaskStatusMapper<Done, Pending, Action>>>>,
    ) -> Self {
        let wrapper = FnReady::new(f);
        OnNext::new(t, wrapper, mappers.unwrap_or(Vec::new()))
    }
}

impl<F, Engine, Action, Task, Done, Pending>
    OnNext<
        Engine,
        Action,
        FnMutReady<F, Engine, Action>,
        Box<dyn TaskStatusMapper<Done, Pending, Action>>,
        Task,
        Done,
        Pending,
    >
where
    Engine: ExecutionEngine,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
    F: FnMut(TaskStatus<Done, Pending, Action>, Engine) + 'static,
{
    pub fn on_next_mut(
        t: Task,
        f: F,
        mappers: Option<Vec<Box<dyn TaskStatusMapper<Done, Pending, Action>>>>,
    ) -> Self {
        let wrapper = FnMutReady::new(f);
        OnNext::new(t, wrapper, mappers.unwrap_or(Vec::new()))
    }
}

impl<Engine, Action, Resolver, Mapper, Task, Done: 'static, Pending: 'static>
    Into<BoxedExecutionIterator<Engine>>
    for OnNext<Engine, Action, Resolver, Mapper, Task, Done, Pending>
where
    Engine: ExecutionEngine + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Resolver: TaskReadyResolver<Engine, Action, Done, Pending> + 'static,
    Action: ExecutionAction<Engine = Engine> + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator<Engine> {
        Box::new(self)
    }
}

impl<Engine, Task, Resolver, Mapper, Done, Pending, Action> ExecutionIterator
    for OnNext<Engine, Action, Resolver, Mapper, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Engine, Action, Done, Pending>,
    Action: ExecutionAction<Engine = Engine>,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    type Executor = Engine;

    fn next(&mut self, entry: Entry, executor: Self::Executor) -> Option<State> {
        Some(match self.task.next() {
            Some(inner) => {
                let mut previous_response = Some(inner);
                for mapper in &mut self.local_mappers {
                    previous_response = mapper.map(previous_response);
                }
                match previous_response {
                    Some(value) => match value {
                        TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
                        TaskStatus::Pending(_) => State::Pending(None),
                        TaskStatus::Init => State::Pending(None),
                        TaskStatus::Spawn(action) => match action.apply(entry, executor) {
                            Ok(_) => State::Progressed,
                            Err(err) => {
                                tracing::error!("Failed to apply ExectionAction: {:?}", err);
                                State::SpawnFailed
                            }
                        },
                        TaskStatus::Ready(content) => {
                            self.resolver.handle(TaskStatus::Ready(content), executor);
                            State::Progressed
                        }
                    },
                    None => State::Pending(None),
                }
            }
            None => State::Done,
        })
    }
}
