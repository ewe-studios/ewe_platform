//! ClouddeployProvider - State-aware clouddeploy API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       clouddeploy API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::clouddeploy::{
    clouddeploy_projects_locations_custom_target_types_create_builder, clouddeploy_projects_locations_custom_target_types_create_task,
    clouddeploy_projects_locations_custom_target_types_delete_builder, clouddeploy_projects_locations_custom_target_types_delete_task,
    clouddeploy_projects_locations_custom_target_types_patch_builder, clouddeploy_projects_locations_custom_target_types_patch_task,
    clouddeploy_projects_locations_custom_target_types_set_iam_policy_builder, clouddeploy_projects_locations_custom_target_types_set_iam_policy_task,
    clouddeploy_projects_locations_delivery_pipelines_create_builder, clouddeploy_projects_locations_delivery_pipelines_create_task,
    clouddeploy_projects_locations_delivery_pipelines_delete_builder, clouddeploy_projects_locations_delivery_pipelines_delete_task,
    clouddeploy_projects_locations_delivery_pipelines_patch_builder, clouddeploy_projects_locations_delivery_pipelines_patch_task,
    clouddeploy_projects_locations_delivery_pipelines_rollback_target_builder, clouddeploy_projects_locations_delivery_pipelines_rollback_target_task,
    clouddeploy_projects_locations_delivery_pipelines_set_iam_policy_builder, clouddeploy_projects_locations_delivery_pipelines_set_iam_policy_task,
    clouddeploy_projects_locations_delivery_pipelines_test_iam_permissions_builder, clouddeploy_projects_locations_delivery_pipelines_test_iam_permissions_task,
    clouddeploy_projects_locations_delivery_pipelines_automation_runs_cancel_builder, clouddeploy_projects_locations_delivery_pipelines_automation_runs_cancel_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_create_builder, clouddeploy_projects_locations_delivery_pipelines_automations_create_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_delete_builder, clouddeploy_projects_locations_delivery_pipelines_automations_delete_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_patch_builder, clouddeploy_projects_locations_delivery_pipelines_automations_patch_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_abandon_builder, clouddeploy_projects_locations_delivery_pipelines_releases_abandon_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_create_builder, clouddeploy_projects_locations_delivery_pipelines_releases_create_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_advance_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_advance_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_approve_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_approve_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_cancel_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_cancel_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_create_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_create_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_ignore_job_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_ignore_job_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_retry_job_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_retry_job_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_terminate_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_terminate_task,
    clouddeploy_projects_locations_deploy_policies_create_builder, clouddeploy_projects_locations_deploy_policies_create_task,
    clouddeploy_projects_locations_deploy_policies_delete_builder, clouddeploy_projects_locations_deploy_policies_delete_task,
    clouddeploy_projects_locations_deploy_policies_patch_builder, clouddeploy_projects_locations_deploy_policies_patch_task,
    clouddeploy_projects_locations_deploy_policies_set_iam_policy_builder, clouddeploy_projects_locations_deploy_policies_set_iam_policy_task,
    clouddeploy_projects_locations_operations_cancel_builder, clouddeploy_projects_locations_operations_cancel_task,
    clouddeploy_projects_locations_operations_delete_builder, clouddeploy_projects_locations_operations_delete_task,
    clouddeploy_projects_locations_targets_create_builder, clouddeploy_projects_locations_targets_create_task,
    clouddeploy_projects_locations_targets_delete_builder, clouddeploy_projects_locations_targets_delete_task,
    clouddeploy_projects_locations_targets_patch_builder, clouddeploy_projects_locations_targets_patch_task,
    clouddeploy_projects_locations_targets_set_iam_policy_builder, clouddeploy_projects_locations_targets_set_iam_policy_task,
    clouddeploy_projects_locations_targets_test_iam_permissions_builder, clouddeploy_projects_locations_targets_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::clouddeploy::AbandonReleaseResponse;
