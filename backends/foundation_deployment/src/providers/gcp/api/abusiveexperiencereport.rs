//! AbusiveexperiencereportProvider - State-aware abusiveexperiencereport API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       abusiveexperiencereport API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::abusiveexperiencereport::{
    abusiveexperiencereport_sites_get_builder, abusiveexperiencereport_sites_get_task,
    abusiveexperiencereport_violating_sites_list_builder, abusiveexperiencereport_violating_sites_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::abusiveexperiencereport::SiteSummaryResponse;
use crate::providers::gcp::clients::abusiveexperiencereport::ViolatingSitesResponse;
use crate::providers::gcp::clients::abusiveexperiencereport::AbusiveexperiencereportSitesGetArgs;
use crate::providers::gcp::clients::abusiveexperiencereport::AbusiveexperiencereportViolatingSitesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AbusiveexperiencereportProvider with automatic state tracking.
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
/// let provider = AbusiveexperiencereportProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AbusiveexperiencereportProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AbusiveexperiencereportProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AbusiveexperiencereportProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Abusiveexperiencereport sites get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SiteSummaryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn abusiveexperiencereport_sites_get(
        &self,
        args: &AbusiveexperiencereportSitesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SiteSummaryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = abusiveexperiencereport_sites_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = abusiveexperiencereport_sites_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Abusiveexperiencereport violating sites list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ViolatingSitesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn abusiveexperiencereport_violating_sites_list(
        &self,
        args: &AbusiveexperiencereportViolatingSitesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ViolatingSitesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = abusiveexperiencereport_violating_sites_list_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = abusiveexperiencereport_violating_sites_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
