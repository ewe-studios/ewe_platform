//! LocalservicesProvider - State-aware localservices API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       localservices API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::localservices::{
    localservices_account_reports_search_builder, localservices_account_reports_search_task,
    localservices_detailed_lead_reports_search_builder, localservices_detailed_lead_reports_search_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::localservices::GoogleAdsHomeservicesLocalservicesV1SearchAccountReportsResponse;
use crate::providers::gcp::clients::localservices::GoogleAdsHomeservicesLocalservicesV1SearchDetailedLeadReportsResponse;
use crate::providers::gcp::clients::localservices::LocalservicesAccountReportsSearchArgs;
use crate::providers::gcp::clients::localservices::LocalservicesDetailedLeadReportsSearchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// LocalservicesProvider with automatic state tracking.
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
/// let provider = LocalservicesProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct LocalservicesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> LocalservicesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new LocalservicesProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Localservices account reports search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsHomeservicesLocalservicesV1SearchAccountReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn localservices_account_reports_search(
        &self,
        args: &LocalservicesAccountReportsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsHomeservicesLocalservicesV1SearchAccountReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = localservices_account_reports_search_builder(
            &self.http_client,
            &args.endDate.day,
            &args.endDate.month,
            &args.endDate.year,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.startDate.day,
            &args.startDate.month,
            &args.startDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = localservices_account_reports_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Localservices detailed lead reports search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsHomeservicesLocalservicesV1SearchDetailedLeadReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn localservices_detailed_lead_reports_search(
        &self,
        args: &LocalservicesDetailedLeadReportsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsHomeservicesLocalservicesV1SearchDetailedLeadReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = localservices_detailed_lead_reports_search_builder(
            &self.http_client,
            &args.endDate.day,
            &args.endDate.month,
            &args.endDate.year,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.startDate.day,
            &args.startDate.month,
            &args.startDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = localservices_detailed_lead_reports_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
