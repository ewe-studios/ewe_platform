//! Task-spawn builders, decomposed by `ExecutionEngine` verb.
//!
//! WHY: `ExecutionEngine` exposes four dispatch verbs — `schedule`, `lift`,
//! `sequenced`, `broadcast` — and each supports three delivery forms (spawn,
//! recv-iterator, stream-iterator). Their trait-bound needs diverge: only
//! `broadcast` (and the delivery iterators, which park on
//! `State::Depends(QueueVacancyReadiness)` and therefore need `Send` under
//! `multi`) require `Send`; the plain `spawn` form of `schedule`/`lift`/
//! `sequenced` never does. A single mux-everything builder cannot express that
//! without per-method `#[cfg]`.
//!
//! WHAT: a cfg-agnostic [`TaskSpawnConfig`] collects the task/parent/resolver,
//! then transitions (`as_scheduled`/`as_lifted`/`as_sequenced`/`as_broadcast`)
//! into one of four verb builders — [`ScheduleBuilder`], [`LiftBuilder`],
//! [`SequenceBuilder`], [`BroadcastBuilder`]. Each verb builder exposes the same
//! terminal surface: `spawn()`, `recv()`, `stream()`, `stream_with_config()`.
//!
//! HOW: the `Send`-free `spawn()` of the three local verbs lives here (ungated).
//! The `Send`-diverging terminals — every `recv`/`stream` (they build a
//! `*ConsumingIter`) plus `BroadcastBuilder::spawn` — live once in
//! [`sendable`] (`#![cfg(multi)]`, `Send`) and once in [`non_sendable`]
//! (`#![cfg(not multi)]`, no `Send`), re-exported below. The `#[cfg]` is at
//! module scope only: there is no per-function gating.

use std::{any::Any, marker::PhantomData};

use crate::synca::Entry;
use crate::valtron::AnyResult;
use crate::valtron::{
    BoxedExecutionEngine, BoxedPanicHandler, DoNext, ExecutionAction, ExecutorError, FnMutReady,
    FnReady, OnNext, SpawnInfo, TaskIterator, TaskReadyResolver, TaskStatus,
};

// The twins carry only inherent-impl terminals (no exported names), so they are
// `mod`-compiled but not re-exported — the methods attach to the verb-builder
// structs declared below.
#[cfg(feature = "multi")]
mod sendable;

#[cfg(not(feature = "multi"))]
mod non_sendable;

/// Which local verb a [`BroadcastBuilder`] falls back to when `multi` is off.
///
/// WHY: under `multi = off` there is no global queue, so `broadcast` degrades to
/// a local dispatch (`broadcast_or_lift` / `broadcast_or_sequence`). The chosen
/// fallback is captured at the `as_broadcast*` transition and consumed by
/// [`non_sendable`]'s `BroadcastBuilder` terminals.
#[derive(Debug, Clone, Copy)]
pub enum LocalFallback {
    /// `broadcast_or_lift`: lift onto the top of the local queue.
    Lift,
    /// `broadcast_or_sequence`: sequence with the parent on the local queue.
    Sequence,
}

/// The cfg-agnostic configuration stage of the task-spawn builders.
///
/// WHAT: holds the engine handle, the task, an optional parent, an optional
/// ready-`Resolver`, and an optional panic handler. It carries only setters and
/// the verb transitions — never a dispatch — so it needs no `Send` and lives in
/// one ungated impl.
pub struct TaskSpawnConfig<Done, Pending, Action, Resolver, Task>
where
    Action: ExecutionAction,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub(crate) engine: BoxedExecutionEngine,
    pub(crate) task: Option<Task>,
    pub(crate) parent: Option<Entry>,
    pub(crate) resolver: Option<Resolver>,
    pub(crate) panic_handler: Option<BoxedPanicHandler>,
    pub(crate) _marker: PhantomData<(Done, Pending, Action)>,
}

