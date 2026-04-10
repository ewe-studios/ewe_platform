//! DatapipelinesProvider - State-aware datapipelines API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       datapipelines API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::datapipelines::{
    datapipelines_projects_locations_pipelines_create_builder, datapipelines_projects_locations_pipelines_create_task,
    datapipelines_projects_locations_pipelines_delete_builder, datapipelines_projects_locations_pipelines_delete_task,
    datapipelines_projects_locations_pipelines_get_builder, datapipelines_projects_locations_pipelines_get_task,
    datapipelines_projects_locations_pipelines_list_builder, datapipelines_projects_locations_pipelines_list_task,
    datapipelines_projects_locations_pipelines_patch_builder, datapipelines_projects_locations_pipelines_patch_task,
    datapipelines_projects_locations_pipelines_run_builder, datapipelines_projects_locations_pipelines_run_task,
    datapipelines_projects_locations_pipelines_stop_builder, datapipelines_projects_locations_pipelines_stop_task,
    datapipelines_projects_locations_pipelines_jobs_list_builder, datapipelines_projects_locations_pipelines_jobs_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datapipelines::GoogleCloudDatapipelinesV1ListJobsResponse;
use crate::providers::gcp::clients::datapipelines::GoogleCloudDatapipelinesV1ListPipelinesResponse;
use crate::providers::gcp::clients::datapipelines::GoogleCloudDatapipelinesV1Pipeline;
use crate::providers::gcp::clients::datapipelines::GoogleCloudDatapipelinesV1RunPipelineResponse;
use crate::providers::gcp::clients::datapipelines::GoogleProtobufEmpty;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesCreateArgs;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesDeleteArgs;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesGetArgs;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesJobsListArgs;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesListArgs;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesPatchArgs;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesRunArgs;
use crate::providers::gcp::clients::datapipelines::DatapipelinesProjectsLocationsPipelinesStopArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatapipelinesProvider with automatic state tracking.
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
/// let provider = DatapipelinesProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DatapipelinesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DatapipelinesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DatapipelinesProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Datapipelines projects locations pipelines create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatapipelinesV1Pipeline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datapipelines_projects_locations_pipelines_create(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatapipelinesV1Pipeline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datapipelines projects locations pipelines delete.
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
    pub fn datapipelines_projects_locations_pipelines_delete(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datapipelines projects locations pipelines get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatapipelinesV1Pipeline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datapipelines_projects_locations_pipelines_get(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatapipelinesV1Pipeline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datapipelines projects locations pipelines list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatapipelinesV1ListPipelinesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datapipelines_projects_locations_pipelines_list(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatapipelinesV1ListPipelinesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datapipelines projects locations pipelines patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatapipelinesV1Pipeline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datapipelines_projects_locations_pipelines_patch(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatapipelinesV1Pipeline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datapipelines projects locations pipelines run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatapipelinesV1RunPipelineResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datapipelines_projects_locations_pipelines_run(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatapipelinesV1RunPipelineResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datapipelines projects locations pipelines stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatapipelinesV1Pipeline result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datapipelines_projects_locations_pipelines_stop(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatapipelinesV1Pipeline, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datapipelines projects locations pipelines jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatapipelinesV1ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datapipelines_projects_locations_pipelines_jobs_list(
        &self,
        args: &DatapipelinesProjectsLocationsPipelinesJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatapipelinesV1ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datapipelines_projects_locations_pipelines_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datapipelines_projects_locations_pipelines_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
