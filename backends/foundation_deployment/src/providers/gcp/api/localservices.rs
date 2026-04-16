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
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// LocalservicesProvider with automatic state tracking.
///
/// # Type Parameters
///
/// * `S` - StateStore implementation (FileStateStore, SqliteStateStore, etc.)
/// * `R` - DNS resolver type for HTTP client
///
/// # Example
///
/// ```rust
/// let state_store = FileStateStore::new("/path", "my-project", "dev");
/// let http_client = SimpleHttpClient::with_resolver(StaticSocketAddr::new(addr));
/// let client = ProviderClient::new("my-project", "dev", state_store, http_client);
/// let provider = LocalservicesProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct LocalservicesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> LocalservicesProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new LocalservicesProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new LocalservicesProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
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
            &args.endDate_day,
            &args.endDate_month,
            &args.endDate_year,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.startDate_day,
            &args.startDate_month,
            &args.startDate_year,
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
            &args.endDate_day,
            &args.endDate_month,
            &args.endDate_year,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.startDate_day,
            &args.startDate_month,
            &args.startDate_year,
        )
        .map_err(ProviderError::Api)?;

        let task = localservices_detailed_lead_reports_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
