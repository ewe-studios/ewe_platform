//! GmailpostmastertoolsProvider - State-aware gmailpostmastertools API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       gmailpostmastertools API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::gmailpostmastertools::{
    gmailpostmastertools_domain_stats_batch_query_builder, gmailpostmastertools_domain_stats_batch_query_task,
    gmailpostmastertools_domains_get_builder, gmailpostmastertools_domains_get_task,
    gmailpostmastertools_domains_get_compliance_status_builder, gmailpostmastertools_domains_get_compliance_status_task,
    gmailpostmastertools_domains_list_builder, gmailpostmastertools_domains_list_task,
    gmailpostmastertools_domains_domain_stats_query_builder, gmailpostmastertools_domains_domain_stats_query_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gmailpostmastertools::BatchQueryDomainStatsResponse;
use crate::providers::gcp::clients::gmailpostmastertools::Domain;
use crate::providers::gcp::clients::gmailpostmastertools::DomainComplianceStatus;
use crate::providers::gcp::clients::gmailpostmastertools::ListDomainsResponse;
use crate::providers::gcp::clients::gmailpostmastertools::QueryDomainStatsResponse;
use crate::providers::gcp::clients::gmailpostmastertools::GmailpostmastertoolsDomainStatsBatchQueryArgs;
use crate::providers::gcp::clients::gmailpostmastertools::GmailpostmastertoolsDomainsDomainStatsQueryArgs;
use crate::providers::gcp::clients::gmailpostmastertools::GmailpostmastertoolsDomainsGetArgs;
use crate::providers::gcp::clients::gmailpostmastertools::GmailpostmastertoolsDomainsGetComplianceStatusArgs;
use crate::providers::gcp::clients::gmailpostmastertools::GmailpostmastertoolsDomainsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GmailpostmastertoolsProvider with automatic state tracking.
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
/// let provider = GmailpostmastertoolsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GmailpostmastertoolsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GmailpostmastertoolsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GmailpostmastertoolsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Gmailpostmastertools domain stats batch query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchQueryDomainStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmailpostmastertools_domain_stats_batch_query(
        &self,
        args: &GmailpostmastertoolsDomainStatsBatchQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchQueryDomainStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmailpostmastertools_domain_stats_batch_query_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = gmailpostmastertools_domain_stats_batch_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmailpostmastertools domains get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Domain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmailpostmastertools_domains_get(
        &self,
        args: &GmailpostmastertoolsDomainsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Domain, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmailpostmastertools_domains_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gmailpostmastertools_domains_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmailpostmastertools domains get compliance status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DomainComplianceStatus result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmailpostmastertools_domains_get_compliance_status(
        &self,
        args: &GmailpostmastertoolsDomainsGetComplianceStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DomainComplianceStatus, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmailpostmastertools_domains_get_compliance_status_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gmailpostmastertools_domains_get_compliance_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmailpostmastertools domains list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDomainsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmailpostmastertools_domains_list(
        &self,
        args: &GmailpostmastertoolsDomainsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDomainsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmailpostmastertools_domains_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gmailpostmastertools_domains_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gmailpostmastertools domains domain stats query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryDomainStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gmailpostmastertools_domains_domain_stats_query(
        &self,
        args: &GmailpostmastertoolsDomainsDomainStatsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryDomainStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gmailpostmastertools_domains_domain_stats_query_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gmailpostmastertools_domains_domain_stats_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
