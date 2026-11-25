//! The create task: driver choice, launch, the slave readiness wait and the
//! `Prepare` call.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::valtron::{task, Stream, StreamIteratorExt, TaskStatus};
use foundation_errstacks::ErrorTrace;
use futures::future::{select, select_all, Either};

use crate::shared::control::handle::{LocalSandboxOps, SendIterator};
use crate::shared::control::manager::{apply_default_limits, choose_driver, Inner};
use crate::shared::control::record::{created_unix_ms, SandboxRecord};
use crate::shared::control::slave_client::SlaveClient;
use crate::shared::control::stop::{kill_record, spawn_detached, start_watcher};
use crate::shared::control::{wait_until, SandboxState};
use crate::shared::control::{SandboxHandle, SandboxIdPolicy};
use crate::shared::driver::{CreateProgress, LaunchStream, SubstrateHandle};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::CreateStream;
use crate::shared::ids::SandboxId;
use crate::shared::limits::SandboxLimits;
use crate::shared::rpc::messages::{PrepareRequest, SlaveInfo, SlaveLimits};
use crate::shared::spec::{GuestMount, NetworkMode, RootSetup, SandboxSpec};

/// One step of the create task.
pub enum CreateState {
    /// The task chooses the driver, the identifier and the deadline.
    Choose,
    /// The task reads the launch stream of the driver.
    Launch {
        /// The sandbox identifier.
        id: SandboxId,
        /// The spec, with the manager defaults.
        spec: SandboxSpec,
        /// The progress stream of the driver.
        launch: LaunchStream,
        /// The one deadline of the whole create.
        deadline: Instant,
    },
    /// The task calls `Info` until the slave answers.
    AwaitSlave {
        /// The sandbox identifier.
        id: SandboxId,
        /// The spec, with the manager defaults.
        spec: SandboxSpec,
        /// What the driver started.
        substrate: SubstrateParts,
        /// The typed slave client.
        client: Arc<SlaveClient>,
        /// The one deadline of the whole create.
        deadline: Instant,
        /// How many `Info` attempts already failed.
        attempt: u32,
    },
    /// The task calls `Prepare`.
    Prepare {
        /// The sandbox identifier.
        id: SandboxId,
        /// The spec, with the manager defaults.
        spec: SandboxSpec,
        /// The handle of the substrate.
        substrate: Arc<dyn SubstrateHandle>,
        /// The root that the driver asked the slave to prepare.
        root: RootSetup,
        /// The mounts that the driver asked the slave to prepare.
        mounts: Vec<GuestMount>,
        /// The typed slave client.
        client: Arc<SlaveClient>,
        /// What the slave answered in `Info`.
        info: SlaveInfo,
        /// The one deadline of the whole create.
        deadline: Instant,
    },
    /// The task emits the one result.
    Finish {
        /// The ready sandbox, or the error of the create.
        result: SandboxResult<SandboxHandle>,
    },
    /// The task is done.
    Done,
}

/// What one launched substrate gives the control plane.
pub struct SubstrateParts {
    /// The handle of the substrate.
    pub handle: Arc<dyn SubstrateHandle>,
    /// Where the root comes from.
    pub root: RootSetup,
    /// The mounts of the guest.
    pub mounts: Vec<GuestMount>,
}

/// One run of the create story.
pub struct CreateTask {
    /// The shared state of the manager.
    pub inner: Arc<Inner>,
    /// The current step.
    pub state: Option<CreateState>,
    /// The record, once the task chose the identifier.
    pub record: Option<Arc<SandboxRecord>>,
    /// The guard that cancels the create when its owner goes away.
    pub guard: Arc<CreateGuard>,
    /// The highest progress value already passed on.
    pub last_progress: Option<u8>,
    /// The spec that the caller gave.
    pub spec: SandboxSpec,
}

impl CreateTask {
    /// A task for one spec and its guard.
    #[must_use]
    pub fn new(inner: Arc<Inner>, spec: SandboxSpec, guard: Arc<CreateGuard>) -> Self {
        Self {
            inner,
            state: Some(CreateState::Choose),
            record: None,
            guard,
            last_progress: None,
            spec,
        }
    }

    fn next(mut self, state: CreateState) -> Self {
        self.state = Some(state);
        self
    }

