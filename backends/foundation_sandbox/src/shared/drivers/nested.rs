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
use crate::shared::spec::{RootMode, RootSetup, RootSource, SandboxSpec};
use crate::shared::BoxFuture;

/// The driver of a micro virtual machine (VM) inside a parent sandbox.
pub struct NestedDriver {
    /// The manager's own registry. It is set after the manager exists, because
    /// the manager holds this driver.
    inner: Arc<OnceLock<Arc<Inner>>>,
}

impl NestedDriver {
    /// A driver over the shared registry slot that the manager fills.
    #[must_use]
    pub fn with_slot(slot: Arc<OnceLock<Arc<Inner>>>) -> Self {
        Self { inner: slot }
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
}

impl SubstrateHandle for NestedHandle {
    fn exited(&self) -> BoxFuture<'_, SubstrateExit> {
        let ctx = self.ctx.clone();
        let provider = Arc::clone(&self.provider);
        let inner = self.inner.clone();
        let parent_handle = Arc::clone(&self.parent_handle);
        let forced = self.forced.load(Ordering::SeqCst);
        Box::pin(async move {
            let inner_done = Box::pin(poll_stopped(&provider, &inner, &ctx));
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

/// Waits until the inner sandbox is gone, with one `get` every second on the
/// injected clock.
async fn poll_stopped(provider: &SandboxRpcClient, inner: &SandboxId, ctx: &Ctx) {
    loop {
        match provider.get(inner.clone()).await {
            Ok(handle) => {
                let state = handle.info().state;
                if matches!(state, SandboxState::Stopped | SandboxState::Failed) {
                    return;
                }
            }
            // A stopped inner sandbox is not ready, so `get` refuses it.
            Err(_) => return,
        }
        foundation_core::valtron::timer::sleep_for(Duration::from_secs(1), ctx).await;
    }
}
