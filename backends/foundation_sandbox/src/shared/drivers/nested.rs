//! The nested driver (feature 11, section 6.2).
//!
//! A child sandbox runs inside a parent sandbox whose slave also serves the
//! control plane. This driver finds the parent in the manager's own registry,
//! calls `create` on the parent's inner control plane, and gives the outer
//! control plane a substrate whose calls are forwarded to that inner plane.
//!
//! The outer record of a child and the inner sandbox have different
//! identifiers. The endpoint [`SlaveEndpoint::Parent`] names both.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::valtron::{Stream as ValtronStream, StreamSpread};
use futures::future::{select, Either};

use crate::shared::control::manager::Inner;
use crate::shared::control::{SandboxProvider, SandboxRpcClient, SandboxState};
use crate::shared::driver::{
    Capabilities, CreateProgress, HostCheck, HostReport, LaunchStream, SandboxDriver,
    SlaveEndpoint, StoredRecord, Substrate, SubstrateExit, SubstrateHandle,
};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::handles::{CreateStream, ExecFileCalls};
use crate::shared::ids::{DriverId, SandboxId};
use crate::shared::limits::SandboxLimits;
use crate::shared::spec::{RootMode, RootSetup, RootSource, SandboxSpec};
use crate::shared::BoxFuture;

/// The retry policy of the nested child watcher.
///
/// The watcher reads the inner state once per interval. A transport error is
/// not a stopped child, so the watcher keeps polling until the child answers or
/// the child's boot timeout passes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NestedWatchConfig {
    /// The time between two state reads.
    ///
    /// Default 1 s. A local control plane answers in microseconds, and one
    /// second keeps the polling load small on a parent with many children.
    pub poll_interval: Duration,
}

impl Default for NestedWatchConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(1),
        }
    }
}

impl NestedWatchConfig {
    /// Replaces [`Self::poll_interval`].
    #[must_use]
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }
}

/// What the nested child watcher learned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchOutcome {
    /// The child stopped, failed or no longer exists.
    Gone,
    /// Transport errors lasted for the child's whole boot timeout.
    Timeout,
}

/// Watches one child until it is gone or its transport retries time out.
///
/// `read` returns the inner state. [`SandboxError::NotFound`] and
/// [`SandboxError::SandboxStopped`] are the gone cases. Every other error is a
/// transport error: the watcher keeps polling and gives up only after
/// `limits.boot_timeout` of retries, counted as the time that it waited.
pub async fn watch_child<F, Fut>(
    ctx: &Ctx,
    limits: &SandboxLimits,
    config: &NestedWatchConfig,
    mut read: F,
) -> WatchOutcome
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = SandboxResult<SandboxState>>,
{
    // The time spent retrying transport errors, since the last good answer.
    let mut retried = Duration::ZERO;
    loop {
        match read().await {
            Ok(SandboxState::Stopped | SandboxState::Failed) => return WatchOutcome::Gone,
            Ok(_) => retried = Duration::ZERO,
            Err(error)
                if matches!(
                    error.current_context(),
                    SandboxError::NotFound { .. } | SandboxError::SandboxStopped
                ) =>
            {
                return WatchOutcome::Gone;
            }
            Err(_) => {
                if retried >= limits.boot_timeout {
                    return WatchOutcome::Timeout;
                }
                retried = retried.saturating_add(config.poll_interval);
            }
        }
        // The injected timer works on a native host and on wasm.
        let deadline = ctx.clock().now() + config.poll_interval;
        if crate::shared::control::wait_until(deadline, ctx)
            .await
            .is_err()
        {
            return WatchOutcome::Timeout;
        }
    }
}

/// The driver of a micro virtual machine (VM) inside a parent sandbox.
pub struct NestedDriver {
    /// The manager's own registry. It is set after the manager exists, because
    /// the manager holds this driver.
    inner: Arc<OnceLock<Arc<Inner>>>,
    /// The retry policy of the child watcher.
    watch: NestedWatchConfig,
}

impl NestedDriver {
    /// A driver over the shared registry slot that the manager fills.
    #[must_use]
    pub fn with_slot(slot: Arc<OnceLock<Arc<Inner>>>) -> Self {
        Self::with_slot_and_watch(slot, NestedWatchConfig::default())
    }

