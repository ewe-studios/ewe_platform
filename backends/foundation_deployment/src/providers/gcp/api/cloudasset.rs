//! CloudassetProvider - State-aware cloudasset API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudasset API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudasset::{
    cloudasset_assets_list_builder, cloudasset_assets_list_task,
    cloudasset_effective_iam_policies_batch_get_builder, cloudasset_effective_iam_policies_batch_get_task,
    cloudasset_feeds_create_builder, cloudasset_feeds_create_task,
    cloudasset_feeds_delete_builder, cloudasset_feeds_delete_task,
    cloudasset_feeds_get_builder, cloudasset_feeds_get_task,
    cloudasset_feeds_list_builder, cloudasset_feeds_list_task,
    cloudasset_feeds_patch_builder, cloudasset_feeds_patch_task,
    cloudasset_operations_get_builder, cloudasset_operations_get_task,
    cloudasset_saved_queries_create_builder, cloudasset_saved_queries_create_task,
    cloudasset_saved_queries_delete_builder, cloudasset_saved_queries_delete_task,
    cloudasset_saved_queries_get_builder, cloudasset_saved_queries_get_task,
    cloudasset_saved_queries_list_builder, cloudasset_saved_queries_list_task,
    cloudasset_saved_queries_patch_builder, cloudasset_saved_queries_patch_task,
    cloudasset_analyze_iam_policy_builder, cloudasset_analyze_iam_policy_task,
    cloudasset_analyze_iam_policy_longrunning_builder, cloudasset_analyze_iam_policy_longrunning_task,
    cloudasset_analyze_move_builder, cloudasset_analyze_move_task,
    cloudasset_analyze_org_policies_builder, cloudasset_analyze_org_policies_task,
    cloudasset_analyze_org_policy_governed_assets_builder, cloudasset_analyze_org_policy_governed_assets_task,
    cloudasset_analyze_org_policy_governed_containers_builder, cloudasset_analyze_org_policy_governed_containers_task,
    cloudasset_batch_get_assets_history_builder, cloudasset_batch_get_assets_history_task,
    cloudasset_export_assets_builder, cloudasset_export_assets_task,
    cloudasset_query_assets_builder, cloudasset_query_assets_task,
    cloudasset_search_all_iam_policies_builder, cloudasset_search_all_iam_policies_task,
    cloudasset_search_all_resources_builder, cloudasset_search_all_resources_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudasset::AnalyzeIamPolicyResponse;
