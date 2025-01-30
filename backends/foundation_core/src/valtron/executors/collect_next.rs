use std::marker::PhantomData;

use super::{
    BoxedExecutionIterator, ExecutionAction, ExecutionEngine, ExecutionIterator, State,
    TaskIterator, TaskStatus,
};
use crate::synca::Entry;

/// CollectNext provides an implementer of `ExecutionIterator` which is focused on
/// making progress for your `TaskIterator` which focuses on making progress in
/// an async manner with out and most importantly not using the results the
/// `TaskIterator` pushes out.
///
/// The `CollectNext` is focused around just driving the underlying process/operation
/// your iterator performs.
pub struct CollectNext<'a, Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Task,
    list: &'a mut Vec<TaskStatus<Done, Pending, Action>>,
    _marker: PhantomData<(Engine, Action, Done, Pending)>,
}

impl<'a, Engine, Action, Task, Done, Pending> CollectNext<'a, Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine,
    Action: ExecutionAction + 'a,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    pub fn new(iter: Task, list: &'a mut Vec<TaskStatus<Done, Pending, Action>>) -> Self {
        Self {
            list,
            task: iter,
            _marker: PhantomData::default(),
        }
    }
}

impl<'a: 'static, Engine, Action, Task, Done: 'a, Pending: 'a> Into<BoxedExecutionIterator<Engine>>
    for CollectNext<'a, Engine, Action, Task, Done, Pending>
where
    Engine: ExecutionEngine + 'a,
    Action: ExecutionAction<Executor = Engine> + 'a,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'a,
{
    fn into(self) -> BoxedExecutionIterator<Engine> {
        Box::new(self)
    }
}

impl<'a, Engine, Task, Done, Pending, Action> ExecutionIterator
    for CollectNext<'a, Engine, Action, Task, Done, Pending>
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
                    Ok(_) => State::Progressed,
                    Err(err) => {
                        tracing::error!("Failed to apply ExectionAction: {:?}", err);
                        State::SpawnFailed
                    }
                },
                TaskStatus::Ready(next) => {
                    self.list.push(TaskStatus::Ready(next));
                    State::Progressed
                }
            },
            None => State::Done,
        })
    }
}
