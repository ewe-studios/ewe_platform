#![allow(clippy::type_complexity)]
#![allow(clippy::items_after_test_module)]

use std::{any::Any, marker::PhantomData};

use crate::compati::Mutex;

use crate::synca::Entry;
use crate::valtron::executors::local::NotifyQueue;
use crate::valtron::iterators::Stream;
use concurrent_queue::PushError;

use crate::valtron::{
    task::{TaskSpread, TaskStatus}, BoxedExecutionEngine, BoxedPanicHandler, ExecutionAction, TaskIterator,
};
use crate::valtron::{
    BoxedExecutionIterator, ExecutionIterator, QueueVacancyReadiness, State,
};
// Only the `multi`-gated `Into<BoxedSendExecutionIterator>` impls use this alias.
#[cfg(feature = "multi")]
use crate::valtron::BoxedSendExecutionIterator;

/// [`StreamConsumingIter`] provides an implementer of `ExecutionIterator` which is focused
/// consuming the produced [`Stream`] output values from the execution of actual tasks
/// except for the [`TaskStatus::Spawn`] variant.
///
/// This means you will only ever get the values from a [`Stream::Next`], [`Stream::Init`]
/// [`Stream::Delayed`] and [`Stream::Pending`] variants through
/// a wrapped [`ConcurrentQueue`] via the [`crate::synca::mpp::RecvIterator`].
///
/// This also means these types must be send-safe.
pub struct StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<Stream<Done, Pending>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<Stream<Done, Pending>>,
    /// Spread items being iterated (for Spread)
    spread_items: Vec<Stream<Done, Pending>>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Action, Task, Done, Pending> StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(
        iter: Task,
        chan: std::sync::Arc<NotifyQueue<Stream<Done, Pending>>>,
    ) -> Self {
        Self {
            channel: chan,
            alive: Some(()),
            panic_handler: None,
            task: Mutex::new(iter),
            pending_msg: None,
            spread_items: Vec::new(),
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

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: Send + 'static, Pending: Send + 'static>
    Into<BoxedSendExecutionIterator> for StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedExecutionIterator>
    for StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[cfg(not(feature = "multi"))]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Action, Task, Done, Pending> StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    /// Shared `next()` body.  `on_full` builds the `State::Depends(...)` for a
    /// full delivery queue — the closure is constructed in the cfg-gated
    /// `ExecutionIterator::next` wrappers so that the `Send` bounds needed under
    /// `multi` (for `QueueVacancyReadiness` coercion into `EventReadinessPtr`)
    /// are supplied by the wrapper's own bounds rather than leaking into this
    /// inherent method.
    fn drive(&mut self, entry: Entry, executor: BoxedExecutionEngine, on_full: impl FnOnce() -> State) -> Option<State> {
        if self.alive.is_none() {
            return None;
        }

        // Retry stashed pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return Some(on_full());
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return Some(State::Done);
                }
            }
        }

        // Drain spread items before polling the task
        if !self.spread_items.is_empty() {
            let item = self.spread_items.remove(0);
            match self.channel.push(item) {
                Ok(()) => return Some(State::Pending(None)),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return Some(on_full());
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return Some(State::Done);
                }
            }
        }

        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status()
        })) {
            Ok(inner) => inner,
            Err(panic_error) => {
                if let Some(panic_handler) = &self.panic_handler {
                    (panic_handler)(panic_error);
                }

                // A panicked task produces nothing further, so its result
                // channel must be closed exactly as on the `Done` path above.
                //
                // Leaving it open strands every collector: `sync_collect_one`
                // (and so `block_on_future`, and so `#[valtron_test]`) waits on
                // a stream that will never yield and never end. A failing
                // assertion inside a task then wedges the whole test in a futex
                // instead of reporting a failure — the panic is swallowed here,
                // and the harness never even prints `test result:`.
                self.channel.close();
                self.alive.take();
                return Some(State::Panicked);
            }
        };

        if task_response.is_none() {
            self.channel.close();
            self.alive.take();
            return Some(State::Done);
        }

        let inner = task_response.unwrap();
        Some(match inner {
            TaskStatus::Spawn(mut action) => match action.apply(Some(entry), executor) {
                Ok(info) => State::SpawnFinished(info),
                Err(err) => {
                    tracing::error!("Failed to to spawn action: {:?}", err);
                    State::SpawnFailed(entry)
                }
            },
            TaskStatus::Ignore => match self.channel.push(Stream::Ignore) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(Stream::Ignore);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Delayed(inner) => match self.channel.push(Stream::Delayed(inner)) {
                Ok(()) => State::Pending(Some(inner)),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(Stream::Delayed(inner));
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Init => match self.channel.push(Stream::Init) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(Stream::Init);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Pending(inner) => match self.channel.push(Stream::Pending(inner)) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Ready(inner) => match self.channel.push(Stream::Next(inner)) {
                Ok(()) => State::ReadyValue(entry),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Wait => match self.channel.push(Stream::Wait) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(Stream::Wait);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Spread(items) => {
                self.spread_items = items
                    .into_iter()
                    .map(|item| match item {
                        TaskSpread::Ready(d) => Stream::Next(d),
                        TaskSpread::Pending(p) => Stream::Pending(p),
                    })
                    .collect();
                if !self.spread_items.is_empty() {
                    let item = self.spread_items.remove(0);
                    match self.channel.push(item) {
                        Ok(()) => State::Pending(None),
                        Err(PushError::Full(msg)) => {
                            self.pending_msg = Some(msg);
                            on_full()
                        }
                        Err(PushError::Closed(_)) => {
                            self.channel.close();
                            self.alive.take();
                            State::Done
                        }
                    }
                } else {
                    State::Pending(None)
                }
            }
            TaskStatus::Depends(signal) => State::Depends(signal),
        })
    }
}

