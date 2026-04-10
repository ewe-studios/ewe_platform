//! ToolresultsProvider - State-aware toolresults API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       toolresults API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::toolresults::{
    toolresults_projects_get_settings_builder, toolresults_projects_get_settings_task,
    toolresults_projects_initialize_settings_builder, toolresults_projects_initialize_settings_task,
    toolresults_projects_histories_create_builder, toolresults_projects_histories_create_task,
    toolresults_projects_histories_get_builder, toolresults_projects_histories_get_task,
    toolresults_projects_histories_list_builder, toolresults_projects_histories_list_task,
    toolresults_projects_histories_executions_create_builder, toolresults_projects_histories_executions_create_task,
    toolresults_projects_histories_executions_get_builder, toolresults_projects_histories_executions_get_task,
    toolresults_projects_histories_executions_list_builder, toolresults_projects_histories_executions_list_task,
    toolresults_projects_histories_executions_patch_builder, toolresults_projects_histories_executions_patch_task,
    toolresults_projects_histories_executions_clusters_get_builder, toolresults_projects_histories_executions_clusters_get_task,
    toolresults_projects_histories_executions_clusters_list_builder, toolresults_projects_histories_executions_clusters_list_task,
    toolresults_projects_histories_executions_environments_get_builder, toolresults_projects_histories_executions_environments_get_task,
    toolresults_projects_histories_executions_environments_list_builder, toolresults_projects_histories_executions_environments_list_task,
    toolresults_projects_histories_executions_steps_accessibility_clusters_builder, toolresults_projects_histories_executions_steps_accessibility_clusters_task,
    toolresults_projects_histories_executions_steps_create_builder, toolresults_projects_histories_executions_steps_create_task,
    toolresults_projects_histories_executions_steps_get_builder, toolresults_projects_histories_executions_steps_get_task,
    toolresults_projects_histories_executions_steps_get_perf_metrics_summary_builder, toolresults_projects_histories_executions_steps_get_perf_metrics_summary_task,
    toolresults_projects_histories_executions_steps_list_builder, toolresults_projects_histories_executions_steps_list_task,
    toolresults_projects_histories_executions_steps_patch_builder, toolresults_projects_histories_executions_steps_patch_task,
    toolresults_projects_histories_executions_steps_publish_xunit_xml_files_builder, toolresults_projects_histories_executions_steps_publish_xunit_xml_files_task,
    toolresults_projects_histories_executions_steps_perf_metrics_summary_create_builder, toolresults_projects_histories_executions_steps_perf_metrics_summary_create_task,
    toolresults_projects_histories_executions_steps_perf_sample_series_create_builder, toolresults_projects_histories_executions_steps_perf_sample_series_create_task,
    toolresults_projects_histories_executions_steps_perf_sample_series_get_builder, toolresults_projects_histories_executions_steps_perf_sample_series_get_task,
    toolresults_projects_histories_executions_steps_perf_sample_series_list_builder, toolresults_projects_histories_executions_steps_perf_sample_series_list_task,
    toolresults_projects_histories_executions_steps_perf_sample_series_samples_batch_create_builder, toolresults_projects_histories_executions_steps_perf_sample_series_samples_batch_create_task,
    toolresults_projects_histories_executions_steps_perf_sample_series_samples_list_builder, toolresults_projects_histories_executions_steps_perf_sample_series_samples_list_task,
    toolresults_projects_histories_executions_steps_test_cases_get_builder, toolresults_projects_histories_executions_steps_test_cases_get_task,
    toolresults_projects_histories_executions_steps_test_cases_list_builder, toolresults_projects_histories_executions_steps_test_cases_list_task,
    toolresults_projects_histories_executions_steps_thumbnails_list_builder, toolresults_projects_histories_executions_steps_thumbnails_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::toolresults::BatchCreatePerfSamplesResponse;
