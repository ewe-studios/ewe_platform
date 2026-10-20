#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::type_complexity)]
#![allow(clippy::extra_unused_lifetimes)]

use std::{any::Any, marker::PhantomData};

use crate::synca::Entry;
use crate::valtron::{
    BoxedExecutionEngine, BoxedExecutionIterator, BoxedPanicHandler, BoxedSendExecutionIterator,
    Ctx, ExecutionAction, ExecutionIterator, State, TaskIterator,
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
pub struct OnNext<Resolver, Task, Done, Pending>
where
    Resolver: TaskReadyResolver<Done, Pending>,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    resolver: Resolver,
    panic_handler: Option<BoxedPanicHandler>,
    _types: PhantomData<(Pending, Done)>,
}

impl<Resolver, Task, Done, Pending> OnNext<Resolver, Task, Done, Pending>
where
    Resolver: TaskReadyResolver<Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done>,
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

#[allow(clippy::from_over_into)]
impl<Resolver, Task, Done, Pending> Into<BoxedExecutionIterator>
    for OnNext<Resolver, Task, Done, Pending>
where
    Done: 'static,
    Pending: 'static,
    Resolver: TaskReadyResolver<Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::needless_lifetimes)]
#[allow(clippy::from_over_into)]
impl<'a: 'static, Resolver, Task, Done: Send + 'a, Pending: Send + 'a>
    Into<BoxedSendExecutionIterator> for OnNext<Resolver, Task, Done, Pending>
where
    Done: Send,
    Pending: Send,
    Resolver: TaskReadyResolver<Done, Pending> + Send + 'a,
    Task: TaskIterator<Pending = Pending, Ready = Done> + Send + 'a,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

impl<Task, Resolver, Done, Pending> ExecutionIterator
    for OnNext<Resolver, Task, Done, Pending>
where
    Resolver: TaskReadyResolver<Done, Pending>,
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
