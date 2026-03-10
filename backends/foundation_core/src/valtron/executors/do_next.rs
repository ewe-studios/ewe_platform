use std::{any::Any, marker::PhantomData};

use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    ExecutionAction, ExecutionIterator, State, TaskIterator, TaskStatus,
};

use crate::compati::Mutex;
use crate::synca::Entry;

/// [`DoNext`] is a [`ExecutionIterator`] that only cares about
/// actions and progress. It ignores the values of [`TaskStatus::Ready`]
/// and only ensurs your task iterator progresses but specifcally ensures
/// to handle your [`TaskStatus::Spawn`] against the executor.
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

    #[must_use]
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
                TaskStatus::Spawn(mut action) => match action.apply(Some(entry), executor) {
                    Ok(info) => State::SpawnFinished(info),
                    Err(err) => {
                        tracing::error!("Failed to apply ExecutionAction: {:?}", err);
                        State::SpawnFailed(entry)
                    }
                },
                TaskStatus::Ready(_) => State::ReadyValue(entry),
            },
            None => State::Done,
        })
    }
}
