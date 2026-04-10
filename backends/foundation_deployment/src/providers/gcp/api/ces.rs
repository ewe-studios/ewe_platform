//! CesProvider - State-aware ces API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       ces API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::ces::{
    ces_projects_locations_get_builder, ces_projects_locations_get_task,
    ces_projects_locations_list_builder, ces_projects_locations_list_task,
    ces_projects_locations_apps_create_builder, ces_projects_locations_apps_create_task,
    ces_projects_locations_apps_delete_builder, ces_projects_locations_apps_delete_task,
    ces_projects_locations_apps_execute_tool_builder, ces_projects_locations_apps_execute_tool_task,
    ces_projects_locations_apps_export_app_builder, ces_projects_locations_apps_export_app_task,
    ces_projects_locations_apps_get_builder, ces_projects_locations_apps_get_task,
    ces_projects_locations_apps_import_app_builder, ces_projects_locations_apps_import_app_task,
    ces_projects_locations_apps_list_builder, ces_projects_locations_apps_list_task,
    ces_projects_locations_apps_patch_builder, ces_projects_locations_apps_patch_task,
    ces_projects_locations_apps_retrieve_tool_schema_builder, ces_projects_locations_apps_retrieve_tool_schema_task,
    ces_projects_locations_apps_agents_create_builder, ces_projects_locations_apps_agents_create_task,
    ces_projects_locations_apps_agents_delete_builder, ces_projects_locations_apps_agents_delete_task,
    ces_projects_locations_apps_agents_get_builder, ces_projects_locations_apps_agents_get_task,
    ces_projects_locations_apps_agents_list_builder, ces_projects_locations_apps_agents_list_task,
    ces_projects_locations_apps_agents_patch_builder, ces_projects_locations_apps_agents_patch_task,
    ces_projects_locations_apps_changelogs_get_builder, ces_projects_locations_apps_changelogs_get_task,
    ces_projects_locations_apps_changelogs_list_builder, ces_projects_locations_apps_changelogs_list_task,
    ces_projects_locations_apps_conversations_batch_delete_builder, ces_projects_locations_apps_conversations_batch_delete_task,
    ces_projects_locations_apps_conversations_delete_builder, ces_projects_locations_apps_conversations_delete_task,
    ces_projects_locations_apps_conversations_get_builder, ces_projects_locations_apps_conversations_get_task,
    ces_projects_locations_apps_conversations_list_builder, ces_projects_locations_apps_conversations_list_task,
    ces_projects_locations_apps_deployments_create_builder, ces_projects_locations_apps_deployments_create_task,
    ces_projects_locations_apps_deployments_delete_builder, ces_projects_locations_apps_deployments_delete_task,
    ces_projects_locations_apps_deployments_get_builder, ces_projects_locations_apps_deployments_get_task,
    ces_projects_locations_apps_deployments_list_builder, ces_projects_locations_apps_deployments_list_task,
    ces_projects_locations_apps_deployments_patch_builder, ces_projects_locations_apps_deployments_patch_task,
    ces_projects_locations_apps_examples_create_builder, ces_projects_locations_apps_examples_create_task,
    ces_projects_locations_apps_examples_delete_builder, ces_projects_locations_apps_examples_delete_task,
    ces_projects_locations_apps_examples_get_builder, ces_projects_locations_apps_examples_get_task,
    ces_projects_locations_apps_examples_list_builder, ces_projects_locations_apps_examples_list_task,
    ces_projects_locations_apps_examples_patch_builder, ces_projects_locations_apps_examples_patch_task,
    ces_projects_locations_apps_guardrails_create_builder, ces_projects_locations_apps_guardrails_create_task,
    ces_projects_locations_apps_guardrails_delete_builder, ces_projects_locations_apps_guardrails_delete_task,
    ces_projects_locations_apps_guardrails_get_builder, ces_projects_locations_apps_guardrails_get_task,
    ces_projects_locations_apps_guardrails_list_builder, ces_projects_locations_apps_guardrails_list_task,
    ces_projects_locations_apps_guardrails_patch_builder, ces_projects_locations_apps_guardrails_patch_task,
    ces_projects_locations_apps_sessions_generate_chat_token_builder, ces_projects_locations_apps_sessions_generate_chat_token_task,
    ces_projects_locations_apps_sessions_run_session_builder, ces_projects_locations_apps_sessions_run_session_task,
    ces_projects_locations_apps_sessions_stream_run_session_builder, ces_projects_locations_apps_sessions_stream_run_session_task,
    ces_projects_locations_apps_tools_create_builder, ces_projects_locations_apps_tools_create_task,
    ces_projects_locations_apps_tools_delete_builder, ces_projects_locations_apps_tools_delete_task,
    ces_projects_locations_apps_tools_get_builder, ces_projects_locations_apps_tools_get_task,
    ces_projects_locations_apps_tools_list_builder, ces_projects_locations_apps_tools_list_task,
    ces_projects_locations_apps_tools_patch_builder, ces_projects_locations_apps_tools_patch_task,
    ces_projects_locations_apps_toolsets_create_builder, ces_projects_locations_apps_toolsets_create_task,
    ces_projects_locations_apps_toolsets_delete_builder, ces_projects_locations_apps_toolsets_delete_task,
    ces_projects_locations_apps_toolsets_get_builder, ces_projects_locations_apps_toolsets_get_task,
    ces_projects_locations_apps_toolsets_list_builder, ces_projects_locations_apps_toolsets_list_task,
    ces_projects_locations_apps_toolsets_patch_builder, ces_projects_locations_apps_toolsets_patch_task,
    ces_projects_locations_apps_toolsets_retrieve_tools_builder, ces_projects_locations_apps_toolsets_retrieve_tools_task,
    ces_projects_locations_apps_versions_create_builder, ces_projects_locations_apps_versions_create_task,
    ces_projects_locations_apps_versions_delete_builder, ces_projects_locations_apps_versions_delete_task,
    ces_projects_locations_apps_versions_get_builder, ces_projects_locations_apps_versions_get_task,
    ces_projects_locations_apps_versions_list_builder, ces_projects_locations_apps_versions_list_task,
    ces_projects_locations_apps_versions_restore_builder, ces_projects_locations_apps_versions_restore_task,
    ces_projects_locations_operations_cancel_builder, ces_projects_locations_operations_cancel_task,
    ces_projects_locations_operations_delete_builder, ces_projects_locations_operations_delete_task,
    ces_projects_locations_operations_get_builder, ces_projects_locations_operations_get_task,
    ces_projects_locations_operations_list_builder, ces_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::ces::Agent;
