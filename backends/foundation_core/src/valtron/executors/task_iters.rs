#![allow(clippy::type_complexity)]
#![allow(clippy::items_after_test_module)]

use std::{any::Any, marker::PhantomData};

use crate::compati::Mutex;

use crate::synca::Entry;
use crate::valtron::executors::local::NotifyQueue;
use crate::valtron::iterators::Stream;
use concurrent_queue::PushError;

use crate::valtron::{
    task::TaskStatus, BoxedExecutionEngine, BoxedPanicHandler, ExecutionAction, TaskIterator,
};
use crate::valtron::{
    BoxedExecutionIterator, BoxedSendExecutionIterator, DrivenStreamIterator, ExecutionIterator,
    GenericResult, State, StreamIterator, TaskStatusMapper,
};

/// [`StreamConsumingIter`] provides an implementer of `ExecutionIterator` which is focused
/// consuming the produced [`Stream`] output values from the execution of actual tasks
/// except for the [`TaskStatus::Spawn`] variant.
///
/// This means you will only ever get the values from a [`Stream::Next`], [`Stream::Init`]
/// [`Stream::Delayed`] and [`Stream::Pending`] variants through
/// a wrapped [`ConcurrentQueue`] via the [`crate::synca::mpp::RecvIterator`].
///
/// This also means these types must be send-safe.
pub struct StreamConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    local_mappers: Vec<Mapper>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<Stream<Done, Pending>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<Stream<Done, Pending>>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Mapper, Action, Task, Done, Pending> StreamConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(
        iter: Task,
        mappers: Vec<Mapper>,
        chan: std::sync::Arc<NotifyQueue<Stream<Done, Pending>>>,
    ) -> Self {
        Self {
            channel: chan,
            alive: Some(()),
            panic_handler: None,
            local_mappers: mappers,
            task: Mutex::new(iter),
            pending_msg: None,
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
impl<Mapper, Action, Task, Done: Send + 'static, Pending: Send + 'static>
    Into<BoxedSendExecutionIterator> for StreamConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::from_over_into)]