    /// A driver with a caller's watcher policy.
    #[must_use]
    pub fn with_slot_and_watch(slot: Arc<OnceLock<Arc<Inner>>>, watch: NestedWatchConfig) -> Self {
        Self { inner: slot, watch }
    }

    /// The manager that holds this driver, when it is bound.
    fn registry(&self) -> Option<Arc<Inner>> {
        self.inner.get().cloned()
    }

    /// Starts the remote create and gives the iterator that passes its
    /// progress on.
    fn prepare_launch(
        &self,
        ctx: &Ctx,
        _id: &SandboxId,
        spec: &SandboxSpec,
    ) -> SandboxResult<NestedLaunch> {
        let parent_id = spec
            .parent
            .clone()
            .ok_or_else(|| SandboxError::InvalidSpec {
                field: "parent".to_string(),
                reason: "a nested child needs a parent".to_string(),
            })?;
        let inner = self
            .registry()
            .ok_or_else(|| SandboxError::DriverUnavailable {
                driver: DriverId::Nested,
                reason: "the nested driver is not bound to a manager".to_string(),
            })?;
        let parent = inner
            .record(&parent_id)
            .ok_or_else(|| SandboxError::NotFound {
                what: "parent".to_string(),
            })?;
        let (state, endpoint, parent_handle, can_control) = {
            let state = parent
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                state.state,
                state.endpoint.clone(),
                state.substrate.clone(),
                state.slave.as_ref().is_some_and(|info| info.can_control),
            )
        };
        if state != SandboxState::Ready {
            return Err(SandboxError::SandboxStopped.into());
        }
        if !can_control {
            return Err(SandboxError::DriverUnavailable {
                driver: DriverId::Nested,
                reason: "the parent's slave cannot control".to_string(),
            }
            .into());
        }
        let endpoint = endpoint.ok_or_else(|| SandboxError::DriverUnavailable {
            driver: DriverId::Nested,
            reason: "the parent has no slave endpoint".to_string(),
        })?;
        let parent_handle = parent_handle.ok_or_else(|| SandboxError::DriverUnavailable {
            driver: DriverId::Nested,
            reason: "the parent has no substrate".to_string(),
        })?;
        let transport = inner.connector.connect(ctx, &endpoint, &spec.limits)?;
        let provider = Arc::new(SandboxRpcClient::new(
            ctx.clone(),
            transport,
            spec.limits.clone(),
        ));
        // The inner control plane runs the Firecracker driver; the child has no
        // parent of its own (feature 11, rule 4.5: one level only).
        let mut child = spec.clone();
        child.driver = Some(DriverId::Firecracker);
        child.parent = None;
        child.nested_host = false;
        let stream = provider.create_with_progress(child);
        Ok(NestedLaunch {
            stream,
            ctx: ctx.clone(),
            provider,
            parent_handle,
            parent: parent_id,
            limits: spec.limits.clone(),
            watch: self.watch.clone(),
        })
    }
}

impl std::fmt::Debug for NestedDriver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NestedDriver")
            .field("bound", &self.inner.get().is_some())
            .finish_non_exhaustive()
    }
}

/// The capabilities of a child. They match the Firecracker driver: a child is
/// a Firecracker micro virtual machine (VM) in the parent.
fn capabilities() -> Capabilities {
    Capabilities {
        rootfs_directory: false,
        rootfs_ext4: true,
        rootfs_squashfs: true,
        rootfs_container_image: false,
        root_modes: vec![RootMode::Overlay, RootMode::ReadOnly],
        host_mounts: false,
        network_none: true,
        network_host: false,
        max_vcpus: 32,
    }
}

impl SandboxDriver for NestedDriver {
    fn id(&self) -> DriverId {
        DriverId::Nested
    }