use crate::providers::gcp::clients::clouddeploy::AdvanceRolloutResponse;
use crate::providers::gcp::clients::clouddeploy::ApproveRolloutResponse;
use crate::providers::gcp::clients::clouddeploy::CancelAutomationRunResponse;
use crate::providers::gcp::clients::clouddeploy::CancelRolloutResponse;
use crate::providers::gcp::clients::clouddeploy::Empty;
use crate::providers::gcp::clients::clouddeploy::IgnoreJobResponse;
use crate::providers::gcp::clients::clouddeploy::Operation;
use crate::providers::gcp::clients::clouddeploy::Policy;
use crate::providers::gcp::clients::clouddeploy::RetryJobResponse;
use crate::providers::gcp::clients::clouddeploy::RollbackTargetResponse;
use crate::providers::gcp::clients::clouddeploy::TerminateJobRunResponse;
use crate::providers::gcp::clients::clouddeploy::TestIamPermissionsResponse;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationRunsCancelArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesAbandonArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsAdvanceArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsApproveArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsCancelArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsIgnoreJobArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsJobRunsTerminateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsRetryJobArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesRollbackTargetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesTestIamPermissionsArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ClouddeployProvider with automatic state tracking.
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
/// let provider = ClouddeployProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ClouddeployProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ClouddeployProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ClouddeployProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Clouddeploy projects locations custom target types create.
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
    pub fn clouddeploy_projects_locations_custom_target_types_create(
        &self,
        args: &ClouddeployProjectsLocationsCustomTargetTypesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_custom_target_types_create_builder(
            &self.http_client,
            &args.parent,
            &args.customTargetTypeId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_custom_target_types_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types delete.
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
    pub fn clouddeploy_projects_locations_custom_target_types_delete(
        &self,
        args: &ClouddeployProjectsLocationsCustomTargetTypesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_custom_target_types_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_custom_target_types_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types patch.
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
    pub fn clouddeploy_projects_locations_custom_target_types_patch(
        &self,
        args: &ClouddeployProjectsLocationsCustomTargetTypesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_custom_target_types_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_custom_target_types_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types set iam policy.
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
    pub fn clouddeploy_projects_locations_custom_target_types_set_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsCustomTargetTypesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_custom_target_types_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_custom_target_types_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines create.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_create(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_create_builder(
            &self.http_client,
            &args.parent,
            &args.deliveryPipelineId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines delete.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_delete(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines patch.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_patch(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines rollback target.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RollbackTargetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_rollback_target(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesRollbackTargetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RollbackTargetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_rollback_target_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_rollback_target_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines set iam policy.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_set_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines test iam permissions.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_test_iam_permissions(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines automation runs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelAutomationRunResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_automation_runs_cancel(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationRunsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelAutomationRunResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automation_runs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automation_runs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines automations create.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_automations_create(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automations_create_builder(
            &self.http_client,
            &args.parent,
            &args.automationId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines automations delete.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_automations_delete(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automations_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines automations patch.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_automations_patch(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automations_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases abandon.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AbandonReleaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_abandon(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesAbandonArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AbandonReleaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_abandon_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_abandon_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases create.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_create(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_create_builder(
            &self.http_client,
            &args.parent,
            &args.overrideDeployPolicy,
            &args.releaseId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts advance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdvanceRolloutResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_advance(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsAdvanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdvanceRolloutResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_advance_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_advance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts approve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApproveRolloutResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_approve(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApproveRolloutResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelRolloutResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_cancel(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelRolloutResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts create.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_create(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_create_builder(
            &self.http_client,
            &args.parent,
            &args.overrideDeployPolicy,
            &args.requestId,
            &args.rolloutId,
            &args.startingPhaseId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts ignore job.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the IgnoreJobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_ignore_job(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsIgnoreJobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<IgnoreJobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_ignore_job_builder(
            &self.http_client,
            &args.rollout,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_ignore_job_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts retry job.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetryJobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_retry_job(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsRetryJobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetryJobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_retry_job_builder(
            &self.http_client,
            &args.rollout,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_retry_job_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts job runs terminate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TerminateJobRunResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_terminate(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsJobRunsTerminateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TerminateJobRunResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_terminate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_terminate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations deploy policies create.
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
    pub fn clouddeploy_projects_locations_deploy_policies_create(
        &self,
        args: &ClouddeployProjectsLocationsDeployPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_deploy_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.deployPolicyId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_deploy_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations deploy policies delete.
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
    pub fn clouddeploy_projects_locations_deploy_policies_delete(
        &self,
        args: &ClouddeployProjectsLocationsDeployPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_deploy_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_deploy_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations deploy policies patch.
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
    pub fn clouddeploy_projects_locations_deploy_policies_patch(
        &self,
        args: &ClouddeployProjectsLocationsDeployPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_deploy_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_deploy_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations deploy policies set iam policy.
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
    pub fn clouddeploy_projects_locations_deploy_policies_set_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsDeployPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_deploy_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_deploy_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations operations cancel.
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
    pub fn clouddeploy_projects_locations_operations_cancel(
        &self,
        args: &ClouddeployProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations operations delete.
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
    pub fn clouddeploy_projects_locations_operations_delete(
        &self,
        args: &ClouddeployProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets create.
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
    pub fn clouddeploy_projects_locations_targets_create(
        &self,
        args: &ClouddeployProjectsLocationsTargetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.targetId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets delete.
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
    pub fn clouddeploy_projects_locations_targets_delete(
        &self,
        args: &ClouddeployProjectsLocationsTargetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets patch.
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
    pub fn clouddeploy_projects_locations_targets_patch(
        &self,
        args: &ClouddeployProjectsLocationsTargetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets set iam policy.
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
    pub fn clouddeploy_projects_locations_targets_set_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsTargetsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets test iam permissions.
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
    pub fn clouddeploy_projects_locations_targets_test_iam_permissions(
        &self,
        args: &ClouddeployProjectsLocationsTargetsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
