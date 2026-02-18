#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::type_complexity)]
#![allow(clippy::extra_unused_lifetimes)]

use std::{any::Any, marker::PhantomData};

use crate::synca::Entry;
use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    ExecutionAction, ExecutionIterator, FnMutReady, FnReady, State, TaskIterator,
    TaskReadyResolver, TaskStatus, TaskStatusMapper,
};

use crate::compati::Mutex;

/// [`OnNext`] is unique in that you provided it a [`Resolver`]
/// which is applied to every [`TaskStatus::Ready`] message received.
///
/// It provides us a means to perform specific behaviours to the results
/// whilst forwarding other signals: pending, delayed to others for insights.
///
/// It will apply your [`TaskStatus::Spawn`] directly to the executor as it receives them.
pub struct OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    resolver: Resolver,
    local_mappers: Vec<Mapper>,
    panic_handler: Option<BoxedPanicHandler>,
    _types: PhantomData<(Action, Pending, Done)>,
}

impl<Action, Resolver, Mapper, Task, Done, Pending>
    OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(iter: Task, resolver: Resolver, mappers: Vec<Mapper>) -> Self {
        Self {
            resolver,
            panic_handler: None,
            task: Mutex::new(iter),
            local_mappers: mappers,
            _types: PhantomData,
        }
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
    pub fn add_mapper(mut self, mapper: Mapper) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

#[allow(clippy::self_named_constructors)]
impl<F, Action, Task, Done, Pending>
    OnNext<
        Action,
        FnReady<F, Action>,
        Box<dyn TaskStatusMapper<Done, Pending, Action> + Send>,
        Task,
        Done,
        Pending,
    >
where
    Done: Send,
    Pending: Send,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next(
        t: Task,
        f: F,
        mappers: Option<Vec<Box<dyn TaskStatusMapper<Done, Pending, Action> + Send>>>,
    ) -> Self {
        let wrapper = FnReady::new(f);
        OnNext::new(t, wrapper, mappers.unwrap_or_default())
    }
}

impl<F, Action, Task, Done, Pending>
    OnNext<
        Action,
        FnMutReady<F, Action>,
        Box<dyn TaskStatusMapper<Done, Pending, Action> + Send>,
        Task,
        Done,
        Pending,
    >
where
    Done: Send,
    Pending: Send,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next_mut(
        t: Task,
        f: F,
        mappers: Option<Vec<Box<dyn TaskStatusMapper<Done, Pending, Action> + Send>>>,
    ) -> Self {
        let wrapper = FnMutReady::new(f);
        OnNext::new(t, wrapper, mappers.unwrap_or_default())
    }
}

#[allow(clippy::from_over_into)]
impl<Action, Resolver, Mapper, Task, Done, Pending> Into<BoxedExecutionIterator>
    for OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::needless_lifetimes)]
#[allow(clippy::from_over_into)]
impl<'a: 'static, Action, Resolver, Mapper, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Done: Send,
    Pending: Send,
    Action: ExecutionAction + Send + 'a,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'a,
    Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'a,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

impl<Task, Resolver, Mapper, Done, Pending, Action> ExecutionIterator
    for OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let task_response = match std::panic::catch_unwind(|| self.task.lock().unwrap().next()) {
            Ok(inner) => inner,
            Err(panic_error) => {
                if let Some(panic_handler) = &self.panic_handler {
                    (panic_handler)(panic_error);
                }
                return Some(State::Panicked);
            }
        };

        Some(match task_response {
            Some(inner) => {
                let mut previous_response = Some(inner);
                for mapper in &mut self.local_mappers {
                    previous_response = mapper.map(previous_response);
                }
                match previous_response {
                    Some(value) => match value {
                        TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
                        TaskStatus::Init | TaskStatus::Pending(_) => State::Pending(None),
                        TaskStatus::Spawn(mut action) => {
                            match action.apply(Some(entry), executor) {
                                Ok(info) => State::SpawnFinished(info),
                                Err(err) => {
                                    tracing::error!("Failed to apply ExecutionAction: {:?}", err);
                                    State::SpawnFailed(entry)
                                }
                            }
                        }
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
