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
    clouddeploy_projects_locations_get_builder, clouddeploy_projects_locations_get_task,
    clouddeploy_projects_locations_get_config_builder, clouddeploy_projects_locations_get_config_task,
    clouddeploy_projects_locations_list_builder, clouddeploy_projects_locations_list_task,
    clouddeploy_projects_locations_custom_target_types_create_builder, clouddeploy_projects_locations_custom_target_types_create_task,
    clouddeploy_projects_locations_custom_target_types_delete_builder, clouddeploy_projects_locations_custom_target_types_delete_task,
    clouddeploy_projects_locations_custom_target_types_get_builder, clouddeploy_projects_locations_custom_target_types_get_task,
    clouddeploy_projects_locations_custom_target_types_get_iam_policy_builder, clouddeploy_projects_locations_custom_target_types_get_iam_policy_task,
    clouddeploy_projects_locations_custom_target_types_list_builder, clouddeploy_projects_locations_custom_target_types_list_task,
    clouddeploy_projects_locations_custom_target_types_patch_builder, clouddeploy_projects_locations_custom_target_types_patch_task,
    clouddeploy_projects_locations_custom_target_types_set_iam_policy_builder, clouddeploy_projects_locations_custom_target_types_set_iam_policy_task,
    clouddeploy_projects_locations_delivery_pipelines_create_builder, clouddeploy_projects_locations_delivery_pipelines_create_task,
    clouddeploy_projects_locations_delivery_pipelines_delete_builder, clouddeploy_projects_locations_delivery_pipelines_delete_task,
    clouddeploy_projects_locations_delivery_pipelines_get_builder, clouddeploy_projects_locations_delivery_pipelines_get_task,
    clouddeploy_projects_locations_delivery_pipelines_get_iam_policy_builder, clouddeploy_projects_locations_delivery_pipelines_get_iam_policy_task,
    clouddeploy_projects_locations_delivery_pipelines_list_builder, clouddeploy_projects_locations_delivery_pipelines_list_task,
    clouddeploy_projects_locations_delivery_pipelines_patch_builder, clouddeploy_projects_locations_delivery_pipelines_patch_task,
    clouddeploy_projects_locations_delivery_pipelines_rollback_target_builder, clouddeploy_projects_locations_delivery_pipelines_rollback_target_task,
    clouddeploy_projects_locations_delivery_pipelines_set_iam_policy_builder, clouddeploy_projects_locations_delivery_pipelines_set_iam_policy_task,
    clouddeploy_projects_locations_delivery_pipelines_test_iam_permissions_builder, clouddeploy_projects_locations_delivery_pipelines_test_iam_permissions_task,
    clouddeploy_projects_locations_delivery_pipelines_automation_runs_cancel_builder, clouddeploy_projects_locations_delivery_pipelines_automation_runs_cancel_task,
    clouddeploy_projects_locations_delivery_pipelines_automation_runs_get_builder, clouddeploy_projects_locations_delivery_pipelines_automation_runs_get_task,
    clouddeploy_projects_locations_delivery_pipelines_automation_runs_list_builder, clouddeploy_projects_locations_delivery_pipelines_automation_runs_list_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_create_builder, clouddeploy_projects_locations_delivery_pipelines_automations_create_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_delete_builder, clouddeploy_projects_locations_delivery_pipelines_automations_delete_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_get_builder, clouddeploy_projects_locations_delivery_pipelines_automations_get_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_list_builder, clouddeploy_projects_locations_delivery_pipelines_automations_list_task,
    clouddeploy_projects_locations_delivery_pipelines_automations_patch_builder, clouddeploy_projects_locations_delivery_pipelines_automations_patch_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_abandon_builder, clouddeploy_projects_locations_delivery_pipelines_releases_abandon_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_create_builder, clouddeploy_projects_locations_delivery_pipelines_releases_create_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_get_builder, clouddeploy_projects_locations_delivery_pipelines_releases_get_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_list_builder, clouddeploy_projects_locations_delivery_pipelines_releases_list_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_advance_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_advance_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_approve_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_approve_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_cancel_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_cancel_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_create_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_create_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_get_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_get_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_ignore_job_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_ignore_job_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_list_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_list_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_retry_job_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_retry_job_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_get_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_get_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_list_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_list_task,
    clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_terminate_builder, clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_terminate_task,
    clouddeploy_projects_locations_deploy_policies_create_builder, clouddeploy_projects_locations_deploy_policies_create_task,
    clouddeploy_projects_locations_deploy_policies_delete_builder, clouddeploy_projects_locations_deploy_policies_delete_task,
    clouddeploy_projects_locations_deploy_policies_get_builder, clouddeploy_projects_locations_deploy_policies_get_task,
    clouddeploy_projects_locations_deploy_policies_get_iam_policy_builder, clouddeploy_projects_locations_deploy_policies_get_iam_policy_task,
    clouddeploy_projects_locations_deploy_policies_list_builder, clouddeploy_projects_locations_deploy_policies_list_task,
    clouddeploy_projects_locations_deploy_policies_patch_builder, clouddeploy_projects_locations_deploy_policies_patch_task,
    clouddeploy_projects_locations_deploy_policies_set_iam_policy_builder, clouddeploy_projects_locations_deploy_policies_set_iam_policy_task,
    clouddeploy_projects_locations_operations_cancel_builder, clouddeploy_projects_locations_operations_cancel_task,
    clouddeploy_projects_locations_operations_delete_builder, clouddeploy_projects_locations_operations_delete_task,
    clouddeploy_projects_locations_operations_get_builder, clouddeploy_projects_locations_operations_get_task,
    clouddeploy_projects_locations_operations_list_builder, clouddeploy_projects_locations_operations_list_task,
    clouddeploy_projects_locations_targets_create_builder, clouddeploy_projects_locations_targets_create_task,
    clouddeploy_projects_locations_targets_delete_builder, clouddeploy_projects_locations_targets_delete_task,
    clouddeploy_projects_locations_targets_get_builder, clouddeploy_projects_locations_targets_get_task,
    clouddeploy_projects_locations_targets_get_iam_policy_builder, clouddeploy_projects_locations_targets_get_iam_policy_task,
    clouddeploy_projects_locations_targets_list_builder, clouddeploy_projects_locations_targets_list_task,
    clouddeploy_projects_locations_targets_patch_builder, clouddeploy_projects_locations_targets_patch_task,
    clouddeploy_projects_locations_targets_set_iam_policy_builder, clouddeploy_projects_locations_targets_set_iam_policy_task,
    clouddeploy_projects_locations_targets_test_iam_permissions_builder, clouddeploy_projects_locations_targets_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::clouddeploy::AbandonReleaseResponse;