impl<Done, Pending, Action, Resolver, Task> TaskSpawnConfig<Done, Pending, Action, Resolver, Task>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    /// Create an empty config bound to `engine`.
    #[must_use]
    pub fn new(engine: BoxedExecutionEngine) -> Self {
        Self {
            engine,
            task: None,
            parent: None,
            resolver: None,
            panic_handler: None,
            _marker: PhantomData,
        }
    }

    /// Set the task to spawn.
    #[must_use]
    pub fn with_task(mut self, task: Task) -> Self {
        self.task = Some(task);
        self
    }

    /// Set the parent entry (used by `lift`/`sequence`/broadcast fallback).
    #[must_use]
    pub fn with_parent(mut self, parent: Entry) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Set the parent entry from an `Option`.
    #[must_use]
    pub fn maybe_parent(mut self, parent: Option<Entry>) -> Self {
        self.parent = parent;
        self
    }

    /// Set the ready-`Resolver` (the `spawn`-form ready handler).
    #[must_use]
    pub fn with_resolver(mut self, resolver: Resolver) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// Attach a panic handler invoked if the task panics.
    #[must_use]
    pub fn with_panic_handler<T>(mut self, handler: T) -> Self
    where
        T: Fn(Box<dyn Any + Send>) + Send + Sync + 'static,
    {
        self.panic_handler = Some(Box::new(handler));
        self
    }

    /// Transition to a [`ScheduleBuilder`] (bottom of the local queue).
    #[must_use]
    pub fn as_scheduled(self) -> ScheduleBuilder<Done, Pending, Action, Resolver, Task> {
        ScheduleBuilder { config: self }
    }

    /// Transition to a [`LiftBuilder`] (top of the local queue).
    #[must_use]
    pub fn as_lifted(self) -> LiftBuilder<Done, Pending, Action, Resolver, Task> {
        LiftBuilder { config: self }
    }

    /// Transition to a [`SequenceBuilder`] linked to `parent`.
    #[must_use]
    pub fn as_sequenced(
        mut self,
        parent: Entry,
    ) -> SequenceBuilder<Done, Pending, Action, Resolver, Task> {
        self.parent = Some(parent);
        SequenceBuilder { config: self }
    }

    /// Transition to a [`BroadcastBuilder`] (global queue under `multi`, else
    /// lift fallback).
    #[must_use]
    pub fn as_broadcast(self) -> BroadcastBuilder<Done, Pending, Action, Resolver, Task> {
        BroadcastBuilder {
            config: self,
            fallback: LocalFallback::Lift,
        }
    }

    /// Transition to a [`BroadcastBuilder`] whose `multi = off` fallback is
    /// `lift` (optional parent honored).
    #[must_use]
    pub fn as_broadcast_or_lift(self) -> BroadcastBuilder<Done, Pending, Action, Resolver, Task> {
        BroadcastBuilder {
            config: self,
            fallback: LocalFallback::Lift,
        }
    }

    /// Transition to a [`BroadcastBuilder`] whose `multi = off` fallback is
    /// `sequence` with `parent`.
    #[must_use]
    pub fn as_broadcast_or_sequenced(
        mut self,
        parent: Entry,
    ) -> BroadcastBuilder<Done, Pending, Action, Resolver, Task> {
        self.parent = Some(parent);
        BroadcastBuilder {
            config: self,
            fallback: LocalFallback::Sequence,
        }
    }
}

// ---------------------------------------------------------------------------
// on_next / on_send_next resolver setters (Resolver is a concrete Fn wrapper)
// ---------------------------------------------------------------------------

impl<F, Done, Pending, Action, Task>
    TaskSpawnConfig<Done, Pending, Action, FnReady<F, Action>, Task>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
{
    /// Install a `Fn` ready-handler as the resolver.
    #[must_use]
    pub fn on_next(self, action: F) -> Self {
        self.with_resolver(FnReady::new(action))
    }
}

impl<F, Done, Pending, Action, Task>
    TaskSpawnConfig<Done, Pending, Action, FnMutReady<F, Action>, Task>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
{
    /// Install a `FnMut` ready-handler as the resolver.
    #[must_use]
    pub fn on_next_mut(self, action: F) -> Self
    where
        F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
    {
        self.with_resolver(FnMutReady::new(action))
    }
}

impl<F, Done, Pending, Action, Task>
    TaskSpawnConfig<Done, Pending, Action, FnReady<F, Action>, Task>
where
    Done: Send + 'static,
    Pending: Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    /// Install a `Send` `Fn` ready-handler as the resolver (broadcast path).
    #[must_use]
    pub fn on_send_next(self, action: F) -> Self {
        self.with_resolver(FnReady::new(action))
    }
}

impl<F, Done, Pending, Action, Task>
    TaskSpawnConfig<Done, Pending, Action, FnMutReady<F, Action>, Task>
where
    Done: Send + 'static,
    Pending: Send + 'static,
    Action: ExecutionAction + Send + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + Send + 'static,
    F: FnMut(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + Send + 'static,
{
    /// Install a `Send` `FnMut` ready-handler as the resolver (broadcast path).
    #[must_use]
    pub fn on_send_next_mut(self, action: F) -> Self
    where
        F: Fn(TaskStatus<Done, Pending, Action>, BoxedExecutionEngine) + 'static,
    {
        self.with_resolver(FnMutReady::new(action))
    }
}

// ---------------------------------------------------------------------------
// Verb builders (structs declared once, ungated)
// ---------------------------------------------------------------------------

/// Dispatches to the **bottom** of the local queue (`ExecutionEngine::schedule`).
pub struct ScheduleBuilder<Done, Pending, Action, Resolver, Task>
where
    Action: ExecutionAction,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub(crate) config: TaskSpawnConfig<Done, Pending, Action, Resolver, Task>,
}

/// Dispatches to the **top** of the local queue (`ExecutionEngine::lift`).
pub struct LiftBuilder<Done, Pending, Action, Resolver, Task>
where
    Action: ExecutionAction,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub(crate) config: TaskSpawnConfig<Done, Pending, Action, Resolver, Task>,
}

