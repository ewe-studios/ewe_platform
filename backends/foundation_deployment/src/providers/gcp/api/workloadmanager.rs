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
    workloadmanager_projects_locations_get_builder, workloadmanager_projects_locations_get_task,
    workloadmanager_projects_locations_list_builder, workloadmanager_projects_locations_list_task,
    workloadmanager_projects_locations_deployments_create_builder, workloadmanager_projects_locations_deployments_create_task,
    workloadmanager_projects_locations_deployments_delete_builder, workloadmanager_projects_locations_deployments_delete_task,
    workloadmanager_projects_locations_deployments_get_builder, workloadmanager_projects_locations_deployments_get_task,
    workloadmanager_projects_locations_deployments_list_builder, workloadmanager_projects_locations_deployments_list_task,
    workloadmanager_projects_locations_deployments_actuations_create_builder, workloadmanager_projects_locations_deployments_actuations_create_task,
    workloadmanager_projects_locations_deployments_actuations_delete_builder, workloadmanager_projects_locations_deployments_actuations_delete_task,
    workloadmanager_projects_locations_deployments_actuations_get_builder, workloadmanager_projects_locations_deployments_actuations_get_task,
    workloadmanager_projects_locations_deployments_actuations_list_builder, workloadmanager_projects_locations_deployments_actuations_list_task,
    workloadmanager_projects_locations_discoveredprofiles_get_builder, workloadmanager_projects_locations_discoveredprofiles_get_task,
    workloadmanager_projects_locations_discoveredprofiles_list_builder, workloadmanager_projects_locations_discoveredprofiles_list_task,
    workloadmanager_projects_locations_discoveredprofiles_health_get_builder, workloadmanager_projects_locations_discoveredprofiles_health_get_task,
    workloadmanager_projects_locations_evaluations_create_builder, workloadmanager_projects_locations_evaluations_create_task,
    workloadmanager_projects_locations_evaluations_delete_builder, workloadmanager_projects_locations_evaluations_delete_task,
    workloadmanager_projects_locations_evaluations_get_builder, workloadmanager_projects_locations_evaluations_get_task,
    workloadmanager_projects_locations_evaluations_list_builder, workloadmanager_projects_locations_evaluations_list_task,
    workloadmanager_projects_locations_evaluations_patch_builder, workloadmanager_projects_locations_evaluations_patch_task,
    workloadmanager_projects_locations_evaluations_executions_delete_builder, workloadmanager_projects_locations_evaluations_executions_delete_task,
    workloadmanager_projects_locations_evaluations_executions_get_builder, workloadmanager_projects_locations_evaluations_executions_get_task,
    workloadmanager_projects_locations_evaluations_executions_list_builder, workloadmanager_projects_locations_evaluations_executions_list_task,
    workloadmanager_projects_locations_evaluations_executions_run_builder, workloadmanager_projects_locations_evaluations_executions_run_task,
    workloadmanager_projects_locations_evaluations_executions_results_list_builder, workloadmanager_projects_locations_evaluations_executions_results_list_task,
    workloadmanager_projects_locations_evaluations_executions_scanned_resources_list_builder, workloadmanager_projects_locations_evaluations_executions_scanned_resources_list_task,
    workloadmanager_projects_locations_insights_delete_builder, workloadmanager_projects_locations_insights_delete_task,
    workloadmanager_projects_locations_insights_write_insight_builder, workloadmanager_projects_locations_insights_write_insight_task,
    workloadmanager_projects_locations_operations_cancel_builder, workloadmanager_projects_locations_operations_cancel_task,
    workloadmanager_projects_locations_operations_delete_builder, workloadmanager_projects_locations_operations_delete_task,
    workloadmanager_projects_locations_operations_get_builder, workloadmanager_projects_locations_operations_get_task,
    workloadmanager_projects_locations_operations_list_builder, workloadmanager_projects_locations_operations_list_task,
    workloadmanager_projects_locations_rules_list_builder, workloadmanager_projects_locations_rules_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::workloadmanager::Actuation;
