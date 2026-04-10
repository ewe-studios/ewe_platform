//! FirebasedataconnectProvider - State-aware firebasedataconnect API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       firebasedataconnect API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::firebasedataconnect::{
    firebasedataconnect_projects_locations_get_builder, firebasedataconnect_projects_locations_get_task,
    firebasedataconnect_projects_locations_list_builder, firebasedataconnect_projects_locations_list_task,
    firebasedataconnect_projects_locations_operations_cancel_builder, firebasedataconnect_projects_locations_operations_cancel_task,
    firebasedataconnect_projects_locations_operations_delete_builder, firebasedataconnect_projects_locations_operations_delete_task,
    firebasedataconnect_projects_locations_operations_get_builder, firebasedataconnect_projects_locations_operations_get_task,
    firebasedataconnect_projects_locations_operations_list_builder, firebasedataconnect_projects_locations_operations_list_task,
    firebasedataconnect_projects_locations_services_create_builder, firebasedataconnect_projects_locations_services_create_task,
    firebasedataconnect_projects_locations_services_delete_builder, firebasedataconnect_projects_locations_services_delete_task,
    firebasedataconnect_projects_locations_services_execute_graphql_builder, firebasedataconnect_projects_locations_services_execute_graphql_task,
    firebasedataconnect_projects_locations_services_execute_graphql_read_builder, firebasedataconnect_projects_locations_services_execute_graphql_read_task,
    firebasedataconnect_projects_locations_services_get_builder, firebasedataconnect_projects_locations_services_get_task,
    firebasedataconnect_projects_locations_services_introspect_graphql_builder, firebasedataconnect_projects_locations_services_introspect_graphql_task,
    firebasedataconnect_projects_locations_services_list_builder, firebasedataconnect_projects_locations_services_list_task,
    firebasedataconnect_projects_locations_services_patch_builder, firebasedataconnect_projects_locations_services_patch_task,
    firebasedataconnect_projects_locations_services_connectors_create_builder, firebasedataconnect_projects_locations_services_connectors_create_task,
    firebasedataconnect_projects_locations_services_connectors_delete_builder, firebasedataconnect_projects_locations_services_connectors_delete_task,
    firebasedataconnect_projects_locations_services_connectors_execute_mutation_builder, firebasedataconnect_projects_locations_services_connectors_execute_mutation_task,
    firebasedataconnect_projects_locations_services_connectors_execute_query_builder, firebasedataconnect_projects_locations_services_connectors_execute_query_task,
    firebasedataconnect_projects_locations_services_connectors_get_builder, firebasedataconnect_projects_locations_services_connectors_get_task,
    firebasedataconnect_projects_locations_services_connectors_impersonate_mutation_builder, firebasedataconnect_projects_locations_services_connectors_impersonate_mutation_task,
    firebasedataconnect_projects_locations_services_connectors_impersonate_query_builder, firebasedataconnect_projects_locations_services_connectors_impersonate_query_task,
    firebasedataconnect_projects_locations_services_connectors_list_builder, firebasedataconnect_projects_locations_services_connectors_list_task,
    firebasedataconnect_projects_locations_services_connectors_patch_builder, firebasedataconnect_projects_locations_services_connectors_patch_task,
    firebasedataconnect_projects_locations_services_schemas_create_builder, firebasedataconnect_projects_locations_services_schemas_create_task,
    firebasedataconnect_projects_locations_services_schemas_delete_builder, firebasedataconnect_projects_locations_services_schemas_delete_task,
    firebasedataconnect_projects_locations_services_schemas_get_builder, firebasedataconnect_projects_locations_services_schemas_get_task,
    firebasedataconnect_projects_locations_services_schemas_list_builder, firebasedataconnect_projects_locations_services_schemas_list_task,
    firebasedataconnect_projects_locations_services_schemas_patch_builder, firebasedataconnect_projects_locations_services_schemas_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::firebasedataconnect::Connector;