    fn probe(&self, _ctx: Ctx) -> BoxFuture<'static, HostReport> {
        Box::pin(async move {
            HostReport {
                driver: DriverId::Nested,
                usable: true,
                checks: vec![HostCheck {
                    name: "needs_parent".to_string(),
                    ok: true,
                    message: "set `parent` to a sandbox whose slave can control".to_string(),
                }],
                capabilities: capabilities(),
            }
        })
    }

    fn launch(
        &self,
        ctx: Ctx,
        id: SandboxId,
        spec: SandboxSpec,
        _deadline: Instant,
    ) -> LaunchStream {
        match self.prepare_launch(&ctx, &id, &spec) {
            Ok(launch) => Box::new(launch),
            Err(error) => Box::new(std::iter::once(ValtronStream::Next(Err(error)))),
        }
    }

    fn attach(
        &self,
        ctx: Ctx,
        record: &StoredRecord,
    ) -> BoxFuture<'_, SandboxResult<Option<Substrate>>> {
        let registry = self.registry();
        let record = record.clone();
        Box::pin(async move {
            let Some(inner) = registry else {
                return Ok(None);
            };
            let Some(SlaveEndpoint::Parent {
                parent,
                inner: inner_id,
            }) = record.endpoint.clone()
            else {
                return Ok(None);
            };
            let Some(parent_record) = inner.record(&parent) else {
                return Ok(None);
            };
            let (endpoint, parent_handle) = {
                let state = parent_record
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                (state.endpoint.clone(), state.substrate.clone())
            };
            let (Some(endpoint), Some(parent_handle)) = (endpoint, parent_handle) else {
                return Ok(None);
            };
            let Ok(transport) = inner
                .connector
                .connect(&ctx, &endpoint, &record.spec.limits)
            else {
                return Ok(None);
            };
            let provider = Arc::new(SandboxRpcClient::new(
                ctx.clone(),
                transport,
                record.spec.limits.clone(),
            ));
            let Ok(handle) = provider.get(inner_id.clone()).await else {
                return Ok(None);
            };
            let info = handle.info();
            let calls = handle.calls();
            let nested = NestedHandle {
                ctx: ctx.clone(),
                provider,
                parent_handle,
                inner: inner_id.clone(),
                forced: AtomicBool::new(false),
                limits: record.spec.limits.clone(),
                watch: self.watch.clone(),
            };
            Ok(Some(Substrate {
                endpoint: SlaveEndpoint::Parent {
                    parent,
                    inner: inner_id,
                },
                root: RootSetup {
                    source: RootSource::Plain,
                    mode: RootMode::Overlay,
                },
                mounts: Vec::new(),
                handle: Box::new(nested),
                calls: Some(calls),
                slave: info.slave,
            }))
        })
    }
}

/// The progress and the one result of a remote create.
struct NestedLaunch {
    stream: CreateStream,
    ctx: Ctx,
    provider: Arc<SandboxRpcClient>,
    parent_handle: Arc<dyn SubstrateHandle>,
    parent: SandboxId,
    limits: SandboxLimits,
    watch: NestedWatchConfig,
}

impl NestedLaunch {
    /// The substrate of one created inner sandbox.
    fn substrate(&self, handle: &crate::shared::control::SandboxHandle) -> Substrate {
        let inner_id = handle.id().clone();
        let info = handle.info();
        let calls: Arc<dyn ExecFileCalls> = handle.calls();
        let nested = NestedHandle {
            ctx: self.ctx.clone(),
            provider: Arc::clone(&self.provider),
            parent_handle: Arc::clone(&self.parent_handle),
            inner: inner_id.clone(),
            forced: AtomicBool::new(false),
            limits: self.limits.clone(),
            watch: self.watch.clone(),
        };
        Substrate {
            endpoint: SlaveEndpoint::Parent {
                parent: self.parent.clone(),
                inner: inner_id,
            },
            root: RootSetup {
                source: RootSource::Plain,
                mode: RootMode::Overlay,
            },
            mounts: Vec::new(),
            handle: Box::new(nested),
            calls: Some(calls),
            slave: info.slave,
        }
    }
}

