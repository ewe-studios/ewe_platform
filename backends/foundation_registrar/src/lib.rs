//! `foundation_registrar` is the registrar sidecar.
//!
//! WHY: the proxy must route only to backends that are installed, running and
//! ready. This crate runs beside the apps, reads the manifest, discovers them,
//! probes them, reads their metrics and reports to the proxy registration
//! authority (spec 82 feature 01).
//!
//! WHAT: the module map and the [`RegistrarContext`] bundle. The library holds
//! no global side effect: the clock, timers and randomness come from
//! [`Ctx`](foundation_context::Ctx); the process runner and the Docker client
//! come from the context. Only the `ewe-registrar` binary builds the real ones.
//!
//! HOW: the binary parses a [`RegistrarConfig`], connects a
//! [`RegistrationClient`], assembles the discovery adapters, the prober and the
//! metrics sources, and drives the [`Reconciler`].

pub mod client;
pub mod config;
pub mod discovery;
pub mod error;
pub mod manifest;
pub mod metrics;
pub mod probe;
pub mod proto;
pub mod reconcile;

use std::sync::Arc;

use foundation_context::Ctx;
use foundation_deployment_docker::DockerClient;

pub use client::{
    IdentityStore, RegistrationApi, RegistrationClient, StoredIdentity, CA_FILE, CLIENT_CERT_FILE,
    CLIENT_KEY_FILE,
};
pub use config::{RegistrarConfig, RegistrarMode};
pub use discovery::docker::{
    ContainerApi, DockerContainerApi, DockerDiscovery, UnavailableDocker, ENTRY_LABEL,
    VERSION_LABEL,
};
pub use discovery::systemd::SystemdDiscovery;
pub use discovery::{
    CommandError, CommandOutput, CommandRunner, Discovered, Discovery, DiscoverySet, RunHandle,
    SystemCommandRunner,
};
pub use error::RegistrarError;
pub use manifest::{
    DeployView, EntryView, ManifestView, ProbeCheck, ProbeSpec, ProbesBlock, ProxyBlock,
    RouteBlock, SourceView, SUPPORTED_MANIFEST_VERSION,
};
pub use metrics::{AppMetrics, CgroupMetrics, CompositeMetrics, DockerMetrics, MetricsSource};
pub use probe::{
    host_prober, NetworkProber, ProbeExecutor, ProbeOutcome, ProbeSettings, ProbeVerdict, Prober,
    SystemTcpConnector, TcpConnector,
};
pub use reconcile::{Backoff, ManifestSource, Observation, Reconciler};

/// The bundle that discovery, probes and metrics read.
///
/// The context holds the injected clock and randomness, the configuration, the
/// process runner and the Docker client. A test replaces the runner and the
/// client, so the code under test does not change.
#[derive(Clone)]
pub struct RegistrarContext {
    /// The side-effect bundle: clock, randomness and the entry for other
    /// injected sources.
    pub ctx: Ctx,
    /// The sidecar configuration.
    pub config: Arc<RegistrarConfig>,
    /// Runs `systemctl` and an `exec` probe.
    pub commands: Arc<dyn CommandRunner>,
    /// The Docker daemon, when this host has one.
    pub docker: Option<Arc<DockerClient>>,
}

impl std::fmt::Debug for RegistrarContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistrarContext")
            .field("host_id", &self.config.host_id)
            .field("docker", &self.docker.is_some())
            .finish_non_exhaustive()
    }
}

impl RegistrarContext {
    /// A context over the four parts.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        config: Arc<RegistrarConfig>,
        commands: Arc<dyn CommandRunner>,
        docker: Option<Arc<DockerClient>>,
    ) -> Self {
        Self {
            ctx,
            config,
            commands,
            docker,
        }
    }

    /// The context's clock.
    #[must_use]
    pub fn now(&self) -> std::time::Instant {
        self.ctx.clock().now()
    }
}
