use std::{any::Any, marker::PhantomData};

/// Outcome of the pre-poll retry and spread drain.
enum PollGate {
    /// Poll the task now.
    Proceed,
    /// One spread item was delivered; yield a `Pending` state.
    Drained,
    /// The queue is full; park on the vacancy signal.
    Full,
    /// The queue is closed; the task is done.
    Closed,
}

use crate::compati::Mutex;

use crate::synca::Entry;
use crate::valtron::executors::local::NotifyQueue;
use crate::valtron::iterators::Stream;
use concurrent_queue::PushError;

use crate::valtron::{
    task::{TaskSpread, TaskStatus},
    BoxedExecutionEngine, BoxedPanicHandler, ExecutionAction, TaskIterator,
};
use crate::valtron::{BoxedExecutionIterator, Ctx, ExecutionIterator, State};
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
pub struct StreamConsumingIter<Task, Done, Pending>
where
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
    _marker: PhantomData<(Done, Pending)>,
}

impl<Task, Done, Pending> StreamConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
{
    pub fn new(iter: Task, chan: std::sync::Arc<NotifyQueue<Stream<Done, Pending>>>) -> Self {
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
impl<Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedSendExecutionIterator>
    for StreamConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedExecutionIterator>
    for StreamConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[cfg(not(feature = "multi"))]
#[allow(clippy::from_over_into)]
impl<Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for StreamConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Task, Done, Pending> StreamConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
{
    /// Shared `next()` body.  `on_full` builds the `State::Depends(...)` for a
    /// full delivery queue — the closure is constructed in the cfg-gated
    /// `ExecutionIterator::next` wrappers so that the `Send` bounds needed under
    /// `multi` (for `QueueVacancyReadiness` coercion into `EventReadinessPtr`)
    /// are supplied by the wrapper's own bounds rather than leaking into this
    /// inherent method.
    /// Retries a stashed message and drains one spread item before polling.
    ///
    /// Returns `Some(state)` when the caller must return that state now.
    fn retry_before_poll(&mut self) -> PollGate {
        // Retry stashed pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return PollGate::Full;
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return PollGate::Closed;
                }
            }
        }

        // Drain spread items before polling the task
        if !self.spread_items.is_empty() {
            let item = self.spread_items.remove(0);
            match self.channel.push(item) {
                Ok(()) => return PollGate::Drained,
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return PollGate::Full;
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return PollGate::Closed;
                }
            }
        }

        PollGate::Proceed
    }

    /// Pushes one stream state and maps the result to an executor state.
    fn deliver(
        &mut self,
        status: Stream<Done, Pending>,
        on_full: impl FnOnce() -> State,
        ok: impl FnOnce() -> State,
    ) -> State {
        match self.channel.push(status) {
            Ok(()) => ok(),
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
    }

    fn drive(
        &mut self,
        entry: Entry,
        executor: BoxedExecutionEngine,
        ctx: Ctx,
        on_full: impl FnOnce() -> State,
    ) -> Option<State> {
        self.alive?;

        match self.retry_before_poll() {
            PollGate::Proceed => {}
            PollGate::Drained => return Some(State::Pending(None)),
            PollGate::Full => return Some(on_full()),
            PollGate::Closed => return Some(State::Done),
        }

        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status(ctx)
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
                // instead of reporting a failure.
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
            TaskStatus::Ignore => self.deliver(Stream::Ignore, on_full, || State::Pending(None)),
            TaskStatus::Delayed(inner) => self.deliver(Stream::Delayed(inner), on_full, || {
                State::Pending(Some(inner))
            }),
            TaskStatus::Init => self.deliver(Stream::Init, on_full, || State::Pending(None)),
            TaskStatus::Pending(inner) => {
                self.deliver(Stream::Pending(inner), on_full, || State::Pending(None))
            }
            TaskStatus::Ready(inner) => {
                self.deliver(Stream::Next(inner), on_full, || State::ReadyValue(entry))
            }
            // `Wait` is the task's "no data yet, queue still open" signal: a
            // yield, not a park. Under `multi` re-poll at once instead of
            // parking a worker with no sleeper to wake it.
            TaskStatus::Wait => {
                #[cfg(not(feature = "multi"))]
                let ok = || State::Wait;
                #[cfg(feature = "multi")]
                let ok = || State::Pending(None);
                self.deliver(Stream::Wait, on_full, ok)
            }
            TaskStatus::Spread(items) => {
                self.spread_items = items
                    .into_iter()
                    .map(|item| match item {
                        TaskSpread::Ready(d) => Stream::Next(d),
                        TaskSpread::Pending(p) => Stream::Pending(p),
                    })
                    .collect();
                if self.spread_items.is_empty() {
                    State::Pending(None)
                } else {
                    let item = self.spread_items.remove(0);
                    self.deliver(item, on_full, || State::Pending(None))
                }
            }
            TaskStatus::Depends(signal) => State::Depends(signal),
        })
    }
}

