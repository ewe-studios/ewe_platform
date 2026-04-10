//! BigqueryProvider - State-aware bigquery API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       bigquery API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::bigquery::{
    bigquery_datasets_delete_builder, bigquery_datasets_delete_task,
    bigquery_datasets_get_builder, bigquery_datasets_get_task,
    bigquery_datasets_insert_builder, bigquery_datasets_insert_task,
    bigquery_datasets_list_builder, bigquery_datasets_list_task,
    bigquery_datasets_patch_builder, bigquery_datasets_patch_task,
    bigquery_datasets_undelete_builder, bigquery_datasets_undelete_task,
    bigquery_datasets_update_builder, bigquery_datasets_update_task,
    bigquery_jobs_cancel_builder, bigquery_jobs_cancel_task,
    bigquery_jobs_delete_builder, bigquery_jobs_delete_task,
    bigquery_jobs_get_builder, bigquery_jobs_get_task,
    bigquery_jobs_get_query_results_builder, bigquery_jobs_get_query_results_task,
    bigquery_jobs_insert_builder, bigquery_jobs_insert_task,
    bigquery_jobs_list_builder, bigquery_jobs_list_task,
    bigquery_jobs_query_builder, bigquery_jobs_query_task,
    bigquery_models_delete_builder, bigquery_models_delete_task,
    bigquery_models_get_builder, bigquery_models_get_task,
    bigquery_models_list_builder, bigquery_models_list_task,
    bigquery_models_patch_builder, bigquery_models_patch_task,
    bigquery_projects_get_service_account_builder, bigquery_projects_get_service_account_task,
    bigquery_projects_list_builder, bigquery_projects_list_task,
    bigquery_routines_delete_builder, bigquery_routines_delete_task,
    bigquery_routines_get_builder, bigquery_routines_get_task,
    bigquery_routines_get_iam_policy_builder, bigquery_routines_get_iam_policy_task,
    bigquery_routines_insert_builder, bigquery_routines_insert_task,
    bigquery_routines_list_builder, bigquery_routines_list_task,
    bigquery_routines_set_iam_policy_builder, bigquery_routines_set_iam_policy_task,
    bigquery_routines_test_iam_permissions_builder, bigquery_routines_test_iam_permissions_task,
    bigquery_routines_update_builder, bigquery_routines_update_task,
    bigquery_row_access_policies_batch_delete_builder, bigquery_row_access_policies_batch_delete_task,
    bigquery_row_access_policies_delete_builder, bigquery_row_access_policies_delete_task,
    bigquery_row_access_policies_get_builder, bigquery_row_access_policies_get_task,
    bigquery_row_access_policies_get_iam_policy_builder, bigquery_row_access_policies_get_iam_policy_task,
    bigquery_row_access_policies_insert_builder, bigquery_row_access_policies_insert_task,
    bigquery_row_access_policies_list_builder, bigquery_row_access_policies_list_task,
    bigquery_row_access_policies_test_iam_permissions_builder, bigquery_row_access_policies_test_iam_permissions_task,
    bigquery_row_access_policies_update_builder, bigquery_row_access_policies_update_task,
    bigquery_tabledata_insert_all_builder, bigquery_tabledata_insert_all_task,
    bigquery_tabledata_list_builder, bigquery_tabledata_list_task,
    bigquery_tables_delete_builder, bigquery_tables_delete_task,
    bigquery_tables_get_builder, bigquery_tables_get_task,
    bigquery_tables_get_iam_policy_builder, bigquery_tables_get_iam_policy_task,
    bigquery_tables_insert_builder, bigquery_tables_insert_task,
    bigquery_tables_list_builder, bigquery_tables_list_task,
    bigquery_tables_patch_builder, bigquery_tables_patch_task,
    bigquery_tables_set_iam_policy_builder, bigquery_tables_set_iam_policy_task,
    bigquery_tables_test_iam_permissions_builder, bigquery_tables_test_iam_permissions_task,
    bigquery_tables_update_builder, bigquery_tables_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigquery::Dataset;
