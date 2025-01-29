// Implements Vultron executors based on a multi-threaded model of thread pools that can communicate
// via a ConcurrentQueue that allows different threads in the pool to pull task off the thread at their
// own respective pace.

use std::marker::PhantomData;

use super::{
    BoxedExecutionIterator, BoxedTaskReadyResolver, ExecutionAction, ExecutionEngine,
    ExecutionIterator, FnMutReady, FnReady, LocalExecutorEngine, State, TaskIterator, TaskStatus,
    TaskStatusMapper,
};
use crate::synca::Entry;

pub struct OnNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction,
    Task: TaskIterator,
{
    pub task: Task,
    pub resolver: BoxedTaskReadyResolver<Engine, Action, Done, Pending>,
    pub local_mappers: Vec<Box<dyn TaskStatusMapper<Done, Pending, Action>>>,
    _engine: PhantomData<Engine>,
}

impl<Engine, Action, Task, Done, Pending> OnNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    pub fn new(
        iter: Task,
        resolver: BoxedTaskReadyResolver<Engine, Action, Done, Pending>,
        mappers: Vec<Box<dyn TaskStatusMapper<Done, Pending, Action>>>,
    ) -> Self {
        Self {
            resolver,
            task: iter,
            local_mappers: mappers,
            _engine: PhantomData::default(),
        }
    }

    pub fn on_next_mut<F>(t: Task, f: F) -> Self
    where
        Engine: ExecutionEngine<Executor = LocalExecutorEngine> + 'static,
        F: FnMut(TaskStatus<Done, Pending, Action>, Engine) + 'static,
    {
        let wrapper = FnMutReady::new(f);
        OnNext::new(t, Box::new(wrapper), Vec::new())
    }

    pub fn on_next<F>(t: Task, f: F) -> Self
    where
        Engine: ExecutionEngine<Executor = LocalExecutorEngine> + 'static,
        F: Fn(TaskStatus<Done, Pending, Action>, Engine) + 'static,
    {
        let wrapper = FnReady::new(f);
        OnNext::new(t, Box::new(wrapper), Vec::new())
    }

    pub fn add_mapper(mut self, mapper: Box<dyn TaskStatusMapper<Done, Pending, Action>>) -> Self {
        self.local_mappers.push(mapper);
        self
    }
}

impl<Engine, Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator<Engine>>
    for OnNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine + 'static,
    Action: ExecutionAction<Engine = Engine> + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator<Engine> {
        Box::new(self)
    }
}

impl<Engine, Task, Done, Pending, Action> ExecutionIterator
    for OnNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
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
