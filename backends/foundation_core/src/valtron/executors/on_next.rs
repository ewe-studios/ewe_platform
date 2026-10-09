#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::type_complexity)]
#![allow(clippy::extra_unused_lifetimes)]

use std::{any::Any, marker::PhantomData};

use crate::synca::Entry;
use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    ExecutionAction, ExecutionIterator, FnMutReady, FnReady, State, TaskIterator,
    TaskReadyResolver, TaskStatus,
};

use crate::compati::Mutex;

/// [`OnNext`] is unique in that you provided it a `Resolver`
/// which is applied to every [`TaskStatus::Ready`] message received.
///
/// It provides us a means to perform specific behaviours to the results
/// whilst forwarding other signals: pending, delayed to others for insights.
///
/// It will apply your [`TaskStatus::Spawn`] directly to the executor as it receives them.
///
/// WHY: post-task transformation (mapping/filtering the produced `TaskStatus`
/// stream) is expressed upstream via `TaskIteratorExt` combinators (`map_ready`,
/// `map_pending`, `filter_ready`, …) applied to the `Task` before it reaches the
/// builder — so `OnNext` carries no `Mapper` machinery, only the ready-`Resolver`.
pub struct OnNext<Action, Resolver, Task, Done, Pending>
where
    Action: ExecutionAction,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    resolver: Resolver,
    panic_handler: Option<BoxedPanicHandler>,
    _types: PhantomData<(Action, Pending, Done)>,
}

impl<Action, Resolver, Task, Done, Pending> OnNext<Action, Resolver, Task, Done, Pending>
where
    Action: ExecutionAction,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(iter: Task, resolver: Resolver) -> Self {
        Self {
            resolver,
            panic_handler: None,
            task: Mutex::new(iter),
            _types: PhantomData,
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

#[allow(clippy::self_named_constructors)]
impl<F, Action, Task, Done, Pending> OnNext<Action, FnReady<F, Action>, Task, Done, Pending>
where
    Done: Send,
    Pending: Send,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next(t: Task, f: F) -> Self {
        OnNext::new(t, FnReady::new(f))
    }
}

impl<F, Action, Task, Done, Pending> OnNext<Action, FnMutReady<F, Action>, Task, Done, Pending>
where
    Done: Send,
    Pending: Send,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    pub fn on_next_mut(t: Task, f: F) -> Self {
        OnNext::new(t, FnMutReady::new(f))
    }
}

#[allow(clippy::from_over_into)]
impl<Action, Resolver, Task, Done, Pending> Into<BoxedExecutionIterator>
    for OnNext<Action, Resolver, Task, Done, Pending>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::needless_lifetimes)]
#[allow(clippy::from_over_into)]
impl<'a: 'static, Action, Resolver, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for OnNext<Action, Resolver, Task, Done, Pending>
where
    Done: Send,
    Pending: Send,
    Action: ExecutionAction + Send + 'a,
    Resolver: TaskReadyResolver<Action, Done, Pending> + Send + 'a,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

impl<Task, Resolver, Done, Pending, Action> ExecutionIterator
    for OnNext<Action, Resolver, Task, Done, Pending>
where
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status()
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
            Some(value) => match value {
                TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
                TaskStatus::Init | TaskStatus::Pending(_) => State::Pending(None),
                TaskStatus::Spawn(mut action) => match action.apply(Some(entry), executor) {
                    Ok(info) => State::SpawnFinished(info),
                    Err(err) => {
                        tracing::error!("Failed to apply ExecutionAction: {:?}", err);
                        State::SpawnFailed(entry)
                    }
                },
                TaskStatus::Ready(content) => {
                    self.resolver.handle(TaskStatus::Ready(content), executor);
                    State::Progressed
                }
                TaskStatus::Ignore => State::Pending(None),
                TaskStatus::Wait => State::Wait,
                TaskStatus::Spread(_) => State::Pending(None),
                TaskStatus::Depends(signal) => State::Depends(signal),
            },
            None => State::Done,
        })
    }
}