impl Iterator for NestedLaunch {
    type Item = ValtronStream<SandboxResult<Substrate>, CreateProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.stream.next()?;
        Some(match item {
            ValtronStream::Next(Ok(handle)) => ValtronStream::Next(Ok(self.substrate(&handle))),
            ValtronStream::Next(Err(error)) => ValtronStream::Next(Err(error)),
            ValtronStream::Pending(progress) => ValtronStream::Pending(progress),
            ValtronStream::Delayed(delay) => ValtronStream::Delayed(delay),
            ValtronStream::Init => ValtronStream::Init,
            ValtronStream::Ignore => ValtronStream::Ignore,
            ValtronStream::Wait => ValtronStream::Wait,
            ValtronStream::Spread(items) => ValtronStream::Spread(
                items
                    .into_iter()
                    .map(|spread| match spread {
                        StreamSpread::Pending(progress) => StreamSpread::Pending(progress),
                        StreamSpread::Done(Ok(handle)) => {
                            StreamSpread::Done(Ok(self.substrate(&handle)))
                        }
                        StreamSpread::Done(Err(error)) => StreamSpread::Done(Err(error)),
                    })
                    .collect(),
            ),
        })
    }
}

/// The handle of one child.
///
/// It watches the inner sandbox and the parent. `terminate` and `kill` send the
/// stop to the parent's inner control plane.
struct NestedHandle {
    ctx: Ctx,
    provider: Arc<SandboxRpcClient>,
    parent_handle: Arc<dyn SubstrateHandle>,
    inner: SandboxId,
    forced: AtomicBool,
    limits: SandboxLimits,
    watch: NestedWatchConfig,
}

impl SubstrateHandle for NestedHandle {
    fn exited(&self) -> BoxFuture<'_, SubstrateExit> {
        let ctx = self.ctx.clone();
        let provider = Arc::clone(&self.provider);
        let inner = self.inner.clone();
        let parent_handle = Arc::clone(&self.parent_handle);
        let forced = self.forced.load(Ordering::SeqCst);
        let limits = self.limits.clone();
        let watch = self.watch.clone();
        Box::pin(async move {
            let inner_done = Box::pin(poll_stopped(&provider, &inner, &ctx, &limits, &watch));
            let parent_done = Box::pin(parent_handle.exited());
            match select(inner_done, parent_done).await {
                Either::Left(((), _parent)) => SubstrateExit {
                    code: None,
                    signal: None,
                    forced,
                },
                Either::Right((exit, _inner)) => exit,
            }
        })
    }

    fn terminate(&self, _grace: Duration) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        let provider = Arc::clone(&self.provider);
        let inner = self.inner.clone();
        Box::pin(async move {
            let report = provider.stop(inner).await?;
            Ok(SubstrateExit {
                code: report.substrate_exit_code,
                signal: None,
                forced: report.killed,
            })
        })
    }

    fn kill(&self) -> BoxFuture<'_, SandboxResult<SubstrateExit>> {
        self.forced.store(true, Ordering::SeqCst);
        let provider = Arc::clone(&self.provider);
        let inner = self.inner.clone();
        Box::pin(async move {
            let report = provider.kill(inner).await?;
            Ok(SubstrateExit {
                code: report.substrate_exit_code,
                signal: None,
                forced: true,
            })
        })
    }

    fn log_tail(&self, _max: usize) -> BoxFuture<'_, Vec<u8>> {
        Box::pin(async { Vec::new() })
    }

    fn cleanup(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let provider = Arc::clone(&self.provider);
        let inner = self.inner.clone();
        Box::pin(async move {
            if provider.get(inner.clone()).await.is_ok() {
                let _ = provider.kill(inner).await;
            }
            Ok(())
        })
    }
}

/// Waits until the inner sandbox is gone, on the injected clock and timer.
///
/// A stopped inner sandbox is not ready, so its control plane refuses `get`
/// with [`SandboxError::SandboxStopped`]; a missing one gives
/// [`SandboxError::NotFound`]. Both mean the child is gone. Every other error
/// is a transport error, and the watcher retries it for the child's boot
/// timeout.
async fn poll_stopped(
    provider: &SandboxRpcClient,
    inner: &SandboxId,
    ctx: &Ctx,
    limits: &SandboxLimits,
    watch: &NestedWatchConfig,
) {
    let _ = watch_child(ctx, limits, watch, || async {
        match provider.get(inner.clone()).await {
            Ok(handle) => Ok(handle.info().state),
            Err(error) => Err(error),
        }
    })
    .await;
}
