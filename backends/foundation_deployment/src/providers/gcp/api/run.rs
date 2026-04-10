//! RunProvider - State-aware run API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       run API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::run::{
    run_projects_locations_export_image_builder, run_projects_locations_export_image_task,
    run_projects_locations_export_image_metadata_builder, run_projects_locations_export_image_metadata_task,
    run_projects_locations_export_metadata_builder, run_projects_locations_export_metadata_task,
    run_projects_locations_export_project_metadata_builder, run_projects_locations_export_project_metadata_task,
    run_projects_locations_builds_submit_builder, run_projects_locations_builds_submit_task,
    run_projects_locations_instances_create_builder, run_projects_locations_instances_create_task,
    run_projects_locations_instances_delete_builder, run_projects_locations_instances_delete_task,
    run_projects_locations_instances_get_builder, run_projects_locations_instances_get_task,
    run_projects_locations_instances_list_builder, run_projects_locations_instances_list_task,
    run_projects_locations_instances_start_builder, run_projects_locations_instances_start_task,
    run_projects_locations_instances_stop_builder, run_projects_locations_instances_stop_task,
    run_projects_locations_jobs_create_builder, run_projects_locations_jobs_create_task,
    run_projects_locations_jobs_delete_builder, run_projects_locations_jobs_delete_task,
    run_projects_locations_jobs_get_builder, run_projects_locations_jobs_get_task,
    run_projects_locations_jobs_get_iam_policy_builder, run_projects_locations_jobs_get_iam_policy_task,
    run_projects_locations_jobs_list_builder, run_projects_locations_jobs_list_task,
    run_projects_locations_jobs_patch_builder, run_projects_locations_jobs_patch_task,
    run_projects_locations_jobs_run_builder, run_projects_locations_jobs_run_task,
    run_projects_locations_jobs_set_iam_policy_builder, run_projects_locations_jobs_set_iam_policy_task,
    run_projects_locations_jobs_test_iam_permissions_builder, run_projects_locations_jobs_test_iam_permissions_task,
    run_projects_locations_jobs_executions_cancel_builder, run_projects_locations_jobs_executions_cancel_task,
    run_projects_locations_jobs_executions_delete_builder, run_projects_locations_jobs_executions_delete_task,
    run_projects_locations_jobs_executions_export_status_builder, run_projects_locations_jobs_executions_export_status_task,
    run_projects_locations_jobs_executions_get_builder, run_projects_locations_jobs_executions_get_task,
    run_projects_locations_jobs_executions_list_builder, run_projects_locations_jobs_executions_list_task,
    run_projects_locations_jobs_executions_tasks_get_builder, run_projects_locations_jobs_executions_tasks_get_task,
    run_projects_locations_jobs_executions_tasks_list_builder, run_projects_locations_jobs_executions_tasks_list_task,
    run_projects_locations_operations_delete_builder, run_projects_locations_operations_delete_task,
    run_projects_locations_operations_get_builder, run_projects_locations_operations_get_task,
    run_projects_locations_operations_list_builder, run_projects_locations_operations_list_task,
    run_projects_locations_operations_wait_builder, run_projects_locations_operations_wait_task,
    run_projects_locations_services_create_builder, run_projects_locations_services_create_task,
    run_projects_locations_services_delete_builder, run_projects_locations_services_delete_task,
    run_projects_locations_services_get_builder, run_projects_locations_services_get_task,
    run_projects_locations_services_get_iam_policy_builder, run_projects_locations_services_get_iam_policy_task,
    run_projects_locations_services_list_builder, run_projects_locations_services_list_task,
    run_projects_locations_services_patch_builder, run_projects_locations_services_patch_task,
    run_projects_locations_services_set_iam_policy_builder, run_projects_locations_services_set_iam_policy_task,
    run_projects_locations_services_test_iam_permissions_builder, run_projects_locations_services_test_iam_permissions_task,
    run_projects_locations_services_revisions_delete_builder, run_projects_locations_services_revisions_delete_task,
    run_projects_locations_services_revisions_export_status_builder, run_projects_locations_services_revisions_export_status_task,
    run_projects_locations_services_revisions_get_builder, run_projects_locations_services_revisions_get_task,
    run_projects_locations_services_revisions_list_builder, run_projects_locations_services_revisions_list_task,
    run_projects_locations_worker_pools_create_builder, run_projects_locations_worker_pools_create_task,
    run_projects_locations_worker_pools_delete_builder, run_projects_locations_worker_pools_delete_task,
    run_projects_locations_worker_pools_get_builder, run_projects_locations_worker_pools_get_task,
    run_projects_locations_worker_pools_get_iam_policy_builder, run_projects_locations_worker_pools_get_iam_policy_task,
    run_projects_locations_worker_pools_list_builder, run_projects_locations_worker_pools_list_task,
    run_projects_locations_worker_pools_patch_builder, run_projects_locations_worker_pools_patch_task,
    run_projects_locations_worker_pools_set_iam_policy_builder, run_projects_locations_worker_pools_set_iam_policy_task,
    run_projects_locations_worker_pools_test_iam_permissions_builder, run_projects_locations_worker_pools_test_iam_permissions_task,
    run_projects_locations_worker_pools_revisions_delete_builder, run_projects_locations_worker_pools_revisions_delete_task,
    run_projects_locations_worker_pools_revisions_get_builder, run_projects_locations_worker_pools_revisions_get_task,
    run_projects_locations_worker_pools_revisions_list_builder, run_projects_locations_worker_pools_revisions_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::run::GoogleCloudRunV2Execution;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ExportImageResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ExportStatusResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2Instance;
