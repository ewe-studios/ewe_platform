//! DiscoveryProvider - State-aware discovery API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       discovery API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::discovery::{
    discovery_apis_get_rest_builder, discovery_apis_get_rest_task,
    discovery_apis_list_builder, discovery_apis_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::discovery::DirectoryList;
use crate::providers::gcp::clients::discovery::RestDescription;
use crate::providers::gcp::clients::discovery::DiscoveryApisGetRestArgs;
use crate::providers::gcp::clients::discovery::DiscoveryApisListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DiscoveryProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let client = ProviderClient::new("my-project", "dev", state_store);
/// let http_client = SimpleHttpClient::new(...);
/// let provider = DiscoveryProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DiscoveryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DiscoveryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DiscoveryProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Discovery apis get rest.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RestDescription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn discovery_apis_get_rest(
        &self,
        args: &DiscoveryApisGetRestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RestDescription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = discovery_apis_get_rest_builder(
            &self.http_client,
            &args.api,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = discovery_apis_get_rest_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Discovery apis list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DirectoryList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn discovery_apis_list(
        &self,
        args: &DiscoveryApisListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DirectoryList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = discovery_apis_list_builder(
            &self.http_client,
            &args.name,
            &args.preferred,
        )
        .map_err(ProviderError::Api)?;

        let task = discovery_apis_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
