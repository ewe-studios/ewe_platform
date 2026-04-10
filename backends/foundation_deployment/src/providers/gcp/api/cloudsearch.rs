//! CloudsearchProvider - State-aware cloudsearch API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudsearch API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudsearch::{
    cloudsearch_debug_datasources_items_check_access_builder, cloudsearch_debug_datasources_items_check_access_task,
    cloudsearch_debug_datasources_items_search_by_view_url_builder, cloudsearch_debug_datasources_items_search_by_view_url_task,
    cloudsearch_debug_datasources_items_unmappedids_list_builder, cloudsearch_debug_datasources_items_unmappedids_list_task,
    cloudsearch_debug_identitysources_items_list_forunmappedidentity_builder, cloudsearch_debug_identitysources_items_list_forunmappedidentity_task,
    cloudsearch_debug_identitysources_unmappedids_list_builder, cloudsearch_debug_identitysources_unmappedids_list_task,
    cloudsearch_indexing_datasources_delete_schema_builder, cloudsearch_indexing_datasources_delete_schema_task,
    cloudsearch_indexing_datasources_get_schema_builder, cloudsearch_indexing_datasources_get_schema_task,
    cloudsearch_indexing_datasources_update_schema_builder, cloudsearch_indexing_datasources_update_schema_task,
    cloudsearch_indexing_datasources_items_delete_builder, cloudsearch_indexing_datasources_items_delete_task,
    cloudsearch_indexing_datasources_items_delete_queue_items_builder, cloudsearch_indexing_datasources_items_delete_queue_items_task,
    cloudsearch_indexing_datasources_items_get_builder, cloudsearch_indexing_datasources_items_get_task,
    cloudsearch_indexing_datasources_items_index_builder, cloudsearch_indexing_datasources_items_index_task,
    cloudsearch_indexing_datasources_items_list_builder, cloudsearch_indexing_datasources_items_list_task,
    cloudsearch_indexing_datasources_items_poll_builder, cloudsearch_indexing_datasources_items_poll_task,
    cloudsearch_indexing_datasources_items_push_builder, cloudsearch_indexing_datasources_items_push_task,
    cloudsearch_indexing_datasources_items_unreserve_builder, cloudsearch_indexing_datasources_items_unreserve_task,
    cloudsearch_indexing_datasources_items_upload_builder, cloudsearch_indexing_datasources_items_upload_task,
    cloudsearch_media_upload_builder, cloudsearch_media_upload_task,
    cloudsearch_operations_get_builder, cloudsearch_operations_get_task,
    cloudsearch_operations_lro_list_builder, cloudsearch_operations_lro_list_task,
    cloudsearch_query_remove_activity_builder, cloudsearch_query_remove_activity_task,
    cloudsearch_query_search_builder, cloudsearch_query_search_task,
    cloudsearch_query_suggest_builder, cloudsearch_query_suggest_task,
    cloudsearch_query_sources_list_builder, cloudsearch_query_sources_list_task,
    cloudsearch_settings_get_customer_builder, cloudsearch_settings_get_customer_task,
    cloudsearch_settings_update_customer_builder, cloudsearch_settings_update_customer_task,
    cloudsearch_settings_datasources_create_builder, cloudsearch_settings_datasources_create_task,
    cloudsearch_settings_datasources_delete_builder, cloudsearch_settings_datasources_delete_task,
    cloudsearch_settings_datasources_get_builder, cloudsearch_settings_datasources_get_task,
    cloudsearch_settings_datasources_list_builder, cloudsearch_settings_datasources_list_task,
    cloudsearch_settings_datasources_patch_builder, cloudsearch_settings_datasources_patch_task,
    cloudsearch_settings_datasources_update_builder, cloudsearch_settings_datasources_update_task,
    cloudsearch_settings_searchapplications_create_builder, cloudsearch_settings_searchapplications_create_task,
    cloudsearch_settings_searchapplications_delete_builder, cloudsearch_settings_searchapplications_delete_task,
    cloudsearch_settings_searchapplications_get_builder, cloudsearch_settings_searchapplications_get_task,
    cloudsearch_settings_searchapplications_list_builder, cloudsearch_settings_searchapplications_list_task,
    cloudsearch_settings_searchapplications_patch_builder, cloudsearch_settings_searchapplications_patch_task,
    cloudsearch_settings_searchapplications_reset_builder, cloudsearch_settings_searchapplications_reset_task,
    cloudsearch_settings_searchapplications_update_builder, cloudsearch_settings_searchapplications_update_task,
    cloudsearch_stats_get_index_builder, cloudsearch_stats_get_index_task,
    cloudsearch_stats_get_query_builder, cloudsearch_stats_get_query_task,
    cloudsearch_stats_get_searchapplication_builder, cloudsearch_stats_get_searchapplication_task,
    cloudsearch_stats_get_session_builder, cloudsearch_stats_get_session_task,
    cloudsearch_stats_get_user_builder, cloudsearch_stats_get_user_task,
    cloudsearch_stats_index_datasources_get_builder, cloudsearch_stats_index_datasources_get_task,
    cloudsearch_stats_query_searchapplications_get_builder, cloudsearch_stats_query_searchapplications_get_task,
    cloudsearch_stats_session_searchapplications_get_builder, cloudsearch_stats_session_searchapplications_get_task,
    cloudsearch_stats_user_searchapplications_get_builder, cloudsearch_stats_user_searchapplications_get_task,
    cloudsearch_initialize_customer_builder, cloudsearch_initialize_customer_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudsearch::CheckAccessResponse;
