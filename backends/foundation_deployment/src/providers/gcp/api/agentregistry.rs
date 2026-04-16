//! AgentregistryProvider - State-aware agentregistry API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       agentregistry API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::agentregistry::{
    agentregistry_projects_locations_get_builder, agentregistry_projects_locations_get_task,
    agentregistry_projects_locations_list_builder, agentregistry_projects_locations_list_task,
    agentregistry_projects_locations_agents_get_builder, agentregistry_projects_locations_agents_get_task,
    agentregistry_projects_locations_agents_list_builder, agentregistry_projects_locations_agents_list_task,
    agentregistry_projects_locations_agents_search_builder, agentregistry_projects_locations_agents_search_task,
    agentregistry_projects_locations_bindings_create_builder, agentregistry_projects_locations_bindings_create_task,
    agentregistry_projects_locations_bindings_delete_builder, agentregistry_projects_locations_bindings_delete_task,
    agentregistry_projects_locations_bindings_fetch_available_builder, agentregistry_projects_locations_bindings_fetch_available_task,
    agentregistry_projects_locations_bindings_get_builder, agentregistry_projects_locations_bindings_get_task,
    agentregistry_projects_locations_bindings_list_builder, agentregistry_projects_locations_bindings_list_task,
    agentregistry_projects_locations_bindings_patch_builder, agentregistry_projects_locations_bindings_patch_task,
    agentregistry_projects_locations_endpoints_get_builder, agentregistry_projects_locations_endpoints_get_task,
    agentregistry_projects_locations_endpoints_list_builder, agentregistry_projects_locations_endpoints_list_task,
    agentregistry_projects_locations_mcp_servers_get_builder, agentregistry_projects_locations_mcp_servers_get_task,
    agentregistry_projects_locations_mcp_servers_list_builder, agentregistry_projects_locations_mcp_servers_list_task,
    agentregistry_projects_locations_mcp_servers_search_builder, agentregistry_projects_locations_mcp_servers_search_task,
    agentregistry_projects_locations_operations_cancel_builder, agentregistry_projects_locations_operations_cancel_task,
    agentregistry_projects_locations_operations_delete_builder, agentregistry_projects_locations_operations_delete_task,
    agentregistry_projects_locations_operations_get_builder, agentregistry_projects_locations_operations_get_task,
    agentregistry_projects_locations_operations_list_builder, agentregistry_projects_locations_operations_list_task,
    agentregistry_projects_locations_services_create_builder, agentregistry_projects_locations_services_create_task,
    agentregistry_projects_locations_services_delete_builder, agentregistry_projects_locations_services_delete_task,
    agentregistry_projects_locations_services_get_builder, agentregistry_projects_locations_services_get_task,
    agentregistry_projects_locations_services_list_builder, agentregistry_projects_locations_services_list_task,
    agentregistry_projects_locations_services_patch_builder, agentregistry_projects_locations_services_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::agentregistry::Agent;
