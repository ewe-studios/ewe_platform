//! WorkflowexecutionsProvider - State-aware workflowexecutions API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       workflowexecutions API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::workflowexecutions::{
    workflowexecutions_projects_locations_workflows_trigger_pubsub_execution_builder, workflowexecutions_projects_locations_workflows_trigger_pubsub_execution_task,
    workflowexecutions_projects_locations_workflows_executions_cancel_builder, workflowexecutions_projects_locations_workflows_executions_cancel_task,
    workflowexecutions_projects_locations_workflows_executions_create_builder, workflowexecutions_projects_locations_workflows_executions_create_task,
    workflowexecutions_projects_locations_workflows_executions_delete_execution_history_builder, workflowexecutions_projects_locations_workflows_executions_delete_execution_history_task,
    workflowexecutions_projects_locations_workflows_executions_export_data_builder, workflowexecutions_projects_locations_workflows_executions_export_data_task,
    workflowexecutions_projects_locations_workflows_executions_get_builder, workflowexecutions_projects_locations_workflows_executions_get_task,
    workflowexecutions_projects_locations_workflows_executions_list_builder, workflowexecutions_projects_locations_workflows_executions_list_task,
    workflowexecutions_projects_locations_workflows_executions_callbacks_list_builder, workflowexecutions_projects_locations_workflows_executions_callbacks_list_task,
    workflowexecutions_projects_locations_workflows_executions_step_entries_get_builder, workflowexecutions_projects_locations_workflows_executions_step_entries_get_task,
    workflowexecutions_projects_locations_workflows_executions_step_entries_list_builder, workflowexecutions_projects_locations_workflows_executions_step_entries_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::workflowexecutions::Empty;
use crate::providers::gcp::clients::workflowexecutions::Execution;
use crate::providers::gcp::clients::workflowexecutions::ExportDataResponse;
use crate::providers::gcp::clients::workflowexecutions::ListCallbacksResponse;
use crate::providers::gcp::clients::workflowexecutions::ListExecutionsResponse;
use crate::providers::gcp::clients::workflowexecutions::ListStepEntriesResponse;
use crate::providers::gcp::clients::workflowexecutions::StepEntry;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsCallbacksListArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsCancelArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsCreateArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsDeleteExecutionHistoryArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsExportDataArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsGetArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsListArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsStepEntriesGetArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsExecutionsStepEntriesListArgs;
use crate::providers::gcp::clients::workflowexecutions::WorkflowexecutionsProjectsLocationsWorkflowsTriggerPubsubExecutionArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WorkflowexecutionsProvider with automatic state tracking.
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
/// let provider = WorkflowexecutionsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct WorkflowexecutionsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> WorkflowexecutionsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new WorkflowexecutionsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Workflowexecutions projects locations workflows trigger pubsub execution.
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
    pub fn workflowexecutions_projects_locations_workflows_trigger_pubsub_execution(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsTriggerPubsubExecutionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_trigger_pubsub_execution_builder(
            &self.http_client,
            &args.workflow,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_trigger_pubsub_execution_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions cancel.
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
    pub fn workflowexecutions_projects_locations_workflows_executions_cancel(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions create.
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
    pub fn workflowexecutions_projects_locations_workflows_executions_create(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions delete execution history.
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
    pub fn workflowexecutions_projects_locations_workflows_executions_delete_execution_history(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsDeleteExecutionHistoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_delete_execution_history_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_delete_execution_history_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions export data.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExportDataResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workflowexecutions_projects_locations_workflows_executions_export_data(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsExportDataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExportDataResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_export_data_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_export_data_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions get.
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
    pub fn workflowexecutions_projects_locations_workflows_executions_get(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions list.
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
    pub fn workflowexecutions_projects_locations_workflows_executions_list(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExecutionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions callbacks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCallbacksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workflowexecutions_projects_locations_workflows_executions_callbacks_list(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsCallbacksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCallbacksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_callbacks_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_callbacks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions step entries get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StepEntry result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workflowexecutions_projects_locations_workflows_executions_step_entries_get(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsStepEntriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StepEntry, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_step_entries_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_step_entries_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workflowexecutions projects locations workflows executions step entries list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStepEntriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workflowexecutions_projects_locations_workflows_executions_step_entries_list(
        &self,
        args: &WorkflowexecutionsProjectsLocationsWorkflowsExecutionsStepEntriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStepEntriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workflowexecutions_projects_locations_workflows_executions_step_entries_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.skip,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = workflowexecutions_projects_locations_workflows_executions_step_entries_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
