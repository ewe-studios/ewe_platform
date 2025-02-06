use std::marker::PhantomData;

use super::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedSendExecutionIterator, ExecutionAction,
    ExecutionIterator, State, TaskIterator, TaskStatus,
};
use crate::synca::Entry;

/// CollectNext provides an implementer of `ExecutionIterator` which is focused on
/// making progress for your `TaskIterator` which focuses on making progress in
/// an async manner with out and most importantly not using the results the
/// `TaskIterator` pushes out.
///
/// The `CollectNext` is focused around just driving the underlying process/operation
/// your iterator performs.
pub struct CollectNext<'a, Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Task,
    list: &'a mut Vec<TaskStatus<Done, Pending, Action>>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<'a, Action, Task, Done, Pending> CollectNext<'a, Action, Task, Done, Pending>
where
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

impl<'a: 'static, Action, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for CollectNext<'a, Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'a,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

impl<'a: 'static, Action, Task, Done: 'a, Pending: 'a> Into<BoxedExecutionIterator>
    for CollectNext<'a, Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'a,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action> + 'a,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<'a, Task, Done, Pending, Action> ExecutionIterator
    for CollectNext<'a, Action, Task, Done, Pending>
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