    /// Passes a progress value on unless it is lower than the last one.
    fn progress(&mut self, progress: CreateProgress) -> bool {
        let rank = progress_rank(progress);
        if self.last_progress.is_some_and(|last| rank < last) {
            return false;
        }
        self.last_progress = Some(rank);
        true
    }
}

/// The order of the progress values of one create.
fn progress_rank(progress: CreateProgress) -> u8 {
    match progress {
        CreateProgress::Preparing => 0,
        CreateProgress::SubstrateStarted => 1,
        CreateProgress::SubstrateConfigured => 2,
        CreateProgress::Booting => 3,
        CreateProgress::SlaveConnected => 4,
        CreateProgress::SlaveReady => 5,
    }
}

impl Drop for CreateTask {
    fn drop(&mut self) {
        self.guard.cancel();
    }
}

/// The clean-up owner of one create.
///
/// The receiver of the create stream and the task share one guard. Dropping
/// either one calls [`CreateGuard::cancel`], which marks the create cancelled,
/// wakes the task out of its waits, and kills and removes any substrate that
/// the create already launched. A `Drop` cannot wait, so the kill and the
/// removal move to a background task.
///
/// The task calls [`CreateGuard::finish`] on its last success or error. A later
/// drop then has nothing to cancel.
pub struct CreateGuard {
    inner: Arc<Inner>,
    state: Mutex<GuardState>,
}

#[derive(Default)]
struct GuardState {
    kind: GuardKind,
    record: Option<Arc<SandboxRecord>>,
    wakers: Vec<Waker>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GuardKind {
    /// The create still runs.
    #[default]
    Idle,
    /// An owner went away; the create must stop.
    Cancelled,
    /// The task gave its last result.
    Finished,
}

impl CreateGuard {
    /// A guard over one manager.
    #[must_use]
    pub fn new(inner: Arc<Inner>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            state: Mutex::new(GuardState::default()),
        })
    }

    /// The record that a cancel must kill and remove.
    fn set_record(&self, record: Arc<SandboxRecord>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .record = Some(record);
    }

    /// Whether an owner cancelled the create.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .kind
            == GuardKind::Cancelled
    }

    /// Marks the create finished, so a later drop does nothing.
    fn finish(&self) {
        let wakers = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.kind != GuardKind::Idle {
                return;
            }
            state.kind = GuardKind::Finished;
            state.record.take();
            std::mem::take(&mut state.wakers)
        };
        for waker in wakers {
            waker.wake();
        }
    }

    /// Cancels the create and starts the kill and the removal of its record.
    fn cancel(&self) {
        let record = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.kind != GuardKind::Idle {
                return;
            }
            state.kind = GuardKind::Cancelled;
            let record = state.record.take();
            for waker in state.wakers.drain(..) {
                waker.wake();
            }
            record
        };
        if let Some(record) = record {
            let inner = Arc::clone(&self.inner);
            spawn_detached(async move {
                let _ = kill_record(inner, record).await;
            });
        }
    }

    /// Resolves when an owner cancels the create or the task finishes.
    async fn cancelled(&self) {
        futures::future::poll_fn(|context| {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.kind != GuardKind::Idle {
                return Poll::Ready(());
            }
            state.wakers.push(context.waker().clone());
            Poll::Pending
        })
        .await;
    }
}

/// The iterator that the caller receives. Its drop cancels the create.
struct GuardedCreateIterator<I> {
    inner: I,
    guard: Arc<CreateGuard>,
}

impl<I: Iterator> Iterator for GuardedCreateIterator<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<I> Drop for GuardedCreateIterator<I> {
    fn drop(&mut self) {
        self.guard.cancel();
    }
}

