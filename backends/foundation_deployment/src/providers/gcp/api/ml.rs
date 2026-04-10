//! MlProvider - State-aware ml API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       ml API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::ml::{
    ml_projects_explain_builder, ml_projects_explain_task,
    ml_projects_get_config_builder, ml_projects_get_config_task,
    ml_projects_predict_builder, ml_projects_predict_task,
    ml_projects_jobs_cancel_builder, ml_projects_jobs_cancel_task,
    ml_projects_jobs_create_builder, ml_projects_jobs_create_task,
    ml_projects_jobs_get_builder, ml_projects_jobs_get_task,
    ml_projects_jobs_get_iam_policy_builder, ml_projects_jobs_get_iam_policy_task,
    ml_projects_jobs_list_builder, ml_projects_jobs_list_task,
    ml_projects_jobs_patch_builder, ml_projects_jobs_patch_task,
    ml_projects_jobs_set_iam_policy_builder, ml_projects_jobs_set_iam_policy_task,
    ml_projects_jobs_test_iam_permissions_builder, ml_projects_jobs_test_iam_permissions_task,
    ml_projects_locations_get_builder, ml_projects_locations_get_task,
    ml_projects_locations_list_builder, ml_projects_locations_list_task,
    ml_projects_locations_operations_cancel_builder, ml_projects_locations_operations_cancel_task,
    ml_projects_locations_operations_get_builder, ml_projects_locations_operations_get_task,
    ml_projects_locations_studies_create_builder, ml_projects_locations_studies_create_task,
    ml_projects_locations_studies_delete_builder, ml_projects_locations_studies_delete_task,
    ml_projects_locations_studies_get_builder, ml_projects_locations_studies_get_task,
    ml_projects_locations_studies_list_builder, ml_projects_locations_studies_list_task,
    ml_projects_locations_studies_trials_add_measurement_builder, ml_projects_locations_studies_trials_add_measurement_task,
    ml_projects_locations_studies_trials_check_early_stopping_state_builder, ml_projects_locations_studies_trials_check_early_stopping_state_task,
    ml_projects_locations_studies_trials_complete_builder, ml_projects_locations_studies_trials_complete_task,
    ml_projects_locations_studies_trials_create_builder, ml_projects_locations_studies_trials_create_task,
    ml_projects_locations_studies_trials_delete_builder, ml_projects_locations_studies_trials_delete_task,
    ml_projects_locations_studies_trials_get_builder, ml_projects_locations_studies_trials_get_task,
    ml_projects_locations_studies_trials_list_builder, ml_projects_locations_studies_trials_list_task,
    ml_projects_locations_studies_trials_list_optimal_trials_builder, ml_projects_locations_studies_trials_list_optimal_trials_task,
    ml_projects_locations_studies_trials_stop_builder, ml_projects_locations_studies_trials_stop_task,
    ml_projects_locations_studies_trials_suggest_builder, ml_projects_locations_studies_trials_suggest_task,
    ml_projects_models_create_builder, ml_projects_models_create_task,
    ml_projects_models_delete_builder, ml_projects_models_delete_task,
    ml_projects_models_get_builder, ml_projects_models_get_task,
    ml_projects_models_get_iam_policy_builder, ml_projects_models_get_iam_policy_task,
    ml_projects_models_list_builder, ml_projects_models_list_task,
    ml_projects_models_patch_builder, ml_projects_models_patch_task,
    ml_projects_models_set_iam_policy_builder, ml_projects_models_set_iam_policy_task,
    ml_projects_models_test_iam_permissions_builder, ml_projects_models_test_iam_permissions_task,
    ml_projects_models_versions_create_builder, ml_projects_models_versions_create_task,
    ml_projects_models_versions_delete_builder, ml_projects_models_versions_delete_task,
    ml_projects_models_versions_get_builder, ml_projects_models_versions_get_task,
    ml_projects_models_versions_list_builder, ml_projects_models_versions_list_task,
    ml_projects_models_versions_patch_builder, ml_projects_models_versions_patch_task,
    ml_projects_models_versions_set_default_builder, ml_projects_models_versions_set_default_task,
    ml_projects_operations_cancel_builder, ml_projects_operations_cancel_task,
    ml_projects_operations_get_builder, ml_projects_operations_get_task,
    ml_projects_operations_list_builder, ml_projects_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::ml::GoogleApiHttpBody;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1GetConfigResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1Job;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1ListJobsResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1ListLocationsResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1ListModelsResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1ListOptimalTrialsResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1ListStudiesResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1ListTrialsResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1ListVersionsResponse;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1Location;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1Model;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1Study;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1Trial;