#[cfg(feature = "multi")]
impl<Task, Done, Pending, Action> ExecutionIterator
    for StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
    Done: Send + 'static,
    Pending: Send + 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let queue = self.channel.queue_arc();
        let on_full = move || {
            State::Depends(std::sync::Arc::new(QueueVacancyReadiness::new(queue)))
        };
        self.drive(entry, executor, on_full)
    }
}

#[cfg(not(feature = "multi"))]
impl<Task, Done, Pending, Action> ExecutionIterator
    for StreamConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
    Done: 'static,
    Pending: 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let queue = self.channel.queue_arc();
        let on_full = move || {
            State::Depends(std::sync::Arc::new(QueueVacancyReadiness::new(queue)))
        };
        self.drive(entry, executor, on_full)
    }
}

/// [`ConsumingIter`] provides an implementer of `ExecutionIterator` which is focused
/// consuming the produced `TaskStatus` output values from the execution of actual tasks
/// except for the [`TaskStatus::Spawn`] variant.
///
/// This means you will only ever get the values from a [`TaskStatus::Ready`], [`TaskStatus::Init`]
/// [`TaskStatus::Delayed`] and [`TaskStatus::Pending`] variants through
/// a wrapped [`ConcurrentQueue`] via the [`crate::synca::mpp::RecvIterator`].
///
/// This also means these types must be send-safe.
pub struct ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<TaskStatus<Done, Pending, Action>>,
    /// Spread items being iterated (for Spread)
    spread_items: Vec<TaskStatus<Done, Pending, Action>>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Action, Task, Done, Pending> ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(
        iter: Task,
        chan: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    ) -> Self {
        Self {
            channel: chan,
            alive: Some(()),
            panic_handler: None,
            task: Mutex::new(iter),
            pending_msg: None,
            spread_items: Vec::new(),
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

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: Send + 'static, Pending: Send + 'static>
    Into<BoxedSendExecutionIterator> for ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedExecutionIterator>
    for ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[cfg(not(feature = "multi"))]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Action, Task, Done, Pending> ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    fn drive(&mut self, entry: Entry, executor: BoxedExecutionEngine, on_full: impl FnOnce() -> State) -> Option<State> {
        if self.alive.is_none() {
            return None;
        }

        // Retry stashed pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return Some(on_full());
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return Some(State::Done);
                }
            }
        }

        // Drain spread items before polling the task
        if !self.spread_items.is_empty() {
            let item = self.spread_items.remove(0);
            match self.channel.push(item) {
                Ok(()) => return Some(State::Pending(None)),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return Some(on_full());
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return Some(State::Done);
                }
            }
        }

        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status()
        })) {
            Ok(inner) => inner,
            Err(panic_error) => {
                if let Some(panic_handler) = &self.panic_handler {
                    (panic_handler)(panic_error);
                }

                // A panicked task produces nothing further, so its result
                // channel must be closed exactly as on the `Done` path above.
                //
                // Leaving it open strands every collector: `sync_collect_one`
                // (and so `block_on_future`, and so `#[valtron_test]`) waits on
                // a stream that will never yield and never end. A failing
                // assertion inside a task then wedges the whole test in a futex
                // instead of reporting a failure — the panic is swallowed here,
                // and the harness never even prints `test result:`.
                self.channel.close();
                self.alive.take();
                return Some(State::Panicked);
            }
        };

        if task_response.is_none() {
            self.channel.close();
            self.alive.take();
            return Some(State::Done);
        }

        let inner = task_response.unwrap();
        Some(match inner {
            TaskStatus::Spawn(mut action) => match action.apply(Some(entry), executor) {
                Ok(info) => State::SpawnFinished(info),
                Err(err) => {
                    tracing::error!("Failed to to spawn action: {:?}", err);
                    State::SpawnFailed(entry)
                }
            },
            TaskStatus::Delayed(inner) => {
                match self.channel.push(TaskStatus::Delayed(inner)) {
                    Ok(()) => State::Pending(Some(inner)),
                    Err(PushError::Full(_)) => {
                        self.pending_msg = Some(TaskStatus::Delayed(inner));
                        on_full()
                    }
                    Err(PushError::Closed(_)) => {
                        self.channel.close();
                        self.alive.take();
                        State::Done
                    }
                }
            }
            TaskStatus::Init => match self.channel.push(TaskStatus::Init) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(TaskStatus::Init);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Pending(inner) => match self.channel.push(TaskStatus::Pending(inner)) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Ready(inner) => match self.channel.push(TaskStatus::Ready(inner)) {
                Ok(()) => State::ReadyValue(entry),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Ignore => State::Pending(None),
            TaskStatus::Wait => match self.channel.push(TaskStatus::Wait) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(TaskStatus::Wait);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Spread(items) => {
                self.spread_items = items
                    .into_iter()
                    .map(|item| match item {
                        TaskSpread::Ready(d) => TaskStatus::Ready(d),
                        TaskSpread::Pending(p) => TaskStatus::Pending(p),
                    })
                    .collect();
                if !self.spread_items.is_empty() {
                    let item = self.spread_items.remove(0);
                    match self.channel.push(item) {
                        Ok(()) => State::Pending(None),
                        Err(PushError::Full(msg)) => {
                            self.pending_msg = Some(msg);
                            on_full()
                        }
                        Err(PushError::Closed(_)) => {
                            self.channel.close();
                            self.alive.take();
                            State::Done
                        }
                    }
                } else {
                    State::Pending(None)
                }
            }
            TaskStatus::Depends(signal) => State::Depends(signal),
        })
    }
}

