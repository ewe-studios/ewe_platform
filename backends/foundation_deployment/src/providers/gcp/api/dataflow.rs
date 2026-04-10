//! DataflowProvider - State-aware dataflow API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dataflow API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dataflow::{
    dataflow_projects_delete_snapshots_builder, dataflow_projects_delete_snapshots_task,
    dataflow_projects_worker_messages_builder, dataflow_projects_worker_messages_task,
    dataflow_projects_jobs_create_builder, dataflow_projects_jobs_create_task,
    dataflow_projects_jobs_snapshot_builder, dataflow_projects_jobs_snapshot_task,
    dataflow_projects_jobs_update_builder, dataflow_projects_jobs_update_task,
    dataflow_projects_jobs_debug_get_config_builder, dataflow_projects_jobs_debug_get_config_task,
    dataflow_projects_jobs_debug_send_capture_builder, dataflow_projects_jobs_debug_send_capture_task,
    dataflow_projects_jobs_work_items_lease_builder, dataflow_projects_jobs_work_items_lease_task,
    dataflow_projects_jobs_work_items_report_status_builder, dataflow_projects_jobs_work_items_report_status_task,
    dataflow_projects_locations_worker_messages_builder, dataflow_projects_locations_worker_messages_task,
    dataflow_projects_locations_flex_templates_launch_builder, dataflow_projects_locations_flex_templates_launch_task,
    dataflow_projects_locations_jobs_create_builder, dataflow_projects_locations_jobs_create_task,
    dataflow_projects_locations_jobs_snapshot_builder, dataflow_projects_locations_jobs_snapshot_task,
    dataflow_projects_locations_jobs_update_builder, dataflow_projects_locations_jobs_update_task,
    dataflow_projects_locations_jobs_debug_get_config_builder, dataflow_projects_locations_jobs_debug_get_config_task,
    dataflow_projects_locations_jobs_debug_get_worker_stacktraces_builder, dataflow_projects_locations_jobs_debug_get_worker_stacktraces_task,
    dataflow_projects_locations_jobs_debug_send_capture_builder, dataflow_projects_locations_jobs_debug_send_capture_task,
    dataflow_projects_locations_jobs_work_items_lease_builder, dataflow_projects_locations_jobs_work_items_lease_task,
    dataflow_projects_locations_jobs_work_items_report_status_builder, dataflow_projects_locations_jobs_work_items_report_status_task,
    dataflow_projects_locations_snapshots_delete_builder, dataflow_projects_locations_snapshots_delete_task,
    dataflow_projects_locations_templates_create_builder, dataflow_projects_locations_templates_create_task,
    dataflow_projects_locations_templates_launch_builder, dataflow_projects_locations_templates_launch_task,
    dataflow_projects_templates_create_builder, dataflow_projects_templates_create_task,
    dataflow_projects_templates_launch_builder, dataflow_projects_templates_launch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dataflow::DeleteSnapshotResponse;
