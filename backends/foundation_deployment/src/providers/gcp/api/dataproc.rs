//! DataprocProvider - State-aware dataproc API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dataproc API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dataproc::{
    dataproc_projects_locations_autoscaling_policies_create_builder, dataproc_projects_locations_autoscaling_policies_create_task,
    dataproc_projects_locations_autoscaling_policies_delete_builder, dataproc_projects_locations_autoscaling_policies_delete_task,
    dataproc_projects_locations_autoscaling_policies_get_iam_policy_builder, dataproc_projects_locations_autoscaling_policies_get_iam_policy_task,
    dataproc_projects_locations_autoscaling_policies_set_iam_policy_builder, dataproc_projects_locations_autoscaling_policies_set_iam_policy_task,
    dataproc_projects_locations_autoscaling_policies_test_iam_permissions_builder, dataproc_projects_locations_autoscaling_policies_test_iam_permissions_task,
    dataproc_projects_locations_autoscaling_policies_update_builder, dataproc_projects_locations_autoscaling_policies_update_task,
    dataproc_projects_locations_batches_analyze_builder, dataproc_projects_locations_batches_analyze_task,
    dataproc_projects_locations_batches_create_builder, dataproc_projects_locations_batches_create_task,
    dataproc_projects_locations_batches_delete_builder, dataproc_projects_locations_batches_delete_task,
    dataproc_projects_locations_batches_spark_applications_write_builder, dataproc_projects_locations_batches_spark_applications_write_task,
    dataproc_projects_locations_operations_cancel_builder, dataproc_projects_locations_operations_cancel_task,
    dataproc_projects_locations_operations_delete_builder, dataproc_projects_locations_operations_delete_task,
    dataproc_projects_locations_session_templates_create_builder, dataproc_projects_locations_session_templates_create_task,
    dataproc_projects_locations_session_templates_delete_builder, dataproc_projects_locations_session_templates_delete_task,
    dataproc_projects_locations_session_templates_patch_builder, dataproc_projects_locations_session_templates_patch_task,
    dataproc_projects_locations_sessions_create_builder, dataproc_projects_locations_sessions_create_task,
    dataproc_projects_locations_sessions_delete_builder, dataproc_projects_locations_sessions_delete_task,
    dataproc_projects_locations_sessions_terminate_builder, dataproc_projects_locations_sessions_terminate_task,
    dataproc_projects_locations_sessions_spark_applications_write_builder, dataproc_projects_locations_sessions_spark_applications_write_task,
    dataproc_projects_locations_workflow_templates_create_builder, dataproc_projects_locations_workflow_templates_create_task,
    dataproc_projects_locations_workflow_templates_delete_builder, dataproc_projects_locations_workflow_templates_delete_task,
    dataproc_projects_locations_workflow_templates_get_iam_policy_builder, dataproc_projects_locations_workflow_templates_get_iam_policy_task,
    dataproc_projects_locations_workflow_templates_instantiate_builder, dataproc_projects_locations_workflow_templates_instantiate_task,
    dataproc_projects_locations_workflow_templates_instantiate_inline_builder, dataproc_projects_locations_workflow_templates_instantiate_inline_task,
    dataproc_projects_locations_workflow_templates_set_iam_policy_builder, dataproc_projects_locations_workflow_templates_set_iam_policy_task,
    dataproc_projects_locations_workflow_templates_test_iam_permissions_builder, dataproc_projects_locations_workflow_templates_test_iam_permissions_task,
    dataproc_projects_locations_workflow_templates_update_builder, dataproc_projects_locations_workflow_templates_update_task,
    dataproc_projects_regions_autoscaling_policies_create_builder, dataproc_projects_regions_autoscaling_policies_create_task,
    dataproc_projects_regions_autoscaling_policies_delete_builder, dataproc_projects_regions_autoscaling_policies_delete_task,
    dataproc_projects_regions_autoscaling_policies_get_iam_policy_builder, dataproc_projects_regions_autoscaling_policies_get_iam_policy_task,
    dataproc_projects_regions_autoscaling_policies_set_iam_policy_builder, dataproc_projects_regions_autoscaling_policies_set_iam_policy_task,
    dataproc_projects_regions_autoscaling_policies_test_iam_permissions_builder, dataproc_projects_regions_autoscaling_policies_test_iam_permissions_task,
    dataproc_projects_regions_autoscaling_policies_update_builder, dataproc_projects_regions_autoscaling_policies_update_task,
    dataproc_projects_regions_clusters_create_builder, dataproc_projects_regions_clusters_create_task,
    dataproc_projects_regions_clusters_delete_builder, dataproc_projects_regions_clusters_delete_task,
    dataproc_projects_regions_clusters_diagnose_builder, dataproc_projects_regions_clusters_diagnose_task,
    dataproc_projects_regions_clusters_get_iam_policy_builder, dataproc_projects_regions_clusters_get_iam_policy_task,
    dataproc_projects_regions_clusters_inject_credentials_builder, dataproc_projects_regions_clusters_inject_credentials_task,
    dataproc_projects_regions_clusters_patch_builder, dataproc_projects_regions_clusters_patch_task,
    dataproc_projects_regions_clusters_repair_builder, dataproc_projects_regions_clusters_repair_task,
    dataproc_projects_regions_clusters_set_iam_policy_builder, dataproc_projects_regions_clusters_set_iam_policy_task,
    dataproc_projects_regions_clusters_start_builder, dataproc_projects_regions_clusters_start_task,
    dataproc_projects_regions_clusters_stop_builder, dataproc_projects_regions_clusters_stop_task,
    dataproc_projects_regions_clusters_test_iam_permissions_builder, dataproc_projects_regions_clusters_test_iam_permissions_task,
    dataproc_projects_regions_clusters_node_groups_create_builder, dataproc_projects_regions_clusters_node_groups_create_task,
    dataproc_projects_regions_clusters_node_groups_repair_builder, dataproc_projects_regions_clusters_node_groups_repair_task,
    dataproc_projects_regions_clusters_node_groups_resize_builder, dataproc_projects_regions_clusters_node_groups_resize_task,
    dataproc_projects_regions_jobs_cancel_builder, dataproc_projects_regions_jobs_cancel_task,
    dataproc_projects_regions_jobs_delete_builder, dataproc_projects_regions_jobs_delete_task,
    dataproc_projects_regions_jobs_get_iam_policy_builder, dataproc_projects_regions_jobs_get_iam_policy_task,
    dataproc_projects_regions_jobs_patch_builder, dataproc_projects_regions_jobs_patch_task,
    dataproc_projects_regions_jobs_set_iam_policy_builder, dataproc_projects_regions_jobs_set_iam_policy_task,
    dataproc_projects_regions_jobs_submit_builder, dataproc_projects_regions_jobs_submit_task,
    dataproc_projects_regions_jobs_submit_as_operation_builder, dataproc_projects_regions_jobs_submit_as_operation_task,
    dataproc_projects_regions_jobs_test_iam_permissions_builder, dataproc_projects_regions_jobs_test_iam_permissions_task,
    dataproc_projects_regions_operations_cancel_builder, dataproc_projects_regions_operations_cancel_task,
    dataproc_projects_regions_operations_delete_builder, dataproc_projects_regions_operations_delete_task,
    dataproc_projects_regions_operations_get_iam_policy_builder, dataproc_projects_regions_operations_get_iam_policy_task,
    dataproc_projects_regions_operations_set_iam_policy_builder, dataproc_projects_regions_operations_set_iam_policy_task,
    dataproc_projects_regions_operations_test_iam_permissions_builder, dataproc_projects_regions_operations_test_iam_permissions_task,
    dataproc_projects_regions_workflow_templates_create_builder, dataproc_projects_regions_workflow_templates_create_task,
    dataproc_projects_regions_workflow_templates_delete_builder, dataproc_projects_regions_workflow_templates_delete_task,
    dataproc_projects_regions_workflow_templates_get_iam_policy_builder, dataproc_projects_regions_workflow_templates_get_iam_policy_task,
    dataproc_projects_regions_workflow_templates_instantiate_builder, dataproc_projects_regions_workflow_templates_instantiate_task,
    dataproc_projects_regions_workflow_templates_instantiate_inline_builder, dataproc_projects_regions_workflow_templates_instantiate_inline_task,
    dataproc_projects_regions_workflow_templates_set_iam_policy_builder, dataproc_projects_regions_workflow_templates_set_iam_policy_task,
    dataproc_projects_regions_workflow_templates_test_iam_permissions_builder, dataproc_projects_regions_workflow_templates_test_iam_permissions_task,
    dataproc_projects_regions_workflow_templates_update_builder, dataproc_projects_regions_workflow_templates_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dataproc::AutoscalingPolicy;
