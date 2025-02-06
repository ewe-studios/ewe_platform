// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::marker::PhantomData;

use super::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedSendExecutionIterator, ExecutionAction,
    ExecutionIterator, FnMutReady, FnReady, State, TaskIterator, TaskReadyResolver, TaskStatus,
    TaskStatusMapper,
};
use crate::synca::Entry;

pub struct OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator,
{
    pub task: Task,
    pub resolver: Resolver,
    pub local_mappers: Vec<Mapper>,
    _types: PhantomData<(Action, Pending, Done)>,
}

impl<Action, Resolver, Mapper, Task, Done, Pending>
    OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    pub fn new(iter: Task, resolver: Resolver, mappers: Vec<Mapper>) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
            _types: PhantomData::default(),
        }
    }

    pub fn add_mapper(mut self, mapper: Mapper) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

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
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next(
        t: Task,
        f: F,
        mappers: Option<Vec<Box<dyn TaskStatusMapper<Done, Pending, Action> + Send>>>,
    ) -> Self {
        let wrapper = FnReady::new(f);
        OnNext::new(t, wrapper, mappers.unwrap_or(Vec::new()))
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
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next_mut(
        t: Task,
        f: F,
        mappers: Option<Vec<Box<dyn TaskStatusMapper<Done, Pending, Action> + Send>>>,
    ) -> Self {
        let wrapper = FnMutReady::new(f);
        OnNext::new(t, wrapper, mappers.unwrap_or(Vec::new()))
    }
}

impl<Action, Resolver, Mapper, Task, Done, Pending> Into<BoxedExecutionIterator>
    for OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<'a: 'static, Action, Resolver, Mapper, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for OnNext<Action, Resolver, Mapper, Task, Done, Pending>
where
    Done: Send,
    Pending: Send,
    Action: ExecutionAction + Send + 'a,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'a,
    Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'a,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'a,
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
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
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
                            Ok(_) => State::SpawnFinished,
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

// impl<Task, Resolver, Mapper, Done, Pending, Action> ExecutionIterator
//     for OnNext<Action, Resolver, Mapper, Task, Done, Pending>
// where
//     Done: Send,
//     Pending: Send,
//     Mapper: TaskStatusMapper<Done, Pending, Action> + Send,
//     Resolver: TaskReadyResolver<Action, Done, Pending> + Send,
//     Action: ExecutionAction + Send + 'static,
//     Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'static,
// {
//     fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
//         Some(match self.task.next() {
//             Some(inner) => {
//                 let mut previous_response = Some(inner);
//                 for mapper in &mut self.local_mappers {
//                     previous_response = mapper.map(previous_response);
//                 }
//                 match previous_response {
//                     Some(value) => match value {
//                         TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
//                         TaskStatus::Pending(_) => State::Pending(None),
//                         TaskStatus::Init => State::Pending(None),
//                         TaskStatus::Spawn(action) => match action.apply(entry, executor) {
//                             Ok(_) => State::SpawnFinished,
//                             Err(err) => {
//                                 tracing::error!("Failed to apply ExectionAction: {:?}", err);
//                                 State::SpawnFailed
//                             }
//                         },
//                         TaskStatus::Ready(content) => {
//                             self.resolver.handle(TaskStatus::Ready(content), executor);
//                             State::Progressed
//                         }
//                     },
//                     None => State::Pending(None),
//                 }
//             }
//             None => State::Done,
//         })
//     }
// }