use crate::providers::gcp::clients::run::GoogleCloudRunV2Job;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ListExecutionsResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ListInstancesResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ListJobsResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ListRevisionsResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ListServicesResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ListTasksResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2ListWorkerPoolsResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2Metadata;
use crate::providers::gcp::clients::run::GoogleCloudRunV2Revision;
use crate::providers::gcp::clients::run::GoogleCloudRunV2Service;
use crate::providers::gcp::clients::run::GoogleCloudRunV2SubmitBuildResponse;
use crate::providers::gcp::clients::run::GoogleCloudRunV2Task;
use crate::providers::gcp::clients::run::GoogleCloudRunV2WorkerPool;
use crate::providers::gcp::clients::run::GoogleIamV1Policy;
use crate::providers::gcp::clients::run::GoogleIamV1TestIamPermissionsResponse;
use crate::providers::gcp::clients::run::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::run::GoogleLongrunningOperation;
use crate::providers::gcp::clients::run::GoogleProtobufEmpty;
use crate::providers::gcp::clients::run::RunProjectsLocationsBuildsSubmitArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsExportImageArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsExportImageMetadataArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsExportMetadataArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsExportProjectMetadataArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsInstancesCreateArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsInstancesDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsInstancesGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsInstancesListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsInstancesStartArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsInstancesStopArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsCreateArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsExecutionsCancelArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsExecutionsDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsExecutionsExportStatusArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsExecutionsGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsExecutionsListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsExecutionsTasksGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsExecutionsTasksListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsGetIamPolicyArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsPatchArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsRunArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsSetIamPolicyArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsJobsTestIamPermissionsArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsOperationsWaitArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesCreateArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesGetIamPolicyArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesPatchArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesRevisionsDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesRevisionsExportStatusArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesRevisionsGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesRevisionsListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesSetIamPolicyArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsServicesTestIamPermissionsArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsCreateArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsGetIamPolicyArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsPatchArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsRevisionsDeleteArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsRevisionsGetArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsRevisionsListArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsSetIamPolicyArgs;
use crate::providers::gcp::clients::run::RunProjectsLocationsWorkerPoolsTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RunProvider with automatic state tracking.
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
/// let provider = RunProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct RunProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> RunProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new RunProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Run projects locations export image.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ExportImageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_export_image(
        &self,
        args: &RunProjectsLocationsExportImageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ExportImageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_export_image_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_export_image_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations export image metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Metadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_export_image_metadata(
        &self,
        args: &RunProjectsLocationsExportImageMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Metadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_export_image_metadata_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_export_image_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations export metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Metadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_export_metadata(
        &self,
        args: &RunProjectsLocationsExportMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Metadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_export_metadata_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_export_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations export project metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Metadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_export_project_metadata(
        &self,
        args: &RunProjectsLocationsExportProjectMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Metadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_export_project_metadata_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_export_project_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations builds submit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2SubmitBuildResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_builds_submit(
        &self,
        args: &RunProjectsLocationsBuildsSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2SubmitBuildResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_builds_submit_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_builds_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations instances create.
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
    pub fn run_projects_locations_instances_create(
        &self,
        args: &RunProjectsLocationsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations instances delete.
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
    pub fn run_projects_locations_instances_delete(
        &self,
        args: &RunProjectsLocationsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_instances_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Instance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_instances_get(
        &self,
        args: &RunProjectsLocationsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListInstancesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_instances_list(
        &self,
        args: &RunProjectsLocationsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations instances start.
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
    pub fn run_projects_locations_instances_start(
        &self,
        args: &RunProjectsLocationsInstancesStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_instances_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_instances_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations instances stop.
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
    pub fn run_projects_locations_instances_stop(
        &self,
        args: &RunProjectsLocationsInstancesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_instances_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_instances_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs create.
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
    pub fn run_projects_locations_jobs_create(
        &self,
        args: &RunProjectsLocationsJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.jobId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs delete.
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
    pub fn run_projects_locations_jobs_delete(
        &self,
        args: &RunProjectsLocationsJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_jobs_get(
        &self,
        args: &RunProjectsLocationsJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs get iam policy.
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
    pub fn run_projects_locations_jobs_get_iam_policy(
        &self,
        args: &RunProjectsLocationsJobsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_jobs_list(
        &self,
        args: &RunProjectsLocationsJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs patch.
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
    pub fn run_projects_locations_jobs_patch(
        &self,
        args: &RunProjectsLocationsJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs run.
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
    pub fn run_projects_locations_jobs_run(
        &self,
        args: &RunProjectsLocationsJobsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs set iam policy.
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
    pub fn run_projects_locations_jobs_set_iam_policy(
        &self,
        args: &RunProjectsLocationsJobsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_jobs_test_iam_permissions(
        &self,
        args: &RunProjectsLocationsJobsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs executions cancel.
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
    pub fn run_projects_locations_jobs_executions_cancel(
        &self,
        args: &RunProjectsLocationsJobsExecutionsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_executions_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_executions_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs executions delete.
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
    pub fn run_projects_locations_jobs_executions_delete(
        &self,
        args: &RunProjectsLocationsJobsExecutionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_executions_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_executions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs executions export status.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ExportStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_jobs_executions_export_status(
        &self,
        args: &RunProjectsLocationsJobsExecutionsExportStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ExportStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_executions_export_status_builder(
            &self.http_client,
            &args.name,
            &args.operationId,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_executions_export_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs executions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Execution result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_jobs_executions_get(
        &self,
        args: &RunProjectsLocationsJobsExecutionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_executions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_executions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs executions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListExecutionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_jobs_executions_list(
        &self,
        args: &RunProjectsLocationsJobsExecutionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListExecutionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_executions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_executions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs executions tasks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Task result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_jobs_executions_tasks_get(
        &self,
        args: &RunProjectsLocationsJobsExecutionsTasksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_executions_tasks_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_executions_tasks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations jobs executions tasks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListTasksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_jobs_executions_tasks_list(
        &self,
        args: &RunProjectsLocationsJobsExecutionsTasksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListTasksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_jobs_executions_tasks_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_jobs_executions_tasks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations operations delete.
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
    pub fn run_projects_locations_operations_delete(
        &self,
        args: &RunProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations operations get.
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
    pub fn run_projects_locations_operations_get(
        &self,
        args: &RunProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations operations list.
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
    pub fn run_projects_locations_operations_list(
        &self,
        args: &RunProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations operations wait.
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
    pub fn run_projects_locations_operations_wait(
        &self,
        args: &RunProjectsLocationsOperationsWaitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_operations_wait_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_operations_wait_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services create.
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
    pub fn run_projects_locations_services_create(
        &self,
        args: &RunProjectsLocationsServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_create_builder(
            &self.http_client,
            &args.parent,
            &args.serviceId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services delete.
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
    pub fn run_projects_locations_services_delete(
        &self,
        args: &RunProjectsLocationsServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_services_get(
        &self,
        args: &RunProjectsLocationsServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services get iam policy.
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
    pub fn run_projects_locations_services_get_iam_policy(
        &self,
        args: &RunProjectsLocationsServicesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_services_list(
        &self,
        args: &RunProjectsLocationsServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services patch.
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
    pub fn run_projects_locations_services_patch(
        &self,
        args: &RunProjectsLocationsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.forceNewRevision,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services set iam policy.
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
    pub fn run_projects_locations_services_set_iam_policy(
        &self,
        args: &RunProjectsLocationsServicesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_services_test_iam_permissions(
        &self,
        args: &RunProjectsLocationsServicesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services revisions delete.
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
    pub fn run_projects_locations_services_revisions_delete(
        &self,
        args: &RunProjectsLocationsServicesRevisionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_revisions_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_revisions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services revisions export status.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ExportStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_services_revisions_export_status(
        &self,
        args: &RunProjectsLocationsServicesRevisionsExportStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ExportStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_revisions_export_status_builder(
            &self.http_client,
            &args.name,
            &args.operationId,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_revisions_export_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services revisions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Revision result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_services_revisions_get(
        &self,
        args: &RunProjectsLocationsServicesRevisionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Revision, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_revisions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_revisions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations services revisions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_services_revisions_list(
        &self,
        args: &RunProjectsLocationsServicesRevisionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_services_revisions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_services_revisions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools create.
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
    pub fn run_projects_locations_worker_pools_create(
        &self,
        args: &RunProjectsLocationsWorkerPoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
            &args.workerPoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools delete.
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
    pub fn run_projects_locations_worker_pools_delete(
        &self,
        args: &RunProjectsLocationsWorkerPoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2WorkerPool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_worker_pools_get(
        &self,
        args: &RunProjectsLocationsWorkerPoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2WorkerPool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools get iam policy.
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
    pub fn run_projects_locations_worker_pools_get_iam_policy(
        &self,
        args: &RunProjectsLocationsWorkerPoolsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListWorkerPoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_worker_pools_list(
        &self,
        args: &RunProjectsLocationsWorkerPoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListWorkerPoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools patch.
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
    pub fn run_projects_locations_worker_pools_patch(
        &self,
        args: &RunProjectsLocationsWorkerPoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.forceNewRevision,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools set iam policy.
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
    pub fn run_projects_locations_worker_pools_set_iam_policy(
        &self,
        args: &RunProjectsLocationsWorkerPoolsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn run_projects_locations_worker_pools_test_iam_permissions(
        &self,
        args: &RunProjectsLocationsWorkerPoolsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools revisions delete.
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
    pub fn run_projects_locations_worker_pools_revisions_delete(
        &self,
        args: &RunProjectsLocationsWorkerPoolsRevisionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_revisions_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_revisions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools revisions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2Revision result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_worker_pools_revisions_get(
        &self,
        args: &RunProjectsLocationsWorkerPoolsRevisionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2Revision, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_revisions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_revisions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Run projects locations worker pools revisions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRunV2ListRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn run_projects_locations_worker_pools_revisions_list(
        &self,
        args: &RunProjectsLocationsWorkerPoolsRevisionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRunV2ListRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = run_projects_locations_worker_pools_revisions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.showDeleted,
        )
        .map_err(ProviderError::Api)?;

        let task = run_projects_locations_worker_pools_revisions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