use crate::providers::gcp::clients::clouddeploy::AdvanceRolloutResponse;
use crate::providers::gcp::clients::clouddeploy::ApproveRolloutResponse;
use crate::providers::gcp::clients::clouddeploy::Automation;
use crate::providers::gcp::clients::clouddeploy::AutomationRun;
use crate::providers::gcp::clients::clouddeploy::CancelAutomationRunResponse;
use crate::providers::gcp::clients::clouddeploy::CancelRolloutResponse;
use crate::providers::gcp::clients::clouddeploy::Config;
use crate::providers::gcp::clients::clouddeploy::CustomTargetType;
use crate::providers::gcp::clients::clouddeploy::DeliveryPipeline;
use crate::providers::gcp::clients::clouddeploy::DeployPolicy;
use crate::providers::gcp::clients::clouddeploy::Empty;
use crate::providers::gcp::clients::clouddeploy::IgnoreJobResponse;
use crate::providers::gcp::clients::clouddeploy::JobRun;
use crate::providers::gcp::clients::clouddeploy::ListAutomationRunsResponse;
use crate::providers::gcp::clients::clouddeploy::ListAutomationsResponse;
use crate::providers::gcp::clients::clouddeploy::ListCustomTargetTypesResponse;
use crate::providers::gcp::clients::clouddeploy::ListDeliveryPipelinesResponse;
use crate::providers::gcp::clients::clouddeploy::ListDeployPoliciesResponse;
use crate::providers::gcp::clients::clouddeploy::ListJobRunsResponse;
use crate::providers::gcp::clients::clouddeploy::ListLocationsResponse;
use crate::providers::gcp::clients::clouddeploy::ListOperationsResponse;
use crate::providers::gcp::clients::clouddeploy::ListReleasesResponse;
use crate::providers::gcp::clients::clouddeploy::ListRolloutsResponse;
use crate::providers::gcp::clients::clouddeploy::ListTargetsResponse;
use crate::providers::gcp::clients::clouddeploy::Location;
use crate::providers::gcp::clients::clouddeploy::Operation;
use crate::providers::gcp::clients::clouddeploy::Policy;
use crate::providers::gcp::clients::clouddeploy::Release;
use crate::providers::gcp::clients::clouddeploy::RetryJobResponse;
use crate::providers::gcp::clients::clouddeploy::RollbackTargetResponse;
use crate::providers::gcp::clients::clouddeploy::Rollout;
use crate::providers::gcp::clients::clouddeploy::Target;
use crate::providers::gcp::clients::clouddeploy::TerminateJobRunResponse;
use crate::providers::gcp::clients::clouddeploy::TestIamPermissionsResponse;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesGetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsCustomTargetTypesSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationRunsCancelArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationRunsGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationRunsListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesAutomationsPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesGetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesAbandonArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsAdvanceArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsApproveArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsCancelArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsIgnoreJobArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsJobRunsGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsJobRunsListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsJobRunsTerminateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsRetryJobArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesRollbackTargetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeliveryPipelinesTestIamPermissionsArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsDeployPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsGetConfigArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsCreateArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsDeleteArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsGetArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsGetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsListArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsPatchArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsSetIamPolicyArgs;
use crate::providers::gcp::clients::clouddeploy::ClouddeployProjectsLocationsTargetsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ClouddeployProvider with automatic state tracking.
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
/// let provider = ClouddeployProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ClouddeployProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ClouddeployProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ClouddeployProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ClouddeployProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Clouddeploy projects locations get.
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
    pub fn clouddeploy_projects_locations_get(
        &self,
        args: &ClouddeployProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations get config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_get_config(
        &self,
        args: &ClouddeployProjectsLocationsGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_get_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations list.
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
    pub fn clouddeploy_projects_locations_list(
        &self,
        args: &ClouddeployProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomTargetType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_custom_target_types_get(
        &self,
        args: &ClouddeployProjectsLocationsCustomTargetTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomTargetType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_custom_target_types_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_custom_target_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types get iam policy.
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
    pub fn clouddeploy_projects_locations_custom_target_types_get_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsCustomTargetTypesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_custom_target_types_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_custom_target_types_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomTargetTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_custom_target_types_list(
        &self,
        args: &ClouddeployProjectsLocationsCustomTargetTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomTargetTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_custom_target_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_custom_target_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types patch.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations custom target types set iam policy.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations delivery pipelines get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeliveryPipeline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_get(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeliveryPipeline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines get iam policy.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_get_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeliveryPipelinesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_list(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeliveryPipelinesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations delivery pipelines automation runs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutomationRun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_automation_runs_get(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationRunsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutomationRun, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automation_runs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automation_runs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines automation runs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutomationRunsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_automation_runs_list(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationRunsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutomationRunsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automation_runs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automation_runs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations delivery pipelines automations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Automation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_automations_get(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Automation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines automations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutomationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_automations_list(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesAutomationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutomationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_automations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_automations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations delivery pipelines releases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Release result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_get(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Release, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases list.
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
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_list(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReleasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations delivery pipelines releases rollouts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Rollout result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_get(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Rollout, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations delivery pipelines releases rollouts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRolloutsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_list(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRolloutsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations delivery pipelines releases rollouts job runs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the JobRun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_get(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsJobRunsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<JobRun, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations delivery pipelines releases rollouts job runs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListJobRunsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_list(
        &self,
        args: &ClouddeployProjectsLocationsDeliveryPipelinesReleasesRolloutsJobRunsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListJobRunsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_delivery_pipelines_releases_rollouts_job_runs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations deploy policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeployPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_deploy_policies_get(
        &self,
        args: &ClouddeployProjectsLocationsDeployPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeployPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_deploy_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_deploy_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations deploy policies get iam policy.
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
    pub fn clouddeploy_projects_locations_deploy_policies_get_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsDeployPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_deploy_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_deploy_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations deploy policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeployPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_deploy_policies_list(
        &self,
        args: &ClouddeployProjectsLocationsDeployPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeployPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_deploy_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_deploy_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Clouddeploy projects locations operations get.
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
    pub fn clouddeploy_projects_locations_operations_get(
        &self,
        args: &ClouddeployProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations operations list.
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
    pub fn clouddeploy_projects_locations_operations_list(
        &self,
        args: &ClouddeployProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Target result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_targets_get(
        &self,
        args: &ClouddeployProjectsLocationsTargetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Target, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets get iam policy.
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
    pub fn clouddeploy_projects_locations_targets_get_iam_policy(
        &self,
        args: &ClouddeployProjectsLocationsTargetsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTargetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn clouddeploy_projects_locations_targets_list(
        &self,
        args: &ClouddeployProjectsLocationsTargetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTargetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = clouddeploy_projects_locations_targets_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = clouddeploy_projects_locations_targets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets patch.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets set iam policy.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Clouddeploy projects locations targets test iam permissions.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
