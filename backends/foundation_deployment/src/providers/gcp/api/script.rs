//! ScriptProvider - State-aware script API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       script API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::script::{
    script_processes_list_builder, script_processes_list_task,
    script_processes_list_script_processes_builder, script_processes_list_script_processes_task,
    script_projects_create_builder, script_projects_create_task,
    script_projects_get_builder, script_projects_get_task,
    script_projects_get_content_builder, script_projects_get_content_task,
    script_projects_get_metrics_builder, script_projects_get_metrics_task,
    script_projects_update_content_builder, script_projects_update_content_task,
    script_projects_deployments_create_builder, script_projects_deployments_create_task,
    script_projects_deployments_delete_builder, script_projects_deployments_delete_task,
    script_projects_deployments_get_builder, script_projects_deployments_get_task,
    script_projects_deployments_list_builder, script_projects_deployments_list_task,
    script_projects_deployments_update_builder, script_projects_deployments_update_task,
    script_projects_versions_create_builder, script_projects_versions_create_task,
    script_projects_versions_get_builder, script_projects_versions_get_task,
    script_projects_versions_list_builder, script_projects_versions_list_task,
    script_scripts_run_builder, script_scripts_run_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::script::Content;
use crate::providers::gcp::clients::script::Deployment;
use crate::providers::gcp::clients::script::Empty;
use crate::providers::gcp::clients::script::ListDeploymentsResponse;
use crate::providers::gcp::clients::script::ListScriptProcessesResponse;
use crate::providers::gcp::clients::script::ListUserProcessesResponse;
use crate::providers::gcp::clients::script::ListVersionsResponse;
use crate::providers::gcp::clients::script::Metrics;
use crate::providers::gcp::clients::script::Operation;
use crate::providers::gcp::clients::script::Project;
use crate::providers::gcp::clients::script::Version;
use crate::providers::gcp::clients::script::ScriptProcessesListArgs;
use crate::providers::gcp::clients::script::ScriptProcessesListScriptProcessesArgs;
use crate::providers::gcp::clients::script::ScriptProjectsCreateArgs;
use crate::providers::gcp::clients::script::ScriptProjectsDeploymentsCreateArgs;
use crate::providers::gcp::clients::script::ScriptProjectsDeploymentsDeleteArgs;
use crate::providers::gcp::clients::script::ScriptProjectsDeploymentsGetArgs;
use crate::providers::gcp::clients::script::ScriptProjectsDeploymentsListArgs;
use crate::providers::gcp::clients::script::ScriptProjectsDeploymentsUpdateArgs;
use crate::providers::gcp::clients::script::ScriptProjectsGetArgs;
use crate::providers::gcp::clients::script::ScriptProjectsGetContentArgs;
use crate::providers::gcp::clients::script::ScriptProjectsGetMetricsArgs;
use crate::providers::gcp::clients::script::ScriptProjectsUpdateContentArgs;
use crate::providers::gcp::clients::script::ScriptProjectsVersionsCreateArgs;
use crate::providers::gcp::clients::script::ScriptProjectsVersionsGetArgs;
use crate::providers::gcp::clients::script::ScriptProjectsVersionsListArgs;
use crate::providers::gcp::clients::script::ScriptScriptsRunArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ScriptProvider with automatic state tracking.
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
/// let provider = ScriptProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ScriptProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ScriptProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ScriptProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Script processes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUserProcessesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn script_processes_list(
        &self,
        args: &ScriptProcessesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUserProcessesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_processes_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.userProcessFilter.deploymentId,
            &args.userProcessFilter.endTime,
            &args.userProcessFilter.functionName,
            &args.userProcessFilter.projectName,
            &args.userProcessFilter.scriptId,
            &args.userProcessFilter.startTime,
            &args.userProcessFilter.statuses,
            &args.userProcessFilter.types,
            &args.userProcessFilter.userAccessLevels,
        )
        .map_err(ProviderError::Api)?;

        let task = script_processes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script processes list script processes.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListScriptProcessesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn script_processes_list_script_processes(
        &self,
        args: &ScriptProcessesListScriptProcessesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListScriptProcessesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_processes_list_script_processes_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
            &args.scriptId,
            &args.scriptProcessFilter.deploymentId,
            &args.scriptProcessFilter.endTime,
            &args.scriptProcessFilter.functionName,
            &args.scriptProcessFilter.startTime,
            &args.scriptProcessFilter.statuses,
            &args.scriptProcessFilter.types,
            &args.scriptProcessFilter.userAccessLevels,
        )
        .map_err(ProviderError::Api)?;

        let task = script_processes_list_script_processes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Project result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn script_projects_create(
        &self,
        args: &ScriptProjectsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Project, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Project result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn script_projects_get(
        &self,
        args: &ScriptProjectsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Project, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_get_builder(
            &self.http_client,
            &args.scriptId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects get content.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Content result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn script_projects_get_content(
        &self,
        args: &ScriptProjectsGetContentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Content, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_get_content_builder(
            &self.http_client,
            &args.scriptId,
            &args.versionNumber,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_get_content_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects get metrics.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Metrics result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn script_projects_get_metrics(
        &self,
        args: &ScriptProjectsGetMetricsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Metrics, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_get_metrics_builder(
            &self.http_client,
            &args.scriptId,
            &args.metricsFilter.deploymentId,
            &args.metricsGranularity,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_get_metrics_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects update content.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Content result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn script_projects_update_content(
        &self,
        args: &ScriptProjectsUpdateContentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Content, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_update_content_builder(
            &self.http_client,
            &args.scriptId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_update_content_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects deployments create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn script_projects_deployments_create(
        &self,
        args: &ScriptProjectsDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_deployments_create_builder(
            &self.http_client,
            &args.scriptId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects deployments delete.
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
    pub fn script_projects_deployments_delete(
        &self,
        args: &ScriptProjectsDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_deployments_delete_builder(
            &self.http_client,
            &args.scriptId,
            &args.deploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects deployments get.
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
    pub fn script_projects_deployments_get(
        &self,
        args: &ScriptProjectsDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_deployments_get_builder(
            &self.http_client,
            &args.scriptId,
            &args.deploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects deployments list.
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
    pub fn script_projects_deployments_list(
        &self,
        args: &ScriptProjectsDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_deployments_list_builder(
            &self.http_client,
            &args.scriptId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects deployments update.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn script_projects_deployments_update(
        &self,
        args: &ScriptProjectsDeploymentsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_deployments_update_builder(
            &self.http_client,
            &args.scriptId,
            &args.deploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_deployments_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn script_projects_versions_create(
        &self,
        args: &ScriptProjectsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_versions_create_builder(
            &self.http_client,
            &args.scriptId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Version result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn script_projects_versions_get(
        &self,
        args: &ScriptProjectsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Version, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_versions_get_builder(
            &self.http_client,
            &args.scriptId,
            &args.versionNumber,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script projects versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn script_projects_versions_list(
        &self,
        args: &ScriptProjectsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_projects_versions_list_builder(
            &self.http_client,
            &args.scriptId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = script_projects_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Script scripts run.
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
    pub fn script_scripts_run(
        &self,
        args: &ScriptScriptsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = script_scripts_run_builder(
            &self.http_client,
            &args.scriptId,
        )
        .map_err(ProviderError::Api)?;

        let task = script_scripts_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
