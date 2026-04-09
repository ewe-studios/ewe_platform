//! GCP Provider Client - Central client wrapping StateStore.
//!
//! WHY: Users need a single entry point that manages state tracking
//!      for all API operations in a project/stage context.
//!
//! WHAT: Generic wrapper around any StateStore implementation,
//!       storing project and stage metadata.
//!
//! HOW: Holds Arc<StateStore> for thread-safe sharing across
//!       per-API provider instances.

use foundation_db::state::traits::StateStore;
use std::sync::Arc;

/// Central provider client wrapping a StateStore.
///
/// WHY: Users need a single entry point that manages state tracking
///      for all API operations in a project/stage context.
///
/// WHAT: Generic wrapper around any StateStore implementation,
///       storing project and stage metadata.
///
/// HOW: Holds Arc<StateStore> for thread-safe sharing across
///       per-API provider instances.
#[derive(Debug, Clone)]
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
}

impl<S> ProviderClient<S>
where
    S: StateStore + Send + Sync + 'static,
{
    /// Create new provider client with state store.
    ///
    /// # Arguments
    ///
    /// * `project` - Project name for state namespacing
    /// * `stage` - Stage name (dev, staging, prod)
    /// * `state_store` - State store implementation
    ///
    /// # Example
    ///
    /// ```rust
    /// let state_store = FileStateStore::new("/path", "my-project", "dev");
    /// let client = ProviderClient::new("my-project", "dev", state_store);
    /// ```
    pub fn new(project: &str, stage: &str, state_store: S) -> Self {
        Self {
            state_store: Arc::new(state_store),
            project: project.to_string(),
            stage: stage.to_string(),
        }
    }

    /// Get reference to state store.
    pub fn state_store(&self) -> &S {
        &self.state_store
    }

    /// Get project name.
    pub fn project(&self) -> &str {
        &self.project
    }

    /// Get stage name.
    pub fn stage(&self) -> &str {
        &self.stage
    }
}