#[cfg(feature = "multi")]
impl<Task, Done, Pending> ExecutionIterator for StreamConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
    Done: Send + 'static,
    Pending: Send + 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        let vacancy = self.channel.vacancy_readiness();
        let on_full = move || State::Depends(std::sync::Arc::new(vacancy));
        self.drive(entry, executor, ctx, on_full)
    }
}

#[cfg(not(feature = "multi"))]
impl<Task, Done, Pending> ExecutionIterator for StreamConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
    Done: 'static,
    Pending: 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        let vacancy = self.channel.vacancy_readiness();
        let on_full = move || State::Depends(std::sync::Arc::new(vacancy));
        self.drive(entry, executor, ctx, on_full)
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
pub struct ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<TaskStatus<Done, Pending>>,
    /// Spread items being iterated (for Spread)
    spread_items: Vec<TaskStatus<Done, Pending>>,
    _marker: PhantomData<(Done, Pending)>,
}

impl<Task, Done, Pending> ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
{
    pub fn new(iter: Task, chan: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending>>>) -> Self {
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
impl<Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedSendExecutionIterator>
    for ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedExecutionIterator>
    for ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[cfg(not(feature = "multi"))]
#[allow(clippy::from_over_into)]
impl<Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Task, Done, Pending> ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
{
    /// Retries a stashed message and drains one spread item before polling.
    ///
    /// Returns `Some(state)` when the caller must return that state now.
    fn retry_before_poll(&mut self) -> PollGate {
        // Retry stashed pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return PollGate::Full;
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return PollGate::Closed;
                }
            }
        }

        // Drain spread items before polling the task
        if !self.spread_items.is_empty() {
            let item = self.spread_items.remove(0);
            match self.channel.push(item) {
                Ok(()) => return PollGate::Drained,
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return PollGate::Full;
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return PollGate::Closed;
                }
            }
        }

        PollGate::Proceed
    }

    /// Pushes one task status and maps the result to an executor state.
    fn deliver(
        &mut self,
        status: TaskStatus<Done, Pending>,
        on_full: impl FnOnce() -> State,
        ok: impl FnOnce() -> State,
    ) -> State {
        match self.channel.push(status) {
            Ok(()) => ok(),
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
    }

    fn drive(
        &mut self,
        entry: Entry,
        executor: BoxedExecutionEngine,
        ctx: Ctx,
        on_full: impl FnOnce() -> State,
    ) -> Option<State> {
        self.alive?;

        match self.retry_before_poll() {
            PollGate::Proceed => {}
            PollGate::Drained => return Some(State::Pending(None)),
            PollGate::Full => return Some(on_full()),
            PollGate::Closed => return Some(State::Done),
        }

        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status(ctx)
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
                // instead of reporting a failure.
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
            TaskStatus::Delayed(inner) => self.deliver(TaskStatus::Delayed(inner), on_full, || {
                State::Pending(Some(inner))
            }),
            TaskStatus::Init => self.deliver(TaskStatus::Init, on_full, || State::Pending(None)),
            TaskStatus::Pending(inner) => {
                self.deliver(TaskStatus::Pending(inner), on_full, || State::Pending(None))
            }
            TaskStatus::Ready(inner) => self.deliver(TaskStatus::Ready(inner), on_full, || {
                State::ReadyValue(entry)
            }),
            TaskStatus::Ignore => {
                self.deliver(TaskStatus::Ignore, on_full, || State::Pending(None))
            }
            TaskStatus::Wait => self.deliver(TaskStatus::Wait, on_full, || State::Pending(None)),
            TaskStatus::Spread(items) => {
                self.spread_items = items
                    .into_iter()
                    .map(|item| match item {
                        TaskSpread::Ready(d) => TaskStatus::Ready(d),
                        TaskSpread::Pending(p) => TaskStatus::Pending(p),
                    })
                    .collect();
                if self.spread_items.is_empty() {
                    State::Pending(None)
                } else {
                    let item = self.spread_items.remove(0);
                    self.deliver(item, on_full, || State::Pending(None))
                }
            }
            TaskStatus::Depends(signal) => State::Depends(signal),
        })
    }
}

#[cfg(feature = "multi")]
impl<Task, Done, Pending> ExecutionIterator for ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
    Done: Send + 'static,
    Pending: Send + 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        let vacancy = self.channel.vacancy_readiness();
        let on_full = move || State::Depends(std::sync::Arc::new(vacancy));
        self.drive(entry, executor, ctx, on_full)
    }
}

