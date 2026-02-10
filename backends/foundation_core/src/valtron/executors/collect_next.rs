use std::{any::Any, marker::PhantomData};

use crate::synca::Entry;
use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    ExecutionAction, ExecutionIterator, State, TaskIterator, TaskStatus,
};

use crate::compati::Mutex;

/// [`CollectNext`] provides an implementer of [`ExecutionIterator`] collects all the .
/// ready state [`TaskStatus::Ready`] into a provided [`Vec`] that will be the container
/// of all the results.
///
/// It ensures to move your task [`TaskIterator`] forward, applying the actions correctly
/// and forwarding the sgianls (delay, pending) properly.
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
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(iter: Task, list: &'a mut Vec<TaskStatus<Done, Pending, Action>>) -> Self {
        Self {
            list,
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
impl<'a: 'static, Action, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for CollectNext<'a, Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'a,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::from_over_into)]
impl<'a: 'static, Action, Task, Done: 'a, Pending: 'a> Into<BoxedExecutionIterator>
    for CollectNext<'a, Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'a,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'a,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::needless_lifetimes)]
impl<Task, Done, Pending, Action> ExecutionIterator for CollectNext<'_, Action, Task, Done, Pending>
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
                TaskStatus::Ready(next) => {
                    self.list.push(TaskStatus::Ready(next));
                    State::ReadyValue(entry.clone())
                }
            },
            None => State::Done,
        })
    }
}
