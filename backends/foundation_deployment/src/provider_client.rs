//! Provider Client - Central client wrapping `StateStore`.
//!
//! WHY: Users need a single entry point that manages state tracking
//!      for all API operations in a project/stage context.
//!
//! WHAT: Generic wrapper around any `StateStore` implementation,
//!       storing project and stage metadata, with integrated HTTP client.
//!
//! HOW: Holds `Arc<StateStore>` and a `DynNetClient` for thread-safe sharing
//!       across per-API provider instances.

use std::sync::Arc;

use foundation_core::valtron::Clock;
use foundation_db::core::state::traits::StateStore;
use foundation_netio::{DynNetClient, HttpClientBuilder};

/// Central provider client wrapping a `StateStore` with HTTP client.
///
/// WHY: Users need a single entry point that manages state tracking
///      for all API operations in a project/stage context.
///
/// WHAT: Generic wrapper around any `StateStore` implementation,
///       storing project and stage metadata, with integrated HTTP client.
///
/// HOW: Holds `Arc<StateStore>` and a [`DynNetClient`] (the platform's network
///       client: the native client, or the fetch client on wasm32) for
///       thread-safe sharing across per-API provider instances.
pub struct ProviderClient<S>
where
    S: StateStore + Send + Sync + 'static,
{
    /// State store for persistence
    pub state_store: Arc<S>,
    /// Project name for namespacing
    pub project: String,
    /// Stage (dev/staging/prod)
    pub stage: String,
    /// HTTP client for API calls. A clone is cheap (`Arc`) and shares the
    /// client's connection pool.
    pub http_client: DynNetClient,
    /// The clock that stamps the state a deploy records.
    ///
    /// The host builds the real clock once and passes it in. A test passes a
    /// `SimulatedClock`, so a recorded `created_at` is deterministic.
    pub clock: Arc<dyn Clock>,
}

impl<S> Clone for ProviderClient<S>
where
    S: StateStore + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state_store: self.state_store.clone(),
            project: self.project.clone(),
            stage: self.stage.clone(),
            http_client: self.http_client.clone(),
            clock: self.clock.clone(),
        }
    }
}

impl<S> ProviderClient<S>
where
    S: StateStore + Send + Sync + 'static,
{
    /// Create a provider client with a state store and the platform's default
    /// HTTP client (built with [`HttpClientBuilder`]).
    ///
    /// # Arguments
    ///
    /// * `project` - Project name for state namespacing
    /// * `stage` - Stage name (dev, staging, prod)
    /// * `state_store` - State store implementation
    /// * `clock` - The clock that stamps the state a deploy records
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::path::Path;
    /// use std::sync::Arc;
    /// use foundation_core::valtron::MonotonicClock;
    /// use foundation_deployment::provider_client::ProviderClient;
    /// use foundation_db::core::state::FileStateStore;
    ///
    /// let state_store = FileStateStore::new(Path::new("/path"), "my-project", "dev");
    /// let client = ProviderClient::new(
    ///     "my-project",
    ///     "dev",
    ///     state_store,
    ///     Arc::new(MonotonicClock::new()),
    /// );
    /// ```
    pub fn new(project: &str, stage: &str, state_store: S, clock: Arc<dyn Clock>) -> Self {
        Self::with_client(
            HttpClientBuilder::new().build(),
            project,
            stage,
            state_store,
            clock,
        )
    }

    /// Create a provider client that sends its HTTP calls through
    /// `http_client` (a shared client, or a client with its own settings).
    pub fn with_client(
        http_client: DynNetClient,
        project: &str,
        stage: &str,
        state_store: S,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            state_store: Arc::new(state_store),
            project: project.to_string(),
            stage: stage.to_string(),
            http_client,
            clock,
        }
    }

    /// Get reference to state store.
    #[must_use]
    pub fn state_store(&self) -> &S {
        &self.state_store
    }

    /// Get project name.
    #[must_use]
    pub fn project(&self) -> &str {
        &self.project
    }

    /// Get stage name.
    #[must_use]
    pub fn stage(&self) -> &str {
        &self.stage
    }

    /// Get reference to HTTP client.
    #[must_use]
    pub fn http_client(&self) -> &DynNetClient {
        &self.http_client
    }

    /// Get the clock that stamps recorded state.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }
}

/// Re-export `ProviderError` for use in per-API providers.
pub use foundation_db::core::state::store_state_task::ProviderError;

/// Re-export `StoreStateIdentifierTask` for state-aware operations.
pub use foundation_db::core::state::store_state_task::StoreStateIdentifierTask;