/// Calls cancel when the create future is dropped before it gives a result.
struct CancelOnDrop(Arc<CreateGuard>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// Ends a cancelled create without a result.
fn stopped(task: CreateTask) -> (CreateTask, Status) {
    task.guard.finish();
    (
        task.next(CreateState::Done),
        TaskStatus::Ready(Err(SandboxError::SandboxStopped.into())),
    )
}

type Status = TaskStatus<SandboxResult<SandboxHandle>, CreateProgress>;

/// The create state machine.
#[task]
pub async fn create_sandbox(
    ctx: Ctx,
    state: CreateTask,
) -> Option<(
    CreateTask,
    TaskStatus<SandboxResult<SandboxHandle>, CreateProgress>,
)> {
    let mut task = state;
    // The executor supplies its own context. Every clock and timer read must use
    // the manager's context, which the host seeded with the injected effects.
    drop(ctx);
    let ctx = task.inner.ctx.clone();
    if task.guard.is_cancelled() {
        return None;
    }
    let step = task.state.take()?;
    match step {
        CreateState::Choose => Some(choose(ctx, task).await),
        CreateState::Launch {
            id,
            spec,
            launch,
            deadline,
        } => Some(launch_state(ctx, task, id, spec, launch, deadline).await),
        CreateState::AwaitSlave {
            id,
            spec,
            substrate,
            client,
            deadline,
            attempt,
        } => Some(await_slave(ctx, task, id, spec, substrate, client, deadline, attempt).await),
        CreateState::Prepare {
            id,
            spec,
            substrate,
            root,
            mounts,
            client,
            info,
            deadline,
        } => Some(
            prepare(
                ctx, task, id, spec, substrate, root, mounts, client, info, deadline,
            )
            .await,
        ),
        CreateState::Finish { result } => {
            Some((task.next(CreateState::Done), TaskStatus::Ready(result)))
        }
        CreateState::Done => None,
    }
}

async fn choose(ctx: Ctx, mut task: CreateTask) -> (CreateTask, Status) {
    let spec = apply_default_limits(&task.spec, &task.inner.config.limits);
    let (driver, _report) = match choose_driver(&task.inner, &spec).await {
        Ok(chosen) => chosen,
        Err(error) => return fail(task, error.current_context().clone()).await,
    };
    if task.guard.is_cancelled() {
        return stopped(task);
    }
    if task.inner.live_count() >= task.inner.config.max_sandboxes {
        return fail(task, SandboxError::TooManySessions).await;
    }
    let id = match &task.inner.config.id_policy {
        SandboxIdPolicy::Random => {
            let mut chosen = SandboxId::generate(&ctx);
            while task.inner.record(&chosen).is_some() {
                chosen = SandboxId::generate(&ctx);
            }
            chosen
        }
        SandboxIdPolicy::Fixed(id) => {
            if task.inner.record(id).is_some() {
                return fail(task, SandboxError::TooManySessions).await;
            }
            id.clone()
        }
    };
    let created = created_unix_ms(&ctx);
    let record = Arc::new(SandboxRecord::new(
        id.clone(),
        driver.id(),
        spec.clone(),
        created,
    ));
    task.inner.insert(Arc::clone(&record));
    task.record = Some(Arc::clone(&record));
    task.guard.set_record(Arc::clone(&record));
    if let Err(error) = task.inner.store.put(record.stored()).await {
        return fail(task, error.current_context().clone()).await;
    }
    if task.guard.is_cancelled() {
        return stopped(task);
    }
    let deadline = ctx.clock().now() + spec.limits.boot_timeout;
    let launch = driver.launch(ctx, id.clone(), spec.clone(), deadline);
    (
        task.next(CreateState::Launch {
            id,
            spec,
            launch,
            deadline,
        }),
        TaskStatus::Ignore,
    )
}

async fn launch_state(
    ctx: Ctx,
    mut task: CreateTask,
    id: SandboxId,
    spec: SandboxSpec,
    mut launch: LaunchStream,
    deadline: Instant,
) -> (CreateTask, Status) {
    match launch.next() {
        None => fail(task, SandboxError::SlaveHandshake).await,
        Some(Stream::Pending(progress)) => {
            let status = if task.progress(progress) {
                TaskStatus::Pending(progress)
            } else {
                TaskStatus::Ignore
            };
            (
                task.next(CreateState::Launch {
                    id,
                    spec,
                    launch,
                    deadline,
                }),
                status,
            )
        }
        Some(Stream::Next(Ok(substrate))) => {
            let endpoint = substrate.endpoint;
            let root = substrate.root;
            let mounts = substrate.mounts;
            let handle: Arc<dyn SubstrateHandle> = Arc::from(substrate.handle);
            if let Some(record) = &task.record {
                let mut state = record
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.endpoint = Some(endpoint.clone());
                state.substrate = Some(Arc::clone(&handle));
            }
            if task.guard.is_cancelled() {
                return stopped(task);
            }
            let transport = match task.inner.connector.connect(&ctx, &endpoint, &spec.limits) {
                Ok(transport) => transport,
                Err(error) => return fail(task, error.current_context().clone()).await,
            };
            let client = match SlaveClient::new(ctx.clone(), transport, spec.limits.clone()) {
                Ok(client) => Arc::new(client),
                Err(error) => return fail(task, error.current_context().clone()).await,
            };
            if let Some(record) = &task.record {
                record
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .client = Some(Arc::clone(&client));
                if let Err(error) = task.inner.store.put(record.stored()).await {
                    return fail(task, error.current_context().clone()).await;
                }
            }
            if task.guard.is_cancelled() {
                return stopped(task);
            }
            (
                task.next(CreateState::AwaitSlave {
                    id,
                    spec,
                    substrate: SubstrateParts {
                        handle,
                        root,
                        mounts,
                    },
                    client,
                    deadline,
                    attempt: 0,
                }),
                TaskStatus::Ignore,
            )
        }
        Some(Stream::Next(Err(error))) => {
            // The driver cleaned up its own partial work.
            if let Some(record) = &task.record {
                task.inner.remove(&record.id);
                let _ = task.inner.store.remove(&record.id).await;
            }
            task.guard.finish();
            (task.next(CreateState::Done), TaskStatus::Ready(Err(error)))
        }
        Some(Stream::Delayed(delay)) => (
            task.next(CreateState::Launch {
                id,
                spec,
                launch,
                deadline,
            }),
            TaskStatus::Delayed(delay),
        ),
        Some(Stream::Init | Stream::Ignore | Stream::Wait | Stream::Spread(_)) => (
            task.next(CreateState::Launch {
                id,
                spec,
                launch,
                deadline,
            }),
            TaskStatus::Ignore,
        ),
    }
}

/// One candidate of the readiness wait.
enum Wait {
    /// The slave answered `Info`.
    Info(SandboxResult<SlaveInfo>),
    /// The substrate exited.
    Exit(crate::shared::driver::SubstrateExit),
    /// The create deadline passed.
    Deadline,
    /// The timer failed.
    Timer(SandboxResult<()>),
    /// An owner dropped the create.
    Cancelled,
}

#[allow(
    clippy::too_many_arguments,
    reason = "one transition carries the state fields that the design names"
)]
async fn await_slave(
    ctx: Ctx,
    mut task: CreateTask,
    id: SandboxId,
    spec: SandboxSpec,
    substrate: SubstrateParts,
    client: Arc<SlaveClient>,
    deadline: Instant,
    attempt: u32,
) -> (CreateTask, Status) {
    let now = ctx.clock().now();
    if now >= deadline {
        return boot_timeout(task, &substrate.handle, "slave").await;
    }
    let left = deadline.saturating_duration_since(now);
    let timeout = left.min(spec.limits.rpc_timeout);
    let info_client = Arc::clone(&client);
    let exit_handle = Arc::clone(&substrate.handle);
    let timer_ctx = ctx.clone();
    let cancel_guard = Arc::clone(&task.guard);
    let futures: Vec<Pin<Box<dyn Future<Output = Wait> + Send>>> = vec![
        Box::pin(async move { Wait::Info(info_client.info(timeout).await) }),
        Box::pin(async move { Wait::Exit(exit_handle.exited().await) }),
        Box::pin(async move {
            cancel_guard.cancelled().await;
            Wait::Cancelled
        }),
        Box::pin(async move {
            match wait_until(deadline, &timer_ctx).await {
                Ok(()) => Wait::Deadline,
                Err(error) => Wait::Timer(Err(error)),
            }
        }),
    ];
    let (candidate, _index, _rest) = select_all(futures).await;
    match candidate {
        Wait::Info(Ok(info)) => {
            let status = if task.progress(CreateProgress::SlaveConnected) {
                TaskStatus::Pending(CreateProgress::SlaveConnected)
            } else {
                TaskStatus::Ignore
            };
            (
                task.next(CreateState::Prepare {
                    id,
                    spec,
                    substrate: substrate.handle,
                    root: substrate.root,
                    mounts: substrate.mounts,
                    client,
                    info,
                    deadline,
                }),
                status,
            )
        }
        Wait::Info(Err(error)) => {
            if is_retryable_info_error(&error) && ctx.clock().now() < deadline {
                let backoff = retry_backoff(
                    attempt,
                    Duration::from_millis(5),
                    Duration::from_millis(100),
                );
                if let Err(error) = wait_until(ctx.clock().now() + backoff, &ctx).await {
                    return fail(task, error.current_context().clone()).await;
                }
                (
                    task.next(CreateState::AwaitSlave {
                        id,
                        spec,
                        substrate,
                        client,
                        deadline,
                        attempt: attempt.saturating_add(1),
                    }),
                    TaskStatus::Ignore,
                )
            } else {
                let error = error.current_context().clone();
                fail(task, error).await
            }
        }
        Wait::Exit(exit) => {
            let log_tail = substrate
                .handle
                .log_tail(spec.limits.max_error_message)
                .await;
            let error = substrate.handle.exit_error(&exit, log_tail);
            let _ = substrate.handle.cleanup().await;
            fail_without_kill(task, error).await
        }
        Wait::Cancelled => stopped(task),
        Wait::Deadline | Wait::Timer(Ok(())) => {
            boot_timeout(task, &substrate.handle, "slave").await
        }
        Wait::Timer(Err(error)) => {
            let error = error.current_context().clone();
            fail(task, error).await
        }
    }
}

