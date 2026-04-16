//! CloudtasksProvider - State-aware cloudtasks API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudtasks API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudtasks::{
    cloudtasks_projects_locations_get_builder, cloudtasks_projects_locations_get_task,
    cloudtasks_projects_locations_get_cmek_config_builder, cloudtasks_projects_locations_get_cmek_config_task,
    cloudtasks_projects_locations_list_builder, cloudtasks_projects_locations_list_task,
    cloudtasks_projects_locations_update_cmek_config_builder, cloudtasks_projects_locations_update_cmek_config_task,
    cloudtasks_projects_locations_queues_create_builder, cloudtasks_projects_locations_queues_create_task,
    cloudtasks_projects_locations_queues_delete_builder, cloudtasks_projects_locations_queues_delete_task,
    cloudtasks_projects_locations_queues_get_builder, cloudtasks_projects_locations_queues_get_task,
    cloudtasks_projects_locations_queues_get_iam_policy_builder, cloudtasks_projects_locations_queues_get_iam_policy_task,
    cloudtasks_projects_locations_queues_list_builder, cloudtasks_projects_locations_queues_list_task,
    cloudtasks_projects_locations_queues_patch_builder, cloudtasks_projects_locations_queues_patch_task,
    cloudtasks_projects_locations_queues_pause_builder, cloudtasks_projects_locations_queues_pause_task,
    cloudtasks_projects_locations_queues_purge_builder, cloudtasks_projects_locations_queues_purge_task,
    cloudtasks_projects_locations_queues_resume_builder, cloudtasks_projects_locations_queues_resume_task,
    cloudtasks_projects_locations_queues_set_iam_policy_builder, cloudtasks_projects_locations_queues_set_iam_policy_task,
    cloudtasks_projects_locations_queues_test_iam_permissions_builder, cloudtasks_projects_locations_queues_test_iam_permissions_task,
    cloudtasks_projects_locations_queues_tasks_buffer_builder, cloudtasks_projects_locations_queues_tasks_buffer_task,
    cloudtasks_projects_locations_queues_tasks_create_builder, cloudtasks_projects_locations_queues_tasks_create_task,
    cloudtasks_projects_locations_queues_tasks_delete_builder, cloudtasks_projects_locations_queues_tasks_delete_task,
    cloudtasks_projects_locations_queues_tasks_get_builder, cloudtasks_projects_locations_queues_tasks_get_task,
    cloudtasks_projects_locations_queues_tasks_list_builder, cloudtasks_projects_locations_queues_tasks_list_task,
    cloudtasks_projects_locations_queues_tasks_run_builder, cloudtasks_projects_locations_queues_tasks_run_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudtasks::BufferTaskResponse;
use crate::providers::gcp::clients::cloudtasks::CmekConfig;
use crate::providers::gcp::clients::cloudtasks::Empty;
use crate::providers::gcp::clients::cloudtasks::ListLocationsResponse;
use crate::providers::gcp::clients::cloudtasks::ListQueuesResponse;
use crate::providers::gcp::clients::cloudtasks::ListTasksResponse;
use crate::providers::gcp::clients::cloudtasks::Location;
use crate::providers::gcp::clients::cloudtasks::Policy;
use crate::providers::gcp::clients::cloudtasks::Queue;
use crate::providers::gcp::clients::cloudtasks::Task;
use crate::providers::gcp::clients::cloudtasks::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsGetArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsGetCmekConfigArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsListArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesCreateArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesDeleteArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesGetArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesListArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesPatchArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesPauseArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesPurgeArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesResumeArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksBufferArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksCreateArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksDeleteArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksGetArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksListArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksRunArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsUpdateCmekConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudtasksProvider with automatic state tracking.
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
/// let provider = CloudtasksProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudtasksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudtasksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudtasksProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudtasksProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudtasks projects locations get.
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
    pub fn cloudtasks_projects_locations_get(
        &self,
        args: &CloudtasksProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations get cmek config.
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
    pub fn cloudtasks_projects_locations_get_cmek_config(
        &self,
        args: &CloudtasksProjectsLocationsGetCmekConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_get_cmek_config_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_get_cmek_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations list.
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
    pub fn cloudtasks_projects_locations_list(
        &self,
        args: &CloudtasksProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations update cmek config.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_update_cmek_config(
        &self,
        args: &CloudtasksProjectsLocationsUpdateCmekConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CmekConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_update_cmek_config_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_update_cmek_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Queue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_create(
        &self,
        args: &CloudtasksProjectsLocationsQueuesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Queue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues delete.
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
    pub fn cloudtasks_projects_locations_queues_delete(
        &self,
        args: &CloudtasksProjectsLocationsQueuesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Queue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudtasks_projects_locations_queues_get(
        &self,
        args: &CloudtasksProjectsLocationsQueuesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Queue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues get iam policy.
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
    pub fn cloudtasks_projects_locations_queues_get_iam_policy(
        &self,
        args: &CloudtasksProjectsLocationsQueuesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListQueuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudtasks_projects_locations_queues_list(
        &self,
        args: &CloudtasksProjectsLocationsQueuesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListQueuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Queue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_patch(
        &self,
        args: &CloudtasksProjectsLocationsQueuesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Queue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues pause.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Queue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_pause(
        &self,
        args: &CloudtasksProjectsLocationsQueuesPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Queue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_pause_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues purge.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Queue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_purge(
        &self,
        args: &CloudtasksProjectsLocationsQueuesPurgeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Queue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_purge_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_purge_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Queue result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_resume(
        &self,
        args: &CloudtasksProjectsLocationsQueuesResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Queue, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues set iam policy.
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
    pub fn cloudtasks_projects_locations_queues_set_iam_policy(
        &self,
        args: &CloudtasksProjectsLocationsQueuesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues test iam permissions.
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
    pub fn cloudtasks_projects_locations_queues_test_iam_permissions(
        &self,
        args: &CloudtasksProjectsLocationsQueuesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues tasks buffer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BufferTaskResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_tasks_buffer(
        &self,
        args: &CloudtasksProjectsLocationsQueuesTasksBufferArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BufferTaskResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_tasks_buffer_builder(
            &self.http_client,
            &args.queue,
            &args.taskId,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_tasks_buffer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues tasks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Task result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_tasks_create(
        &self,
        args: &CloudtasksProjectsLocationsQueuesTasksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_tasks_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_tasks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues tasks delete.
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
    pub fn cloudtasks_projects_locations_queues_tasks_delete(
        &self,
        args: &CloudtasksProjectsLocationsQueuesTasksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_tasks_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_tasks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues tasks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Task result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudtasks_projects_locations_queues_tasks_get(
        &self,
        args: &CloudtasksProjectsLocationsQueuesTasksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_tasks_get_builder(
            &self.http_client,
            &args.name,
            &args.responseView,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_tasks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues tasks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTasksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudtasks_projects_locations_queues_tasks_list(
        &self,
        args: &CloudtasksProjectsLocationsQueuesTasksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTasksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_tasks_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.responseView,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_tasks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudtasks projects locations queues tasks run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Task result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudtasks_projects_locations_queues_tasks_run(
        &self,
        args: &CloudtasksProjectsLocationsQueuesTasksRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudtasks_projects_locations_queues_tasks_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudtasks_projects_locations_queues_tasks_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
