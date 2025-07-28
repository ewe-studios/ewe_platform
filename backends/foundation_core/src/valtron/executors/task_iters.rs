#![allow(clippy::type_complexity)]
#![allow(clippy::items_after_test_module)]

use std::{any::Any, marker::PhantomData};

use concurrent_queue::ConcurrentQueue;

use crate::compati::Mutex;

use crate::synca::Entry;

use super::{
    task::TaskStatus, BoxedExecutionEngine, BoxedPanicHandler, ExecutionAction, TaskIterator,
};
use super::{
    BoxedExecutionIterator, BoxedSendExecutionIterator, ExecutionIterator, State, TaskStatusMapper,
};

/// [`ConsumingIter`] provides an implementer of `ExecutionIterator` which is focused
/// consuming the produced [`TaskStatus`] output values from the execution of actual tasks
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
    local_mappers: Vec<Mapper>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>>,
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
        chan: std::sync::Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>>,
    ) -> Self {
        Self {
            channel: chan,
            panic_handler: None,
            local_mappers: mappers,
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
        let task_response = match std::panic::catch_unwind(|| self.task.lock().unwrap().next()) {
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

            // send State::Done
            return Some(State::Done);
        }

        Some(match previous_response.unwrap() {
            TaskStatus::Spawn(action) => match action.apply(entry, executor) {
                Ok(_) => State::Progressed,
                Err(err) => {
                    tracing::error!("Failed to to spawn action: {:?}", err);
                    State::SpawnFailed
                }
            },
            TaskStatus::Delayed(inner) => match self.channel.push(TaskStatus::Delayed(inner)) {
                Ok(_) => State::Pending(Some(inner)),
                Err(_) => {
                    tracing::error!("Failed to deliver status to channel, closing task",);

                    // close the queue
                    self.channel.close();

                    State::Done
                }
            },
            TaskStatus::Init => match self.channel.push(TaskStatus::Init) {
                Ok(_) => State::Pending(None),
                Err(_) => {
                    tracing::error!("Failed to deliver status to channel, closing task",);

                    // close the queue
                    self.channel.close();

                    State::Done
                }
            },
            TaskStatus::Pending(inner) => match self.channel.push(TaskStatus::Pending(inner)) {
                Ok(_) => State::Pending(None),
                Err(_) => {
                    tracing::error!("Failed to deliver status to channel, closing task",);

                    // close the queue
                    self.channel.close();

                    State::Done
                }
            },
            TaskStatus::Ready(inner) => match self.channel.push(TaskStatus::Ready(inner)) {
                Ok(_) => State::Progressed,
                Err(_) => {
                    tracing::error!("Failed to deliver status to channel, closing task");

                    // close the queue
                    self.channel.close();

                    State::Done
                }
            },
        })
    }
}

/// [`ReadyConsumingIter`] provides an implementer of `ExecutionIterator` which is focused
/// consuming the produced [`TaskStatus::Ready`] output values from the execution of actual tasks.
///
/// This means unlike the [`ConsumingIter`] you will only ever get the values from a [`TaskStatus::Ready`]
/// and not the plethoral of all the variants of a [`TaskStatus`] through a wrapped
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
    mappers: Vec<Mapper>,
    panic_handler: Option<BoxedPanicHandler>,
    channel: std::sync::Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>>,
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
        chan: std::sync::Arc<ConcurrentQueue<TaskStatus<Done, Pending, Action>>>,
    ) -> Self {
        Self {
            mappers,
            channel: chan,
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
        let task_response = match std::panic::catch_unwind(|| self.task.lock().unwrap().next()) {
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

            // send State::Done
            return Some(State::Done);
        }

        Some(match previous_response.unwrap() {
            TaskStatus::Spawn(action) => match action.apply(entry, executor) {
                Ok(_) => State::Progressed,
                Err(err) => {
                    tracing::error!("Failed to to spawn action: {:?}", err);
                    State::SpawnFailed
                }
            },
            TaskStatus::Delayed(dur) => State::Pending(Some(dur)),
            TaskStatus::Pending(_) => State::Pending(None),
            TaskStatus::Init => State::Pending(None),
            TaskStatus::Ready(inner) => match self.channel.push(TaskStatus::Ready(inner)) {
                Ok(_) => State::Progressed,
                Err(_) => {
                    tracing::error!("Failed to deliver status to channel, closing task");

                    // close the queue
                    self.channel.close();

                    State::Done
                }
            },
        })
    }
}