use crate::providers::gcp::clients::toolresults::Environment;
use crate::providers::gcp::clients::toolresults::Execution;
use crate::providers::gcp::clients::toolresults::History;
use crate::providers::gcp::clients::toolresults::ListEnvironmentsResponse;
use crate::providers::gcp::clients::toolresults::ListExecutionsResponse;
use crate::providers::gcp::clients::toolresults::ListHistoriesResponse;
use crate::providers::gcp::clients::toolresults::ListPerfSampleSeriesResponse;
use crate::providers::gcp::clients::toolresults::ListPerfSamplesResponse;
use crate::providers::gcp::clients::toolresults::ListScreenshotClustersResponse;
use crate::providers::gcp::clients::toolresults::ListStepAccessibilityClustersResponse;
use crate::providers::gcp::clients::toolresults::ListStepThumbnailsResponse;
use crate::providers::gcp::clients::toolresults::ListStepsResponse;
use crate::providers::gcp::clients::toolresults::ListTestCasesResponse;
use crate::providers::gcp::clients::toolresults::PerfMetricsSummary;
use crate::providers::gcp::clients::toolresults::PerfSampleSeries;
use crate::providers::gcp::clients::toolresults::ProjectSettings;
use crate::providers::gcp::clients::toolresults::ScreenshotCluster;
use crate::providers::gcp::clients::toolresults::Step;
use crate::providers::gcp::clients::toolresults::TestCase;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsGetSettingsArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesCreateArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsClustersGetArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsClustersListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsCreateArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsEnvironmentsGetArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsEnvironmentsListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsGetArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsPatchArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsAccessibilityClustersArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsCreateArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsGetArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsGetPerfMetricsSummaryArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPatchArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPerfMetricsSummaryCreateArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesCreateArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesGetArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesSamplesBatchCreateArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesSamplesListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsPublishXunitXmlFilesArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsTestCasesGetArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsTestCasesListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesExecutionsStepsThumbnailsListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesGetArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsHistoriesListArgs;
use crate::providers::gcp::clients::toolresults::ToolresultsProjectsInitializeSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ToolresultsProvider with automatic state tracking.
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
/// let provider = ToolresultsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ToolresultsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ToolresultsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ToolresultsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Toolresults projects get settings.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_get_settings(
        &self,
        args: &ToolresultsProjectsGetSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_get_settings_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_get_settings_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects initialize settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_initialize_settings(
        &self,
        args: &ToolresultsProjectsInitializeSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_initialize_settings_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_initialize_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the History result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_create(
        &self,
        args: &ToolresultsProjectsHistoriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<History, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_create_builder(
            &self.http_client,
            &args.projectId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the History result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_get(
        &self,
        args: &ToolresultsProjectsHistoriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<History, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_get_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHistoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_list(
        &self,
        args: &ToolresultsProjectsHistoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHistoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_list_builder(
            &self.http_client,
            &args.projectId,
            &args.filterByName,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Execution result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_create(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_create_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Execution result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_get(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_get_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExecutionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExecutionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Execution result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_patch(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ScreenshotCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_clusters_get(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ScreenshotCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_clusters_get_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListScreenshotClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_clusters_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListScreenshotClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_clusters_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions environments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Environment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_environments_get(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsEnvironmentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Environment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_environments_get_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.environmentId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_environments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions environments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEnvironmentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_environments_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsEnvironmentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEnvironmentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_environments_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_environments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps accessibility clusters.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStepAccessibilityClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_accessibility_clusters(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsAccessibilityClustersArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStepAccessibilityClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_accessibility_clusters_builder(
            &self.http_client,
            &args.name,
            &args.locale,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_accessibility_clusters_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Step result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_steps_create(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Step, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_create_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Step result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_get(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Step, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_get_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps get perf metrics summary.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PerfMetricsSummary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_get_perf_metrics_summary(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsGetPerfMetricsSummaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PerfMetricsSummary, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_get_perf_metrics_summary_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_get_perf_metrics_summary_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStepsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStepsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Step result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_steps_patch(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Step, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_patch_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps publish xunit xml files.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Step result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_steps_publish_xunit_xml_files(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPublishXunitXmlFilesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Step, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_publish_xunit_xml_files_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_publish_xunit_xml_files_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps perf metrics summary create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PerfMetricsSummary result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_steps_perf_metrics_summary_create(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPerfMetricsSummaryCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PerfMetricsSummary, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_perf_metrics_summary_create_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_perf_metrics_summary_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps perf sample series create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PerfSampleSeries result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_steps_perf_sample_series_create(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PerfSampleSeries, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_perf_sample_series_create_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_perf_sample_series_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps perf sample series get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PerfSampleSeries result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_perf_sample_series_get(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PerfSampleSeries, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_perf_sample_series_get_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.sampleSeriesId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_perf_sample_series_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps perf sample series list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPerfSampleSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_perf_sample_series_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPerfSampleSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_perf_sample_series_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.filter,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_perf_sample_series_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps perf sample series samples batch create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreatePerfSamplesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn toolresults_projects_histories_executions_steps_perf_sample_series_samples_batch_create(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesSamplesBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreatePerfSamplesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_perf_sample_series_samples_batch_create_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.sampleSeriesId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_perf_sample_series_samples_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps perf sample series samples list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPerfSamplesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_perf_sample_series_samples_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsPerfSampleSeriesSamplesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPerfSamplesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_perf_sample_series_samples_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.sampleSeriesId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_perf_sample_series_samples_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps test cases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestCase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_test_cases_get(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsTestCasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestCase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_test_cases_get_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.testCaseId,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_test_cases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps test cases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTestCasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_test_cases_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsTestCasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTestCasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_test_cases_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_test_cases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Toolresults projects histories executions steps thumbnails list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStepThumbnailsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn toolresults_projects_histories_executions_steps_thumbnails_list(
        &self,
        args: &ToolresultsProjectsHistoriesExecutionsStepsThumbnailsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStepThumbnailsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = toolresults_projects_histories_executions_steps_thumbnails_list_builder(
            &self.http_client,
            &args.projectId,
            &args.historyId,
            &args.executionId,
            &args.stepId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = toolresults_projects_histories_executions_steps_thumbnails_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
