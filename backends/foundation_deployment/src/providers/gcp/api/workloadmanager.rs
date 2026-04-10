//! WorkloadmanagerProvider - State-aware workloadmanager API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       workloadmanager API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::workloadmanager::{
    workloadmanager_projects_locations_deployments_create_builder, workloadmanager_projects_locations_deployments_create_task,
    workloadmanager_projects_locations_deployments_delete_builder, workloadmanager_projects_locations_deployments_delete_task,
    workloadmanager_projects_locations_deployments_actuations_create_builder, workloadmanager_projects_locations_deployments_actuations_create_task,
    workloadmanager_projects_locations_deployments_actuations_delete_builder, workloadmanager_projects_locations_deployments_actuations_delete_task,
    workloadmanager_projects_locations_evaluations_create_builder, workloadmanager_projects_locations_evaluations_create_task,
    workloadmanager_projects_locations_evaluations_delete_builder, workloadmanager_projects_locations_evaluations_delete_task,
    workloadmanager_projects_locations_evaluations_patch_builder, workloadmanager_projects_locations_evaluations_patch_task,
    workloadmanager_projects_locations_evaluations_executions_delete_builder, workloadmanager_projects_locations_evaluations_executions_delete_task,
    workloadmanager_projects_locations_evaluations_executions_run_builder, workloadmanager_projects_locations_evaluations_executions_run_task,
    workloadmanager_projects_locations_insights_delete_builder, workloadmanager_projects_locations_insights_delete_task,
    workloadmanager_projects_locations_insights_write_insight_builder, workloadmanager_projects_locations_insights_write_insight_task,
    workloadmanager_projects_locations_operations_cancel_builder, workloadmanager_projects_locations_operations_cancel_task,
    workloadmanager_projects_locations_operations_delete_builder, workloadmanager_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::workloadmanager::Empty;
use crate::providers::gcp::clients::workloadmanager::Operation;
use crate::providers::gcp::clients::workloadmanager::WriteInsightResponse;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsActuationsCreateArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsActuationsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsCreateArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsCreateArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsRunArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsPatchArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsInsightsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsInsightsWriteInsightArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WorkloadmanagerProvider with automatic state tracking.
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
/// let provider = WorkloadmanagerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct WorkloadmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> WorkloadmanagerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new WorkloadmanagerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Workloadmanager projects locations deployments create.
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
    pub fn workloadmanager_projects_locations_deployments_create(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.deploymentId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations deployments delete.
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
    pub fn workloadmanager_projects_locations_deployments_delete(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations deployments actuations create.
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
    pub fn workloadmanager_projects_locations_deployments_actuations_create(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsActuationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_actuations_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_actuations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations deployments actuations delete.
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
    pub fn workloadmanager_projects_locations_deployments_actuations_delete(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsActuationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_actuations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_actuations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations create.
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
    pub fn workloadmanager_projects_locations_evaluations_create(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_create_builder(
            &self.http_client,
            &args.parent,
            &args.evaluationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations delete.
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
    pub fn workloadmanager_projects_locations_evaluations_delete(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations patch.
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
    pub fn workloadmanager_projects_locations_evaluations_patch(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations executions delete.
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
    pub fn workloadmanager_projects_locations_evaluations_executions_delete(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsExecutionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_executions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_executions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations executions run.
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
    pub fn workloadmanager_projects_locations_evaluations_executions_run(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsExecutionsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_executions_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_executions_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations insights delete.
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
    pub fn workloadmanager_projects_locations_insights_delete(
        &self,
        args: &WorkloadmanagerProjectsLocationsInsightsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_insights_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_insights_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations insights write insight.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WriteInsightResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn workloadmanager_projects_locations_insights_write_insight(
        &self,
        args: &WorkloadmanagerProjectsLocationsInsightsWriteInsightArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WriteInsightResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_insights_write_insight_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_insights_write_insight_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations operations cancel.
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
    pub fn workloadmanager_projects_locations_operations_cancel(
        &self,
        args: &WorkloadmanagerProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations operations delete.
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
    pub fn workloadmanager_projects_locations_operations_delete(
        &self,
        args: &WorkloadmanagerProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
