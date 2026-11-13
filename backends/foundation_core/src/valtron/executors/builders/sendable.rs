//! `Send` (multi) delivery terminals for the verb builders.
//!
//! WHY: `recv`/`stream` build a `*ConsumingIter` that parks on
//! `State::Depends(QueueVacancyReadiness)`; under `multi` that readiness must be
//! `Send + Sync`, so the produced values must be `Send`. `broadcast` likewise
//! hands a `Send` `GlobalTask` to the global queue. These are the only terminals
//! that need `Send`, so they live here under a single module-scope `#![cfg]`.
#![cfg(feature = "multi")]

use std::{sync::Arc, time};

use crate::synca::Entry;
use crate::valtron::executors::DEFAULT_MAX_TURNS;
use crate::valtron::AnyResult;
use crate::valtron::{
    BoxedExecutionEngine, ConsumingIter, DoNext, ExecutorError, NotifyQueue,
    NotifyQueueStreamIterator, NotifyRecvIterator, OnNext, SpawnInfo, Stream, StreamConsumingIter,
    TaskIterator, TaskReadyResolver, TaskStatus,
};

use super::{BroadcastBuilder, LiftBuilder, ScheduleBuilder, SequenceBuilder, TaskSpawnConfig};

/// Build a `ConsumingIter` delivery and dispatch it, returning a `recv` iterator.
fn recv_via<Done, Pending, Resolver, Task>(
    config: TaskSpawnConfig<Done, Pending, Resolver, Task>,
    wait: time::Duration,
    capacity: Option<usize>,
    dispatch: impl FnOnce(
        &BoxedExecutionEngine,
        ConsumingIter<Task, Done, Pending>,
        Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError>,
) -> AnyResult<NotifyRecvIterator<TaskStatus<Done, Pending>>, ExecutorError>
where
    Done: Send + 'static,
    Pending: Send + 'static,
    Resolver: TaskReadyResolver<Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    if config.resolver.is_some() {
        return Err(ExecutorError::NotSupported);
    }
    let parent = config.parent;
    let Some(task) = config.task else {
        return Err(ExecutorError::TaskRequired);
    };
    let iter_chan: Arc<NotifyQueue<TaskStatus<Done, Pending>>> = Arc::new(match capacity {
        Some(c) => NotifyQueue::bounded(c),
        None => NotifyQueue::unbounded(),
    });
    let iter = ConsumingIter::new(task, iter_chan.clone());
    let iter = match config.panic_handler {
        Some(handler) => iter.with_panic_handler(handler),
        None => iter,
    };
    let ctx = config.engine.fork_ctx();
    dispatch(&config.engine, iter, parent)
        .map(|_| NotifyRecvIterator::from_notify_queue(iter_chan, wait, ctx))
}

/// Build a `StreamConsumingIter` delivery and dispatch it, returning a `stream`.
fn stream_via<Done, Pending, Resolver, Task>(
    config: TaskSpawnConfig<Done, Pending, Resolver, Task>,
    wait: time::Duration,
    max_turns: usize,
    capacity: Option<usize>,
    dispatch: impl FnOnce(
        &BoxedExecutionEngine,
        StreamConsumingIter<Task, Done, Pending>,
        Option<Entry>,
    ) -> AnyResult<SpawnInfo, ExecutorError>,
) -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError>
where
    Done: Send + 'static,
    Pending: Send + 'static,
    Resolver: TaskReadyResolver<Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
{
    if config.resolver.is_some() {
        return Err(ExecutorError::NotSupported);
    }
    let parent = config.parent;
    let Some(task) = config.task else {
        return Err(ExecutorError::TaskRequired);
    };
    let iter_chan: Arc<NotifyQueue<Stream<Done, Pending>>> = Arc::new(match capacity {
        Some(c) => NotifyQueue::bounded(c),
        None => NotifyQueue::unbounded(),
    });
    let iter = StreamConsumingIter::new(task, iter_chan.clone());
    let iter = match config.panic_handler {
        Some(handler) => iter.with_panic_handler(handler),
        None => iter,
    };
    let ctx = config.engine.fork_ctx();
    dispatch(&config.engine, iter, parent)
        .map(|_| NotifyQueueStreamIterator::new(iter_chan, max_turns, wait, ctx))
}