use crate::providers::gcp::clients::cloudsearch::CustomerSettings;
use crate::providers::gcp::clients::cloudsearch::DataSource;
use crate::providers::gcp::clients::cloudsearch::GetCustomerIndexStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetCustomerQueryStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetCustomerSearchApplicationStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetCustomerSessionStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetCustomerUserStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetDataSourceIndexStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetSearchApplicationQueryStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetSearchApplicationSessionStatsResponse;
use crate::providers::gcp::clients::cloudsearch::GetSearchApplicationUserStatsResponse;
use crate::providers::gcp::clients::cloudsearch::Item;
use crate::providers::gcp::clients::cloudsearch::ListDataSourceResponse;
use crate::providers::gcp::clients::cloudsearch::ListItemNamesForUnmappedIdentityResponse;
use crate::providers::gcp::clients::cloudsearch::ListItemsResponse;
use crate::providers::gcp::clients::cloudsearch::ListOperationsResponse;
use crate::providers::gcp::clients::cloudsearch::ListQuerySourcesResponse;
use crate::providers::gcp::clients::cloudsearch::ListSearchApplicationsResponse;
use crate::providers::gcp::clients::cloudsearch::ListUnmappedIdentitiesResponse;
use crate::providers::gcp::clients::cloudsearch::Media;
use crate::providers::gcp::clients::cloudsearch::Operation;
use crate::providers::gcp::clients::cloudsearch::PollItemsResponse;
use crate::providers::gcp::clients::cloudsearch::RemoveActivityResponse;
use crate::providers::gcp::clients::cloudsearch::Schema;
use crate::providers::gcp::clients::cloudsearch::SearchApplication;
use crate::providers::gcp::clients::cloudsearch::SearchItemsByViewUrlResponse;
use crate::providers::gcp::clients::cloudsearch::SearchResponse;
use crate::providers::gcp::clients::cloudsearch::SuggestResponse;
use crate::providers::gcp::clients::cloudsearch::UploadItemRef;
use crate::providers::gcp::clients::cloudsearch::CloudsearchDebugDatasourcesItemsCheckAccessArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchDebugDatasourcesItemsSearchByViewUrlArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchDebugDatasourcesItemsUnmappedidsListArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchDebugIdentitysourcesItemsListForunmappedidentityArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchDebugIdentitysourcesUnmappedidsListArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesDeleteSchemaArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesGetSchemaArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsDeleteArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsDeleteQueueItemsArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsGetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsIndexArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsListArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsPollArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsPushArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsUnreserveArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesItemsUploadArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchIndexingDatasourcesUpdateSchemaArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchInitializeCustomerArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchMediaUploadArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchOperationsGetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchOperationsLroListArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchQueryRemoveActivityArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchQuerySearchArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchQuerySourcesListArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchQuerySuggestArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsDatasourcesCreateArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsDatasourcesDeleteArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsDatasourcesGetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsDatasourcesListArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsDatasourcesPatchArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsDatasourcesUpdateArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsGetCustomerArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsSearchapplicationsCreateArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsSearchapplicationsDeleteArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsSearchapplicationsGetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsSearchapplicationsListArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsSearchapplicationsPatchArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsSearchapplicationsResetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsSearchapplicationsUpdateArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchSettingsUpdateCustomerArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsGetIndexArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsGetQueryArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsGetSearchapplicationArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsGetSessionArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsGetUserArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsIndexDatasourcesGetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsQuerySearchapplicationsGetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsSessionSearchapplicationsGetArgs;
use crate::providers::gcp::clients::cloudsearch::CloudsearchStatsUserSearchapplicationsGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudsearchProvider with automatic state tracking.
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
/// let provider = CloudsearchProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudsearchProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudsearchProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudsearchProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Cloudsearch debug datasources items check access.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckAccessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_debug_datasources_items_check_access(
        &self,
        args: &CloudsearchDebugDatasourcesItemsCheckAccessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckAccessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_debug_datasources_items_check_access_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_debug_datasources_items_check_access_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch debug datasources items search by view url.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchItemsByViewUrlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_debug_datasources_items_search_by_view_url(
        &self,
        args: &CloudsearchDebugDatasourcesItemsSearchByViewUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchItemsByViewUrlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_debug_datasources_items_search_by_view_url_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_debug_datasources_items_search_by_view_url_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch debug datasources items unmappedids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUnmappedIdentitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_debug_datasources_items_unmappedids_list(
        &self,
        args: &CloudsearchDebugDatasourcesItemsUnmappedidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUnmappedIdentitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_debug_datasources_items_unmappedids_list_builder(
            &self.http_client,
            &args.parent,
            &args.debugOptions.enableDebugging,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_debug_datasources_items_unmappedids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch debug identitysources items list forunmappedidentity.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListItemNamesForUnmappedIdentityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_debug_identitysources_items_list_forunmappedidentity(
        &self,
        args: &CloudsearchDebugIdentitysourcesItemsListForunmappedidentityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListItemNamesForUnmappedIdentityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_debug_identitysources_items_list_forunmappedidentity_builder(
            &self.http_client,
            &args.parent,
            &args.debugOptions.enableDebugging,
            &args.groupResourceName,
            &args.pageSize,
            &args.pageToken,
            &args.userResourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_debug_identitysources_items_list_forunmappedidentity_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch debug identitysources unmappedids list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUnmappedIdentitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_debug_identitysources_unmappedids_list(
        &self,
        args: &CloudsearchDebugIdentitysourcesUnmappedidsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUnmappedIdentitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_debug_identitysources_unmappedids_list_builder(
            &self.http_client,
            &args.parent,
            &args.debugOptions.enableDebugging,
            &args.pageSize,
            &args.pageToken,
            &args.resolutionStatusCode,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_debug_identitysources_unmappedids_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources delete schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_delete_schema(
        &self,
        args: &CloudsearchIndexingDatasourcesDeleteSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_delete_schema_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_delete_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources get schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_get_schema(
        &self,
        args: &CloudsearchIndexingDatasourcesGetSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_get_schema_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_get_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources update schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_update_schema(
        &self,
        args: &CloudsearchIndexingDatasourcesUpdateSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_update_schema_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_update_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_delete(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_delete_builder(
            &self.http_client,
            &args.name,
            &args.connectorName,
            &args.debugOptions.enableDebugging,
            &args.mode,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items delete queue items.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_delete_queue_items(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsDeleteQueueItemsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_delete_queue_items_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_delete_queue_items_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Item result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_get(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Item, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_get_builder(
            &self.http_client,
            &args.name,
            &args.connectorName,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items index.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_index(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsIndexArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_index_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_index_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListItemsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_list(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListItemsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_list_builder(
            &self.http_client,
            &args.name,
            &args.brief,
            &args.connectorName,
            &args.debugOptions.enableDebugging,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items poll.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PollItemsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_poll(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsPollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PollItemsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_poll_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_poll_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items push.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Item result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_push(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsPushArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Item, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_push_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_push_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items unreserve.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_unreserve(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsUnreserveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_unreserve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_unreserve_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch indexing datasources items upload.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UploadItemRef result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_indexing_datasources_items_upload(
        &self,
        args: &CloudsearchIndexingDatasourcesItemsUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UploadItemRef, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_indexing_datasources_items_upload_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_indexing_datasources_items_upload_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch media upload.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Media result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_media_upload(
        &self,
        args: &CloudsearchMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Media, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_media_upload_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_operations_get(
        &self,
        args: &CloudsearchOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch operations lro list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_operations_lro_list(
        &self,
        args: &CloudsearchOperationsLroListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_operations_lro_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_operations_lro_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch query remove activity.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoveActivityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_query_remove_activity(
        &self,
        args: &CloudsearchQueryRemoveActivityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoveActivityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_query_remove_activity_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_query_remove_activity_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch query search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_query_search(
        &self,
        args: &CloudsearchQuerySearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_query_search_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_query_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch query suggest.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SuggestResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_query_suggest(
        &self,
        args: &CloudsearchQuerySuggestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SuggestResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_query_suggest_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_query_suggest_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch query sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListQuerySourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_query_sources_list(
        &self,
        args: &CloudsearchQuerySourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListQuerySourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_query_sources_list_builder(
            &self.http_client,
            &args.pageToken,
            &args.requestOptions.debugOptions.enableDebugging,
            &args.requestOptions.languageCode,
            &args.requestOptions.searchApplicationId,
            &args.requestOptions.timeZone,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_query_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings get customer.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_get_customer(
        &self,
        args: &CloudsearchSettingsGetCustomerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_get_customer_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_get_customer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings update customer.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_update_customer(
        &self,
        args: &CloudsearchSettingsUpdateCustomerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_update_customer_builder(
            &self.http_client,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_update_customer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings datasources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudsearch_settings_datasources_create(
        &self,
        args: &CloudsearchSettingsDatasourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_datasources_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_datasources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings datasources delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_datasources_delete(
        &self,
        args: &CloudsearchSettingsDatasourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_datasources_delete_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_datasources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings datasources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_datasources_get(
        &self,
        args: &CloudsearchSettingsDatasourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_datasources_get_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_datasources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings datasources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataSourceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_datasources_list(
        &self,
        args: &CloudsearchSettingsDatasourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataSourceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_datasources_list_builder(
            &self.http_client,
            &args.debugOptions.enableDebugging,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_datasources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings datasources patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_datasources_patch(
        &self,
        args: &CloudsearchSettingsDatasourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_datasources_patch_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_datasources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings datasources update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_datasources_update(
        &self,
        args: &CloudsearchSettingsDatasourcesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_datasources_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_datasources_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings searchapplications create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudsearch_settings_searchapplications_create(
        &self,
        args: &CloudsearchSettingsSearchapplicationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_searchapplications_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_searchapplications_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings searchapplications delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_searchapplications_delete(
        &self,
        args: &CloudsearchSettingsSearchapplicationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_searchapplications_delete_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_searchapplications_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings searchapplications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchApplication result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_searchapplications_get(
        &self,
        args: &CloudsearchSettingsSearchapplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchApplication, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_searchapplications_get_builder(
            &self.http_client,
            &args.name,
            &args.debugOptions.enableDebugging,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_searchapplications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings searchapplications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSearchApplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_searchapplications_list(
        &self,
        args: &CloudsearchSettingsSearchapplicationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSearchApplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_searchapplications_list_builder(
            &self.http_client,
            &args.debugOptions.enableDebugging,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_searchapplications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings searchapplications patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_searchapplications_patch(
        &self,
        args: &CloudsearchSettingsSearchapplicationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_searchapplications_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_searchapplications_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings searchapplications reset.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_searchapplications_reset(
        &self,
        args: &CloudsearchSettingsSearchapplicationsResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_searchapplications_reset_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_searchapplications_reset_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch settings searchapplications update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_settings_searchapplications_update(
        &self,
        args: &CloudsearchSettingsSearchapplicationsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_settings_searchapplications_update_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_settings_searchapplications_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats get index.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetCustomerIndexStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_get_index(
        &self,
        args: &CloudsearchStatsGetIndexArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetCustomerIndexStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_get_index_builder(
            &self.http_client,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_get_index_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats get query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetCustomerQueryStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_get_query(
        &self,
        args: &CloudsearchStatsGetQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetCustomerQueryStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_get_query_builder(
            &self.http_client,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_get_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats get searchapplication.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetCustomerSearchApplicationStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_get_searchapplication(
        &self,
        args: &CloudsearchStatsGetSearchapplicationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetCustomerSearchApplicationStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_get_searchapplication_builder(
            &self.http_client,
            &args.endDate.day,
            &args.endDate.month,
            &args.endDate.year,
            &args.startDate.day,
            &args.startDate.month,
            &args.startDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_get_searchapplication_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats get session.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetCustomerSessionStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_get_session(
        &self,
        args: &CloudsearchStatsGetSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetCustomerSessionStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_get_session_builder(
            &self.http_client,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_get_session_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats get user.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetCustomerUserStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_get_user(
        &self,
        args: &CloudsearchStatsGetUserArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetCustomerUserStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_get_user_builder(
            &self.http_client,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_get_user_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats index datasources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetDataSourceIndexStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_index_datasources_get(
        &self,
        args: &CloudsearchStatsIndexDatasourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetDataSourceIndexStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_index_datasources_get_builder(
            &self.http_client,
            &args.name,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_index_datasources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats query searchapplications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetSearchApplicationQueryStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_query_searchapplications_get(
        &self,
        args: &CloudsearchStatsQuerySearchapplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetSearchApplicationQueryStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_query_searchapplications_get_builder(
            &self.http_client,
            &args.name,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_query_searchapplications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats session searchapplications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetSearchApplicationSessionStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_session_searchapplications_get(
        &self,
        args: &CloudsearchStatsSessionSearchapplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetSearchApplicationSessionStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_session_searchapplications_get_builder(
            &self.http_client,
            &args.name,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_session_searchapplications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch stats user searchapplications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetSearchApplicationUserStatsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_stats_user_searchapplications_get(
        &self,
        args: &CloudsearchStatsUserSearchapplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetSearchApplicationUserStatsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_stats_user_searchapplications_get_builder(
            &self.http_client,
            &args.name,
            &args.fromDate.day,
            &args.fromDate.month,
            &args.fromDate.year,
            &args.toDate.day,
            &args.toDate.month,
            &args.toDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_stats_user_searchapplications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudsearch initialize customer.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudsearch_initialize_customer(
        &self,
        args: &CloudsearchInitializeCustomerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudsearch_initialize_customer_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudsearch_initialize_customer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