use crate::providers::gcp::clients::ces::App;
use crate::providers::gcp::clients::ces::AppVersion;
use crate::providers::gcp::clients::ces::Changelog;
use crate::providers::gcp::clients::ces::Conversation;
use crate::providers::gcp::clients::ces::Deployment;
use crate::providers::gcp::clients::ces::Empty;
use crate::providers::gcp::clients::ces::Example;
use crate::providers::gcp::clients::ces::ExecuteToolResponse;
use crate::providers::gcp::clients::ces::GenerateChatTokenResponse;
use crate::providers::gcp::clients::ces::Guardrail;
use crate::providers::gcp::clients::ces::ListAgentsResponse;
use crate::providers::gcp::clients::ces::ListAppVersionsResponse;
use crate::providers::gcp::clients::ces::ListAppsResponse;
use crate::providers::gcp::clients::ces::ListChangelogsResponse;
use crate::providers::gcp::clients::ces::ListConversationsResponse;
use crate::providers::gcp::clients::ces::ListDeploymentsResponse;
use crate::providers::gcp::clients::ces::ListExamplesResponse;
use crate::providers::gcp::clients::ces::ListGuardrailsResponse;
use crate::providers::gcp::clients::ces::ListLocationsResponse;
use crate::providers::gcp::clients::ces::ListOperationsResponse;
use crate::providers::gcp::clients::ces::ListToolsResponse;
use crate::providers::gcp::clients::ces::ListToolsetsResponse;
use crate::providers::gcp::clients::ces::Location;
use crate::providers::gcp::clients::ces::Operation;
use crate::providers::gcp::clients::ces::RetrieveToolSchemaResponse;
use crate::providers::gcp::clients::ces::RetrieveToolsResponse;
use crate::providers::gcp::clients::ces::RunSessionResponse;
use crate::providers::gcp::clients::ces::Tool;
use crate::providers::gcp::clients::ces::Toolset;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsAgentsCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsAgentsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsAgentsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsAgentsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsAgentsPatchArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsChangelogsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsChangelogsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsConversationsBatchDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsConversationsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsConversationsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsConversationsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsDeploymentsCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsDeploymentsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsDeploymentsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsDeploymentsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsDeploymentsPatchArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsExamplesCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsExamplesDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsExamplesGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsExamplesListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsExamplesPatchArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsExecuteToolArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsExportAppArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsGuardrailsCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsGuardrailsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsGuardrailsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsGuardrailsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsGuardrailsPatchArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsImportAppArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsPatchArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsRetrieveToolSchemaArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsSessionsGenerateChatTokenArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsSessionsRunSessionArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsSessionsStreamRunSessionArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsPatchArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsetsCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsetsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsetsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsetsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsetsPatchArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsToolsetsRetrieveToolsArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsVersionsCreateArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsVersionsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsVersionsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsVersionsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsAppsVersionsRestoreArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsListArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::ces::CesProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CesProvider with automatic state tracking.
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
/// let provider = CesProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CesProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Ces projects locations get.
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
    pub fn ces_projects_locations_get(
        &self,
        args: &CesProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations list.
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
    pub fn ces_projects_locations_list(
        &self,
        args: &CesProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps create.
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
    pub fn ces_projects_locations_apps_create(
        &self,
        args: &CesProjectsLocationsAppsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_create_builder(
            &self.http_client,
            &args.parent,
            &args.appId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps delete.
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
    pub fn ces_projects_locations_apps_delete(
        &self,
        args: &CesProjectsLocationsAppsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps execute tool.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteToolResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_execute_tool(
        &self,
        args: &CesProjectsLocationsAppsExecuteToolArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteToolResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_execute_tool_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_execute_tool_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps export app.
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
    pub fn ces_projects_locations_apps_export_app(
        &self,
        args: &CesProjectsLocationsAppsExportAppArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_export_app_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_export_app_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the App result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_get(
        &self,
        args: &CesProjectsLocationsAppsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<App, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps import app.
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
    pub fn ces_projects_locations_apps_import_app(
        &self,
        args: &CesProjectsLocationsAppsImportAppArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_import_app_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_import_app_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_list(
        &self,
        args: &CesProjectsLocationsAppsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the App result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_patch(
        &self,
        args: &CesProjectsLocationsAppsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<App, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps retrieve tool schema.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetrieveToolSchemaResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_retrieve_tool_schema(
        &self,
        args: &CesProjectsLocationsAppsRetrieveToolSchemaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetrieveToolSchemaResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_retrieve_tool_schema_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_retrieve_tool_schema_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps agents create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Agent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_agents_create(
        &self,
        args: &CesProjectsLocationsAppsAgentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Agent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_agents_create_builder(
            &self.http_client,
            &args.parent,
            &args.agentId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_agents_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps agents delete.
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
    pub fn ces_projects_locations_apps_agents_delete(
        &self,
        args: &CesProjectsLocationsAppsAgentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_agents_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_agents_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps agents get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Agent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_agents_get(
        &self,
        args: &CesProjectsLocationsAppsAgentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Agent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_agents_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_agents_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps agents list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAgentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_agents_list(
        &self,
        args: &CesProjectsLocationsAppsAgentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAgentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_agents_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_agents_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps agents patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Agent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_agents_patch(
        &self,
        args: &CesProjectsLocationsAppsAgentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Agent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_agents_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_agents_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps changelogs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Changelog result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_changelogs_get(
        &self,
        args: &CesProjectsLocationsAppsChangelogsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Changelog, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_changelogs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_changelogs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps changelogs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListChangelogsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_changelogs_list(
        &self,
        args: &CesProjectsLocationsAppsChangelogsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListChangelogsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_changelogs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_changelogs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps conversations batch delete.
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
    pub fn ces_projects_locations_apps_conversations_batch_delete(
        &self,
        args: &CesProjectsLocationsAppsConversationsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_conversations_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_conversations_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps conversations delete.
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
    pub fn ces_projects_locations_apps_conversations_delete(
        &self,
        args: &CesProjectsLocationsAppsConversationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_conversations_delete_builder(
            &self.http_client,
            &args.name,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_conversations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps conversations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Conversation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_conversations_get(
        &self,
        args: &CesProjectsLocationsAppsConversationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Conversation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_conversations_get_builder(
            &self.http_client,
            &args.name,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_conversations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps conversations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConversationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_conversations_list(
        &self,
        args: &CesProjectsLocationsAppsConversationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConversationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_conversations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.source,
            &args.sources,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_conversations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps deployments create.
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
    pub fn ces_projects_locations_apps_deployments_create(
        &self,
        args: &CesProjectsLocationsAppsDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_deployments_create_builder(
            &self.http_client,
            &args.parent,
            &args.deploymentId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps deployments delete.
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
    pub fn ces_projects_locations_apps_deployments_delete(
        &self,
        args: &CesProjectsLocationsAppsDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_deployments_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps deployments get.
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
    pub fn ces_projects_locations_apps_deployments_get(
        &self,
        args: &CesProjectsLocationsAppsDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps deployments list.
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
    pub fn ces_projects_locations_apps_deployments_list(
        &self,
        args: &CesProjectsLocationsAppsDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps deployments patch.
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
    pub fn ces_projects_locations_apps_deployments_patch(
        &self,
        args: &CesProjectsLocationsAppsDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Deployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps examples create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Example result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_examples_create(
        &self,
        args: &CesProjectsLocationsAppsExamplesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Example, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_examples_create_builder(
            &self.http_client,
            &args.parent,
            &args.exampleId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_examples_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps examples delete.
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
    pub fn ces_projects_locations_apps_examples_delete(
        &self,
        args: &CesProjectsLocationsAppsExamplesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_examples_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_examples_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps examples get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Example result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_examples_get(
        &self,
        args: &CesProjectsLocationsAppsExamplesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Example, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_examples_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_examples_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps examples list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExamplesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_examples_list(
        &self,
        args: &CesProjectsLocationsAppsExamplesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExamplesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_examples_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_examples_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps examples patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Example result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_examples_patch(
        &self,
        args: &CesProjectsLocationsAppsExamplesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Example, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_examples_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_examples_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps guardrails create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Guardrail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_guardrails_create(
        &self,
        args: &CesProjectsLocationsAppsGuardrailsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Guardrail, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_guardrails_create_builder(
            &self.http_client,
            &args.parent,
            &args.guardrailId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_guardrails_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps guardrails delete.
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
    pub fn ces_projects_locations_apps_guardrails_delete(
        &self,
        args: &CesProjectsLocationsAppsGuardrailsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_guardrails_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_guardrails_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps guardrails get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Guardrail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_guardrails_get(
        &self,
        args: &CesProjectsLocationsAppsGuardrailsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Guardrail, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_guardrails_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_guardrails_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps guardrails list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGuardrailsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_guardrails_list(
        &self,
        args: &CesProjectsLocationsAppsGuardrailsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGuardrailsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_guardrails_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_guardrails_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps guardrails patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Guardrail result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_guardrails_patch(
        &self,
        args: &CesProjectsLocationsAppsGuardrailsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Guardrail, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_guardrails_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_guardrails_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps sessions generate chat token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateChatTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_sessions_generate_chat_token(
        &self,
        args: &CesProjectsLocationsAppsSessionsGenerateChatTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateChatTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_sessions_generate_chat_token_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_sessions_generate_chat_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps sessions run session.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunSessionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_sessions_run_session(
        &self,
        args: &CesProjectsLocationsAppsSessionsRunSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunSessionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_sessions_run_session_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_sessions_run_session_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps sessions stream run session.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunSessionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_sessions_stream_run_session(
        &self,
        args: &CesProjectsLocationsAppsSessionsStreamRunSessionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunSessionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_sessions_stream_run_session_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_sessions_stream_run_session_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps tools create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_tools_create(
        &self,
        args: &CesProjectsLocationsAppsToolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_tools_create_builder(
            &self.http_client,
            &args.parent,
            &args.toolId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_tools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps tools delete.
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
    pub fn ces_projects_locations_apps_tools_delete(
        &self,
        args: &CesProjectsLocationsAppsToolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_tools_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_tools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps tools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_tools_get(
        &self,
        args: &CesProjectsLocationsAppsToolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_tools_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_tools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps tools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListToolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_tools_list(
        &self,
        args: &CesProjectsLocationsAppsToolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListToolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_tools_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_tools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps tools patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Tool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_tools_patch(
        &self,
        args: &CesProjectsLocationsAppsToolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Tool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_tools_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_tools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps toolsets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Toolset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_toolsets_create(
        &self,
        args: &CesProjectsLocationsAppsToolsetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Toolset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_toolsets_create_builder(
            &self.http_client,
            &args.parent,
            &args.toolsetId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_toolsets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps toolsets delete.
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
    pub fn ces_projects_locations_apps_toolsets_delete(
        &self,
        args: &CesProjectsLocationsAppsToolsetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_toolsets_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_toolsets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps toolsets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Toolset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_toolsets_get(
        &self,
        args: &CesProjectsLocationsAppsToolsetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Toolset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_toolsets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_toolsets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps toolsets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListToolsetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_toolsets_list(
        &self,
        args: &CesProjectsLocationsAppsToolsetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListToolsetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_toolsets_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_toolsets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps toolsets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Toolset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_toolsets_patch(
        &self,
        args: &CesProjectsLocationsAppsToolsetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Toolset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_toolsets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_toolsets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps toolsets retrieve tools.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetrieveToolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_toolsets_retrieve_tools(
        &self,
        args: &CesProjectsLocationsAppsToolsetsRetrieveToolsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetrieveToolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_toolsets_retrieve_tools_builder(
            &self.http_client,
            &args.toolset,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_toolsets_retrieve_tools_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps versions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn ces_projects_locations_apps_versions_create(
        &self,
        args: &CesProjectsLocationsAppsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.appVersionId,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps versions delete.
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
    pub fn ces_projects_locations_apps_versions_delete(
        &self,
        args: &CesProjectsLocationsAppsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_versions_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_versions_get(
        &self,
        args: &CesProjectsLocationsAppsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAppVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn ces_projects_locations_apps_versions_list(
        &self,
        args: &CesProjectsLocationsAppsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAppVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations apps versions restore.
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
    pub fn ces_projects_locations_apps_versions_restore(
        &self,
        args: &CesProjectsLocationsAppsVersionsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_apps_versions_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_apps_versions_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations operations cancel.
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
    pub fn ces_projects_locations_operations_cancel(
        &self,
        args: &CesProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations operations delete.
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
    pub fn ces_projects_locations_operations_delete(
        &self,
        args: &CesProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations operations get.
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
    pub fn ces_projects_locations_operations_get(
        &self,
        args: &CesProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Ces projects locations operations list.
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
    pub fn ces_projects_locations_operations_list(
        &self,
        args: &CesProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = ces_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = ces_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
