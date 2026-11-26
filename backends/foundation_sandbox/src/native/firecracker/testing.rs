//! The test doubles of the Firecracker driver.
//!
//! [`FakeFirecrackerApi`] records every API call and scripts its answers, so the
//! launch tests need no process and no socket. The fake process spawner and the
//! fake host file system come from
//! [`crate::shared::control::testing`].

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crate::native::firecracker::driver::{ConfigStep, FirecrackerApi, FirecrackerApiFactory};
use crate::native::firecracker::generated::boot_source::BootSource;
use crate::native::firecracker::generated::default::{InstanceInfo, InstanceInfoState};
use crate::native::firecracker::generated::drives::Drive;
use crate::native::firecracker::generated::machine_config::MachineConfiguration;
use crate::native::firecracker::generated::vsock::Vsock;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::BoxFuture;

/// One call that the fake API received.
#[allow(
    clippy::large_enum_variant,
    reason = "the test records the exact generated bodies; boxing them hides the fields the tests read"
)]
#[derive(Clone, Debug)]
pub enum ApiCall {
    /// `GET /`.
    Describe,
    /// `PUT /machine-config`.
    MachineConfig(MachineConfiguration),
    /// `PUT /boot-source`.
    BootSource(BootSource),
    /// `PUT /drives/rootfs`.
    Drive(Drive),
    /// `PUT /vsock`.
    Vsock(Vsock),
    /// `PUT /actions`.
    Start,
}

impl ApiCall {
    /// The configuration step of this call, or `None` for `Describe`.
    #[must_use]
    pub fn step(&self) -> Option<ConfigStep> {
        match self {
            Self::Describe => None,
            Self::MachineConfig(_) => Some(ConfigStep::MachineConfig),
            Self::BootSource(_) => Some(ConfigStep::BootSource),
            Self::Drive(_) => Some(ConfigStep::Drive),
            Self::Vsock(_) => Some(ConfigStep::Vsock),
            Self::Start => Some(ConfigStep::Start),
        }
    }
}

#[derive(Default)]
struct Script {
    calls: Vec<ApiCall>,
    describe_failures: usize,
    describe_pending: bool,
    describe_delay: Option<Duration>,
    ctx: Option<foundation_context::Ctx>,
    put_error: Option<(ConfigStep, SandboxError)>,
}

/// A Firecracker API that a test scripts.
pub struct FakeFirecrackerApi {
    state: Mutex<Script>,
}

impl std::fmt::Debug for FakeFirecrackerApi {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("FakeFirecrackerApi")
    }
}

impl FakeFirecrackerApi {
    /// An API that answers every call.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(Script::default()),
        })
    }

    /// An API whose next `count` `GET /` calls fail.
    #[must_use]
    pub fn with_describe_failures(count: usize) -> Arc<Self> {
        let api = Self::new();
        api.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .describe_failures = count;
        api
    }

    /// An API whose `GET /` never answers.
    #[must_use]
    pub fn with_describe_pending() -> Arc<Self> {
        let api = Self::new();
        api.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .describe_pending = true;
        api
    }

    /// An API whose `GET /` answers after `delay` on the injected clock.
    #[must_use]
    pub fn with_describe_delay(delay: Duration) -> Arc<Self> {
        let api = Self::new();
        api.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .describe_delay = Some(delay);
        api
    }

    /// An API that rejects one step with `error`.
    #[must_use]
    pub fn with_put_error(step: ConfigStep, error: SandboxError) -> Arc<Self> {
        let api = Self::new();
        api.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .put_error = Some((step, error));
        api
    }

    /// The calls that the API received, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<ApiCall> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .calls
            .clone()
    }

    /// A factory that gives this API to the driver.
    #[must_use]
    pub fn factory(self: &Arc<Self>) -> FirecrackerApiFactory {
        let api = Arc::clone(self);
        Arc::new(move |ctx, _socket, _limits| {
            api.state.lock().unwrap_or_else(PoisonError::into_inner).ctx = Some(ctx.clone());
            Ok(Arc::clone(&api) as Arc<dyn FirecrackerApi>)
        })
    }

    fn record(&self, call: &ApiCall) -> SandboxResult<()> {
        let mut script = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        script.calls.push(call.clone());
        if let Some((step, error)) = &script.put_error {
            if call.step() == Some(*step) {
                return Err(error.clone().into());
            }
        }
        Ok(())
    }
}

/// A default instance answer that says that the VM is not started.
fn instance_info() -> InstanceInfo {
    InstanceInfo {
        app_name: "Firecracker".to_string(),
        id: "test".to_string(),
        state: InstanceInfoState::NotStarted,
        vmm_version: "1.15.0".to_string(),
    }
}

impl FirecrackerApi for FakeFirecrackerApi {
    fn describe_instance(&self) -> BoxFuture<'_, SandboxResult<InstanceInfo>> {
        let (failures, pending, delay, ctx) = {
            let mut script = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            script.calls.push(ApiCall::Describe);
            let failures = script.describe_failures;
            if failures > 0 {
                script.describe_failures -= 1;
            }
            let pending = script.describe_pending;
            let delay = script.describe_delay.take();
            (failures, pending, delay, script.ctx.clone())
        };
        if failures > 0 {
            return Box::pin(async {
                Err(SandboxError::Io {
                    op: "firecracker api".to_string(),
                    message: "not ready".to_string(),
                }
                .into())
            });
        }
        if pending {
            return Box::pin(std::future::pending());
        }
        if let (Some(delay), Some(ctx)) = (delay, ctx) {
            return Box::pin(async move {
                if let Ok(env) = ctx.require_env() {
                    let timer = env.timer();
                    let deadline = ctx.clock().now() + delay;
                    timer.sleep_until(ctx.clone(), deadline).await;
                }
                Ok(instance_info())
            });
        }
        Box::pin(async { Ok(instance_info()) })
    }

    fn put_machine_config<'a>(
        &'a self,
        body: &'a MachineConfiguration,
    ) -> BoxFuture<'a, SandboxResult<()>> {
        let body = body.clone();
        Box::pin(async move { self.record(&ApiCall::MachineConfig(body)) })
    }

    fn put_boot_source<'a>(&'a self, body: &'a BootSource) -> BoxFuture<'a, SandboxResult<()>> {
        let body = body.clone();
        Box::pin(async move { self.record(&ApiCall::BootSource(body)) })
    }

    fn put_drive<'a>(&'a self, body: &'a Drive) -> BoxFuture<'a, SandboxResult<()>> {
        let body = body.clone();
        Box::pin(async move { self.record(&ApiCall::Drive(body)) })
    }

    fn put_vsock<'a>(&'a self, body: &'a Vsock) -> BoxFuture<'a, SandboxResult<()>> {
        let body = body.clone();
        Box::pin(async move { self.record(&ApiCall::Vsock(body)) })
    }

    fn instance_start(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move { self.record(&ApiCall::Start) })
    }
}
