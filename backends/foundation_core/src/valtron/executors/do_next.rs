use std::marker::PhantomData;

use super::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedSendExecutionIterator, ExecutionAction,
    ExecutionIterator, State, TaskIterator, TaskStatus,
};
use crate::synca::Entry;

/// DoNext provides an implementer of `ExecutionIterator` which is focused on
/// making progress for your `TaskIterator` which focuses on making progress in
/// an async manner with out and most importantly not using the results the
/// `TaskIterator` pushes out.
///
/// The `DoNext` is focused around just driving the underlying process/operation
/// your iterator performs.
pub struct DoNext<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator,
{
    pub task: Task,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Action, Task, Done, Pending> DoNext<Action, Task, Done, Pending>
where
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

impl<Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for DoNext<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<'a: 'static, Action, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for DoNext<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'a,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

impl<Task, Done, Pending, Action> ExecutionIterator for DoNext<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
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