#[cfg(not(feature = "multi"))]
impl<Task, Done, Pending> ExecutionIterator for ConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
    Done: 'static,
    Pending: 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        let vacancy = self.channel.vacancy_readiness();
        let on_full = move || State::Depends(std::sync::Arc::new(vacancy));
        self.drive(entry, executor, ctx, on_full)
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
pub struct ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<TaskStatus<Done, Pending>>,
    /// Spread items being iterated (for Spread)
    spread_items: Vec<TaskStatus<Done, Pending>>,
    _marker: PhantomData<(Done, Pending)>,
}

impl<Task, Done, Pending> ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
{
    pub fn new(iter: Task, chan: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending>>>) -> Self {
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
impl<Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedSendExecutionIterator>
    for ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[cfg(feature = "multi")]
#[allow(clippy::from_over_into)]
impl<Task, Done: Send + 'static, Pending: Send + 'static> Into<BoxedExecutionIterator>
    for ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

#[cfg(not(feature = "multi"))]
#[allow(clippy::from_over_into)]
impl<Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Task, Done, Pending> ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
{
    /// Retries a stashed message and drains one spread item before polling.
    ///
    /// Returns `Some(state)` when the caller must return that state now.
    fn retry_before_poll(&mut self) -> PollGate {
        // Retry stashed pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return PollGate::Full;
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return PollGate::Closed;
                }
            }
        }

        // Drain spread items before polling the task
        if !self.spread_items.is_empty() {
            let item = self.spread_items.remove(0);
            match self.channel.push(item) {
                Ok(()) => return PollGate::Drained,
                Err(PushError::Full(msg)) => {
                    self.pending_msg = Some(msg);
                    return PollGate::Full;
                }
                Err(PushError::Closed(_)) => {
                    self.channel.close();
                    self.alive.take();
                    return PollGate::Closed;
                }
            }
        }

        PollGate::Proceed
    }

    /// Pushes one task status and maps the result to an executor state.
    fn deliver(
        &mut self,
        status: TaskStatus<Done, Pending>,
        on_full: impl FnOnce() -> State,
        ok: impl FnOnce() -> State,
    ) -> State {
        match self.channel.push(status) {
            Ok(()) => ok(),
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
    }

    fn drive(
        &mut self,
        entry: Entry,
        executor: BoxedExecutionEngine,
        ctx: Ctx,
        on_full: impl FnOnce() -> State,
    ) -> Option<State> {
        self.alive?;

        match self.retry_before_poll() {
            PollGate::Proceed => {}
            PollGate::Drained => return Some(State::Pending(None)),
            PollGate::Full => return Some(on_full()),
            PollGate::Closed => return Some(State::Done),
        }

        let task_response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.task.lock().unwrap().next_status(ctx)
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
                // instead of reporting a failure.
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
            TaskStatus::Init | TaskStatus::Pending(_) | TaskStatus::Ignore => State::Pending(None),
            TaskStatus::Ready(inner) => self.deliver(TaskStatus::Ready(inner), on_full, || {
                State::ReadyValue(entry)
            }),
            TaskStatus::Wait => {
                self.deliver(TaskStatus::Wait, on_full, || State::ReadyValue(entry))
            }
            TaskStatus::Spread(items) => {
                self.spread_items = items
                    .into_iter()
                    .filter_map(|item| match item {
                        TaskSpread::Ready(d) => Some(TaskStatus::Ready(d)),
                        TaskSpread::Pending(_) => None,
                    })
                    .collect();
                if self.spread_items.is_empty() {
                    State::Pending(None)
                } else {
                    let item = self.spread_items.remove(0);
                    self.deliver(item, on_full, || State::ReadyValue(entry))
                }
            }
            TaskStatus::Depends(signal) => State::Depends(signal),
        })
    }
}

#[cfg(feature = "multi")]
impl<Task, Done, Pending> ExecutionIterator for ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
    Done: Send + 'static,
    Pending: Send + 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        let vacancy = self.channel.vacancy_readiness();
        let on_full = move || State::Depends(std::sync::Arc::new(vacancy));
        self.drive(entry, executor, ctx, on_full)
    }
}

#[cfg(not(feature = "multi"))]
impl<Task, Done, Pending> ExecutionIterator for ReadyConsumingIter<Task, Done, Pending>
where
    Task: TaskIterator<Pending = Pending, Ready = Done>,
    Done: 'static,
    Pending: 'static,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine, ctx: Ctx) -> Option<State> {
        let vacancy = self.channel.vacancy_readiness();
        let on_full = move || State::Depends(std::sync::Arc::new(vacancy));
        self.drive(entry, executor, ctx, on_full)
    }
}
