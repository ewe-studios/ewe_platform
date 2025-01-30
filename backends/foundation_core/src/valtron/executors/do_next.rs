use std::marker::PhantomData;

use super::{
    BoxedExecutionIterator, ExecutionAction, ExecutionEngine, ExecutionIterator, State,
    TaskIterator, TaskStatus,
};
use crate::synca::Entry;

/// DoNext provides an implementer of `ExecutionIterator` which is focused on
/// making progress for your `TaskIterator` which focuses on making progress in
/// an async manner with out and most importantly not using the results the
/// `TaskIterator` pushes out.
///
/// The `DoNext` is focused around just driving the underlying process/operation
/// your iterator performs.
pub struct DoNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction,
    Task: TaskIterator,
{
    pub task: Task,
    _marker: PhantomData<(Engine, Action, Done, Pending)>,
}

impl<Engine, Action, Task, Done, Pending> DoNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    pub fn new(iter: Task) -> Self {
        Self {
            task: iter,
            _marker: PhantomData::default(),
        }
    }
}

impl<Engine, Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator<Engine>>
    for DoNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine + 'static,
    Action: ExecutionAction<Executor = Engine> + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator<Engine> {
        Box::new(self)
    }
}

impl<Engine, Task, Done, Pending, Action> ExecutionIterator
    for DoNext<Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction<Executor = Engine>,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    type Executor = Engine;

    fn next(&mut self, entry: Entry, executor: Self::Executor) -> Option<State> {
        Some(match self.task.next() {
            Some(inner) => match inner {
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
                TaskStatus::Ready(_) => State::Progressed,
            },
            None => State::Done,
        })
    }
}
