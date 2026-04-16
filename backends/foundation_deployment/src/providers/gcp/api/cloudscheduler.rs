//! CloudschedulerProvider - State-aware cloudscheduler API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudscheduler API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudscheduler::{
    cloudscheduler_projects_locations_get_builder, cloudscheduler_projects_locations_get_task,
    cloudscheduler_projects_locations_get_cmek_config_builder, cloudscheduler_projects_locations_get_cmek_config_task,
    cloudscheduler_projects_locations_list_builder, cloudscheduler_projects_locations_list_task,
    cloudscheduler_projects_locations_update_cmek_config_builder, cloudscheduler_projects_locations_update_cmek_config_task,
    cloudscheduler_projects_locations_jobs_create_builder, cloudscheduler_projects_locations_jobs_create_task,
    cloudscheduler_projects_locations_jobs_delete_builder, cloudscheduler_projects_locations_jobs_delete_task,
    cloudscheduler_projects_locations_jobs_get_builder, cloudscheduler_projects_locations_jobs_get_task,
    cloudscheduler_projects_locations_jobs_list_builder, cloudscheduler_projects_locations_jobs_list_task,
    cloudscheduler_projects_locations_jobs_patch_builder, cloudscheduler_projects_locations_jobs_patch_task,
    cloudscheduler_projects_locations_jobs_pause_builder, cloudscheduler_projects_locations_jobs_pause_task,
    cloudscheduler_projects_locations_jobs_resume_builder, cloudscheduler_projects_locations_jobs_resume_task,
    cloudscheduler_projects_locations_jobs_run_builder, cloudscheduler_projects_locations_jobs_run_task,
    cloudscheduler_projects_locations_operations_cancel_builder, cloudscheduler_projects_locations_operations_cancel_task,
    cloudscheduler_projects_locations_operations_delete_builder, cloudscheduler_projects_locations_operations_delete_task,
    cloudscheduler_projects_locations_operations_get_builder, cloudscheduler_projects_locations_operations_get_task,
    cloudscheduler_projects_locations_operations_list_builder, cloudscheduler_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudscheduler::CmekConfig;
use crate::providers::gcp::clients::cloudscheduler::Empty;
use crate::providers::gcp::clients::cloudscheduler::Job;
use crate::providers::gcp::clients::cloudscheduler::ListJobsResponse;
use crate::providers::gcp::clients::cloudscheduler::ListLocationsResponse;
use crate::providers::gcp::clients::cloudscheduler::ListOperationsResponse;
use crate::providers::gcp::clients::cloudscheduler::Location;
use crate::providers::gcp::clients::cloudscheduler::Operation;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsGetArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsGetCmekConfigArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsCreateArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsDeleteArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsGetArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsListArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsPatchArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsPauseArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsResumeArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsJobsRunArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsListArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::cloudscheduler::CloudschedulerProjectsLocationsUpdateCmekConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudschedulerProvider with automatic state tracking.
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
/// let provider = CloudschedulerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudschedulerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudschedulerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudschedulerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudschedulerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudscheduler projects locations get.
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
    pub fn cloudscheduler_projects_locations_get(
        &self,
        args: &CloudschedulerProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations get cmek config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CmekConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudscheduler_projects_locations_get_cmek_config(
        &self,
        args: &CloudschedulerProjectsLocationsGetCmekConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_get_cmek_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_get_cmek_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations list.
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
    pub fn cloudscheduler_projects_locations_list(
        &self,
        args: &CloudschedulerProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations update cmek config.
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
    pub fn cloudscheduler_projects_locations_update_cmek_config(
        &self,
        args: &CloudschedulerProjectsLocationsUpdateCmekConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_update_cmek_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_update_cmek_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs create.
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
    pub fn cloudscheduler_projects_locations_jobs_create(
        &self,
        args: &CloudschedulerProjectsLocationsJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs delete.
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
    pub fn cloudscheduler_projects_locations_jobs_delete(
        &self,
        args: &CloudschedulerProjectsLocationsJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs get.
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
    pub fn cloudscheduler_projects_locations_jobs_get(
        &self,
        args: &CloudschedulerProjectsLocationsJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudscheduler_projects_locations_jobs_list(
        &self,
        args: &CloudschedulerProjectsLocationsJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs patch.
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
    pub fn cloudscheduler_projects_locations_jobs_patch(
        &self,
        args: &CloudschedulerProjectsLocationsJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs pause.
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
    pub fn cloudscheduler_projects_locations_jobs_pause(
        &self,
        args: &CloudschedulerProjectsLocationsJobsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs resume.
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
    pub fn cloudscheduler_projects_locations_jobs_resume(
        &self,
        args: &CloudschedulerProjectsLocationsJobsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations jobs run.
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
    pub fn cloudscheduler_projects_locations_jobs_run(
        &self,
        args: &CloudschedulerProjectsLocationsJobsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_jobs_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_jobs_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations operations cancel.
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
    pub fn cloudscheduler_projects_locations_operations_cancel(
        &self,
        args: &CloudschedulerProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations operations delete.
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
    pub fn cloudscheduler_projects_locations_operations_delete(
        &self,
        args: &CloudschedulerProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations operations get.
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
    pub fn cloudscheduler_projects_locations_operations_get(
        &self,
        args: &CloudschedulerProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudscheduler projects locations operations list.
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
    pub fn cloudscheduler_projects_locations_operations_list(
        &self,
        args: &CloudschedulerProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudscheduler_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudscheduler_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