/// Emit the uniform `recv`/`stream`/`stream_with_config` terminals for a verb
/// builder. `$capacity` bounds the delivery queue; `$recv`/`$stream` are the
/// per-verb engine dispatch closures.
macro_rules! delivery_terminals {
    ($builder:ident, $bounded:literal, $recv:expr, $stream:expr) => {
        impl<Done, Pending, Resolver, Task> $builder<Done, Pending, Resolver, Task>
        where
            Done: Send + 'static,
            Pending: Send + 'static,
            Resolver: TaskReadyResolver<Done, Pending>,
            Task: TaskIterator<Pending = Pending, Ready = Done> + 'static,
        {
            /// Receive produced `TaskStatus` values via a `NotifyRecvIterator`.
            ///
            /// # Errors
            /// Returns `ExecutorError` when the engine cannot accept the task.
            pub fn recv(
                self,
                wait: time::Duration,
            ) -> AnyResult<NotifyRecvIterator<TaskStatus<Done, Pending>>, ExecutorError> {
                let capacity = if $bounded {
                    self.config.delivery_capacity
                } else {
                    None
                };
                recv_via(self.config, wait, capacity, $recv)
            }

            /// Receive produced `Stream` values via a `NotifyQueueStreamIterator`
            /// using the default max-turns.
            ///
            /// # Errors
            /// Returns `ExecutorError` when the engine cannot accept the task.
            pub fn stream(
                self,
                wait: time::Duration,
            ) -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError> {
                self.stream_with_config(wait, DEFAULT_MAX_TURNS)
            }

            /// Receive produced `Stream` values with a configurable max-turns.
            ///
            /// # Errors
            /// Returns `ExecutorError` when the engine cannot accept the task.
            pub fn stream_with_config(
                self,
                wait: time::Duration,
                max_turns: usize,
            ) -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError> {
                let capacity = if $bounded {
                    self.config.delivery_capacity
                } else {
                    None
                };
                stream_via(self.config, wait, max_turns, capacity, $stream)
            }
        }
    };
}

delivery_terminals!(
    ScheduleBuilder,
    false,
    |e: &BoxedExecutionEngine, it, _parent| e.schedule(it.into()),
    |e: &BoxedExecutionEngine, it, _parent| e.schedule(it.into())
);
delivery_terminals!(
    LiftBuilder,
    false,
    |e: &BoxedExecutionEngine, it, parent| e.lift(it.into(), parent),
    |e: &BoxedExecutionEngine, it, parent| e.lift(it.into(), parent)
);
delivery_terminals!(
    SequenceBuilder,
    true,
    |e: &BoxedExecutionEngine, it, parent: Option<Entry>| e.sequenced(
        it.into(),
        parent.ok_or(ExecutorError::ParentMustBeSupplied)?
    ),
    |e: &BoxedExecutionEngine, it, parent: Option<Entry>| e.sequenced(
        it.into(),
        parent.ok_or(ExecutorError::ParentMustBeSupplied)?
    )
);

// -----------------------------------------------------------------------
// action() — deferred dispatch: returns an InlineAction + receiver pair
// -----------------------------------------------------------------------

macro_rules! action_terminal {
    ($builder:ident, $behaviour:expr) => {
        impl<Done, Pending, Resolver, Task> $builder<Done, Pending, Resolver, Task>
        where
            Done: Send + 'static,
            Pending: Send + 'static,
            Resolver: TaskReadyResolver<Done, Pending>,
            Task: TaskIterator<Pending = Pending, Ready = Done> + Send + 'static,
        {
            pub fn action(
                self,
                wait: time::Duration,
            ) -> (
                crate::valtron::InlineAction<Done, Pending, Task>,
                NotifyRecvIterator<TaskStatus<Done, Pending>>,
            ) {
                let task = self.config.task.expect("task required");
                let channel: Arc<NotifyQueue<TaskStatus<Done, Pending>>> =
                    Arc::new(NotifyQueue::unbounded());
                let mut action =
                    crate::valtron::InlineAction::from_parts($behaviour, task, channel.clone());
                if let Some(handler) = self.config.panic_handler {
                    action = action.with_panic_handler(handler);
                }
                let receiver = NotifyRecvIterator::from_notify_queue(
                    channel,
                    wait,
                    self.config.engine.fork_ctx(),
                );
                (action, receiver)
            }
        }
    };
}