fn is_retryable_info_error(error: &ErrorTrace<SandboxError>) -> bool {
    matches!(
        error.current_context(),
        SandboxError::Rpc { code, .. } if code == "unavailable"
    ) || matches!(error.current_context(), SandboxError::Io { .. })
}

fn retry_backoff(attempt: u32, base: Duration, cap: Duration) -> Duration {
    let factor = 1_u32.checked_shl(attempt.min(16)).unwrap_or(u32::MAX);
    base.saturating_mul(factor).min(cap)
}

async fn boot_timeout(
    task: CreateTask,
    substrate: &Arc<dyn SubstrateHandle>,
    stage: &str,
) -> (CreateTask, Status) {
    let limits = task
        .record
        .as_ref()
        .map_or_else(SandboxLimits::default, |record| record.spec.limits.clone());
    let log_tail = substrate.log_tail(limits.max_error_message).await;
    let error = SandboxError::BootTimeout {
        stage: stage.to_string(),
        log_tail,
    };
    fail(task, error).await
}

#[allow(
    clippy::too_many_arguments,
    reason = "one transition carries the state fields that the design names"
)]
async fn prepare(
    ctx: Ctx,
    mut task: CreateTask,
    id: SandboxId,
    spec: SandboxSpec,
    substrate: Arc<dyn SubstrateHandle>,
    root: RootSetup,
    mounts: Vec<GuestMount>,
    client: Arc<SlaveClient>,
    info: SlaveInfo,
    deadline: Instant,
) -> (CreateTask, Status) {
    let now = ctx.clock().now();
    if now >= deadline {
        return boot_timeout(task, &substrate, "prepare").await;
    }
    let left = deadline.saturating_duration_since(now);
    let request = PrepareRequest {
        root,
        workdir: spec.workdir.clone(),
        env: spec.env.clone(),
        mounts,
        dns: match spec.network {
            NetworkMode::Host => spec.dns.clone(),
            NetworkMode::None => Vec::new(),
        },
        limits: slave_limits(&spec.limits),
    };
    let cancel_guard = Arc::clone(&task.guard);
    let prepare = client.prepare(request, left);
    let cancel = async move { cancel_guard.cancelled().await };
    match select(Box::pin(prepare), Box::pin(cancel)).await {
        Either::Right(((), _prepare)) => stopped(task),
        Either::Left((Ok(_answer), _cancel)) => {
            let Some(record) = task.record.clone() else {
                return fail(task, SandboxError::SlaveHandshake).await;
            };
            {
                let mut state = record
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.state = SandboxState::Ready;
                state.slave = Some(SlaveInfo {
                    prepared: true,
                    ..info
                });
            }
            if let Err(error) = task.inner.store.put(record.stored()).await {
                return fail(task, error.current_context().clone()).await;
            }
            start_watcher(Arc::clone(&task.inner), Arc::clone(&record));
            let ops: Arc<dyn crate::shared::control::SandboxOps> = Arc::new(LocalSandboxOps::new(
                Arc::clone(&task.inner),
                record,
                task.inner.ctx.clone(),
                spec.limits.clone(),
            ));
            let handle = SandboxHandle::new(id, ops);
            task.guard.finish();
            let status = if task.progress(CreateProgress::SlaveReady) {
                TaskStatus::Pending(CreateProgress::SlaveReady)
            } else {
                TaskStatus::Ignore
            };
            (
                task.next(CreateState::Finish { result: Ok(handle) }),
                status,
            )
        }
        Either::Left((Err(error), _cancel)) => {
            let error = error.current_context().clone();
            fail(task, error).await
        }
    }
}