use crate::providers::gcp::clients::firebasedataconnect::Empty;
use crate::providers::gcp::clients::firebasedataconnect::ExecuteMutationResponse;
use crate::providers::gcp::clients::firebasedataconnect::ExecuteQueryResponse;
use crate::providers::gcp::clients::firebasedataconnect::GraphqlResponse;
use crate::providers::gcp::clients::firebasedataconnect::ListConnectorsResponse;
use crate::providers::gcp::clients::firebasedataconnect::ListLocationsResponse;
use crate::providers::gcp::clients::firebasedataconnect::ListOperationsResponse;
use crate::providers::gcp::clients::firebasedataconnect::ListSchemasResponse;
use crate::providers::gcp::clients::firebasedataconnect::ListServicesResponse;
use crate::providers::gcp::clients::firebasedataconnect::Location;
use crate::providers::gcp::clients::firebasedataconnect::Operation;
use crate::providers::gcp::clients::firebasedataconnect::Schema;
use crate::providers::gcp::clients::firebasedataconnect::Service;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsGetArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsListArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsCreateArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsDeleteArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsExecuteMutationArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsExecuteQueryArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsGetArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsImpersonateMutationArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsImpersonateQueryArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsListArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesConnectorsPatchArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesCreateArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesDeleteArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesExecuteGraphqlArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesExecuteGraphqlReadArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesGetArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesIntrospectGraphqlArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesListArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesPatchArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesSchemasCreateArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesSchemasDeleteArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesSchemasGetArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesSchemasListArgs;
use crate::providers::gcp::clients::firebasedataconnect::FirebasedataconnectProjectsLocationsServicesSchemasPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// FirebasedataconnectProvider with automatic state tracking.
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
/// let provider = FirebasedataconnectProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct FirebasedataconnectProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> FirebasedataconnectProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new FirebasedataconnectProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Firebasedataconnect projects locations get.
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
    pub fn firebasedataconnect_projects_locations_get(
        &self,
        args: &FirebasedataconnectProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations list.
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
    pub fn firebasedataconnect_projects_locations_list(
        &self,
        args: &FirebasedataconnectProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations operations cancel.
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
    pub fn firebasedataconnect_projects_locations_operations_cancel(
        &self,
        args: &FirebasedataconnectProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations operations delete.
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
    pub fn firebasedataconnect_projects_locations_operations_delete(
        &self,
        args: &FirebasedataconnectProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations operations get.
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
    pub fn firebasedataconnect_projects_locations_operations_get(
        &self,
        args: &FirebasedataconnectProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations operations list.
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
    pub fn firebasedataconnect_projects_locations_operations_list(
        &self,
        args: &FirebasedataconnectProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services create.
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
    pub fn firebasedataconnect_projects_locations_services_create(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services delete.
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
    pub fn firebasedataconnect_projects_locations_services_delete(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services execute graphql.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GraphqlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedataconnect_projects_locations_services_execute_graphql(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesExecuteGraphqlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GraphqlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_execute_graphql_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_execute_graphql_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services execute graphql read.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GraphqlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedataconnect_projects_locations_services_execute_graphql_read(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesExecuteGraphqlReadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GraphqlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_execute_graphql_read_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_execute_graphql_read_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services get.
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
    pub fn firebasedataconnect_projects_locations_services_get(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Service, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services introspect graphql.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GraphqlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedataconnect_projects_locations_services_introspect_graphql(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesIntrospectGraphqlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GraphqlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_introspect_graphql_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_introspect_graphql_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services list.
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
    pub fn firebasedataconnect_projects_locations_services_list(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services patch.
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
    pub fn firebasedataconnect_projects_locations_services_patch(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors create.
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
    pub fn firebasedataconnect_projects_locations_services_connectors_create(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectorId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors delete.
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
    pub fn firebasedataconnect_projects_locations_services_connectors_delete(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors execute mutation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteMutationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedataconnect_projects_locations_services_connectors_execute_mutation(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsExecuteMutationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteMutationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_execute_mutation_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_execute_mutation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors execute query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteQueryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedataconnect_projects_locations_services_connectors_execute_query(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsExecuteQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteQueryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_execute_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_execute_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedataconnect_projects_locations_services_connectors_get(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors impersonate mutation.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GraphqlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn firebasedataconnect_projects_locations_services_connectors_impersonate_mutation(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsImpersonateMutationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GraphqlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_impersonate_mutation_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_impersonate_mutation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors impersonate query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GraphqlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedataconnect_projects_locations_services_connectors_impersonate_query(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsImpersonateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GraphqlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_impersonate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_impersonate_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedataconnect_projects_locations_services_connectors_list(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services connectors patch.
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
    pub fn firebasedataconnect_projects_locations_services_connectors_patch(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesConnectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_connectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_connectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services schemas create.
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
    pub fn firebasedataconnect_projects_locations_services_schemas_create(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesSchemasCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_schemas_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.schemaId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_schemas_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services schemas delete.
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
    pub fn firebasedataconnect_projects_locations_services_schemas_delete(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesSchemasDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_schemas_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_schemas_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services schemas get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Schema result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedataconnect_projects_locations_services_schemas_get(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesSchemasGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Schema, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_schemas_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_schemas_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services schemas list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSchemasResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn firebasedataconnect_projects_locations_services_schemas_list(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesSchemasListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSchemasResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_schemas_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_schemas_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Firebasedataconnect projects locations services schemas patch.
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
    pub fn firebasedataconnect_projects_locations_services_schemas_patch(
        &self,
        args: &FirebasedataconnectProjectsLocationsServicesSchemasPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = firebasedataconnect_projects_locations_services_schemas_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = firebasedataconnect_projects_locations_services_schemas_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