use crate::providers::gcp::clients::cloudasset::AnalyzeMoveResponse;
use crate::providers::gcp::clients::cloudasset::AnalyzeOrgPoliciesResponse;
use crate::providers::gcp::clients::cloudasset::AnalyzeOrgPolicyGovernedAssetsResponse;
use crate::providers::gcp::clients::cloudasset::AnalyzeOrgPolicyGovernedContainersResponse;
use crate::providers::gcp::clients::cloudasset::BatchGetAssetsHistoryResponse;
use crate::providers::gcp::clients::cloudasset::BatchGetEffectiveIamPoliciesResponse;
use crate::providers::gcp::clients::cloudasset::Empty;
use crate::providers::gcp::clients::cloudasset::Feed;
use crate::providers::gcp::clients::cloudasset::ListAssetsResponse;
use crate::providers::gcp::clients::cloudasset::ListFeedsResponse;
use crate::providers::gcp::clients::cloudasset::ListSavedQueriesResponse;
use crate::providers::gcp::clients::cloudasset::Operation;
use crate::providers::gcp::clients::cloudasset::QueryAssetsResponse;
use crate::providers::gcp::clients::cloudasset::SavedQuery;
use crate::providers::gcp::clients::cloudasset::SearchAllIamPoliciesResponse;
use crate::providers::gcp::clients::cloudasset::SearchAllResourcesResponse;
use crate::providers::gcp::clients::cloudasset::CloudassetAnalyzeIamPolicyArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetAnalyzeIamPolicyLongrunningArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetAnalyzeMoveArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetAnalyzeOrgPoliciesArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetAnalyzeOrgPolicyGovernedAssetsArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetAnalyzeOrgPolicyGovernedContainersArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetAssetsListArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetBatchGetAssetsHistoryArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetEffectiveIamPoliciesBatchGetArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetExportAssetsArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetFeedsCreateArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetFeedsDeleteArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetFeedsGetArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetFeedsListArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetFeedsPatchArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetOperationsGetArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetQueryAssetsArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetSavedQueriesCreateArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetSavedQueriesDeleteArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetSavedQueriesGetArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetSavedQueriesListArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetSavedQueriesPatchArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetSearchAllIamPoliciesArgs;
use crate::providers::gcp::clients::cloudasset::CloudassetSearchAllResourcesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudassetProvider with automatic state tracking.
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
/// let provider = CloudassetProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudassetProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudassetProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudassetProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Cloudasset assets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_assets_list(
        &self,
        args: &CloudassetAssetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_assets_list_builder(
            &self.http_client,
            &args.parent,
            &args.assetTypes,
            &args.contentType,
            &args.pageSize,
            &args.pageToken,
            &args.readTime,
            &args.relationshipTypes,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_assets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset effective iam policies batch get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetEffectiveIamPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_effective_iam_policies_batch_get(
        &self,
        args: &CloudassetEffectiveIamPoliciesBatchGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetEffectiveIamPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_effective_iam_policies_batch_get_builder(
            &self.http_client,
            &args.scope,
            &args.names,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_effective_iam_policies_batch_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset feeds create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Feed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_feeds_create(
        &self,
        args: &CloudassetFeedsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Feed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_feeds_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_feeds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset feeds delete.
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
    pub fn cloudasset_feeds_delete(
        &self,
        args: &CloudassetFeedsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_feeds_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_feeds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset feeds get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Feed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_feeds_get(
        &self,
        args: &CloudassetFeedsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Feed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_feeds_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_feeds_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset feeds list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListFeedsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_feeds_list(
        &self,
        args: &CloudassetFeedsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListFeedsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_feeds_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_feeds_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset feeds patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Feed result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_feeds_patch(
        &self,
        args: &CloudassetFeedsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Feed, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_feeds_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_feeds_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset operations get.
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
    pub fn cloudasset_operations_get(
        &self,
        args: &CloudassetOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset saved queries create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SavedQuery result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_saved_queries_create(
        &self,
        args: &CloudassetSavedQueriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_saved_queries_create_builder(
            &self.http_client,
            &args.parent,
            &args.savedQueryId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_saved_queries_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset saved queries delete.
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
    pub fn cloudasset_saved_queries_delete(
        &self,
        args: &CloudassetSavedQueriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_saved_queries_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_saved_queries_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset saved queries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SavedQuery result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_saved_queries_get(
        &self,
        args: &CloudassetSavedQueriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_saved_queries_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_saved_queries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset saved queries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSavedQueriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_saved_queries_list(
        &self,
        args: &CloudassetSavedQueriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSavedQueriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_saved_queries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_saved_queries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset saved queries patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SavedQuery result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_saved_queries_patch(
        &self,
        args: &CloudassetSavedQueriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SavedQuery, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_saved_queries_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_saved_queries_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset analyze iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeIamPolicyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_analyze_iam_policy(
        &self,
        args: &CloudassetAnalyzeIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeIamPolicyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_analyze_iam_policy_builder(
            &self.http_client,
            &args.scope,
            &args.analysisQuery.accessSelector.permissions,
            &args.analysisQuery.accessSelector.roles,
            &args.analysisQuery.conditionContext.accessTime,
            &args.analysisQuery.identitySelector.identity,
            &args.analysisQuery.options.analyzeServiceAccountImpersonation,
            &args.analysisQuery.options.expandGroups,
            &args.analysisQuery.options.expandResources,
            &args.analysisQuery.options.expandRoles,
            &args.analysisQuery.options.outputGroupEdges,
            &args.analysisQuery.options.outputResourceEdges,
            &args.analysisQuery.resourceSelector.fullResourceName,
            &args.executionTimeout,
            &args.savedAnalysisQuery,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_analyze_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset analyze iam policy longrunning.
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
    pub fn cloudasset_analyze_iam_policy_longrunning(
        &self,
        args: &CloudassetAnalyzeIamPolicyLongrunningArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_analyze_iam_policy_longrunning_builder(
            &self.http_client,
            &args.scope,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_analyze_iam_policy_longrunning_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset analyze move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeMoveResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_analyze_move(
        &self,
        args: &CloudassetAnalyzeMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeMoveResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_analyze_move_builder(
            &self.http_client,
            &args.resource,
            &args.destinationParent,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_analyze_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset analyze org policies.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeOrgPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_analyze_org_policies(
        &self,
        args: &CloudassetAnalyzeOrgPoliciesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeOrgPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_analyze_org_policies_builder(
            &self.http_client,
            &args.scope,
            &args.constraint,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_analyze_org_policies_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset analyze org policy governed assets.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeOrgPolicyGovernedAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_analyze_org_policy_governed_assets(
        &self,
        args: &CloudassetAnalyzeOrgPolicyGovernedAssetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeOrgPolicyGovernedAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_analyze_org_policy_governed_assets_builder(
            &self.http_client,
            &args.scope,
            &args.constraint,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_analyze_org_policy_governed_assets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset analyze org policy governed containers.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AnalyzeOrgPolicyGovernedContainersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudasset_analyze_org_policy_governed_containers(
        &self,
        args: &CloudassetAnalyzeOrgPolicyGovernedContainersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AnalyzeOrgPolicyGovernedContainersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_analyze_org_policy_governed_containers_builder(
            &self.http_client,
            &args.scope,
            &args.constraint,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_analyze_org_policy_governed_containers_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset batch get assets history.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchGetAssetsHistoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_batch_get_assets_history(
        &self,
        args: &CloudassetBatchGetAssetsHistoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchGetAssetsHistoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_batch_get_assets_history_builder(
            &self.http_client,
            &args.parent,
            &args.assetNames,
            &args.contentType,
            &args.readTimeWindow.endTime,
            &args.readTimeWindow.startTime,
            &args.relationshipTypes,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_batch_get_assets_history_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset export assets.
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
    pub fn cloudasset_export_assets(
        &self,
        args: &CloudassetExportAssetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_export_assets_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_export_assets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset query assets.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_query_assets(
        &self,
        args: &CloudassetQueryAssetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_query_assets_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_query_assets_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset search all iam policies.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchAllIamPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_search_all_iam_policies(
        &self,
        args: &CloudassetSearchAllIamPoliciesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchAllIamPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_search_all_iam_policies_builder(
            &self.http_client,
            &args.scope,
            &args.assetTypes,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.query,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_search_all_iam_policies_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudasset search all resources.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchAllResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudasset_search_all_resources(
        &self,
        args: &CloudassetSearchAllResourcesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchAllResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudasset_search_all_resources_builder(
            &self.http_client,
            &args.scope,
            &args.assetTypes,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.query,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudasset_search_all_resources_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
