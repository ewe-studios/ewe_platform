//! ConnectorsProvider - State-aware connectors API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       connectors API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::connectors::{
    connectors_projects_locations_connections_check_readiness_builder, connectors_projects_locations_connections_check_readiness_task,
    connectors_projects_locations_connections_check_status_builder, connectors_projects_locations_connections_check_status_task,
    connectors_projects_locations_connections_exchange_auth_code_builder, connectors_projects_locations_connections_exchange_auth_code_task,
    connectors_projects_locations_connections_execute_sql_query_builder, connectors_projects_locations_connections_execute_sql_query_task,
    connectors_projects_locations_connections_generate_connection_toolspec_override_builder, connectors_projects_locations_connections_generate_connection_toolspec_override_task,
    connectors_projects_locations_connections_list_custom_tool_names_builder, connectors_projects_locations_connections_list_custom_tool_names_task,
    connectors_projects_locations_connections_refresh_access_token_builder, connectors_projects_locations_connections_refresh_access_token_task,
    connectors_projects_locations_connections_tools_builder, connectors_projects_locations_connections_tools_task,
    connectors_projects_locations_connections_actions_execute_builder, connectors_projects_locations_connections_actions_execute_task,
    connectors_projects_locations_connections_actions_get_builder, connectors_projects_locations_connections_actions_get_task,
    connectors_projects_locations_connections_actions_list_builder, connectors_projects_locations_connections_actions_list_task,
    connectors_projects_locations_connections_entity_types_get_builder, connectors_projects_locations_connections_entity_types_get_task,
    connectors_projects_locations_connections_entity_types_list_builder, connectors_projects_locations_connections_entity_types_list_task,
    connectors_projects_locations_connections_entity_types_entities_create_builder, connectors_projects_locations_connections_entity_types_entities_create_task,
    connectors_projects_locations_connections_entity_types_entities_delete_builder, connectors_projects_locations_connections_entity_types_entities_delete_task,
    connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions_builder, connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions_task,
    connectors_projects_locations_connections_entity_types_entities_get_builder, connectors_projects_locations_connections_entity_types_entities_get_task,
    connectors_projects_locations_connections_entity_types_entities_list_builder, connectors_projects_locations_connections_entity_types_entities_list_task,
    connectors_projects_locations_connections_entity_types_entities_patch_builder, connectors_projects_locations_connections_entity_types_entities_patch_task,
    connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions_builder, connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions_task,
    connectors_projects_locations_connections_resources_get_builder, connectors_projects_locations_connections_resources_get_task,
    connectors_projects_locations_connections_resources_get_resource_post_builder, connectors_projects_locations_connections_resources_get_resource_post_task,
    connectors_projects_locations_connections_resources_list_builder, connectors_projects_locations_connections_resources_list_task,
    connectors_projects_locations_connections_tools_execute_builder, connectors_projects_locations_connections_tools_execute_task,
    connectors_projects_locations_connections_tools_list_builder, connectors_projects_locations_connections_tools_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::connectors::Action;