use crate::providers::gcp::clients::workloadmanager::Deployment;
use crate::providers::gcp::clients::workloadmanager::Empty;
use crate::providers::gcp::clients::workloadmanager::Evaluation;
use crate::providers::gcp::clients::workloadmanager::Execution;
use crate::providers::gcp::clients::workloadmanager::ListActuationsResponse;
use crate::providers::gcp::clients::workloadmanager::ListDeploymentsResponse;
use crate::providers::gcp::clients::workloadmanager::ListDiscoveredProfilesResponse;
use crate::providers::gcp::clients::workloadmanager::ListEvaluationsResponse;
use crate::providers::gcp::clients::workloadmanager::ListExecutionResultsResponse;
use crate::providers::gcp::clients::workloadmanager::ListExecutionsResponse;
use crate::providers::gcp::clients::workloadmanager::ListLocationsResponse;
use crate::providers::gcp::clients::workloadmanager::ListOperationsResponse;
use crate::providers::gcp::clients::workloadmanager::ListRulesResponse;
use crate::providers::gcp::clients::workloadmanager::ListScannedResourcesResponse;
use crate::providers::gcp::clients::workloadmanager::Location;
use crate::providers::gcp::clients::workloadmanager::Operation;
use crate::providers::gcp::clients::workloadmanager::WorkloadProfile;
use crate::providers::gcp::clients::workloadmanager::WorkloadProfileHealth;
use crate::providers::gcp::clients::workloadmanager::WriteInsightResponse;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsActuationsCreateArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsActuationsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsActuationsGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsActuationsListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsCreateArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDeploymentsListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDiscoveredprofilesGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDiscoveredprofilesHealthGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsDiscoveredprofilesListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsCreateArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsResultsListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsRunArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsExecutionsScannedResourcesListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsEvaluationsPatchArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsInsightsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsInsightsWriteInsightArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::workloadmanager::WorkloadmanagerProjectsLocationsRulesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WorkloadmanagerProvider with automatic state tracking.
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
/// let provider = WorkloadmanagerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct WorkloadmanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> WorkloadmanagerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new WorkloadmanagerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new WorkloadmanagerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Workloadmanager projects locations get.
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
    pub fn workloadmanager_projects_locations_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations list.
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
    pub fn workloadmanager_projects_locations_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workloadmanager projects locations deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Deployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_deployments_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_deployments_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workloadmanager projects locations deployments actuations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Actuation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_deployments_actuations_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsActuationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Actuation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_actuations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_actuations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations deployments actuations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListActuationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_deployments_actuations_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsDeploymentsActuationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListActuationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_deployments_actuations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_deployments_actuations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations discoveredprofiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkloadProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_discoveredprofiles_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsDiscoveredprofilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkloadProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_discoveredprofiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_discoveredprofiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations discoveredprofiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDiscoveredProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_discoveredprofiles_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsDiscoveredprofilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDiscoveredProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_discoveredprofiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_discoveredprofiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations discoveredprofiles health get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkloadProfileHealth result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_discoveredprofiles_health_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsDiscoveredprofilesHealthGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkloadProfileHealth, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_discoveredprofiles_health_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_discoveredprofiles_health_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workloadmanager projects locations evaluations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Evaluation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_evaluations_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Evaluation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEvaluationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_evaluations_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEvaluationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workloadmanager projects locations evaluations executions get.
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
    pub fn workloadmanager_projects_locations_evaluations_executions_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsExecutionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Execution, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_executions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_executions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations executions list.
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
    pub fn workloadmanager_projects_locations_evaluations_executions_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsExecutionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExecutionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_executions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_executions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workloadmanager projects locations evaluations executions results list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExecutionResultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_evaluations_executions_results_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsExecutionsResultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExecutionResultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_executions_results_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_executions_results_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations evaluations executions scanned resources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListScannedResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_evaluations_executions_scanned_resources_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsEvaluationsExecutionsScannedResourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListScannedResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_evaluations_executions_scanned_resources_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.rule,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_evaluations_executions_scanned_resources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Workloadmanager projects locations operations get.
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
    pub fn workloadmanager_projects_locations_operations_get(
        &self,
        args: &WorkloadmanagerProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations operations list.
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
    pub fn workloadmanager_projects_locations_operations_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Workloadmanager projects locations rules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn workloadmanager_projects_locations_rules_list(
        &self,
        args: &WorkloadmanagerProjectsLocationsRulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = workloadmanager_projects_locations_rules_list_builder(
            &self.http_client,
            &args.parent,
            &args.customRulesBucket,
            &args.evaluationType,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = workloadmanager_projects_locations_rules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