/// Fails the create: kills and cleans up, removes the record, and emits the
/// one error result.
async fn fail(task: CreateTask, error: SandboxError) -> (CreateTask, Status) {
    if let Some(record) = task.record.clone() {
        let substrate = record
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .substrate
            .clone();
        if let Some(substrate) = substrate {
            let _ = substrate.kill().await;
            let _ = substrate.cleanup().await;
        }
        task.inner.remove(&record.id);
        let _ = task.inner.store.remove(&record.id).await;
    }
    task.guard.finish();
    (
        task.next(CreateState::Done),
        TaskStatus::Ready(Err(error.into())),
    )
}

/// Fails the create after the substrate already stopped: it cleans up and
/// removes the record, and sends the substrate no `kill`.
async fn fail_without_kill(task: CreateTask, error: SandboxError) -> (CreateTask, Status) {
    if let Some(record) = task.record.clone() {
        let substrate = record
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .substrate
            .clone();
        if let Some(substrate) = substrate {
            let _ = substrate.cleanup().await;
        }
        task.inner.remove(&record.id);
        let _ = task.inner.store.remove(&record.id).await;
    }
    task.guard.finish();
    (
        task.next(CreateState::Done),
        TaskStatus::Ready(Err(error.into())),
    )
}

/// The `Prepare` limits of one spec.
#[must_use]
pub fn slave_limits(limits: &SandboxLimits) -> SlaveLimits {
    let chunk = u32::try_from(limits.chunk_bytes).unwrap_or(u32::MAX);
    SlaveLimits {
        chunk_bytes: chunk,
        max_exec_sessions: u32::try_from(limits.max_exec_sessions).unwrap_or(u32::MAX),
        output_drain_timeout_ms: u32::try_from(limits.output_drain_timeout.as_millis())
            .unwrap_or(u32::MAX),
        upload_idle_timeout_ms: u32::try_from(limits.upload_idle_timeout.as_millis())
            .unwrap_or(u32::MAX),
        input_queue_bytes: chunk.saturating_mul(4),
        max_message_bytes: u32::try_from(limits.max_message_bytes).unwrap_or(u32::MAX),
        event_queue_len: 4,
    }
}