#[cfg(feature = "multi")]
impl<Task, Done, Pending, Action> ExecutionIterator
    for ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
    Done: Send + 'static,
    Pending: Send + 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let queue = self.channel.queue_arc();
        let on_full = move || {
            State::Depends(std::sync::Arc::new(QueueVacancyReadiness::new(queue)))
        };
        self.drive(entry, executor, on_full)
    }
}

#[cfg(not(feature = "multi"))]
impl<Task, Done, Pending, Action> ExecutionIterator
    for ConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
    Done: 'static,
    Pending: 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let queue = self.channel.queue_arc();
        let on_full = move || {
            State::Depends(std::sync::Arc::new(QueueVacancyReadiness::new(queue)))
        };
        self.drive(entry, executor, on_full)
    }
}

/// [`ReadyConsumingIter`] provides an implementer of `ExecutionIterator` which is focused on
/// consuming the produced [`TaskStatus::Ready`] output values from the execution of actual tasks.
///
/// This means unlike the [`ConsumingIter`] you will only ever get the values from a [`TaskStatus::Ready`]
/// and not the plethoral of all the variants of a `TaskStatus` through a wrapped
/// [`ConcurrentQueue`] via the [`crate::synca::mpp::RecvIterator`].
///
/// This also means these types must be send-safe.
pub struct ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<TaskStatus<Done, Pending, Action>>,
    /// Spread items being iterated (for Spread)
    spread_items: Vec<TaskStatus<Done, Pending, Action>>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Action, Task, Done, Pending> ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(
        iter: Task,
        chan: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    ) -> Self {
        Self {
            alive: Some(()),
            channel: chan,
            panic_handler: None,
            task: Mutex::new(iter),
            pending_msg: None,
            spread_items: Vec::new(),
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

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: Send + 'static, Pending: Send + 'static>
    Into<BoxedSendExecutionIterator> for ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedExecutionIterator>
    for ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[cfg(not(feature = "multi"))]
#[allow(clippy::from_over_into)]
impl<Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Action, Task, Done, Pending> ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    fn drive(&mut self, entry: Entry, executor: BoxedExecutionEngine, on_full: impl FnOnce() -> State) -> Option<State> {
        self.alive?;

        // Retry stashed pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return Some(on_full());
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return Some(State::Done);
                }
            }
        }

        // Drain spread items before polling the task
        if !self.spread_items.is_empty() {
            let item = self.spread_items.remove(0);
            match self.channel.push(item) {
                Ok(()) => return Some(State::Pending(None)),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return Some(on_full());
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return Some(State::Done);
                }
            }
        }

        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status()
        })) {
            Ok(inner) => inner,
            Err(panic_error) => {
                if let Some(panic_handler) = &self.panic_handler {
                    (panic_handler)(panic_error);
                }

                // A panicked task produces nothing further, so its result
                // channel must be closed exactly as on the `Done` path above.
                //
                // Leaving it open strands every collector: `sync_collect_one`
                // (and so `block_on_future`, and so `#[valtron_test]`) waits on
                // a stream that will never yield and never end. A failing
                // assertion inside a task then wedges the whole test in a futex
                // instead of reporting a failure — the panic is swallowed here,
                // and the harness never even prints `test result:`.
                self.channel.close();
                self.alive.take();
                return Some(State::Panicked);
            }
        };

        if task_response.is_none() {
            self.channel.close();
            self.alive.take();
            return Some(State::Done);
        }

        let inner = task_response.unwrap();
        Some(match inner {
            TaskStatus::Spawn(mut action) => match action.apply(Some(entry), executor) {
                Ok(info) => State::SpawnFinished(info),
                Err(err) => {
                    tracing::error!("Failed to to spawn action: {:?}", err);
                    State::SpawnFailed(entry)
                }
            },
            TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
            TaskStatus::Init | TaskStatus::Pending(_) => State::Pending(None),
            TaskStatus::Ready(inner) => match self.channel.push(TaskStatus::Ready(inner)) {
                Ok(()) => State::ReadyValue(entry),
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Ignore => State::Pending(None),
            TaskStatus::Wait => match self.channel.push(TaskStatus::Wait) {
                Ok(()) => State::ReadyValue(entry),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(TaskStatus::Wait);
                    on_full()
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Spread(items) => {
                self.spread_items = items
                    .into_iter()
                    .filter_map(|item| match item {
                        TaskSpread::Ready(d) => Some(TaskStatus::Ready(d)),
                        TaskSpread::Pending(_) => None,
                    })
                    .collect();
                if !self.spread_items.is_empty() {
                    let item = self.spread_items.remove(0);
                    match self.channel.push(item) {
                        Ok(()) => State::ReadyValue(entry),
                        Err(PushError::Full(msg)) => {
                            self.pending_msg = Some(msg);
                            on_full()
                        }
                        Err(PushError::Closed(_)) => {
                            self.channel.close();
                            self.alive.take();
                            State::Done
                        }
                    }
                } else {
                    State::Pending(None)
                }
            }
            TaskStatus::Depends(signal) => State::Depends(signal),
        })
    }
}

#[cfg(feature = "multi")]
impl<Task, Done, Pending, Action> ExecutionIterator
    for ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
    Done: Send + 'static,
    Pending: Send + 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let queue = self.channel.queue_arc();
        let on_full = move || {
            State::Depends(std::sync::Arc::new(QueueVacancyReadiness::new(queue)))
        };
        self.drive(entry, executor, on_full)
    }
}

#[cfg(not(feature = "multi"))]
impl<Task, Done, Pending, Action> ExecutionIterator
    for ReadyConsumingIter<Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
    Done: 'static,
    Pending: 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        let queue = self.channel.queue_arc();
        let on_full = move || {
            State::Depends(std::sync::Arc::new(QueueVacancyReadiness::new(queue)))
        };
        self.drive(entry, executor, on_full)
    }
}
