//! MarketingplatformadminProvider - State-aware marketingplatformadmin API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       marketingplatformadmin API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::marketingplatformadmin::{
    marketingplatformadmin_organizations_find_sales_partner_managed_clients_builder, marketingplatformadmin_organizations_find_sales_partner_managed_clients_task,
    marketingplatformadmin_organizations_get_builder, marketingplatformadmin_organizations_get_task,
    marketingplatformadmin_organizations_list_builder, marketingplatformadmin_organizations_list_task,
    marketingplatformadmin_organizations_report_property_usage_builder, marketingplatformadmin_organizations_report_property_usage_task,
    marketingplatformadmin_organizations_analytics_account_links_create_builder, marketingplatformadmin_organizations_analytics_account_links_create_task,
    marketingplatformadmin_organizations_analytics_account_links_delete_builder, marketingplatformadmin_organizations_analytics_account_links_delete_task,
    marketingplatformadmin_organizations_analytics_account_links_list_builder, marketingplatformadmin_organizations_analytics_account_links_list_task,
    marketingplatformadmin_organizations_analytics_account_links_set_property_service_level_builder, marketingplatformadmin_organizations_analytics_account_links_set_property_service_level_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::marketingplatformadmin::AnalyticsAccountLink;
use crate::providers::gcp::clients::marketingplatformadmin::Empty;
use crate::providers::gcp::clients::marketingplatformadmin::FindSalesPartnerManagedClientsResponse;
use crate::providers::gcp::clients::marketingplatformadmin::ListAnalyticsAccountLinksResponse;
use crate::providers::gcp::clients::marketingplatformadmin::ListOrganizationsResponse;
use crate::providers::gcp::clients::marketingplatformadmin::Organization;
use crate::providers::gcp::clients::marketingplatformadmin::ReportPropertyUsageResponse;
use crate::providers::gcp::clients::marketingplatformadmin::SetPropertyServiceLevelResponse;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsAnalyticsAccountLinksCreateArgs;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsAnalyticsAccountLinksDeleteArgs;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsAnalyticsAccountLinksListArgs;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsAnalyticsAccountLinksSetPropertyServiceLevelArgs;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsFindSalesPartnerManagedClientsArgs;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsGetArgs;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsListArgs;
use crate::providers::gcp::clients::marketingplatformadmin::MarketingplatformadminOrganizationsReportPropertyUsageArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MarketingplatformadminProvider with automatic state tracking.
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
/// let provider = MarketingplatformadminProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct MarketingplatformadminProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> MarketingplatformadminProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new MarketingplatformadminProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new MarketingplatformadminProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Marketingplatformadmin organizations find sales partner managed clients.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FindSalesPartnerManagedClientsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn marketingplatformadmin_organizations_find_sales_partner_managed_clients(
        &self,
        args: &MarketingplatformadminOrganizationsFindSalesPartnerManagedClientsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FindSalesPartnerManagedClientsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_find_sales_partner_managed_clients_builder(
            &self.http_client,
            &args.organization,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_find_sales_partner_managed_clients_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Marketingplatformadmin organizations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Organization result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn marketingplatformadmin_organizations_get(
        &self,
        args: &MarketingplatformadminOrganizationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Organization, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Marketingplatformadmin organizations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOrganizationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn marketingplatformadmin_organizations_list(
        &self,
        args: &MarketingplatformadminOrganizationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOrganizationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Marketingplatformadmin organizations report property usage.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportPropertyUsageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn marketingplatformadmin_organizations_report_property_usage(
        &self,
        args: &MarketingplatformadminOrganizationsReportPropertyUsageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportPropertyUsageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_report_property_usage_builder(
            &self.http_client,
            &args.organization,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_report_property_usage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Marketingplatformadmin organizations analytics account links create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyticsAccountLink result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn marketingplatformadmin_organizations_analytics_account_links_create(
        &self,
        args: &MarketingplatformadminOrganizationsAnalyticsAccountLinksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyticsAccountLink, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_analytics_account_links_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_analytics_account_links_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Marketingplatformadmin organizations analytics account links delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn marketingplatformadmin_organizations_analytics_account_links_delete(
        &self,
        args: &MarketingplatformadminOrganizationsAnalyticsAccountLinksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_analytics_account_links_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_analytics_account_links_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Marketingplatformadmin organizations analytics account links list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAnalyticsAccountLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn marketingplatformadmin_organizations_analytics_account_links_list(
        &self,
        args: &MarketingplatformadminOrganizationsAnalyticsAccountLinksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAnalyticsAccountLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_analytics_account_links_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_analytics_account_links_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Marketingplatformadmin organizations analytics account links set property service level.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetPropertyServiceLevelResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn marketingplatformadmin_organizations_analytics_account_links_set_property_service_level(
        &self,
        args: &MarketingplatformadminOrganizationsAnalyticsAccountLinksSetPropertyServiceLevelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetPropertyServiceLevelResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = marketingplatformadmin_organizations_analytics_account_links_set_property_service_level_builder(
            &self.http_client,
            &args.analyticsAccountLink,
        )
        .map_err(ProviderError::Api)?;

        let task = marketingplatformadmin_organizations_analytics_account_links_set_property_service_level_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
