use std::{any::Any, marker::PhantomData};

use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    Ctx, ExecutionAction, ExecutionIterator, State, TaskIterator, TaskStatus,
};

use crate::compati::Mutex;
use crate::synca::Entry;

/// [`DoNext`] is a [`ExecutionIterator`] that only cares about
/// actions and progress. It ignores the values of [`TaskStatus::Ready`]
/// and only ensurs your task iterator progresses but specifcally ensures
/// to handle your [`TaskStatus::Spawn`] against the executor.
pub struct DoNext<Task, Done, Pending>
where
    Task: TaskIterator,
{
    // we wrap it in a Mutex to trade of some performance
    // cost due to OS level calls Mutex uses to get a
    // task that is `UnwindSafe`
    task: Mutex<Task>,
    panic_handler: Option<BoxedPanicHandler>,
    _marker: PhantomData<(Done, Pending)>,
}

impl<Task, Done, Pending> DoNext<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
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
impl<Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for DoNext<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::extra_unused_lifetimes)]
#[allow(clippy::from_over_into)]
impl<'a: 'static, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for DoNext<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::needless_lifetimes)]
impl<Task, Done, Pending> ExecutionIterator for DoNext<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status(ctx.clone())
        })) {
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
                TaskStatus::Ignore => State::Pending(None),
                TaskStatus::Wait => State::Wait,
                TaskStatus::Spread(_) => State::Pending(None),
                TaskStatus::Depends(signal) => State::Depends(signal),
            },
            None => State::Done,
        })
    }
}
