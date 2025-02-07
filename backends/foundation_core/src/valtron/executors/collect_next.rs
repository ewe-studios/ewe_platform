use std::{any::Any, marker::PhantomData};

use super::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    ExecutionAction, ExecutionIterator, State, TaskIterator, TaskStatus,
};
use crate::synca::{AbortIfPanic, Entry};

#[cfg(not(feature = "web_spin_lock"))]
use std::sync::Mutex;

#[cfg(feature = "web_spin_lock")]
use wasm_sync::Mutex;

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
    task: Mutex<Task>,
    panic_handler: Option<BoxedPanicHandler>,
    _marker: PhantomData<(Action, Done, Pending)>,
    list: &'a mut Vec<TaskStatus<Done, Pending, Action>>,
}

impl<'a, Action, Task, Done, Pending> CollectNext<'a, Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'a,
    Task: TaskIterator<Pending = Pending, Done = Done, Spawner = Action>,
{
    pub fn new(iter: Task, list: &'a mut Vec<TaskStatus<Done, Pending, Action>>) -> Self {
        Self {
            list,
            panic_handler: None,
            task: Mutex::new(iter),
            _marker: PhantomData::default(),
        }
    }

    pub fn with_panic_handler<T>(mut self, handler: T) -> Self
    where
        T: Fn(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panic_handler = Some(Box::new(handler));
        self
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
        let task_response = match std::panic::catch_unwind(|| self.task.lock().unwrap().next()) {
            Ok(inner) => inner,
            Err(panic_error) => {
                // Guard ensures we can handle panic safely, if guard
                // gets dropped then `PanicHandler` paniced as well, so
                // we must abort immediately.
                let abort_guard = AbortIfPanic::default();
                if let Some(panic_handler) = &self.panic_handler {
                    (panic_handler)(panic_error);
                    std::mem::drop(abort_guard);
                }
                return Some(State::Paniced);
            }
        };

        Some(match task_response {
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
