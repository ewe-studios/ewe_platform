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
    cloudtasks_projects_locations_update_cmek_config_builder, cloudtasks_projects_locations_update_cmek_config_task,
    cloudtasks_projects_locations_queues_create_builder, cloudtasks_projects_locations_queues_create_task,
    cloudtasks_projects_locations_queues_delete_builder, cloudtasks_projects_locations_queues_delete_task,
    cloudtasks_projects_locations_queues_get_iam_policy_builder, cloudtasks_projects_locations_queues_get_iam_policy_task,
    cloudtasks_projects_locations_queues_patch_builder, cloudtasks_projects_locations_queues_patch_task,
    cloudtasks_projects_locations_queues_pause_builder, cloudtasks_projects_locations_queues_pause_task,
    cloudtasks_projects_locations_queues_purge_builder, cloudtasks_projects_locations_queues_purge_task,
    cloudtasks_projects_locations_queues_resume_builder, cloudtasks_projects_locations_queues_resume_task,
    cloudtasks_projects_locations_queues_set_iam_policy_builder, cloudtasks_projects_locations_queues_set_iam_policy_task,
    cloudtasks_projects_locations_queues_test_iam_permissions_builder, cloudtasks_projects_locations_queues_test_iam_permissions_task,
    cloudtasks_projects_locations_queues_tasks_buffer_builder, cloudtasks_projects_locations_queues_tasks_buffer_task,
    cloudtasks_projects_locations_queues_tasks_create_builder, cloudtasks_projects_locations_queues_tasks_create_task,
    cloudtasks_projects_locations_queues_tasks_delete_builder, cloudtasks_projects_locations_queues_tasks_delete_task,
    cloudtasks_projects_locations_queues_tasks_run_builder, cloudtasks_projects_locations_queues_tasks_run_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudtasks::BufferTaskResponse;
use crate::providers::gcp::clients::cloudtasks::CmekConfig;
use crate::providers::gcp::clients::cloudtasks::Empty;
use crate::providers::gcp::clients::cloudtasks::Policy;
use crate::providers::gcp::clients::cloudtasks::Queue;
use crate::providers::gcp::clients::cloudtasks::Task;
use crate::providers::gcp::clients::cloudtasks::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesCreateArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesDeleteArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesPatchArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesPauseArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesPurgeArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesResumeArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksBufferArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksCreateArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksDeleteArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTasksRunArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsQueuesTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudtasks::CloudtasksProjectsLocationsUpdateCmekConfigArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudtasksProvider with automatic state tracking.
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
/// let provider = CloudtasksProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudtasksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudtasksProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudtasksProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

    /// Cloudtasks projects locations queues get iam policy.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