use crate::providers::gcp::clients::connectors::CheckReadinessResponse;
use crate::providers::gcp::clients::connectors::CheckStatusResponse;
use crate::providers::gcp::clients::connectors::Empty;
use crate::providers::gcp::clients::connectors::Entity;
use crate::providers::gcp::clients::connectors::EntityType;
use crate::providers::gcp::clients::connectors::ExchangeAuthCodeResponse;
use crate::providers::gcp::clients::connectors::ExecuteActionResponse;
use crate::providers::gcp::clients::connectors::ExecuteSqlQueryResponse;
use crate::providers::gcp::clients::connectors::ExecuteToolResponse;
use crate::providers::gcp::clients::connectors::GenerateCustomToolspecResponse;
use crate::providers::gcp::clients::connectors::GetResourceResponse;
use crate::providers::gcp::clients::connectors::ListActionsResponse;
use crate::providers::gcp::clients::connectors::ListCustomToolNamesResponse;
use crate::providers::gcp::clients::connectors::ListEntitiesResponse;
use crate::providers::gcp::clients::connectors::ListEntityTypesResponse;
use crate::providers::gcp::clients::connectors::ListResourcesResponse;
use crate::providers::gcp::clients::connectors::ListToolsResponse;
use crate::providers::gcp::clients::connectors::RefreshAccessTokenResponse;
use crate::providers::gcp::clients::connectors::UpdateEntitiesWithConditionsResponse;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsActionsExecuteArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsActionsGetArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsActionsListArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsCheckReadinessArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsCheckStatusArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesCreateArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesDeleteArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesDeleteEntitiesWithConditionsArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesGetArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesListArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesPatchArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesUpdateEntitiesWithConditionsArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesGetArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesListArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsExchangeAuthCodeArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsExecuteSqlQueryArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsGenerateConnectionToolspecOverrideArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsListCustomToolNamesArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsRefreshAccessTokenArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsResourcesGetArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsResourcesGetResourcePostArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsResourcesListArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsToolsArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsToolsExecuteArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsToolsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ConnectorsProvider with automatic state tracking.
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
/// let provider = ConnectorsProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ConnectorsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ConnectorsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ConnectorsProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ConnectorsProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Connectors projects locations connections check readiness.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckReadinessResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_check_readiness(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsCheckReadinessArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckReadinessResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_check_readiness_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_check_readiness_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections check status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_check_status(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsCheckStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_check_status_builder(
            &self.http_client,
            &args.name,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_check_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections exchange auth code.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExchangeAuthCodeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_exchange_auth_code(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsExchangeAuthCodeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExchangeAuthCodeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_exchange_auth_code_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_exchange_auth_code_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections execute sql query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteSqlQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_execute_sql_query(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsExecuteSqlQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteSqlQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_execute_sql_query_builder(
            &self.http_client,
            &args.connection,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_execute_sql_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections generate connection toolspec override.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateCustomToolspecResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_generate_connection_toolspec_override(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsGenerateConnectionToolspecOverrideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateCustomToolspecResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_generate_connection_toolspec_override_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_generate_connection_toolspec_override_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections list custom tool names.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomToolNamesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_list_custom_tool_names(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsListCustomToolNamesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomToolNamesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_list_custom_tool_names_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_list_custom_tool_names_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections refresh access token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RefreshAccessTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_refresh_access_token(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsRefreshAccessTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RefreshAccessTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_refresh_access_token_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_refresh_access_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections tools.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_tools(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsToolsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListToolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_tools_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_tools_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections actions execute.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteActionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_actions_execute(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsActionsExecuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteActionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_actions_execute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_actions_execute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections actions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Action result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_actions_get(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsActionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Action, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_actions_get_builder(
            &self.http_client,
            &args.name,
            &args.executionConfig_headers,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_actions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections actions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListActionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_actions_list(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsActionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListActionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_actions_list_builder(
            &self.http_client,
            &args.parent,
            &args.executionConfig_headers,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_actions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntityType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_entity_types_get(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntityType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_get_builder(
            &self.http_client,
            &args.name,
            &args.contextMetadata,
            &args.executionConfig_headers,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEntityTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_entity_types_list(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEntityTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.executionConfig_headers,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types entities create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_entity_types_entities_create(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_entities_create_builder(
            &self.http_client,
            &args.parent,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types entities delete.
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
    pub fn connectors_projects_locations_connections_entity_types_entities_delete(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_entities_delete_builder(
            &self.http_client,
            &args.name,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types entities delete entities with conditions.
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
    pub fn connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesDeleteEntitiesWithConditionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions_builder(
            &self.http_client,
            &args.entityType,
            &args.conditions,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types entities get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_entity_types_entities_get(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_entities_get_builder(
            &self.http_client,
            &args.name,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types entities list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEntitiesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_entity_types_entities_list(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEntitiesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_entities_list_builder(
            &self.http_client,
            &args.parent,
            &args.conditions,
            &args.executionConfig_headers,
            &args.pageSize,
            &args.pageToken,
            &args.sortBy,
            &args.sortOrder,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types entities patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entity result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_entity_types_entities_patch(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entity, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_entities_patch_builder(
            &self.http_client,
            &args.name,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections entity types entities update entities with conditions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateEntitiesWithConditionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesUpdateEntitiesWithConditionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateEntitiesWithConditionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions_builder(
            &self.http_client,
            &args.entityType,
            &args.conditions,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections resources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetResourceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_resources_get(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsResourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetResourceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_resources_get_builder(
            &self.http_client,
            &args.name,
            &args.executionConfig_headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_resources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections resources get resource post.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetResourceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_resources_get_resource_post(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsResourcesGetResourcePostArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetResourceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_resources_get_resource_post_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_resources_get_resource_post_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections resources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn connectors_projects_locations_connections_resources_list(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsResourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_resources_list_builder(
            &self.http_client,
            &args.parent,
            &args.executionConfig_headers,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_resources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections tools execute.
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
    pub fn connectors_projects_locations_connections_tools_execute(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsToolsExecuteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteToolResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_tools_execute_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_tools_execute_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections tools list.
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
    pub fn connectors_projects_locations_connections_tools_list(
        &self,
        args: &ConnectorsProjectsLocationsConnectionsToolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListToolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = connectors_projects_locations_connections_tools_list_builder(
            &self.http_client,
            &args.parent,
            &args.executionConfig_headers,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_tools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
