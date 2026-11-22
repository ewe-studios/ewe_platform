//! The proxy registration authority.
//!
//! WHY: the proxy routes only to backends that are installed, running and
//! ready. A registrar sidecar (feature 01) learns that from the apps and
//! reports it here. This module owns the proxy end of the link (spec 82
//! feature 00).
//!
//! WHAT:
//! - [`config`] — [`RegistrationConfig`] and [`RegistrationPolicy`], every
//!   limit a field with a documented default.
//! - [`proto`] — the generated `ewe.proxy.registration.v1` messages and
//!   services.
//! - [`policy`] — the address, route-host and service checks.
//! - [`registry`] — [`BackendRegistry`] with sessions, generations, leases, the
//!   rotation rule and the router swap.
//! - [`spawn_sweeper`] — the valtron task that ends expired leases and removes
//!   old entries.
//!
//! HOW: the registry is in memory. Each change builds a fresh immutable
//! [`crate::router::Router`] and swaps it through [`crate::router::RouterHandle`],
//! so a request reads one snapshot for its whole life and no connection drops.

pub mod authority;
pub mod config;
pub mod enroll;
pub mod policy;
pub mod proto;
pub mod registry;
pub mod service;
#[cfg(unix)]
pub mod unix;

use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::{execute, from_future, sleep_async};
use foundation_http::ShutdownSignal;

use crate::config::{ProxyError, ProxyResult};

pub use authority::RegistrationAuthority;
pub use config::{RegistrationConfig, RegistrationPolicy};
pub use enroll::{EnrollmentAuthority, EnrollmentError};
pub use registry::{
    BackendMetricsOwned, BackendRegistry, BackendSpecOwned, PeerIdentity, RegisteredBackend,
    RegistrationError, RegistrationState, Session,
};
pub use service::{AccessMode, EnrollmentHandler, RegistrationHandler};
#[cfg(unix)]
pub use unix::UnixSocketAcceptor;

/// Why the registration authority could not start.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
pub enum RegistrationAuthorityError {
    /// The configuration or a key is not usable.
    #[display("registration configuration: {_0}")]
    Config(String),
    /// The trust store or the state directory failed.
    #[display("registration storage: {_0}")]
    Storage(String),
    /// The certificate authority or the Transport Layer Security (TLS)
    /// configuration failed.
    #[display("registration TLS: {_0}")]
    Tls(String),
    /// The Unix socket could not be bound, or its owner, group or mode is
    /// wrong.
    #[display("registration Unix socket: {_0}")]
    Socket(String),
    /// A listener, a thread or the sweeper failed.
    #[display("registration runtime: {_0}")]
    Runtime(String),
}

impl std::error::Error for RegistrationAuthorityError {}

/// Start the lease sweeper: every `sweep_interval` call
/// [`BackendRegistry::sweep`] until `shutdown` fires.
///
/// The task runs on the valtron pool and parks on the injected clock, so it
/// holds no thread and a deterministic test can drive it.
///
/// # Errors
/// [`ProxyError::Startup`] when the pool cannot accept the task.
pub fn spawn_sweeper(
    registry: Arc<BackendRegistry>,
    shutdown: Arc<ShutdownSignal>,
    ctx: Ctx,
) -> ProxyResult<()> {
    let interval = registry.config().sweep_interval;
    let slice = interval.min(Duration::from_millis(500));
    let task = from_future(async move {
        while !shutdown.probe() {
            registry.sweep();
            let mut waited = Duration::ZERO;
            while waited < interval && !shutdown.probe() {
                sleep_async(slice, &ctx).await;
                waited += slice;
            }
        }
    });
    execute(task, None)
        .map_err(|e| ProxyError::Startup(format!("registration sweeper cannot start: {e}")))?;
    Ok(())
}
