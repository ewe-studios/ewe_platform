//! `ExecutionAction` implementations for HTTP client operations.
//!
//! WHY: Provides spawnable actions for HTTP redirects and TLS upgrades using
//! the valtron executor pattern. These actions enable non-blocking task spawning
//! within the HTTP request lifecycle.
//!
//! WHAT: Implements `RedirectAction`, `TlsUpgradeAction`, and `HttpClientAction`
//! that follow the `ExecutionAction` trait with correct `&mut self` signature
//! and `Option::take()` pattern for idempotent operations.
//!
//! HOW: Each action holds optional state fields (using Option<T>) that are
//! consumed via `take()` during `apply()`, making multiple `apply()` calls safe.
//! Actions use `spawn_builder()` to spawn child tasks with parent linkage.

use std::sync::Arc;

use crate::netcap::{Connection, RawStream};
use crate::synca::mpp::Sender;
use crate::synca::Entry;
use crate::valtron::{
    spawn_builder, BoxedExecutionEngine, ExecutionAction, GenericResult, SpawnInfo, SpawnType,
};
use crate::wire::simple_http::client::{
    DnsResolver, HttpRequestTask, PreparedRequest, TlsHandshakeTask,
};

/// Action for spawning HTTP redirect follow tasks.
///
/// WHY: HTTP redirects require spawning a new request task to follow the redirect
/// location. This action encapsulates the redirect spawning logic.
///
/// WHAT: Holds the redirect request and spawns a new HTTP request task when applied.
///
/// HOW: Uses Option<PreparedRequest> with `take()` to ensure idempotent `apply()`.
/// Spawns using `lift()` for priority execution of redirects.
pub struct RedirectAction<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Connection pool for reuse (optional)
    pool: Option<Arc<super::pool::ConnectionPool>>,
    /// The prepared request for the redirect (consumed on apply)
    request: Option<PreparedRequest>,
    /// DNS resolver for the redirected request
    resolver: R,
    /// Remaining redirect follow attempts
    remaining_redirects: u8,
}

impl<R> RedirectAction<R>
where
    R: DnsResolver + Send + 'static,
{
    /// Creates a new redirect action.
    ///
    /// # Arguments
    ///
    /// * `request` - The prepared request for the redirect URL
    /// * `resolver` - DNS resolver instance
    /// * `remaining_redirects` - Number of remaining redirects allowed
    #[allow(dead_code)]
    pub fn new(
        request: PreparedRequest,
        resolver: R,
        remaining_redirects: u8,
        pool: Option<Arc<super::pool::ConnectionPool>>,
    ) -> Self {
        Self {
            request: Some(request),
            pool,
            resolver,
            remaining_redirects,
        }
    }

    pub fn remaining_redirects(&self) -> u8 {
        self.remaining_redirects
    }

    pub fn prepared_request_ref(&self) -> &Option<PreparedRequest> {
        &self.request
    }

    pub fn into_parts(
        self,
    ) -> (
        Option<Arc<super::pool::ConnectionPool>>,
        Option<PreparedRequest>,
        R,
        u8,
    ) {
        (
            self.pool,
            self.request,
            self.resolver,
            self.remaining_redirects,
        )
    }
}

impl<R> ExecutionAction for RedirectAction<R>
where
    R: DnsResolver + Send + 'static,
{
    fn apply(
        &mut self,
        key: Option<Entry>,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        // Take the request to ensure idempotent apply() calls
        if let Some(request) = self.request.take() {
            // Create a new HTTP request task for the redirect
            let task = if let Some(pool) = self.pool.take() {
                HttpRequestTask::new(
                    request,
                    self.resolver.clone(),
                    self.remaining_redirects,
                    Some(pool),
                )
            } else {
                HttpRequestTask::new(
                    request,
                    self.resolver.clone(),
                    self.remaining_redirects,
                    None,
                )
            };

            // Spawn the task as a child of the current task using lift()
            // for priority execution of redirects
            return spawn_builder(engine)
                .maybe_parent(key)
                .with_task(task)
                .lift()
                .map_err(Into::into);
        }
        Ok(SpawnInfo::new(SpawnType::None, None, None))
    }
}

