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
    bigquery_datasets_insert_builder, bigquery_datasets_insert_task,
    bigquery_datasets_patch_builder, bigquery_datasets_patch_task,
    bigquery_datasets_undelete_builder, bigquery_datasets_undelete_task,
    bigquery_datasets_update_builder, bigquery_datasets_update_task,
    bigquery_jobs_cancel_builder, bigquery_jobs_cancel_task,
    bigquery_jobs_delete_builder, bigquery_jobs_delete_task,
    bigquery_jobs_insert_builder, bigquery_jobs_insert_task,
    bigquery_jobs_query_builder, bigquery_jobs_query_task,
    bigquery_models_delete_builder, bigquery_models_delete_task,
    bigquery_models_patch_builder, bigquery_models_patch_task,
    bigquery_routines_delete_builder, bigquery_routines_delete_task,
    bigquery_routines_get_iam_policy_builder, bigquery_routines_get_iam_policy_task,
    bigquery_routines_insert_builder, bigquery_routines_insert_task,
    bigquery_routines_set_iam_policy_builder, bigquery_routines_set_iam_policy_task,
    bigquery_routines_test_iam_permissions_builder, bigquery_routines_test_iam_permissions_task,
    bigquery_routines_update_builder, bigquery_routines_update_task,
    bigquery_row_access_policies_batch_delete_builder, bigquery_row_access_policies_batch_delete_task,
    bigquery_row_access_policies_delete_builder, bigquery_row_access_policies_delete_task,
    bigquery_row_access_policies_get_iam_policy_builder, bigquery_row_access_policies_get_iam_policy_task,
    bigquery_row_access_policies_insert_builder, bigquery_row_access_policies_insert_task,
    bigquery_row_access_policies_test_iam_permissions_builder, bigquery_row_access_policies_test_iam_permissions_task,
    bigquery_row_access_policies_update_builder, bigquery_row_access_policies_update_task,
    bigquery_tabledata_insert_all_builder, bigquery_tabledata_insert_all_task,
    bigquery_tables_delete_builder, bigquery_tables_delete_task,
    bigquery_tables_get_iam_policy_builder, bigquery_tables_get_iam_policy_task,
    bigquery_tables_insert_builder, bigquery_tables_insert_task,
    bigquery_tables_patch_builder, bigquery_tables_patch_task,
    bigquery_tables_set_iam_policy_builder, bigquery_tables_set_iam_policy_task,
    bigquery_tables_test_iam_permissions_builder, bigquery_tables_test_iam_permissions_task,
    bigquery_tables_update_builder, bigquery_tables_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigquery::Dataset;
use crate::providers::gcp::clients::bigquery::Job;
use crate::providers::gcp::clients::bigquery::JobCancelResponse;
use crate::providers::gcp::clients::bigquery::Model;
use crate::providers::gcp::clients::bigquery::Policy;
use crate::providers::gcp::clients::bigquery::QueryResponse;
use crate::providers::gcp::clients::bigquery::Routine;
use crate::providers::gcp::clients::bigquery::RowAccessPolicy;
use crate::providers::gcp::clients::bigquery::Table;
use crate::providers::gcp::clients::bigquery::TableDataInsertAllResponse;
use crate::providers::gcp::clients::bigquery::TestIamPermissionsResponse;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsPatchArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsUndeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryDatasetsUpdateArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsCancelArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryJobsQueryArgs;
use crate::providers::gcp::clients::bigquery::BigqueryModelsDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryModelsPatchArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRoutinesUpdateArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesBatchDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesInsertArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigquery::BigqueryRowAccessPoliciesUpdateArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTabledataInsertAllArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesDeleteArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigquery::BigqueryTablesInsertArgs;
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Bigquery datasets patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets undelete.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery datasets update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery jobs delete.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Bigquery jobs query.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery models delete.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery models patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines delete.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines get iam policy.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Bigquery routines set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery routines update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies batch delete.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies delete.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies get iam policy.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Bigquery row access policies test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery row access policies update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Bigquery tables delete.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables get iam policy.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Bigquery tables patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigquery tables update.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