action_terminal!(
    ScheduleBuilder,
    crate::valtron::InlineActionBehaviour::Schedule
);
action_terminal!(LiftBuilder, crate::valtron::InlineActionBehaviour::Lift);
action_terminal!(
    SequenceBuilder,
    crate::valtron::InlineActionBehaviour::Sequenced
);

impl<Done, Pending, Resolver, Task> BroadcastBuilder<Done, Pending, Resolver, Task>
where
    Done: Send + 'static,
    Pending: Send + 'static,
    Resolver: TaskReadyResolver<Done, Pending> + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done> + Send + 'static,
{
    /// Receives produced `TaskStatus` values through the global queue.
    ///
    /// # Errors
    /// Returns `ExecutorError` when the engine cannot accept the task.
    pub fn recv(
        self,
        wait: time::Duration,
    ) -> AnyResult<NotifyRecvIterator<TaskStatus<Done, Pending>>, ExecutorError> {
        recv_via(
            self.config,
            wait,
            None,
            |e: &BoxedExecutionEngine, it, _parent| e.broadcast(it.into()),
        )
    }

    /// Receives produced `Stream` values through the global queue.
    ///
    /// # Errors
    /// Returns `ExecutorError` when the engine cannot accept the task.
    pub fn stream(
        self,
        wait: time::Duration,
    ) -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError> {
        self.stream_with_config(wait, DEFAULT_MAX_TURNS)
    }

    /// Receives produced `Stream` values with a configurable max-turns.
    ///
    /// # Errors
    /// Returns `ExecutorError` when the engine cannot accept the task.
    pub fn stream_with_config(
        self,
        wait: time::Duration,
        max_turns: usize,
    ) -> AnyResult<NotifyQueueStreamIterator<Done, Pending>, ExecutorError> {
        stream_via(
            self.config,
            wait,
            max_turns,
            None,
            |e: &BoxedExecutionEngine, it, _parent| e.broadcast(it.into()),
        )
    }

    /// Broadcasts the task to the global queue.
    ///
    /// # Errors
    /// Returns `ExecutorError::TaskRequired` when the builder holds no task,
    /// and the global-queue error when the broadcast cannot accept it.
    pub fn spawn(self) -> AnyResult<SpawnInfo, ExecutorError> {
        let _ = self.fallback;
        let c = self.config;
        let Some(task) = c.task else {
            return Err(ExecutorError::TaskRequired);
        };
        let boxed: crate::valtron::GlobalTask = if let Some(resolver) = c.resolver {
            let iter = OnNext::new(task, resolver);
            match c.panic_handler {
                Some(handler) => iter.with_panic_handler(handler).into(),
                None => iter.into(),
            }
        } else {
            let iter = DoNext::new(task);
            match c.panic_handler {
                Some(handler) => iter.with_panic_handler(handler).into(),
                None => iter.into(),
            }
        };
        c.engine.broadcast(boxed)
    }

    /// Builds the inline action and its result iterator.
    ///
    /// # Panics
    /// Panics if the builder holds no task.
    pub fn action(
        self,
        wait: time::Duration,
    ) -> (
        crate::valtron::InlineAction<Done, Pending, Task>,
        NotifyRecvIterator<TaskStatus<Done, Pending>>,
    ) {
        let _ = self.fallback;
        let task = self.config.task.expect("task required");
        let channel: Arc<NotifyQueue<TaskStatus<Done, Pending>>> =
            Arc::new(NotifyQueue::unbounded());
        let mut action = crate::valtron::InlineAction::from_parts(
            crate::valtron::InlineActionBehaviour::Broadcast,
            task,
            channel.clone(),
        );
        if let Some(handler) = self.config.panic_handler {
            action = action.with_panic_handler(handler);
        }
        let receiver =
            NotifyRecvIterator::from_notify_queue(channel, wait, self.config.engine.fork_ctx());
        (action, receiver)
    }
}
