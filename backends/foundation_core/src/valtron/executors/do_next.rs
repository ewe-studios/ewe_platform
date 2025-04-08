use std::{any::Any, marker::PhantomData};

use super::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    ExecutionAction, ExecutionIterator, State, TaskIterator, TaskStatus,
};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use wasm_sync::Mutex;

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
    // we wrap it in a Mutex to trade of some performance
    // cost due to OS level calls Mutex uses to get a
    // task that is `UnwindSafe`
    task: Mutex<Task>,
    panic_handler: Option<BoxedPanicHandler>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Action, Task, Done, Pending> DoNext<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(iter: Task) -> Self {
        Self {
            panic_handler: None,
            task: Mutex::new(iter),
            _marker: PhantomData,
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

#[allow(clippy::from_over_into)]
impl<Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for DoNext<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::extra_unused_lifetimes)]
#[allow(clippy::from_over_into)]
impl<'a: 'static, Action, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for DoNext<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'a,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::needless_lifetimes)]
impl<Task, Done, Pending, Action> ExecutionIterator for DoNext<Action, Task, Done, Pending>
where
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
            Some(inner) => match inner {
                TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
                TaskStatus::Pending(_) => State::Pending(None),
                TaskStatus::Init => State::Pending(None),
                TaskStatus::Spawn(action) => match action.apply(entry, executor) {
                    Ok(_) => State::SpawnFinished,
                    Err(err) => {
                        tracing::error!("Failed to apply ExecutionAction: {:?}", err);
                        State::SpawnFailed
                    }
                },
                TaskStatus::Ready(_) => State::Progressed,
            },
            None => State::Done,
        })
    }
}
