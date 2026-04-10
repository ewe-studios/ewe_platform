//! YoutubeAnalyticsProvider - State-aware youtubeAnalytics API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       youtubeAnalytics API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::youtubeAnalytics::{
    youtube_analytics_group_items_delete_builder, youtube_analytics_group_items_delete_task,
    youtube_analytics_group_items_insert_builder, youtube_analytics_group_items_insert_task,
    youtube_analytics_group_items_list_builder, youtube_analytics_group_items_list_task,
    youtube_analytics_groups_delete_builder, youtube_analytics_groups_delete_task,
    youtube_analytics_groups_insert_builder, youtube_analytics_groups_insert_task,
    youtube_analytics_groups_list_builder, youtube_analytics_groups_list_task,
    youtube_analytics_groups_update_builder, youtube_analytics_groups_update_task,
    youtube_analytics_reports_query_builder, youtube_analytics_reports_query_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::youtubeAnalytics::EmptyResponse;
use crate::providers::gcp::clients::youtubeAnalytics::Group;
use crate::providers::gcp::clients::youtubeAnalytics::GroupItem;
use crate::providers::gcp::clients::youtubeAnalytics::ListGroupItemsResponse;
use crate::providers::gcp::clients::youtubeAnalytics::ListGroupsResponse;
use crate::providers::gcp::clients::youtubeAnalytics::QueryResponse;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsGroupItemsDeleteArgs;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsGroupItemsInsertArgs;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsGroupItemsListArgs;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsGroupsDeleteArgs;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsGroupsInsertArgs;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsGroupsListArgs;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsGroupsUpdateArgs;
use crate::providers::gcp::clients::youtubeAnalytics::YoutubeAnalyticsReportsQueryArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// YoutubeAnalyticsProvider with automatic state tracking.
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
/// let provider = YoutubeAnalyticsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct YoutubeAnalyticsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> YoutubeAnalyticsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new YoutubeAnalyticsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Youtube analytics group items delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmptyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_analytics_group_items_delete(
        &self,
        args: &YoutubeAnalyticsGroupItemsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmptyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_group_items_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_group_items_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube analytics group items insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GroupItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_analytics_group_items_insert(
        &self,
        args: &YoutubeAnalyticsGroupItemsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GroupItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_group_items_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_group_items_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube analytics group items list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupItemsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_analytics_group_items_list(
        &self,
        args: &YoutubeAnalyticsGroupItemsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupItemsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_group_items_list_builder(
            &self.http_client,
            &args.groupId,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_group_items_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube analytics groups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EmptyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_analytics_groups_delete(
        &self,
        args: &YoutubeAnalyticsGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EmptyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_groups_delete_builder(
            &self.http_client,
            &args.id,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube analytics groups insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_analytics_groups_insert(
        &self,
        args: &YoutubeAnalyticsGroupsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_groups_insert_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_groups_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube analytics groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_analytics_groups_list(
        &self,
        args: &YoutubeAnalyticsGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_groups_list_builder(
            &self.http_client,
            &args.id,
            &args.mine,
            &args.onBehalfOfContentOwner,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube analytics groups update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtube_analytics_groups_update(
        &self,
        args: &YoutubeAnalyticsGroupsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_groups_update_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_groups_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtube analytics reports query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtube_analytics_reports_query(
        &self,
        args: &YoutubeAnalyticsReportsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtube_analytics_reports_query_builder(
            &self.http_client,
            &args.currency,
            &args.dimensions,
            &args.endDate,
            &args.filters,
            &args.ids,
            &args.includeHistoricalChannelData,
            &args.maxResults,
            &args.metrics,
            &args.sort,
            &args.startDate,
            &args.startIndex,
        )
        .map_err(ProviderError::Api)?;

        let task = youtube_analytics_reports_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
