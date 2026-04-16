//! Provider Client - Central client wrapping `StateStore`.
//!
//! WHY: Users need a single entry point that manages state tracking
//!      for all API operations in a project/stage context.
//!
//! WHAT: Generic wrapper around any `StateStore` implementation,
//!       storing project and stage metadata, with integrated HTTP client.
//!
//! HOW: Holds Arc<StateStore> and Arc<SimpleHttpClient> for thread-safe sharing across
//!       per-API provider instances.

use foundation_db::state::traits::StateStore;
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use std::sync::Arc;

/// Central provider client wrapping a `StateStore` with HTTP client.
///
/// WHY: Users need a single entry point that manages state tracking
///      for all API operations in a project/stage context.
///
/// WHAT: Generic wrapper around any `StateStore` implementation,
///       storing project and stage metadata, with integrated HTTP client.
///
/// HOW: Holds Arc<StateStore> and Arc<SimpleHttpClient> for thread-safe sharing across
///       per-API provider instances.
pub struct ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    /// State store for persistence
    pub state_store: Arc<S>,
    /// Project name for namespacing
    pub project: String,
    /// Stage (dev/staging/prod)
    pub stage: String,
    /// HTTP client for API calls (Arc'd for cheap cloning and pool sharing)
    pub http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> Clone for ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state_store: self.state_store.clone(),
            project: self.project.clone(),
            stage: self.stage.clone(),
            http_client: self.http_client.clone(),
        }
    }
}

impl<S, R> ProviderClient<S, R>
where
    S: StateStore + Send + Sync + 'static,
    R: DnsResolver + Clone + 'static,
{
    /// Create new provider client with state store and HTTP client.
    ///
    /// # Arguments
    ///
    /// * `project` - Project name for state namespacing
    /// * `stage` - Stage name (dev, staging, prod)
    /// * `state_store` - State store implementation
    /// * `http_client` - HTTP client for API calls
    ///
    /// # Example
    ///
    /// ```rust
    /// let state_store = FileStateStore::new("/path", "my-project", "dev");
    /// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
    /// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
    /// ```
    pub fn new(project: &str, stage: &str, state_store: S, http_client: SimpleHttpClient<R>) -> Self {
        Self {
            state_store: Arc::new(state_store),
            project: project.to_string(),
            stage: stage.to_string(),
            http_client: Arc::new(http_client),
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
    pub fn http_client(&self) -> &SimpleHttpClient<R> {
        &self.http_client
    }
}

/// Re-export `ProviderError` for use in per-API providers.
pub use foundation_db::state::store_state_task::ProviderError;

/// Re-export `StoreStateIdentifierTask` for state-aware operations.
pub use foundation_db::state::store_state_task::StoreStateIdentifierTask;
