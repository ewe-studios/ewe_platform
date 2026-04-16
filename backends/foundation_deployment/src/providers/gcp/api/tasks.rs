//! TasksProvider - State-aware tasks API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       tasks API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::tasks::{
    tasks_tasklists_delete_builder, tasks_tasklists_delete_task,
    tasks_tasklists_get_builder, tasks_tasklists_get_task,
    tasks_tasklists_insert_builder, tasks_tasklists_insert_task,
    tasks_tasklists_list_builder, tasks_tasklists_list_task,
    tasks_tasklists_patch_builder, tasks_tasklists_patch_task,
    tasks_tasklists_update_builder, tasks_tasklists_update_task,
    tasks_tasks_clear_builder, tasks_tasks_clear_task,
    tasks_tasks_delete_builder, tasks_tasks_delete_task,
    tasks_tasks_get_builder, tasks_tasks_get_task,
    tasks_tasks_insert_builder, tasks_tasks_insert_task,
    tasks_tasks_list_builder, tasks_tasks_list_task,
    tasks_tasks_move_builder, tasks_tasks_move_task,
    tasks_tasks_patch_builder, tasks_tasks_patch_task,
    tasks_tasks_update_builder, tasks_tasks_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::tasks::Task;
use crate::providers::gcp::clients::tasks::TaskList;
use crate::providers::gcp::clients::tasks::TaskLists;
use crate::providers::gcp::clients::tasks::Tasks;
use crate::providers::gcp::clients::tasks::TasksTasklistsDeleteArgs;
use crate::providers::gcp::clients::tasks::TasksTasklistsGetArgs;
use crate::providers::gcp::clients::tasks::TasksTasklistsListArgs;
use crate::providers::gcp::clients::tasks::TasksTasklistsPatchArgs;
use crate::providers::gcp::clients::tasks::TasksTasklistsUpdateArgs;
use crate::providers::gcp::clients::tasks::TasksTasksClearArgs;
use crate::providers::gcp::clients::tasks::TasksTasksDeleteArgs;
use crate::providers::gcp::clients::tasks::TasksTasksGetArgs;
use crate::providers::gcp::clients::tasks::TasksTasksInsertArgs;
use crate::providers::gcp::clients::tasks::TasksTasksListArgs;
use crate::providers::gcp::clients::tasks::TasksTasksMoveArgs;
use crate::providers::gcp::clients::tasks::TasksTasksPatchArgs;
use crate::providers::gcp::clients::tasks::TasksTasksUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// TasksProvider with automatic state tracking.
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
/// let provider = TasksProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct TasksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> TasksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new TasksProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new TasksProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Tasks tasklists delete.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tasks_tasklists_delete(
        &self,
        args: &TasksTasklistsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasklists_delete_builder(
            &self.http_client,
            &args.tasklist,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasklists_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasklists get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaskList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tasks_tasklists_get(
        &self,
        args: &TasksTasklistsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaskList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasklists_get_builder(
            &self.http_client,
            &args.tasklist,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasklists_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasklists insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaskList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tasks_tasklists_insert(
        &self,
        args: &TasksTasklistsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaskList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasklists_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasklists_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasklists list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaskLists result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tasks_tasklists_list(
        &self,
        args: &TasksTasklistsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaskLists, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasklists_list_builder(
            &self.http_client,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasklists_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasklists patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaskList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tasks_tasklists_patch(
        &self,
        args: &TasksTasklistsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaskList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasklists_patch_builder(
            &self.http_client,
            &args.tasklist,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasklists_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasklists update.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TaskList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tasks_tasklists_update(
        &self,
        args: &TasksTasklistsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TaskList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasklists_update_builder(
            &self.http_client,
            &args.tasklist,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasklists_update_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks clear.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tasks_tasks_clear(
        &self,
        args: &TasksTasksClearArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_clear_builder(
            &self.http_client,
            &args.tasklist,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_clear_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tasks_tasks_delete(
        &self,
        args: &TasksTasksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_delete_builder(
            &self.http_client,
            &args.tasklist,
            &args.task,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks get.
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
    pub fn tasks_tasks_get(
        &self,
        args: &TasksTasksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_get_builder(
            &self.http_client,
            &args.tasklist,
            &args.task,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks insert.
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
    pub fn tasks_tasks_insert(
        &self,
        args: &TasksTasksInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_insert_builder(
            &self.http_client,
            &args.tasklist,
            &args.parent,
            &args.previous,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tasks result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tasks_tasks_list(
        &self,
        args: &TasksTasksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tasks, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_list_builder(
            &self.http_client,
            &args.tasklist,
            &args.completedMax,
            &args.completedMin,
            &args.dueMax,
            &args.dueMin,
            &args.maxResults,
            &args.pageToken,
            &args.showAssigned,
            &args.showCompleted,
            &args.showDeleted,
            &args.showHidden,
            &args.updatedMin,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks move.
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
    pub fn tasks_tasks_move(
        &self,
        args: &TasksTasksMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_move_builder(
            &self.http_client,
            &args.tasklist,
            &args.task,
            &args.destinationTasklist,
            &args.parent,
            &args.previous,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks patch.
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
    pub fn tasks_tasks_patch(
        &self,
        args: &TasksTasksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_patch_builder(
            &self.http_client,
            &args.tasklist,
            &args.task,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tasks tasks update.
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
    pub fn tasks_tasks_update(
        &self,
        args: &TasksTasksUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Task, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tasks_tasks_update_builder(
            &self.http_client,
            &args.tasklist,
            &args.task,
        )
        .map_err(ProviderError::Api)?;

        let task = tasks_tasks_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