use crate::providers::gcp::clients::dataflow::GetDebugConfigResponse;
use crate::providers::gcp::clients::dataflow::GetWorkerStacktracesResponse;
use crate::providers::gcp::clients::dataflow::Job;
use crate::providers::gcp::clients::dataflow::LaunchFlexTemplateResponse;
use crate::providers::gcp::clients::dataflow::LaunchTemplateResponse;
use crate::providers::gcp::clients::dataflow::LeaseWorkItemResponse;
use crate::providers::gcp::clients::dataflow::ReportWorkItemStatusResponse;
use crate::providers::gcp::clients::dataflow::SendDebugCaptureResponse;
use crate::providers::gcp::clients::dataflow::SendWorkerMessagesResponse;
use crate::providers::gcp::clients::dataflow::Snapshot;
use crate::providers::gcp::clients::dataflow::DataflowProjectsDeleteSnapshotsArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsJobsCreateArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsJobsDebugGetConfigArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsJobsDebugSendCaptureArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsJobsSnapshotArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsJobsUpdateArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsJobsWorkItemsLeaseArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsJobsWorkItemsReportStatusArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsFlexTemplatesLaunchArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsCreateArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsDebugGetConfigArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsDebugGetWorkerStacktracesArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsDebugSendCaptureArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsSnapshotArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsUpdateArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsWorkItemsLeaseArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsJobsWorkItemsReportStatusArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsSnapshotsDeleteArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsTemplatesCreateArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsTemplatesLaunchArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsLocationsWorkerMessagesArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsTemplatesCreateArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsTemplatesLaunchArgs;
use crate::providers::gcp::clients::dataflow::DataflowProjectsWorkerMessagesArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DataflowProvider with automatic state tracking.
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
/// let provider = DataflowProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DataflowProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DataflowProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DataflowProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dataflow projects delete snapshots.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteSnapshotResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_delete_snapshots(
        &self,
        args: &DataflowProjectsDeleteSnapshotsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteSnapshotResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_delete_snapshots_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.snapshotId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_delete_snapshots_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects worker messages.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendWorkerMessagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_worker_messages(
        &self,
        args: &DataflowProjectsWorkerMessagesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendWorkerMessagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_worker_messages_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_worker_messages_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects jobs create.
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
    pub fn dataflow_projects_jobs_create(
        &self,
        args: &DataflowProjectsJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_jobs_create_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.replaceJobId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects jobs snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Snapshot result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_jobs_snapshot(
        &self,
        args: &DataflowProjectsJobsSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_jobs_snapshot_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_jobs_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects jobs update.
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
    pub fn dataflow_projects_jobs_update(
        &self,
        args: &DataflowProjectsJobsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_jobs_update_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
            &args.location,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_jobs_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects jobs debug get config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetDebugConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_jobs_debug_get_config(
        &self,
        args: &DataflowProjectsJobsDebugGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetDebugConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_jobs_debug_get_config_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_jobs_debug_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects jobs debug send capture.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendDebugCaptureResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_jobs_debug_send_capture(
        &self,
        args: &DataflowProjectsJobsDebugSendCaptureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendDebugCaptureResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_jobs_debug_send_capture_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_jobs_debug_send_capture_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects jobs work items lease.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaseWorkItemResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_jobs_work_items_lease(
        &self,
        args: &DataflowProjectsJobsWorkItemsLeaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaseWorkItemResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_jobs_work_items_lease_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_jobs_work_items_lease_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects jobs work items report status.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportWorkItemStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_jobs_work_items_report_status(
        &self,
        args: &DataflowProjectsJobsWorkItemsReportStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportWorkItemStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_jobs_work_items_report_status_builder(
            &self.http_client,
            &args.projectId,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_jobs_work_items_report_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations worker messages.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendWorkerMessagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_worker_messages(
        &self,
        args: &DataflowProjectsLocationsWorkerMessagesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendWorkerMessagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_worker_messages_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_worker_messages_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations flex templates launch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LaunchFlexTemplateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_flex_templates_launch(
        &self,
        args: &DataflowProjectsLocationsFlexTemplatesLaunchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LaunchFlexTemplateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_flex_templates_launch_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_flex_templates_launch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs create.
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
    pub fn dataflow_projects_locations_jobs_create(
        &self,
        args: &DataflowProjectsLocationsJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_create_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.replaceJobId,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs snapshot.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Snapshot result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_jobs_snapshot(
        &self,
        args: &DataflowProjectsLocationsJobsSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_snapshot_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs update.
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
    pub fn dataflow_projects_locations_jobs_update(
        &self,
        args: &DataflowProjectsLocationsJobsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_update_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.jobId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs debug get config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetDebugConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_jobs_debug_get_config(
        &self,
        args: &DataflowProjectsLocationsJobsDebugGetConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetDebugConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_debug_get_config_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_debug_get_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs debug get worker stacktraces.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetWorkerStacktracesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_jobs_debug_get_worker_stacktraces(
        &self,
        args: &DataflowProjectsLocationsJobsDebugGetWorkerStacktracesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetWorkerStacktracesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_debug_get_worker_stacktraces_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_debug_get_worker_stacktraces_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs debug send capture.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SendDebugCaptureResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_jobs_debug_send_capture(
        &self,
        args: &DataflowProjectsLocationsJobsDebugSendCaptureArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SendDebugCaptureResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_debug_send_capture_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_debug_send_capture_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs work items lease.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LeaseWorkItemResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_jobs_work_items_lease(
        &self,
        args: &DataflowProjectsLocationsJobsWorkItemsLeaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LeaseWorkItemResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_work_items_lease_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_work_items_lease_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations jobs work items report status.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportWorkItemStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_jobs_work_items_report_status(
        &self,
        args: &DataflowProjectsLocationsJobsWorkItemsReportStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportWorkItemStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_jobs_work_items_report_status_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.jobId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_jobs_work_items_report_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations snapshots delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeleteSnapshotResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_snapshots_delete(
        &self,
        args: &DataflowProjectsLocationsSnapshotsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeleteSnapshotResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_snapshots_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.snapshotId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_snapshots_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations templates create.
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
    pub fn dataflow_projects_locations_templates_create(
        &self,
        args: &DataflowProjectsLocationsTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_templates_create_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects locations templates launch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LaunchTemplateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_locations_templates_launch(
        &self,
        args: &DataflowProjectsLocationsTemplatesLaunchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LaunchTemplateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_locations_templates_launch_builder(
            &self.http_client,
            &args.projectId,
            &args.location,
            &args.dynamicTemplate.gcsPath,
            &args.dynamicTemplate.stagingLocation,
            &args.gcsPath,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_locations_templates_launch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects templates create.
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
    pub fn dataflow_projects_templates_create(
        &self,
        args: &DataflowProjectsTemplatesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_templates_create_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_templates_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataflow projects templates launch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LaunchTemplateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataflow_projects_templates_launch(
        &self,
        args: &DataflowProjectsTemplatesLaunchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LaunchTemplateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataflow_projects_templates_launch_builder(
            &self.http_client,
            &args.projectId,
            &args.dynamicTemplate.gcsPath,
            &args.dynamicTemplate.stagingLocation,
            &args.gcsPath,
            &args.location,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = dataflow_projects_templates_launch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