use crate::providers::gcp::clients::dataproc::Empty;
use crate::providers::gcp::clients::dataproc::Job;
use crate::providers::gcp::clients::dataproc::Operation;
use crate::providers::gcp::clients::dataproc::Policy;
use crate::providers::gcp::clients::dataproc::SessionTemplate;
use crate::providers::gcp::clients::dataproc::TestIamPermissionsResponse;
use crate::providers::gcp::clients::dataproc::WorkflowTemplate;
use crate::providers::gcp::clients::dataproc::WriteSessionSparkApplicationContextResponse;
use crate::providers::gcp::clients::dataproc::WriteSparkApplicationContextResponse;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsAutoscalingPoliciesUpdateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesAnalyzeArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsBatchesSparkApplicationsWriteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionTemplatesPatchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsSparkApplicationsWriteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsSessionsTerminateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesInstantiateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesInstantiateInlineArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsLocationsWorkflowTemplatesUpdateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsAutoscalingPoliciesUpdateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersDiagnoseArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersInjectCredentialsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersNodeGroupsCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersNodeGroupsRepairArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersNodeGroupsResizeArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersPatchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersRepairArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersStartArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersStopArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsCancelArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsPatchArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsSubmitArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsSubmitAsOperationArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsJobsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsCancelArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsOperationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesCreateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesDeleteArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesGetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesInstantiateArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesInstantiateInlineArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesSetIamPolicyArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dataproc::DataprocProjectsRegionsWorkflowTemplatesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DataprocProvider with automatic state tracking.
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
/// let provider = DataprocProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DataprocProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DataprocProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DataprocProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dataproc projects locations autoscaling policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_autoscaling_policies_create(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies delete.
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
    pub fn dataproc_projects_locations_autoscaling_policies_delete(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies get iam policy.
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
    pub fn dataproc_projects_locations_autoscaling_policies_get_iam_policy(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies set iam policy.
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
    pub fn dataproc_projects_locations_autoscaling_policies_set_iam_policy(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies test iam permissions.
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
    pub fn dataproc_projects_locations_autoscaling_policies_test_iam_permissions(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations autoscaling policies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_autoscaling_policies_update(
        &self,
        args: &DataprocProjectsLocationsAutoscalingPoliciesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_autoscaling_policies_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_autoscaling_policies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches analyze.
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
    pub fn dataproc_projects_locations_batches_analyze(
        &self,
        args: &DataprocProjectsLocationsBatchesAnalyzeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_analyze_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_analyze_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches create.
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
    pub fn dataproc_projects_locations_batches_create(
        &self,
        args: &DataprocProjectsLocationsBatchesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_create_builder(
            &self.http_client,
            &args.parent,
            &args.batchId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches delete.
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
    pub fn dataproc_projects_locations_batches_delete(
        &self,
        args: &DataprocProjectsLocationsBatchesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations batches spark applications write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteSparkApplicationContextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_batches_spark_applications_write(
        &self,
        args: &DataprocProjectsLocationsBatchesSparkApplicationsWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteSparkApplicationContextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_batches_spark_applications_write_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_batches_spark_applications_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations operations cancel.
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
    pub fn dataproc_projects_locations_operations_cancel(
        &self,
        args: &DataprocProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations operations delete.
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
    pub fn dataproc_projects_locations_operations_delete(
        &self,
        args: &DataprocProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SessionTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_session_templates_create(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SessionTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates delete.
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
    pub fn dataproc_projects_locations_session_templates_delete(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations session templates patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SessionTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_session_templates_patch(
        &self,
        args: &DataprocProjectsLocationsSessionTemplatesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SessionTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_session_templates_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_session_templates_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions create.
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
    pub fn dataproc_projects_locations_sessions_create(
        &self,
        args: &DataprocProjectsLocationsSessionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.sessionId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions delete.
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
    pub fn dataproc_projects_locations_sessions_delete(
        &self,
        args: &DataprocProjectsLocationsSessionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions terminate.
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
    pub fn dataproc_projects_locations_sessions_terminate(
        &self,
        args: &DataprocProjectsLocationsSessionsTerminateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_terminate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_terminate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations sessions spark applications write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteSessionSparkApplicationContextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_sessions_spark_applications_write(
        &self,
        args: &DataprocProjectsLocationsSessionsSparkApplicationsWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteSessionSparkApplicationContextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_sessions_spark_applications_write_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_sessions_spark_applications_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_workflow_templates_create(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates delete.
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
    pub fn dataproc_projects_locations_workflow_templates_delete(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_delete_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates get iam policy.
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
    pub fn dataproc_projects_locations_workflow_templates_get_iam_policy(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates instantiate.
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
    pub fn dataproc_projects_locations_workflow_templates_instantiate(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesInstantiateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_instantiate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_instantiate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates instantiate inline.
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
    pub fn dataproc_projects_locations_workflow_templates_instantiate_inline(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesInstantiateInlineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_instantiate_inline_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_instantiate_inline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates set iam policy.
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
    pub fn dataproc_projects_locations_workflow_templates_set_iam_policy(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates test iam permissions.
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
    pub fn dataproc_projects_locations_workflow_templates_test_iam_permissions(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects locations workflow templates update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_locations_workflow_templates_update(
        &self,
        args: &DataprocProjectsLocationsWorkflowTemplatesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_locations_workflow_templates_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_locations_workflow_templates_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_autoscaling_policies_create(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies delete.
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
    pub fn dataproc_projects_regions_autoscaling_policies_delete(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies get iam policy.
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
    pub fn dataproc_projects_regions_autoscaling_policies_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies set iam policy.
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
    pub fn dataproc_projects_regions_autoscaling_policies_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies test iam permissions.
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
    pub fn dataproc_projects_regions_autoscaling_policies_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions autoscaling policies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutoscalingPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_autoscaling_policies_update(
        &self,
        args: &DataprocProjectsRegionsAutoscalingPoliciesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutoscalingPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_autoscaling_policies_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_autoscaling_policies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters create.
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
    pub fn dataproc_projects_regions_clusters_create(
        &self,
        args: &DataprocProjectsRegionsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_create_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.actionOnFailedPrimaryWorkers,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters delete.
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
    pub fn dataproc_projects_regions_clusters_delete(
        &self,
        args: &DataprocProjectsRegionsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
            &args.clusterUuid,
            &args.gracefulTerminationTimeout,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters diagnose.
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
    pub fn dataproc_projects_regions_clusters_diagnose(
        &self,
        args: &DataprocProjectsRegionsClustersDiagnoseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_diagnose_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_diagnose_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters get iam policy.
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
    pub fn dataproc_projects_regions_clusters_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsClustersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters inject credentials.
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
    pub fn dataproc_projects_regions_clusters_inject_credentials(
        &self,
        args: &DataprocProjectsRegionsClustersInjectCredentialsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_inject_credentials_builder(
            &self.http_client,
            &args.project,
            &args.region,
            &args.cluster,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_inject_credentials_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters patch.
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
    pub fn dataproc_projects_regions_clusters_patch(
        &self,
        args: &DataprocProjectsRegionsClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
            &args.gracefulDecommissionTimeout,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters repair.
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
    pub fn dataproc_projects_regions_clusters_repair(
        &self,
        args: &DataprocProjectsRegionsClustersRepairArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_repair_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_repair_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters set iam policy.
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
    pub fn dataproc_projects_regions_clusters_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters start.
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
    pub fn dataproc_projects_regions_clusters_start(
        &self,
        args: &DataprocProjectsRegionsClustersStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_start_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters stop.
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
    pub fn dataproc_projects_regions_clusters_stop(
        &self,
        args: &DataprocProjectsRegionsClustersStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_stop_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters test iam permissions.
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
    pub fn dataproc_projects_regions_clusters_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters node groups create.
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
    pub fn dataproc_projects_regions_clusters_node_groups_create(
        &self,
        args: &DataprocProjectsRegionsClustersNodeGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_node_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.nodeGroupId,
            &args.parentOperationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_node_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters node groups repair.
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
    pub fn dataproc_projects_regions_clusters_node_groups_repair(
        &self,
        args: &DataprocProjectsRegionsClustersNodeGroupsRepairArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_node_groups_repair_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_node_groups_repair_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions clusters node groups resize.
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
    pub fn dataproc_projects_regions_clusters_node_groups_resize(
        &self,
        args: &DataprocProjectsRegionsClustersNodeGroupsResizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_clusters_node_groups_resize_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_clusters_node_groups_resize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs cancel.
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
    pub fn dataproc_projects_regions_jobs_cancel(
        &self,
        args: &DataprocProjectsRegionsJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_cancel_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs delete.
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
    pub fn dataproc_projects_regions_jobs_delete(
        &self,
        args: &DataprocProjectsRegionsJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs get iam policy.
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
    pub fn dataproc_projects_regions_jobs_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsJobsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs patch.
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
    pub fn dataproc_projects_regions_jobs_patch(
        &self,
        args: &DataprocProjectsRegionsJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
            &args.jobId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs set iam policy.
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
    pub fn dataproc_projects_regions_jobs_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsJobsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs submit.
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
    pub fn dataproc_projects_regions_jobs_submit(
        &self,
        args: &DataprocProjectsRegionsJobsSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_submit_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs submit as operation.
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
    pub fn dataproc_projects_regions_jobs_submit_as_operation(
        &self,
        args: &DataprocProjectsRegionsJobsSubmitAsOperationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_submit_as_operation_builder(
            &self.http_client,
            &args.projectId,
            &args.region,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_submit_as_operation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions jobs test iam permissions.
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
    pub fn dataproc_projects_regions_jobs_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsJobsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_jobs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_jobs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations cancel.
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
    pub fn dataproc_projects_regions_operations_cancel(
        &self,
        args: &DataprocProjectsRegionsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations delete.
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
    pub fn dataproc_projects_regions_operations_delete(
        &self,
        args: &DataprocProjectsRegionsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations get iam policy.
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
    pub fn dataproc_projects_regions_operations_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsOperationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations set iam policy.
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
    pub fn dataproc_projects_regions_operations_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsOperationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions operations test iam permissions.
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
    pub fn dataproc_projects_regions_operations_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsOperationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_operations_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_operations_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_workflow_templates_create(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates delete.
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
    pub fn dataproc_projects_regions_workflow_templates_delete(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_delete_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates get iam policy.
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
    pub fn dataproc_projects_regions_workflow_templates_get_iam_policy(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates instantiate.
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
    pub fn dataproc_projects_regions_workflow_templates_instantiate(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesInstantiateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_instantiate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_instantiate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates instantiate inline.
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
    pub fn dataproc_projects_regions_workflow_templates_instantiate_inline(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesInstantiateInlineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_instantiate_inline_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_instantiate_inline_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates set iam policy.
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
    pub fn dataproc_projects_regions_workflow_templates_set_iam_policy(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates test iam permissions.
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
    pub fn dataproc_projects_regions_workflow_templates_test_iam_permissions(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataproc projects regions workflow templates update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkflowTemplate result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataproc_projects_regions_workflow_templates_update(
        &self,
        args: &DataprocProjectsRegionsWorkflowTemplatesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkflowTemplate, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataproc_projects_regions_workflow_templates_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataproc_projects_regions_workflow_templates_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