/// Runs one create to its one result.
///
/// A caller that drops this future cancels the task and kills any substrate
/// that the task already launched.
///
/// # Errors
/// The error of the create, or [`SandboxError::SlaveHandshake`] when the task
/// ends without a result.
pub(crate) async fn create_ready(
    inner: Arc<Inner>,
    spec: SandboxSpec,
) -> SandboxResult<SandboxHandle> {
    let guard = CreateGuard::new(Arc::clone(&inner));
    let _cancel = CancelOnDrop(Arc::clone(&guard));
    let task = CreateTask::new(inner, spec, Arc::clone(&guard));
    let stream =
        foundation_core::valtron::execute(create_sandbox::new(task), None).map_err(|error| {
            SandboxError::Io {
                op: "create".to_string(),
                message: error.to_string(),
            }
        })?;
    let (result, _rest) = stream
        .into_ready_future()
        .await
        .ok_or(SandboxError::SlaveHandshake)?;
    guard.finish();
    result
}

/// The progress stream of one create.
///
/// Dropping the stream cancels the task and kills any substrate that the task
/// already launched.
pub(crate) fn create_stream(inner: Arc<Inner>, spec: SandboxSpec) -> CreateStream {
    let guard = CreateGuard::new(Arc::clone(&inner));
    let task = CreateTask::new(inner, spec, Arc::clone(&guard));
    match foundation_core::valtron::execute(create_sandbox::new(task), None) {
        Ok(iterator) => CreateStream::new(Box::new(GuardedCreateIterator {
            inner: SendIterator::new(iterator),
            guard,
        })),
        Err(error) => {
            let item: Stream<SandboxResult<SandboxHandle>, CreateProgress> =
                Stream::Next(Err(SandboxError::Io {
                    op: "create".to_string(),
                    message: error.to_string(),
                }
                .into()));
            CreateStream::new(Box::new(std::iter::once(item)))
        }
    }
}
