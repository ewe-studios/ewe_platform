//! MybusinessbusinessinformationProvider - State-aware mybusinessbusinessinformation API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       mybusinessbusinessinformation API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::mybusinessbusinessinformation::{
    mybusinessbusinessinformation_accounts_locations_create_builder, mybusinessbusinessinformation_accounts_locations_create_task,
    mybusinessbusinessinformation_accounts_locations_list_builder, mybusinessbusinessinformation_accounts_locations_list_task,
    mybusinessbusinessinformation_attributes_list_builder, mybusinessbusinessinformation_attributes_list_task,
    mybusinessbusinessinformation_categories_batch_get_builder, mybusinessbusinessinformation_categories_batch_get_task,
    mybusinessbusinessinformation_categories_list_builder, mybusinessbusinessinformation_categories_list_task,
    mybusinessbusinessinformation_chains_get_builder, mybusinessbusinessinformation_chains_get_task,
    mybusinessbusinessinformation_chains_search_builder, mybusinessbusinessinformation_chains_search_task,
    mybusinessbusinessinformation_google_locations_search_builder, mybusinessbusinessinformation_google_locations_search_task,
    mybusinessbusinessinformation_locations_delete_builder, mybusinessbusinessinformation_locations_delete_task,
    mybusinessbusinessinformation_locations_get_builder, mybusinessbusinessinformation_locations_get_task,
    mybusinessbusinessinformation_locations_get_attributes_builder, mybusinessbusinessinformation_locations_get_attributes_task,
    mybusinessbusinessinformation_locations_get_google_updated_builder, mybusinessbusinessinformation_locations_get_google_updated_task,
    mybusinessbusinessinformation_locations_patch_builder, mybusinessbusinessinformation_locations_patch_task,
    mybusinessbusinessinformation_locations_update_attributes_builder, mybusinessbusinessinformation_locations_update_attributes_task,
    mybusinessbusinessinformation_locations_attributes_get_google_updated_builder, mybusinessbusinessinformation_locations_attributes_get_google_updated_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::mybusinessbusinessinformation::Attributes;
use crate::providers::gcp::clients::mybusinessbusinessinformation::BatchGetCategoriesResponse;
use crate::providers::gcp::clients::mybusinessbusinessinformation::Chain;
use crate::providers::gcp::clients::mybusinessbusinessinformation::Empty;
use crate::providers::gcp::clients::mybusinessbusinessinformation::GoogleUpdatedLocation;
use crate::providers::gcp::clients::mybusinessbusinessinformation::ListAttributeMetadataResponse;
use crate::providers::gcp::clients::mybusinessbusinessinformation::ListCategoriesResponse;
use crate::providers::gcp::clients::mybusinessbusinessinformation::ListLocationsResponse;
use crate::providers::gcp::clients::mybusinessbusinessinformation::Location;
use crate::providers::gcp::clients::mybusinessbusinessinformation::SearchChainsResponse;
use crate::providers::gcp::clients::mybusinessbusinessinformation::SearchGoogleLocationsResponse;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationAccountsLocationsCreateArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationAccountsLocationsListArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationAttributesListArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationCategoriesBatchGetArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationCategoriesListArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationChainsGetArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationChainsSearchArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsAttributesGetGoogleUpdatedArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsDeleteArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsGetArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsGetAttributesArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsGetGoogleUpdatedArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsPatchArgs;
use crate::providers::gcp::clients::mybusinessbusinessinformation::MybusinessbusinessinformationLocationsUpdateAttributesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MybusinessbusinessinformationProvider with automatic state tracking.
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
/// let provider = MybusinessbusinessinformationProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct MybusinessbusinessinformationProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> MybusinessbusinessinformationProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new MybusinessbusinessinformationProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new MybusinessbusinessinformationProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Mybusinessbusinessinformation accounts locations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessbusinessinformation_accounts_locations_create(
        &self,
        args: &MybusinessbusinessinformationAccountsLocationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_accounts_locations_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_accounts_locations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation accounts locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_accounts_locations_list(
        &self,
        args: &MybusinessbusinessinformationAccountsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_accounts_locations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_accounts_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation attributes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAttributeMetadataResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_attributes_list(
        &self,
        args: &MybusinessbusinessinformationAttributesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAttributeMetadataResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_attributes_list_builder(
            &self.http_client,
            &args.categoryName,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
            &args.regionCode,
            &args.showAll,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_attributes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation categories batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetCategoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_categories_batch_get(
        &self,
        args: &MybusinessbusinessinformationCategoriesBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetCategoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_categories_batch_get_builder(
            &self.http_client,
            &args.languageCode,
            &args.names,
            &args.regionCode,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_categories_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation categories list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCategoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_categories_list(
        &self,
        args: &MybusinessbusinessinformationCategoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCategoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_categories_list_builder(
            &self.http_client,
            &args.filter,
            &args.languageCode,
            &args.pageSize,
            &args.pageToken,
            &args.regionCode,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_categories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation chains get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Chain result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_chains_get(
        &self,
        args: &MybusinessbusinessinformationChainsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Chain, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_chains_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_chains_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation chains search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchChainsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_chains_search(
        &self,
        args: &MybusinessbusinessinformationChainsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchChainsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_chains_search_builder(
            &self.http_client,
            &args.chainName,
            &args.pageSize,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_chains_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation google locations search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchGoogleLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_google_locations_search(
        &self,
        args: &MybusinessbusinessinformationGoogleLocationsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchGoogleLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_google_locations_search_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_google_locations_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations delete.
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
    pub fn mybusinessbusinessinformation_locations_delete(
        &self,
        args: &MybusinessbusinessinformationLocationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_locations_get(
        &self,
        args: &MybusinessbusinessinformationLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_get_builder(
            &self.http_client,
            &args.name,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations get attributes.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attributes result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_locations_get_attributes(
        &self,
        args: &MybusinessbusinessinformationLocationsGetAttributesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attributes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_get_attributes_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_get_attributes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations get google updated.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleUpdatedLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_locations_get_google_updated(
        &self,
        args: &MybusinessbusinessinformationLocationsGetGoogleUpdatedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleUpdatedLocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_get_google_updated_builder(
            &self.http_client,
            &args.name,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_get_google_updated_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessbusinessinformation_locations_patch(
        &self,
        args: &MybusinessbusinessinformationLocationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations update attributes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attributes result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessbusinessinformation_locations_update_attributes(
        &self,
        args: &MybusinessbusinessinformationLocationsUpdateAttributesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attributes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_update_attributes_builder(
            &self.http_client,
            &args.name,
            &args.attributeMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_update_attributes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessbusinessinformation locations attributes get google updated.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Attributes result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessbusinessinformation_locations_attributes_get_google_updated(
        &self,
        args: &MybusinessbusinessinformationLocationsAttributesGetGoogleUpdatedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Attributes, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessbusinessinformation_locations_attributes_get_google_updated_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessbusinessinformation_locations_attributes_get_google_updated_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
