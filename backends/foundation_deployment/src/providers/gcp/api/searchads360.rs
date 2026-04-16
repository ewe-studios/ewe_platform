//! Searchads360Provider - State-aware searchads360 API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       searchads360 API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::searchads360::{
    searchads360_customers_list_accessible_customers_builder, searchads360_customers_list_accessible_customers_task,
    searchads360_customers_custom_columns_get_builder, searchads360_customers_custom_columns_get_task,
    searchads360_customers_custom_columns_list_builder, searchads360_customers_custom_columns_list_task,
    searchads360_customers_search_ads360_search_builder, searchads360_customers_search_ads360_search_task,
    searchads360_search_ads360_fields_get_builder, searchads360_search_ads360_fields_get_task,
    searchads360_search_ads360_fields_search_builder, searchads360_search_ads360_fields_search_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::searchads360::GoogleAdsSearchads360V0ResourcesCustomColumn;
use crate::providers::gcp::clients::searchads360::GoogleAdsSearchads360V0ResourcesSearchAds360Field;
use crate::providers::gcp::clients::searchads360::GoogleAdsSearchads360V0ServicesListAccessibleCustomersResponse;
use crate::providers::gcp::clients::searchads360::GoogleAdsSearchads360V0ServicesListCustomColumnsResponse;
use crate::providers::gcp::clients::searchads360::GoogleAdsSearchads360V0ServicesSearchSearchAds360FieldsResponse;
use crate::providers::gcp::clients::searchads360::GoogleAdsSearchads360V0ServicesSearchSearchAds360Response;
use crate::providers::gcp::clients::searchads360::Searchads360CustomersCustomColumnsGetArgs;
use crate::providers::gcp::clients::searchads360::Searchads360CustomersCustomColumnsListArgs;
use crate::providers::gcp::clients::searchads360::Searchads360CustomersSearchAds360SearchArgs;
use crate::providers::gcp::clients::searchads360::Searchads360SearchAds360FieldsGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// Searchads360Provider with automatic state tracking.
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
/// let provider = Searchads360Provider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct Searchads360Provider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> Searchads360Provider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new Searchads360Provider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new Searchads360Provider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Searchads360 customers list accessible customers.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsSearchads360V0ServicesListAccessibleCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchads360_customers_list_accessible_customers(
        &self,
        args: &Searchads360CustomersListAccessibleCustomersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsSearchads360V0ServicesListAccessibleCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchads360_customers_list_accessible_customers_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = searchads360_customers_list_accessible_customers_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Searchads360 customers custom columns get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsSearchads360V0ResourcesCustomColumn result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchads360_customers_custom_columns_get(
        &self,
        args: &Searchads360CustomersCustomColumnsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsSearchads360V0ResourcesCustomColumn, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchads360_customers_custom_columns_get_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = searchads360_customers_custom_columns_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Searchads360 customers custom columns list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsSearchads360V0ServicesListCustomColumnsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchads360_customers_custom_columns_list(
        &self,
        args: &Searchads360CustomersCustomColumnsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsSearchads360V0ServicesListCustomColumnsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchads360_customers_custom_columns_list_builder(
            &self.http_client,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = searchads360_customers_custom_columns_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Searchads360 customers search ads360 search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsSearchads360V0ServicesSearchSearchAds360Response result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchads360_customers_search_ads360_search(
        &self,
        args: &Searchads360CustomersSearchAds360SearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsSearchads360V0ServicesSearchSearchAds360Response, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchads360_customers_search_ads360_search_builder(
            &self.http_client,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = searchads360_customers_search_ads360_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Searchads360 search ads360 fields get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsSearchads360V0ResourcesSearchAds360Field result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchads360_search_ads360_fields_get(
        &self,
        args: &Searchads360SearchAds360FieldsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsSearchads360V0ResourcesSearchAds360Field, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchads360_search_ads360_fields_get_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = searchads360_search_ads360_fields_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Searchads360 search ads360 fields search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleAdsSearchads360V0ServicesSearchSearchAds360FieldsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn searchads360_search_ads360_fields_search(
        &self,
        args: &Searchads360SearchAds360FieldsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleAdsSearchads360V0ServicesSearchSearchAds360FieldsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = searchads360_search_ads360_fields_search_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = searchads360_search_ads360_fields_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
