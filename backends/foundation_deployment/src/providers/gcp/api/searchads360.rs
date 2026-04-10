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
use crate::providers::gcp::clients::searchads360::Searchads360CustomersListAccessibleCustomersArgs;
use crate::providers::gcp::clients::searchads360::Searchads360CustomersSearchAds360SearchArgs;
use crate::providers::gcp::clients::searchads360::Searchads360SearchAds360FieldsGetArgs;
use crate::providers::gcp::clients::searchads360::Searchads360SearchAds360FieldsSearchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// Searchads360Provider with automatic state tracking.
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
/// let provider = Searchads360Provider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct Searchads360Provider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> Searchads360Provider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new Searchads360Provider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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
