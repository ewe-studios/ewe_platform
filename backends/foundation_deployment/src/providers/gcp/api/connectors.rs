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
    connectors_projects_locations_connections_exchange_auth_code_builder, connectors_projects_locations_connections_exchange_auth_code_task,
    connectors_projects_locations_connections_execute_sql_query_builder, connectors_projects_locations_connections_execute_sql_query_task,
    connectors_projects_locations_connections_generate_connection_toolspec_override_builder, connectors_projects_locations_connections_generate_connection_toolspec_override_task,
    connectors_projects_locations_connections_refresh_access_token_builder, connectors_projects_locations_connections_refresh_access_token_task,
    connectors_projects_locations_connections_tools_builder, connectors_projects_locations_connections_tools_task,
    connectors_projects_locations_connections_actions_execute_builder, connectors_projects_locations_connections_actions_execute_task,
    connectors_projects_locations_connections_entity_types_entities_create_builder, connectors_projects_locations_connections_entity_types_entities_create_task,
    connectors_projects_locations_connections_entity_types_entities_delete_builder, connectors_projects_locations_connections_entity_types_entities_delete_task,
    connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions_builder, connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions_task,
    connectors_projects_locations_connections_entity_types_entities_patch_builder, connectors_projects_locations_connections_entity_types_entities_patch_task,
    connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions_builder, connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions_task,
    connectors_projects_locations_connections_resources_get_resource_post_builder, connectors_projects_locations_connections_resources_get_resource_post_task,
    connectors_projects_locations_connections_tools_execute_builder, connectors_projects_locations_connections_tools_execute_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::connectors::Empty;
use crate::providers::gcp::clients::connectors::Entity;
use crate::providers::gcp::clients::connectors::ExchangeAuthCodeResponse;
use crate::providers::gcp::clients::connectors::ExecuteActionResponse;
use crate::providers::gcp::clients::connectors::ExecuteSqlQueryResponse;
use crate::providers::gcp::clients::connectors::ExecuteToolResponse;
use crate::providers::gcp::clients::connectors::GenerateCustomToolspecResponse;
use crate::providers::gcp::clients::connectors::GetResourceResponse;
use crate::providers::gcp::clients::connectors::ListToolsResponse;
use crate::providers::gcp::clients::connectors::RefreshAccessTokenResponse;
use crate::providers::gcp::clients::connectors::UpdateEntitiesWithConditionsResponse;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsActionsExecuteArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesCreateArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesDeleteArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesDeleteEntitiesWithConditionsArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesPatchArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsEntityTypesEntitiesUpdateEntitiesWithConditionsArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsExchangeAuthCodeArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsExecuteSqlQueryArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsGenerateConnectionToolspecOverrideArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsRefreshAccessTokenArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsResourcesGetResourcePostArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsToolsArgs;
use crate::providers::gcp::clients::connectors::ConnectorsProjectsLocationsConnectionsToolsExecuteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ConnectorsProvider with automatic state tracking.
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
/// let provider = ConnectorsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ConnectorsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ConnectorsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ConnectorsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.executionConfig.headers,
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
            &args.executionConfig.headers,
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
            &args.executionConfig.headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_delete_entities_with_conditions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.executionConfig.headers,
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
            &args.executionConfig.headers,
        )
        .map_err(ProviderError::Api)?;

        let task = connectors_projects_locations_connections_entity_types_entities_update_entities_with_conditions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Connectors projects locations connections resources get resource post.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