use crate::providers::gcp::clients::ml::GoogleCloudMlV1Version;
use crate::providers::gcp::clients::ml::GoogleIamV1Policy;
use crate::providers::gcp::clients::ml::GoogleIamV1TestIamPermissionsResponse;
use crate::providers::gcp::clients::ml::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::ml::GoogleLongrunningOperation;
use crate::providers::gcp::clients::ml::GoogleProtobufEmpty;
use crate::providers::gcp::clients::ml::MlProjectsExplainArgs;
use crate::providers::gcp::clients::ml::MlProjectsGetConfigArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsCancelArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsCreateArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsGetIamPolicyArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsListArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsPatchArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsSetIamPolicyArgs;
use crate::providers::gcp::clients::ml::MlProjectsJobsTestIamPermissionsArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsListArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesCreateArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesDeleteArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesListArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsAddMeasurementArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsCheckEarlyStoppingStateArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsCompleteArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsCreateArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsDeleteArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsListArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsListOptimalTrialsArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsStopArgs;
use crate::providers::gcp::clients::ml::MlProjectsLocationsStudiesTrialsSuggestArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsCreateArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsDeleteArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsGetIamPolicyArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsListArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsPatchArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsSetIamPolicyArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsTestIamPermissionsArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsVersionsCreateArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsVersionsDeleteArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsVersionsGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsVersionsListArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsVersionsPatchArgs;
use crate::providers::gcp::clients::ml::MlProjectsModelsVersionsSetDefaultArgs;
use crate::providers::gcp::clients::ml::MlProjectsOperationsCancelArgs;
use crate::providers::gcp::clients::ml::MlProjectsOperationsGetArgs;
use crate::providers::gcp::clients::ml::MlProjectsOperationsListArgs;
use crate::providers::gcp::clients::ml::MlProjectsPredictArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MlProvider with automatic state tracking.
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
/// let provider = MlProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MlProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MlProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MlProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Ml projects explain.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleApiHttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_explain(
        &self,
        args: &MlProjectsExplainArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleApiHttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_explain_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_explain_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects get config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1GetConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_get_config(
        &self,
        args: &MlProjectsGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1GetConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects predict.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleApiHttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_predict(
        &self,
        args: &MlProjectsPredictArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleApiHttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_predict_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_predict_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_jobs_cancel(
        &self,
        args: &MlProjectsJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_jobs_create(
        &self,
        args: &MlProjectsJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_jobs_get(
        &self,
        args: &MlProjectsJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_jobs_get_iam_policy(
        &self,
        args: &MlProjectsJobsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_jobs_list(
        &self,
        args: &MlProjectsJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_jobs_patch(
        &self,
        args: &MlProjectsJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_jobs_set_iam_policy(
        &self,
        args: &MlProjectsJobsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects jobs test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_jobs_test_iam_permissions(
        &self,
        args: &MlProjectsJobsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_jobs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_jobs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_get(
        &self,
        args: &MlProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_list(
        &self,
        args: &MlProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_operations_cancel(
        &self,
        args: &MlProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_operations_get(
        &self,
        args: &MlProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Study result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_create(
        &self,
        args: &MlProjectsLocationsStudiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Study, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_create_builder(
            &self.http_client,
            &args.parent,
            &args.studyId,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_delete(
        &self,
        args: &MlProjectsLocationsStudiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Study result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_studies_get(
        &self,
        args: &MlProjectsLocationsStudiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Study, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1ListStudiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_studies_list(
        &self,
        args: &MlProjectsLocationsStudiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1ListStudiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials add measurement.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Trial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_trials_add_measurement(
        &self,
        args: &MlProjectsLocationsStudiesTrialsAddMeasurementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_add_measurement_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_add_measurement_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials check early stopping state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_trials_check_early_stopping_state(
        &self,
        args: &MlProjectsLocationsStudiesTrialsCheckEarlyStoppingStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_check_early_stopping_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_check_early_stopping_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials complete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Trial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_trials_complete(
        &self,
        args: &MlProjectsLocationsStudiesTrialsCompleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_complete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_complete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Trial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_trials_create(
        &self,
        args: &MlProjectsLocationsStudiesTrialsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_trials_delete(
        &self,
        args: &MlProjectsLocationsStudiesTrialsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Trial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_studies_trials_get(
        &self,
        args: &MlProjectsLocationsStudiesTrialsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1ListTrialsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_studies_trials_list(
        &self,
        args: &MlProjectsLocationsStudiesTrialsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1ListTrialsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials list optimal trials.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1ListOptimalTrialsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_locations_studies_trials_list_optimal_trials(
        &self,
        args: &MlProjectsLocationsStudiesTrialsListOptimalTrialsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1ListOptimalTrialsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_list_optimal_trials_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_list_optimal_trials_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Trial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_trials_stop(
        &self,
        args: &MlProjectsLocationsStudiesTrialsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects locations studies trials suggest.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_locations_studies_trials_suggest(
        &self,
        args: &MlProjectsLocationsStudiesTrialsSuggestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_locations_studies_trials_suggest_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_locations_studies_trials_suggest_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Model result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_create(
        &self,
        args: &MlProjectsModelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Model, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_delete(
        &self,
        args: &MlProjectsModelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Model result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_models_get(
        &self,
        args: &MlProjectsModelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Model, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_models_get_iam_policy(
        &self,
        args: &MlProjectsModelsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1ListModelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_models_list(
        &self,
        args: &MlProjectsModelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1ListModelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_patch(
        &self,
        args: &MlProjectsModelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_set_iam_policy(
        &self,
        args: &MlProjectsModelsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_models_test_iam_permissions(
        &self,
        args: &MlProjectsModelsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_versions_create(
        &self,
        args: &MlProjectsModelsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_versions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models versions delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_versions_delete(
        &self,
        args: &MlProjectsModelsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_models_versions_get(
        &self,
        args: &MlProjectsModelsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1ListVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_models_versions_list(
        &self,
        args: &MlProjectsModelsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1ListVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models versions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_versions_patch(
        &self,
        args: &MlProjectsModelsVersionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_versions_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_versions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects models versions set default.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudMlV1Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_models_versions_set_default(
        &self,
        args: &MlProjectsModelsVersionsSetDefaultArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudMlV1Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_models_versions_set_default_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_models_versions_set_default_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ml_projects_operations_cancel(
        &self,
        args: &MlProjectsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_operations_get(
        &self,
        args: &MlProjectsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ml projects operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ml_projects_operations_list(
        &self,
        args: &MlProjectsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ml_projects_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = ml_projects_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
