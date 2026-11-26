//! The slave connector of a Durable Object (DO) service binding.
//!
//! A model slave (decision D16) lives in a second DO. A Worker reaches it
//! through a service binding. The call uses the same `NetClient` abstraction as
//! a container port, so the control-plane core does not change: this connector
//! answers a [`PortSource`] that forwards to the binding.

use std::sync::Arc;

use foundation_connectrpc::shared::transport::Transport;
use foundation_context::Ctx;

use crate::shared::cloudflare::transport::{BindingSource, ContainerPortTransport};
use crate::shared::cloudflare::{PortError, PortRequest, PortResponse};
use crate::shared::driver::SlaveEndpoint;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::SlaveConnector;
use crate::shared::ids::SandboxId;
use crate::shared::limits::SandboxLimits;
use crate::shared::BoxFuture;

/// One service binding that answers a request.
///
/// The wasm adapter calls `stub.fetch("https://slave.rpc/<Service>/<Method>")`
/// in Connect framing. A test gives an in-memory binding.
pub trait ServiceBinding: Send + Sync + 'static {
    /// Sends one request to the named Durable Object of `binding`.
    ///
    /// The response body streams.
    fn fetch(
        &self,
        binding: &str,
        name: &SandboxId,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>>;
}

/// A [`SlaveConnector`] over a service binding.
#[derive(Clone)]
pub struct DoSlaveConnector {
    service: Arc<dyn ServiceBinding>,
}

impl DoSlaveConnector {
    /// A connector over `service`.
    #[must_use]
    pub fn new(service: Arc<dyn ServiceBinding>) -> Self {
        Self { service }
    }
}

impl SlaveConnector for DoSlaveConnector {
    fn connect(
        &self,
        _ctx: &Ctx,
        endpoint: &SlaveEndpoint,
        _limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn Transport>> {
        match endpoint {
            SlaveEndpoint::CloudflareDo { binding, name } => {
                let source =
                    BindingSource::new(binding.clone(), name.clone(), Arc::clone(&self.service));
                Ok(Arc::new(ContainerPortTransport::new(Arc::new(source))))
            }
            _ => Err(SandboxError::InvalidSpec {
                field: "endpoint".to_string(),
                reason: "the Durable Object slave connector reaches only a CloudflareDo endpoint"
                    .to_string(),
            }
            .into()),
        }
    }
}
