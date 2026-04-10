//! CloudlocationfinderProvider - State-aware cloudlocationfinder API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudlocationfinder API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudlocationfinder::{
    cloudlocationfinder_projects_locations_get_builder, cloudlocationfinder_projects_locations_get_task,
    cloudlocationfinder_projects_locations_list_builder, cloudlocationfinder_projects_locations_list_task,
    cloudlocationfinder_projects_locations_cloud_locations_get_builder, cloudlocationfinder_projects_locations_cloud_locations_get_task,
    cloudlocationfinder_projects_locations_cloud_locations_list_builder, cloudlocationfinder_projects_locations_cloud_locations_list_task,
    cloudlocationfinder_projects_locations_cloud_locations_search_builder, cloudlocationfinder_projects_locations_cloud_locations_search_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudlocationfinder::CloudLocation;
use crate::providers::gcp::clients::cloudlocationfinder::ListCloudLocationsResponse;
use crate::providers::gcp::clients::cloudlocationfinder::ListLocationsResponse;
use crate::providers::gcp::clients::cloudlocationfinder::Location;
use crate::providers::gcp::clients::cloudlocationfinder::SearchCloudLocationsResponse;
use crate::providers::gcp::clients::cloudlocationfinder::CloudlocationfinderProjectsLocationsCloudLocationsGetArgs;
use crate::providers::gcp::clients::cloudlocationfinder::CloudlocationfinderProjectsLocationsCloudLocationsListArgs;
use crate::providers::gcp::clients::cloudlocationfinder::CloudlocationfinderProjectsLocationsCloudLocationsSearchArgs;
use crate::providers::gcp::clients::cloudlocationfinder::CloudlocationfinderProjectsLocationsGetArgs;
use crate::providers::gcp::clients::cloudlocationfinder::CloudlocationfinderProjectsLocationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudlocationfinderProvider with automatic state tracking.
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
/// let provider = CloudlocationfinderProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudlocationfinderProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudlocationfinderProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudlocationfinderProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Cloudlocationfinder projects locations get.
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
    pub fn cloudlocationfinder_projects_locations_get(
        &self,
        args: &CloudlocationfinderProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudlocationfinder_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudlocationfinder_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudlocationfinder projects locations list.
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
    pub fn cloudlocationfinder_projects_locations_list(
        &self,
        args: &CloudlocationfinderProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudlocationfinder_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudlocationfinder_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudlocationfinder projects locations cloud locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CloudLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudlocationfinder_projects_locations_cloud_locations_get(
        &self,
        args: &CloudlocationfinderProjectsLocationsCloudLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CloudLocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudlocationfinder_projects_locations_cloud_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudlocationfinder_projects_locations_cloud_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudlocationfinder projects locations cloud locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCloudLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudlocationfinder_projects_locations_cloud_locations_list(
        &self,
        args: &CloudlocationfinderProjectsLocationsCloudLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCloudLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudlocationfinder_projects_locations_cloud_locations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudlocationfinder_projects_locations_cloud_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudlocationfinder projects locations cloud locations search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchCloudLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudlocationfinder_projects_locations_cloud_locations_search(
        &self,
        args: &CloudlocationfinderProjectsLocationsCloudLocationsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchCloudLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudlocationfinder_projects_locations_cloud_locations_search_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.sourceCloudLocation,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudlocationfinder_projects_locations_cloud_locations_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