use crate::providers::gcp::clients::agentregistry::Binding;
use crate::providers::gcp::clients::agentregistry::Empty;
use crate::providers::gcp::clients::agentregistry::Endpoint;
use crate::providers::gcp::clients::agentregistry::FetchAvailableBindingsResponse;
use crate::providers::gcp::clients::agentregistry::ListAgentsResponse;
use crate::providers::gcp::clients::agentregistry::ListBindingsResponse;
use crate::providers::gcp::clients::agentregistry::ListEndpointsResponse;
use crate::providers::gcp::clients::agentregistry::ListLocationsResponse;
use crate::providers::gcp::clients::agentregistry::ListMcpServersResponse;
use crate::providers::gcp::clients::agentregistry::ListOperationsResponse;
use crate::providers::gcp::clients::agentregistry::ListServicesResponse;
use crate::providers::gcp::clients::agentregistry::Location;
use crate::providers::gcp::clients::agentregistry::McpServer;
use crate::providers::gcp::clients::agentregistry::Operation;
use crate::providers::gcp::clients::agentregistry::SearchAgentsResponse;
use crate::providers::gcp::clients::agentregistry::SearchMcpServersResponse;
use crate::providers::gcp::clients::agentregistry::Service;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsAgentsGetArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsAgentsListArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsAgentsSearchArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsBindingsCreateArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsBindingsDeleteArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsBindingsFetchAvailableArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsBindingsGetArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsBindingsListArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsBindingsPatchArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsEndpointsGetArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsEndpointsListArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsGetArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsListArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsMcpServersGetArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsMcpServersListArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsMcpServersSearchArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsServicesCreateArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsServicesDeleteArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsServicesGetArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsServicesListArgs;
use crate::providers::gcp::clients::agentregistry::AgentregistryProjectsLocationsServicesPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AgentregistryProvider with automatic state tracking.
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
/// let provider = AgentregistryProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AgentregistryProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AgentregistryProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AgentregistryProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AgentregistryProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Agentregistry projects locations get.
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
    pub fn agentregistry_projects_locations_get(
        &self,
        args: &AgentregistryProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations list.
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
    pub fn agentregistry_projects_locations_list(
        &self,
        args: &AgentregistryProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations agents get.
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
    pub fn agentregistry_projects_locations_agents_get(
        &self,
        args: &AgentregistryProjectsLocationsAgentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Agent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_agents_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_agents_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations agents list.
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
    pub fn agentregistry_projects_locations_agents_list(
        &self,
        args: &AgentregistryProjectsLocationsAgentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAgentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_agents_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_agents_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations agents search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchAgentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_agents_search(
        &self,
        args: &AgentregistryProjectsLocationsAgentsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchAgentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_agents_search_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_agents_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations bindings create.
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
    pub fn agentregistry_projects_locations_bindings_create(
        &self,
        args: &AgentregistryProjectsLocationsBindingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_bindings_create_builder(
            &self.http_client,
            &args.parent,
            &args.bindingId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_bindings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations bindings delete.
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
    pub fn agentregistry_projects_locations_bindings_delete(
        &self,
        args: &AgentregistryProjectsLocationsBindingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_bindings_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_bindings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations bindings fetch available.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchAvailableBindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_bindings_fetch_available(
        &self,
        args: &AgentregistryProjectsLocationsBindingsFetchAvailableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchAvailableBindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_bindings_fetch_available_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.sourceIdentifier,
            &args.targetIdentifier,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_bindings_fetch_available_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations bindings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Binding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_bindings_get(
        &self,
        args: &AgentregistryProjectsLocationsBindingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Binding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_bindings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_bindings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations bindings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_bindings_list(
        &self,
        args: &AgentregistryProjectsLocationsBindingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_bindings_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_bindings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations bindings patch.
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
    pub fn agentregistry_projects_locations_bindings_patch(
        &self,
        args: &AgentregistryProjectsLocationsBindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_bindings_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_bindings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations endpoints get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Endpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_endpoints_get(
        &self,
        args: &AgentregistryProjectsLocationsEndpointsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Endpoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_endpoints_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_endpoints_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations endpoints list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEndpointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_endpoints_list(
        &self,
        args: &AgentregistryProjectsLocationsEndpointsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEndpointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_endpoints_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_endpoints_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations mcp servers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the McpServer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_mcp_servers_get(
        &self,
        args: &AgentregistryProjectsLocationsMcpServersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<McpServer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_mcp_servers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_mcp_servers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations mcp servers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMcpServersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_mcp_servers_list(
        &self,
        args: &AgentregistryProjectsLocationsMcpServersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMcpServersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_mcp_servers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_mcp_servers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations mcp servers search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SearchMcpServersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_mcp_servers_search(
        &self,
        args: &AgentregistryProjectsLocationsMcpServersSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SearchMcpServersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_mcp_servers_search_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_mcp_servers_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations operations cancel.
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
    pub fn agentregistry_projects_locations_operations_cancel(
        &self,
        args: &AgentregistryProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations operations delete.
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
    pub fn agentregistry_projects_locations_operations_delete(
        &self,
        args: &AgentregistryProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations operations get.
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
    pub fn agentregistry_projects_locations_operations_get(
        &self,
        args: &AgentregistryProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations operations list.
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
    pub fn agentregistry_projects_locations_operations_list(
        &self,
        args: &AgentregistryProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations services create.
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
    pub fn agentregistry_projects_locations_services_create(
        &self,
        args: &AgentregistryProjectsLocationsServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_services_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations services delete.
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
    pub fn agentregistry_projects_locations_services_delete(
        &self,
        args: &AgentregistryProjectsLocationsServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_services_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Service result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_services_get(
        &self,
        args: &AgentregistryProjectsLocationsServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn agentregistry_projects_locations_services_list(
        &self,
        args: &AgentregistryProjectsLocationsServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Agentregistry projects locations services patch.
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
    pub fn agentregistry_projects_locations_services_patch(
        &self,
        args: &AgentregistryProjectsLocationsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = agentregistry_projects_locations_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = agentregistry_projects_locations_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