/// Action for spawning TLS upgrade tasks.
///
/// WHY: HTTPS connections require TLS handshake after TCP connection is established.
/// This action encapsulates the TLS upgrade spawning logic, enabling non-blocking
/// TLS handshakes.
///
/// WHAT: Holds a TCP Connection and spawns a TLS handshake task when applied.
/// The task performs the TLS handshake and sends the upgraded stream via channel.
///
/// HOW: Uses Option<Connection> with `take()` to ensure idempotent `apply()`.
/// Spawns `TlsHandshakeTask` using `lift()` for priority execution of TLS upgrades.
/// Callbacks are invoked upon completion with the upgraded TLS stream.
#[cfg(not(target_arch = "wasm32"))]
pub struct TlsUpgradeAction {
    /// The TCP connection to upgrade (consumed on apply)
    connection: Option<Connection>,
    /// Server Name Indication (SNI) for TLS handshake
    sni: String,
    /// Callback endpoint for sending the result
    on_complete: Option<Sender<Result<RawStream, String>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl TlsUpgradeAction {
    /// Creates a new TLS upgrade action.
    ///
    /// # Arguments
    ///
    /// * `connection` - The TCP connection to upgrade to TLS
    /// * `sni` - Server Name Indication (hostname for TLS)
    /// * `on_complete` - Endpoint to send result when TLS handshake completes
    #[must_use]
    pub fn new(
        connection: Connection,
        sni: String,
        on_complete: Sender<Result<RawStream, String>>,
    ) -> Self {
        Self {
            connection: Some(connection),
            sni,
            on_complete: Some(on_complete),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ExecutionAction for TlsUpgradeAction {
    fn apply(
        &mut self,
        key: Option<Entry>,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        // Extract connection and sender using Option::take() for idempotency
        if let (Some(connection), Some(sender)) = (self.connection.take(), self.on_complete.take())
        {
            // Create TlsHandshakeTask
            let tls_task = TlsHandshakeTask::new(connection, self.sni.clone(), sender);

            tracing::debug!("Spawned TLS handshake task for {}", self.sni);

            // Spawn the task using spawn_builder with priority (lift)
            return spawn_builder(engine)
                .maybe_parent(key)
                .with_task(tls_task)
                .lift()
                .map_err(Into::into);
        }

        Ok(SpawnInfo::new(SpawnType::None, None, None))
    }
}

/// Combined action enum for all HTTP client operations.
///
/// WHY: Provides a unified type for different HTTP client actions that can be
/// used as the Spawner type in `TaskIterator` implementations.
///
/// WHAT: Enum combining all action types (redirects, TLS upgrades) plus a None variant.
///
/// HOW: Delegates `apply()` to the inner action based on the variant.
pub enum HttpClientAction<R>
where
    R: DnsResolver + Send + 'static,
{
    /// No action to perform
    None,
    /// Redirect action
    Redirect(RedirectAction<R>),
    /// TLS upgrade action
    #[cfg(not(target_arch = "wasm32"))]
    TlsUpgrade(TlsUpgradeAction),
}

impl<R> ExecutionAction for HttpClientAction<R>
where
    R: DnsResolver + Send + 'static,
{
    fn apply(
        &mut self,
        key: Option<Entry>,
        engine: BoxedExecutionEngine,
    ) -> GenericResult<SpawnInfo> {
        match self {
            HttpClientAction::None => Ok(SpawnInfo::new(SpawnType::None, None, None)),
            HttpClientAction::Redirect(action) => action.apply(key, engine),
            #[cfg(not(target_arch = "wasm32"))]
            HttpClientAction::TlsUpgrade(action) => action.apply(key, engine),
        }
    }
}
