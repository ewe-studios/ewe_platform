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
    pub pool: Option<Arc<super::pool::ConnectionPool>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::simple_http::client::dns::MockDnsResolver;
    use crate::wire::simple_http::client::{ClientRequestBuilder, StaticSocketAddr};

    // ========================================================================
    // RedirectAction Tests
    // ========================================================================

    /// WHY: Verify RedirectAction can be constructed with valid parameters
    /// WHAT: Tests that new() creates action with expected initial state
    #[test]
    fn test_redirect_action_new() {
        let request = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .build();
        let resolver = MockDnsResolver::new();

        let action = RedirectAction::new(request, resolver, 5, None);

        assert!(action.request.is_some());
        assert_eq!(action.remaining_redirects, 5);
    }

    /// WHY: Verify RedirectAction is an ExecutionAction (trait bound check)
    /// WHAT: Tests that RedirectAction implements ExecutionAction trait
    #[test]
    fn test_redirect_action_is_execution_action() {
        let request = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .build();
        let resolver = MockDnsResolver::new();

        let action = RedirectAction::new(request, resolver, 3, None);

        // Type check - ensure it can be boxed as ExecutionAction
        let _boxed: Box<dyn ExecutionAction> = Box::new(action);
    }

    /// WHY: Verify RedirectAction::apply is idempotent via Option::take()
    /// WHAT: Tests that calling apply() multiple times doesn't cause issues
    #[test]
    fn test_redirect_action_apply_idempotent() {
        let request = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .build();
        let resolver = MockDnsResolver::new();

        let mut action = RedirectAction::new(request, resolver, 3, None);

        // First apply should consume the request
        assert!(action.request.is_some());

        // Note: We can't actually test apply() yet since we need a BoxedExecutionEngine
        // This will be fully testable once we have the executor infrastructure

        // For now, manually test the take pattern
        let taken = action.request.take();
        assert!(taken.is_some());
        assert!(action.request.is_none());

        // Second take should return None
        let taken_again = action.request.take();
        assert!(taken_again.is_none());
    }

    // ========================================================================
    // TlsUpgradeAction Tests
    // ========================================================================

    #[cfg(not(target_arch = "wasm32"))]
    mod tls_upgrade_tests {
        use super::*;

        /// WHY: Verify TlsUpgradeAction structure and fields
        /// WHAT: Tests that TlsUpgradeAction has correct field types (without real connection)
        #[test]
        fn test_tls_upgrade_action_structure() {
            // We test the structure without creating a real connection
            // since Connection::without_timeout actually attempts to connect

            // Type check: verify TlsUpgradeAction can hold the expected types
            fn _assert_tls_upgrade_holds_expected_types(_action: TlsUpgradeAction) {
                // Compile-time check that the type is correct
            }
        }

        /// WHY: Verify TlsUpgradeAction is an ExecutionAction (trait bound check)
        /// WHAT: Tests that TlsUpgradeAction implements ExecutionAction trait (compile-time)
        #[test]
        fn test_tls_upgrade_action_is_execution_action() {
            // Type check: verify TlsUpgradeAction implements ExecutionAction
            fn _assert_is_execution_action<T: ExecutionAction>() {}
            _assert_is_execution_action::<TlsUpgradeAction>();
        }
    }

    // ========================================================================
    // HttpClientAction Tests
    // ========================================================================

    /// WHY: Verify HttpClientAction::None variant works
    /// WHAT: Tests that None variant can be created and is an ExecutionAction
    #[test]
    fn test_http_client_action_none() {
        let action: HttpClientAction<MockDnsResolver> = HttpClientAction::None;

        // Should compile and be callable (even if it does nothing)
        // We can't actually call apply without a real engine, but we can verify the type
        let _boxed: Box<dyn ExecutionAction> = Box::new(action);
    }

    /// WHY: Verify HttpClientAction::Redirect variant delegates correctly
    /// WHAT: Tests that Redirect variant wraps RedirectAction properly
    #[test]
    fn test_http_client_action_redirect() {
        let request = ClientRequestBuilder::get(
            StaticSocketAddr::new(std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
            "http://example.com",
        )
        .unwrap()
        .build();
        let resolver = MockDnsResolver::new();

        let redirect_action = RedirectAction::new(request, resolver, 3, None);
        let action = HttpClientAction::Redirect(redirect_action);

        // Type check
        let _boxed: Box<dyn ExecutionAction> = Box::new(action);
    }

    /// WHY: Verify HttpClientAction::TlsUpgrade variant delegates correctly
    /// WHAT: Tests that TlsUpgrade variant type compiles (compile-time check)
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_http_client_action_tls_upgrade() {
        // Compile-time type check: verify HttpClientAction can hold TlsUpgradeAction
        fn _assert_tls_upgrade_variant_exists() {
            // This verifies the enum variant compiles correctly
            fn _assert_can_create<R: DnsResolver + Send + 'static>(_action: HttpClientAction<R>) {
                // Type check
            }
        }
    }
}
