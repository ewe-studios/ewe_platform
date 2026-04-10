//! VersionhistoryProvider - State-aware versionhistory API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       versionhistory API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::versionhistory::{
    versionhistory_platforms_list_builder, versionhistory_platforms_list_task,
    versionhistory_platforms_channels_list_builder, versionhistory_platforms_channels_list_task,
    versionhistory_platforms_channels_versions_list_builder, versionhistory_platforms_channels_versions_list_task,
    versionhistory_platforms_channels_versions_releases_list_builder, versionhistory_platforms_channels_versions_releases_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::versionhistory::ListChannelsResponse;
use crate::providers::gcp::clients::versionhistory::ListPlatformsResponse;
use crate::providers::gcp::clients::versionhistory::ListReleasesResponse;
use crate::providers::gcp::clients::versionhistory::ListVersionsResponse;
use crate::providers::gcp::clients::versionhistory::VersionhistoryPlatformsChannelsListArgs;
use crate::providers::gcp::clients::versionhistory::VersionhistoryPlatformsChannelsVersionsListArgs;
use crate::providers::gcp::clients::versionhistory::VersionhistoryPlatformsChannelsVersionsReleasesListArgs;
use crate::providers::gcp::clients::versionhistory::VersionhistoryPlatformsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// VersionhistoryProvider with automatic state tracking.
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
/// let provider = VersionhistoryProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct VersionhistoryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> VersionhistoryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new VersionhistoryProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Versionhistory platforms list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPlatformsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn versionhistory_platforms_list(
        &self,
        args: &VersionhistoryPlatformsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPlatformsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = versionhistory_platforms_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = versionhistory_platforms_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Versionhistory platforms channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn versionhistory_platforms_channels_list(
        &self,
        args: &VersionhistoryPlatformsChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = versionhistory_platforms_channels_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = versionhistory_platforms_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Versionhistory platforms channels versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn versionhistory_platforms_channels_versions_list(
        &self,
        args: &VersionhistoryPlatformsChannelsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = versionhistory_platforms_channels_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = versionhistory_platforms_channels_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Versionhistory platforms channels versions releases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReleasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn versionhistory_platforms_channels_versions_releases_list(
        &self,
        args: &VersionhistoryPlatformsChannelsVersionsReleasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReleasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = versionhistory_platforms_channels_versions_releases_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = versionhistory_platforms_channels_versions_releases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