use crate::providers::gcp::clients::bigquery::DatasetList;
use crate::providers::gcp::clients::bigquery::GetQueryResultsResponse;
use crate::providers::gcp::clients::bigquery::GetServiceAccountResponse;
use crate::providers::gcp::clients::bigquery::Job;
use crate::providers::gcp::clients::bigquery::JobCancelResponse;
use crate::providers::gcp::clients::bigquery::JobList;
use crate::providers::gcp::clients::bigquery::ListModelsResponse;
use crate::providers::gcp::clients::bigquery::ListRoutinesResponse;
use crate::providers::gcp::clients::bigquery::ListRowAccessPoliciesResponse;
use crate::providers::gcp::clients::bigquery::Model;
use crate::providers::gcp::clients::bigquery::Policy;
use crate::providers::gcp::clients::bigquery::ProjectList;
use crate::providers::gcp::clients::bigquery::QueryResponse;
use crate::providers::gcp::clients::bigquery::Routine;
use crate::providers::gcp::clients::bigquery::RowAccessPolicy;
use crate::providers::gcp::clients::bigquery::Table;
use crate::providers::gcp::clients::bigquery::TableDataInsertAllResponse;
use crate::providers::gcp::clients::bigquery::TableDataList;
use crate::providers::gcp::clients::bigquery::TableList;
use crate::providers::gcp::clients::bigquery::TestIamPermissionsResponse;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsGetArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsPatchArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsUndeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsUpdateArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsCancelArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsGetArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsGetQueryResultsArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsQueryArgs;
use crate::providers::gcp::clients::bigquery::BigqueryModelsDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryModelsGetArgs;
use crate::providers::gcp::clients::bigquery::BigqueryModelsListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryModelsPatchArgs;
use crate::providers::gcp::clients::bigquery::BigqueryProjectsGetServiceAccountArgs;
use crate::providers::gcp::clients::bigquery::BigqueryProjectsListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesGetArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesUpdateArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesBatchDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesGetArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesUpdateArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTabledataInsertAllArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTabledataListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesGetArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesListArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesPatchArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BigqueryProvider with automatic state tracking.
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
/// let provider = BigqueryProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BigqueryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BigqueryProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BigqueryProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Bigquery datasets delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_datasets_delete(
        &self,
        args: &BigqueryDatasetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_datasets_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.deleteContents,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_datasets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_datasets_get(
        &self,
        args: &BigqueryDatasetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_datasets_get_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.accessPolicyVersion,
            &args.datasetView,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_datasets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquery_datasets_insert(
        &self,
        args: &BigqueryDatasetsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_datasets_insert_builder(
            &self.http_client,
            &args.projectId,
            &args.accessPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_datasets_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatasetList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_datasets_list(
        &self,
        args: &BigqueryDatasetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatasetList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_datasets_list_builder(
            &self.http_client,
            &args.projectId,
            &args.all,
            &args.filter,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_datasets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_datasets_patch(
        &self,
        args: &BigqueryDatasetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_datasets_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.accessPolicyVersion,
            &args.updateMode,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_datasets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets undelete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_datasets_undelete(
        &self,
        args: &BigqueryDatasetsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_datasets_undelete_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_datasets_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Dataset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_datasets_update(
        &self,
        args: &BigqueryDatasetsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Dataset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_datasets_update_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.accessPolicyVersion,
            &args.updateMode,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_datasets_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs cancel.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JobCancelResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_jobs_cancel(
        &self,
        args: &BigqueryJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JobCancelResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_jobs_cancel_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_jobs_delete(
        &self,
        args: &BigqueryJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_jobs_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_jobs_get(
        &self,
        args: &BigqueryJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_jobs_get_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs get query results.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetQueryResultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_jobs_get_query_results(
        &self,
        args: &BigqueryJobsGetQueryResultsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetQueryResultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_jobs_get_query_results_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
            &args.formatOptions.timestampOutputFormat,
            &args.formatOptions.useInt64Timestamp,
            &args.location,
            &args.maxResults,
            &args.pageToken,
            &args.startIndex,
            &args.timeoutMs,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_jobs_get_query_results_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquery_jobs_insert(
        &self,
        args: &BigqueryJobsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_jobs_insert_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_jobs_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JobList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_jobs_list(
        &self,
        args: &BigqueryJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JobList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_jobs_list_builder(
            &self.http_client,
            &args.projectId,
            &args.allUsers,
            &args.maxCreationTime,
            &args.maxResults,
            &args.minCreationTime,
            &args.pageToken,
            &args.parentJobId,
            &args.projection,
            &args.stateFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs query.
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
    pub fn bigquery_jobs_query(
        &self,
        args: &BigqueryJobsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_jobs_query_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_jobs_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery models delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_models_delete(
        &self,
        args: &BigqueryModelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_models_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.modelId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_models_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery models get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Model result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_models_get(
        &self,
        args: &BigqueryModelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Model, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_models_get_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.modelId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_models_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery models list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListModelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_models_list(
        &self,
        args: &BigqueryModelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListModelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_models_list_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_models_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery models patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Model result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_models_patch(
        &self,
        args: &BigqueryModelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Model, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_models_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.modelId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_models_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery projects get service account.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetServiceAccountResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_projects_get_service_account(
        &self,
        args: &BigqueryProjectsGetServiceAccountArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetServiceAccountResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_projects_get_service_account_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_projects_get_service_account_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery projects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_projects_list(
        &self,
        args: &BigqueryProjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_projects_list_builder(
            &self.http_client,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_projects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_routines_delete(
        &self,
        args: &BigqueryRoutinesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.routineId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Routine result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_routines_get(
        &self,
        args: &BigqueryRoutinesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Routine, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_get_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.routineId,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_routines_get_iam_policy(
        &self,
        args: &BigqueryRoutinesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Routine result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquery_routines_insert(
        &self,
        args: &BigqueryRoutinesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Routine, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_insert_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRoutinesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_routines_list(
        &self,
        args: &BigqueryRoutinesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRoutinesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_list_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.filter,
            &args.maxResults,
            &args.pageToken,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines set iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_routines_set_iam_policy(
        &self,
        args: &BigqueryRoutinesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_routines_test_iam_permissions(
        &self,
        args: &BigqueryRoutinesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Routine result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_routines_update(
        &self,
        args: &BigqueryRoutinesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Routine, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_routines_update_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.routineId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_routines_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies batch delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_row_access_policies_batch_delete(
        &self,
        args: &BigqueryRowAccessPoliciesBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_batch_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_row_access_policies_delete(
        &self,
        args: &BigqueryRowAccessPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.policyId,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RowAccessPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_row_access_policies_get(
        &self,
        args: &BigqueryRowAccessPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RowAccessPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_get_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.policyId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_row_access_policies_get_iam_policy(
        &self,
        args: &BigqueryRowAccessPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RowAccessPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquery_row_access_policies_insert(
        &self,
        args: &BigqueryRowAccessPoliciesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RowAccessPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_insert_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRowAccessPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_row_access_policies_list(
        &self,
        args: &BigqueryRowAccessPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRowAccessPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_list_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_row_access_policies_test_iam_permissions(
        &self,
        args: &BigqueryRowAccessPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RowAccessPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_row_access_policies_update(
        &self,
        args: &BigqueryRowAccessPoliciesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RowAccessPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_row_access_policies_update_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.policyId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_row_access_policies_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tabledata insert all.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TableDataInsertAllResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquery_tabledata_insert_all(
        &self,
        args: &BigqueryTabledataInsertAllArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TableDataInsertAllResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tabledata_insert_all_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tabledata_insert_all_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tabledata list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TableDataList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tabledata_list(
        &self,
        args: &BigqueryTabledataListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TableDataList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tabledata_list_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.formatOptions.timestampOutputFormat,
            &args.formatOptions.useInt64Timestamp,
            &args.maxResults,
            &args.pageToken,
            &args.selectedFields,
            &args.startIndex,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tabledata_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_delete(
        &self,
        args: &BigqueryTablesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_get(
        &self,
        args: &BigqueryTablesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_get_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.selectedFields,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_get_iam_policy(
        &self,
        args: &BigqueryTablesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigquery_tables_insert(
        &self,
        args: &BigqueryTablesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_insert_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TableList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_list(
        &self,
        args: &BigqueryTablesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TableList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_list_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_patch(
        &self,
        args: &BigqueryTablesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.autodetect_schema,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables set iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_set_iam_policy(
        &self,
        args: &BigqueryTablesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_test_iam_permissions(
        &self,
        args: &BigqueryTablesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigquery_tables_update(
        &self,
        args: &BigqueryTablesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigquery_tables_update_builder(
            &self.http_client,
            &args.projectId,
            &args.datasetId,
            &args.tableId,
            &args.autodetect_schema,
        )
        .map_err(ProviderError::Api)?;

        let task = bigquery_tables_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