impl<Mapper, Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for StreamConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Mapper, Task, Done, Pending, Action> ExecutionIterator
    for StreamConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        if self.alive.is_none() {
            tracing::debug!("Returning none going forward for consumer: {entry:?}");

            return None;
        }

        // First, try to send any pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    tracing::debug!("Channel still full, re-queueing pending message");
                    self.pending_msg = Some(msg);
                    return Some(State::Pending(None));
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
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
                return Some(State::Panicked);
            }
        };

        if task_response.is_none() {
            // close the queue
            self.channel.close();

            // set alive signal to empty.
            self.alive.take();

            // send State::Done
            return Some(State::Done);
        }

        let inner = task_response.unwrap();
        let mut previous_response = Some(inner);
        for mapper in &mut self.local_mappers {
            previous_response = mapper.map(previous_response);
        }

        if previous_response.is_none() {
            // close the queue
            self.channel.close();

            // set alive signal to empty.
            self.alive.take();

            // send State::Done
            return Some(State::Done);
        }

        Some(match previous_response.unwrap() {
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
                    tracing::debug!("Channel full, storing Stream::Ignore for retry");
                    self.pending_msg = Some(Stream::Ignore);
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Delayed(inner) => match self.channel.push(Stream::Delayed(inner)) {
                Ok(()) => State::Pending(Some(inner)),
                Err(PushError::Full(_)) => {
                    tracing::debug!("Channel full, storing Stream::Delayed for retry");
                    self.pending_msg = Some(Stream::Delayed(inner));
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Init => match self.channel.push(Stream::Init) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    tracing::debug!("Channel full, storing Stream::Init for retry");
                    self.pending_msg = Some(Stream::Init);
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Pending(inner) => match self.channel.push(Stream::Pending(inner)) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(msg)) => {
                    tracing::debug!("Channel full, storing Stream::Pending for retry");
                    self.pending_msg = Some(msg);
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Ready(inner) => match self.channel.push(Stream::Next(inner)) {
                Ok(()) => State::ReadyValue(entry),
                Err(PushError::Full(msg)) => {
                    tracing::debug!("Channel full, storing Stream::Next for retry");
                    self.pending_msg = Some(msg);
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
            TaskStatus::Wait => match self.channel.push(Stream::Wait) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(Stream::Wait);
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
        })
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
pub struct ConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    local_mappers: Vec<Mapper>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<TaskStatus<Done, Pending, Action>>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Mapper, Action, Task, Done, Pending> ConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(
        iter: Task,
        mappers: Vec<Mapper>,
        chan: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    ) -> Self {
        Self {
            channel: chan,
            alive: Some(()),
            panic_handler: None,
            local_mappers: mappers,
            task: Mutex::new(iter),
            pending_msg: None,
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
impl<Mapper, Action, Task, Done: Send + 'static, Pending: Send + 'static>
    Into<BoxedSendExecutionIterator> for ConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::from_over_into)]
impl<Mapper, Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for ConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Mapper, Task, Done, Pending, Action> ExecutionIterator
    for ConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        tracing::debug!("ConsumingIter::next() called for entry: {:?}", entry);
        tracing::debug!("Are ConsumingIter alive?: {:?} -> {:?}", &self.alive, entry);

        if self.alive.is_none() {
            tracing::debug!("Returning none going forward for consumer: {entry:?}");

            return None;
        }

        // First, try to send any pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    tracing::debug!("Channel still full, re-queueing pending message");
                    self.pending_msg = Some(msg);
                    return Some(State::Pending(None));
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    return Some(State::Done);
                }
            }
        }

        tracing::debug!("Get next value from consuming iter: {entry:?}");
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

        tracing::debug!(
            "Response for next value: {entry:?} with value?: {}",
            task_response.is_some()
        );

        if task_response.is_none() {
            // close the queue
            self.channel.close();

            // set alive signal to empty.
            self.alive.take();

            // send State::Done
            return Some(State::Done);
        }

        let inner = task_response.unwrap();
        let mut previous_response = Some(inner);
        for mapper in &mut self.local_mappers {
            previous_response = mapper.map(previous_response);
        }

        if previous_response.is_none() {
            // close the queue
            self.channel.close();

            // set alive signal to empty.
            self.alive.take();

            // send State::Done
            return Some(State::Done);
        }

        Some(match previous_response.unwrap() {
            TaskStatus::Spawn(mut action) => {
                tracing::debug!("Received spawned action: {entry:?}");

                match action.apply(Some(entry), executor) {
                    Ok(info) => State::SpawnFinished(info),
                    Err(err) => {
                        tracing::error!("Failed to to spawn action: {:?}", err);
                        State::SpawnFailed(entry)
                    }
                }
            }
            TaskStatus::Delayed(inner) => {
                tracing::debug!("Got  delayed: {entry:?}");
                match self.channel.push(TaskStatus::Delayed(inner)) {
                    Ok(()) => State::Pending(Some(inner)),
                    Err(PushError::Full(_)) => {
                        tracing::debug!("Channel full, storing TaskStatus::Delayed for retry");
                        self.pending_msg = Some(TaskStatus::Delayed(inner));
                        State::Pending(None)
                    }
                    Err(PushError::Closed(_)) => {
                        tracing::error!("Channel closed, terminating task");
                        self.channel.close();
                        self.alive.take();
                        State::Done
                    }
                }
            }
            TaskStatus::Init => {
                tracing::debug!("Got init: {entry:?}");
                match self.channel.push(TaskStatus::Init) {
                    Ok(()) => {
                        tracing::debug!("Written TaskStatus::Init into receiving channel");
                        State::Pending(None)
                    }
                    Err(PushError::Full(_)) => {
                        tracing::debug!("Channel full, storing TaskStatus::Init for retry");
                        self.pending_msg = Some(TaskStatus::Init);
                        State::Pending(None)
                    }
                    Err(PushError::Closed(_)) => {
                        tracing::error!("Channel closed, terminating task");
                        self.channel.close();
                        self.alive.take();
                        State::Done
                    }
                }
            }
            TaskStatus::Pending(inner) => {
                tracing::debug!("Got pending value");
                match self.channel.push(TaskStatus::Pending(inner)) {
                    Ok(()) => State::Pending(None),
                    Err(PushError::Full(msg)) => {
                        tracing::debug!("Channel full, storing TaskStatus::Pending for retry");
                        self.pending_msg = Some(msg);
                        State::Pending(None)
                    }
                    Err(PushError::Closed(_)) => {
                        tracing::error!("Channel closed, terminating task");
                        self.channel.close();
                        self.alive.take();
                        State::Done
                    }
                }
            }
            TaskStatus::Ready(inner) => {
                tracing::debug!("Got ready value");
                match self.channel.push(TaskStatus::Ready(inner)) {
                    Ok(()) => {
                        tracing::debug!("Written TaskStatus::Ready into receiving channel");
                        State::ReadyValue(entry)
                    }
                    Err(PushError::Full(msg)) => {
                        tracing::debug!("Channel full, storing TaskStatus::Ready for retry");
                        self.pending_msg = Some(msg);
                        State::Pending(None)
                    }
                    Err(PushError::Closed(_)) => {
                        tracing::error!("Channel closed, terminating task");
                        self.channel.close();
                        self.alive.take();
                        State::Done
                    }
                }
            }
            TaskStatus::Ignore => State::Pending(None),
            TaskStatus::Wait => match self.channel.push(TaskStatus::Wait) {
                Ok(()) => State::Pending(None),
                Err(PushError::Full(_)) => {
                    self.pending_msg = Some(TaskStatus::Wait);
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
        })
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
pub struct ReadyConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Action: ExecutionAction,
    Task: TaskIterator,
{
    task: Mutex<Task>,
    alive: Option<()>,
    mappers: Vec<Mapper>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    /// Pending message when channel is full (backpressure)
    pending_msg: Option<TaskStatus<Done, Pending, Action>>,
    _marker: PhantomData<(Action, Done, Pending)>,
}

impl<Mapper, Action, Task, Done, Pending> ReadyConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub fn new(
        iter: Task,
        mappers: Vec<Mapper>,
        chan: std::sync::Arc<NotifyQueue<TaskStatus<Done, Pending, Action>>>,
    ) -> Self {
        Self {
            mappers,
            alive: Some(()),
            channel: chan,
            panic_handler: None,
            task: Mutex::new(iter),
            pending_msg: None,
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
impl<Mapper, Action, Task, Done: Send + 'static, Pending: Send + 'static>
    Into<BoxedSendExecutionIterator> for ReadyConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction + Send + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
{
    fn into(self) -> BoxedSendExecutionIterator {
        Box::new(self)
    }
}

#[allow(clippy::from_over_into)]
impl<Mapper, Action, Task, Done: 'static, Pending: 'static> Into<BoxedExecutionIterator>
    for ReadyConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction + 'static,
    Mapper: TaskStatusMapper<Done, Pending, Action> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    fn into(self) -> BoxedExecutionIterator {
        Box::new(self)
    }
}

impl<Mapper, Task, Done, Pending, Action> ExecutionIterator
    for ReadyConsumingIter<Mapper, Action, Task, Done, Pending>
where
    Action: ExecutionAction,
    Mapper: TaskStatusMapper<Done, Pending, Action>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    fn next(&mut self, entry: Entry, executor: BoxedExecutionEngine) -> Option<State> {
        self.alive?;

        // First, try to send any pending message from previous backpressure
        if let Some(msg) = self.pending_msg.take() {
            match self.channel.push(msg) {
                Ok(()) => {}
                Err(PushError::Full(msg)) => {
                    tracing::debug!("Channel still full, re-queueing pending message");
                    self.pending_msg = Some(msg);
                    return Some(State::Pending(None));
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
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
                return Some(State::Panicked);
            }
        };

        if task_response.is_none() {
            // close the queue
            self.channel.close();

            // set alive signal to empty.
            self.alive.take();

            // send State::Done
            return Some(State::Done);
        }

        let inner = task_response.unwrap();
        let mut previous_response = Some(inner);
        for mapper in &mut self.mappers {
            previous_response = mapper.map(previous_response);
        }

        if previous_response.is_none() {
            // close the queue
            self.channel.close();

            // set alive signal to empty.
            self.alive.take();

            // send State::Done
            return Some(State::Done);
        }

        Some(match previous_response.unwrap() {
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
                    tracing::debug!("Channel full, storing TaskStatus::Ready for retry");
                    self.pending_msg = Some(msg);
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
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
                    State::Pending(None)
                }
                Err(PushError::Closed(_)) => {
                    tracing::error!("Channel closed, terminating task");
                    self.channel.close();
                    self.alive.take();
                    State::Done
                }
            },
        })
    }
}