/// Sequences the task with a parent (`ExecutionEngine::sequenced`). Its delivery
/// queue is **bounded** (Feature 45 Part B), so the child parks on vacancy when
/// it outruns the draining parent.
pub struct SequenceBuilder<Done, Pending, Action, Resolver, Task>
where
    Action: ExecutionAction,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub(crate) config: TaskSpawnConfig<Done, Pending, Action, Resolver, Task>,
}

/// Broadcasts to the global queue under `multi`; falls back to a local verb when
/// `multi` is off (see [`LocalFallback`]).
pub struct BroadcastBuilder<Done, Pending, Action, Resolver, Task>
where
    Action: ExecutionAction,
    Resolver: TaskReadyResolver<Action, Done, Pending>,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action>,
{
    pub(crate) config: TaskSpawnConfig<Done, Pending, Action, Resolver, Task>,
    pub(crate) fallback: LocalFallback,
}

// ---------------------------------------------------------------------------
// Send-free spawn() for the three local verbs (no readiness, no Send).
// ---------------------------------------------------------------------------

/// Build the boxed local task (`OnNext` if a resolver is set, else `DoNext`).
///
/// WHY: the spawn form never touches a delivery queue or a readiness, so it boxes
/// into the non-`Send` `BoxedExecutionIterator` the local engine verbs already
/// accept — no `Send` bound is required, even under `multi`.
fn local_spawn_task<Done, Pending, Action, Resolver, Task>(
    task: Task,
    resolver: Option<Resolver>,
) -> crate::valtron::BoxedExecutionIterator
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    match resolver {
        Some(resolver) => OnNext::new(task, resolver).into(),
        None => Box::new(DoNext::new(task)),
    }
}

impl<Done, Pending, Action, Resolver, Task>
    ScheduleBuilder<Done, Pending, Action, Resolver, Task>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    /// Deliver the task to the bottom of the local queue.
    pub fn spawn(self) -> AnyResult<SpawnInfo, ExecutorError> {
        let c = self.config;
        let Some(task) = c.task else {
            return Err(ExecutorError::TaskRequired);
        };
        c.engine.schedule(local_spawn_task(task, c.resolver))
    }
}

impl<Done, Pending, Action, Resolver, Task> LiftBuilder<Done, Pending, Action, Resolver, Task>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    /// Deliver the task to the top of the local queue.
    pub fn spawn(self) -> AnyResult<SpawnInfo, ExecutorError> {
        let c = self.config;
        let parent = c.parent;
        let Some(task) = c.task else {
            return Err(ExecutorError::TaskRequired);
        };
        c.engine.lift(local_spawn_task(task, c.resolver), parent)
    }
}

impl<Done, Pending, Action, Resolver, Task>
    SequenceBuilder<Done, Pending, Action, Resolver, Task>
where
    Done: 'static,
    Pending: 'static,
    Action: ExecutionAction + 'static,
    Resolver: TaskReadyResolver<Action, Done, Pending> + 'static,
    Task: TaskIterator<Pending = Pending, Ready = Done, Spawner = Action> + 'static,
{
    /// Sequence the task with its parent on the local queue.
    pub fn spawn(self) -> AnyResult<SpawnInfo, ExecutorError> {
        let c = self.config;
        let Some(parent) = c.parent else {
            return Err(ExecutorError::ParentMustBeSupplied);
        };
        let Some(task) = c.task else {
            return Err(ExecutorError::TaskRequired);
        };
        c.engine
            .sequenced(local_spawn_task(task, c.resolver), parent)
    }
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// [`spawn_builder`] returns a [`TaskSpawnConfig`] whose resolver is a boxed
/// `dyn TaskReadyResolver`, for the common local-dispatch paths.
#[must_use]
#[allow(clippy::type_complexity)]
pub fn spawn_builder<Task, Action>(
    engine: BoxedExecutionEngine,
) -> TaskSpawnConfig<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + 'static>,
    Task,
>
where
    Task::Ready: 'static,
    Task::Pending: 'static,
    Task: TaskIterator<Spawner = Action> + 'static,
    Action: ExecutionAction + 'static,
{
    TaskSpawnConfig::new(engine)
}

/// [`spawn_broadcaster`] returns a [`TaskSpawnConfig`] whose resolver is a
/// `Send` boxed `dyn TaskReadyResolver`, for the broadcast path.
#[must_use]
#[allow(clippy::type_complexity)]
pub fn spawn_broadcaster<Task, Action>(
    engine: BoxedExecutionEngine,
) -> TaskSpawnConfig<
    Task::Ready,
    Task::Pending,
    Task::Spawner,
    Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + Send + 'static>,
    Task,
>
where
    Task::Ready: Send + 'static,
    Task::Pending: Send + 'static,
    Task: TaskIterator<Spawner = Action> + Send + 'static,
    Action: ExecutionAction + Send + 'static,
{
    TaskSpawnConfig::new(engine)
}
