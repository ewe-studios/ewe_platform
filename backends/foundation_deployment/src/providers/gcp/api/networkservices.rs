//! NetworkservicesProvider - State-aware networkservices API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       networkservices API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::networkservices::{
    networkservices_projects_locations_authz_extensions_create_builder, networkservices_projects_locations_authz_extensions_create_task,
    networkservices_projects_locations_authz_extensions_delete_builder, networkservices_projects_locations_authz_extensions_delete_task,
    networkservices_projects_locations_authz_extensions_patch_builder, networkservices_projects_locations_authz_extensions_patch_task,
    networkservices_projects_locations_edge_cache_keysets_set_iam_policy_builder, networkservices_projects_locations_edge_cache_keysets_set_iam_policy_task,
    networkservices_projects_locations_edge_cache_keysets_test_iam_permissions_builder, networkservices_projects_locations_edge_cache_keysets_test_iam_permissions_task,
    networkservices_projects_locations_edge_cache_origins_set_iam_policy_builder, networkservices_projects_locations_edge_cache_origins_set_iam_policy_task,
    networkservices_projects_locations_edge_cache_origins_test_iam_permissions_builder, networkservices_projects_locations_edge_cache_origins_test_iam_permissions_task,
    networkservices_projects_locations_edge_cache_services_set_iam_policy_builder, networkservices_projects_locations_edge_cache_services_set_iam_policy_task,
    networkservices_projects_locations_edge_cache_services_test_iam_permissions_builder, networkservices_projects_locations_edge_cache_services_test_iam_permissions_task,
    networkservices_projects_locations_endpoint_policies_create_builder, networkservices_projects_locations_endpoint_policies_create_task,
    networkservices_projects_locations_endpoint_policies_delete_builder, networkservices_projects_locations_endpoint_policies_delete_task,
    networkservices_projects_locations_endpoint_policies_patch_builder, networkservices_projects_locations_endpoint_policies_patch_task,
    networkservices_projects_locations_gateways_create_builder, networkservices_projects_locations_gateways_create_task,
    networkservices_projects_locations_gateways_delete_builder, networkservices_projects_locations_gateways_delete_task,
    networkservices_projects_locations_gateways_patch_builder, networkservices_projects_locations_gateways_patch_task,
    networkservices_projects_locations_grpc_routes_create_builder, networkservices_projects_locations_grpc_routes_create_task,
    networkservices_projects_locations_grpc_routes_delete_builder, networkservices_projects_locations_grpc_routes_delete_task,
    networkservices_projects_locations_grpc_routes_patch_builder, networkservices_projects_locations_grpc_routes_patch_task,
    networkservices_projects_locations_http_routes_create_builder, networkservices_projects_locations_http_routes_create_task,
    networkservices_projects_locations_http_routes_delete_builder, networkservices_projects_locations_http_routes_delete_task,
    networkservices_projects_locations_http_routes_patch_builder, networkservices_projects_locations_http_routes_patch_task,
    networkservices_projects_locations_lb_edge_extensions_create_builder, networkservices_projects_locations_lb_edge_extensions_create_task,
    networkservices_projects_locations_lb_edge_extensions_delete_builder, networkservices_projects_locations_lb_edge_extensions_delete_task,
    networkservices_projects_locations_lb_edge_extensions_patch_builder, networkservices_projects_locations_lb_edge_extensions_patch_task,
    networkservices_projects_locations_lb_route_extensions_create_builder, networkservices_projects_locations_lb_route_extensions_create_task,
    networkservices_projects_locations_lb_route_extensions_delete_builder, networkservices_projects_locations_lb_route_extensions_delete_task,
    networkservices_projects_locations_lb_route_extensions_patch_builder, networkservices_projects_locations_lb_route_extensions_patch_task,
    networkservices_projects_locations_lb_traffic_extensions_create_builder, networkservices_projects_locations_lb_traffic_extensions_create_task,
    networkservices_projects_locations_lb_traffic_extensions_delete_builder, networkservices_projects_locations_lb_traffic_extensions_delete_task,
    networkservices_projects_locations_lb_traffic_extensions_patch_builder, networkservices_projects_locations_lb_traffic_extensions_patch_task,
    networkservices_projects_locations_meshes_create_builder, networkservices_projects_locations_meshes_create_task,
    networkservices_projects_locations_meshes_delete_builder, networkservices_projects_locations_meshes_delete_task,
    networkservices_projects_locations_meshes_patch_builder, networkservices_projects_locations_meshes_patch_task,
    networkservices_projects_locations_operations_cancel_builder, networkservices_projects_locations_operations_cancel_task,
    networkservices_projects_locations_operations_delete_builder, networkservices_projects_locations_operations_delete_task,
    networkservices_projects_locations_service_bindings_create_builder, networkservices_projects_locations_service_bindings_create_task,
    networkservices_projects_locations_service_bindings_delete_builder, networkservices_projects_locations_service_bindings_delete_task,
    networkservices_projects_locations_service_bindings_patch_builder, networkservices_projects_locations_service_bindings_patch_task,
    networkservices_projects_locations_service_lb_policies_create_builder, networkservices_projects_locations_service_lb_policies_create_task,
    networkservices_projects_locations_service_lb_policies_delete_builder, networkservices_projects_locations_service_lb_policies_delete_task,
    networkservices_projects_locations_service_lb_policies_patch_builder, networkservices_projects_locations_service_lb_policies_patch_task,
    networkservices_projects_locations_tcp_routes_create_builder, networkservices_projects_locations_tcp_routes_create_task,
    networkservices_projects_locations_tcp_routes_delete_builder, networkservices_projects_locations_tcp_routes_delete_task,
    networkservices_projects_locations_tcp_routes_patch_builder, networkservices_projects_locations_tcp_routes_patch_task,
    networkservices_projects_locations_tls_routes_create_builder, networkservices_projects_locations_tls_routes_create_task,
    networkservices_projects_locations_tls_routes_delete_builder, networkservices_projects_locations_tls_routes_delete_task,
    networkservices_projects_locations_tls_routes_patch_builder, networkservices_projects_locations_tls_routes_patch_task,
    networkservices_projects_locations_wasm_plugins_create_builder, networkservices_projects_locations_wasm_plugins_create_task,
    networkservices_projects_locations_wasm_plugins_delete_builder, networkservices_projects_locations_wasm_plugins_delete_task,
    networkservices_projects_locations_wasm_plugins_patch_builder, networkservices_projects_locations_wasm_plugins_patch_task,
    networkservices_projects_locations_wasm_plugins_versions_create_builder, networkservices_projects_locations_wasm_plugins_versions_create_task,
    networkservices_projects_locations_wasm_plugins_versions_delete_builder, networkservices_projects_locations_wasm_plugins_versions_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::networkservices::Empty;
use crate::providers::gcp::clients::networkservices::Operation;
use crate::providers::gcp::clients::networkservices::Policy;
use crate::providers::gcp::clients::networkservices::TestIamPermissionsResponse;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheKeysetsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheKeysetsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheOriginsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheOriginsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheServicesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheServicesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsVersionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsVersionsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NetworkservicesProvider with automatic state tracking.
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
/// let provider = NetworkservicesProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct NetworkservicesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> NetworkservicesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new NetworkservicesProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Networkservices projects locations authz extensions create.
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
    pub fn networkservices_projects_locations_authz_extensions_create(
        &self,
        args: &NetworkservicesProjectsLocationsAuthzExtensionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_authz_extensions_create_builder(
            &self.http_client,
            &args.parent,
            &args.authzExtensionId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_authz_extensions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations authz extensions delete.
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
    pub fn networkservices_projects_locations_authz_extensions_delete(
        &self,
        args: &NetworkservicesProjectsLocationsAuthzExtensionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_authz_extensions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_authz_extensions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations authz extensions patch.
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
    pub fn networkservices_projects_locations_authz_extensions_patch(
        &self,
        args: &NetworkservicesProjectsLocationsAuthzExtensionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_authz_extensions_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_authz_extensions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations edge cache keysets set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkservices_projects_locations_edge_cache_keysets_set_iam_policy(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheKeysetsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_keysets_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_keysets_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations edge cache keysets test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkservices_projects_locations_edge_cache_keysets_test_iam_permissions(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheKeysetsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_keysets_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_keysets_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations edge cache origins set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkservices_projects_locations_edge_cache_origins_set_iam_policy(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheOriginsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_origins_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_origins_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations edge cache origins test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkservices_projects_locations_edge_cache_origins_test_iam_permissions(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheOriginsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_origins_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_origins_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations edge cache services set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkservices_projects_locations_edge_cache_services_set_iam_policy(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheServicesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_services_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_services_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations edge cache services test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkservices_projects_locations_edge_cache_services_test_iam_permissions(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheServicesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_services_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_services_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations endpoint policies create.
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
    pub fn networkservices_projects_locations_endpoint_policies_create(
        &self,
        args: &NetworkservicesProjectsLocationsEndpointPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_endpoint_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.endpointPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_endpoint_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations endpoint policies delete.
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
    pub fn networkservices_projects_locations_endpoint_policies_delete(
        &self,
        args: &NetworkservicesProjectsLocationsEndpointPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_endpoint_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_endpoint_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations endpoint policies patch.
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
    pub fn networkservices_projects_locations_endpoint_policies_patch(
        &self,
        args: &NetworkservicesProjectsLocationsEndpointPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_endpoint_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_endpoint_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations gateways create.
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
    pub fn networkservices_projects_locations_gateways_create(
        &self,
        args: &NetworkservicesProjectsLocationsGatewaysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_gateways_create_builder(
            &self.http_client,
            &args.parent,
            &args.gatewayId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_gateways_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations gateways delete.
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
    pub fn networkservices_projects_locations_gateways_delete(
        &self,
        args: &NetworkservicesProjectsLocationsGatewaysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_gateways_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_gateways_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations gateways patch.
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
    pub fn networkservices_projects_locations_gateways_patch(
        &self,
        args: &NetworkservicesProjectsLocationsGatewaysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_gateways_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_gateways_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations grpc routes create.
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
    pub fn networkservices_projects_locations_grpc_routes_create(
        &self,
        args: &NetworkservicesProjectsLocationsGrpcRoutesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_grpc_routes_create_builder(
            &self.http_client,
            &args.parent,
            &args.grpcRouteId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_grpc_routes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations grpc routes delete.
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
    pub fn networkservices_projects_locations_grpc_routes_delete(
        &self,
        args: &NetworkservicesProjectsLocationsGrpcRoutesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_grpc_routes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_grpc_routes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations grpc routes patch.
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
    pub fn networkservices_projects_locations_grpc_routes_patch(
        &self,
        args: &NetworkservicesProjectsLocationsGrpcRoutesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_grpc_routes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_grpc_routes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations http routes create.
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
    pub fn networkservices_projects_locations_http_routes_create(
        &self,
        args: &NetworkservicesProjectsLocationsHttpRoutesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_http_routes_create_builder(
            &self.http_client,
            &args.parent,
            &args.httpRouteId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_http_routes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations http routes delete.
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
    pub fn networkservices_projects_locations_http_routes_delete(
        &self,
        args: &NetworkservicesProjectsLocationsHttpRoutesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_http_routes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_http_routes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations http routes patch.
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
    pub fn networkservices_projects_locations_http_routes_patch(
        &self,
        args: &NetworkservicesProjectsLocationsHttpRoutesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_http_routes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_http_routes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb edge extensions create.
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
    pub fn networkservices_projects_locations_lb_edge_extensions_create(
        &self,
        args: &NetworkservicesProjectsLocationsLbEdgeExtensionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_edge_extensions_create_builder(
            &self.http_client,
            &args.parent,
            &args.lbEdgeExtensionId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_edge_extensions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb edge extensions delete.
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
    pub fn networkservices_projects_locations_lb_edge_extensions_delete(
        &self,
        args: &NetworkservicesProjectsLocationsLbEdgeExtensionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_edge_extensions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_edge_extensions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb edge extensions patch.
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
    pub fn networkservices_projects_locations_lb_edge_extensions_patch(
        &self,
        args: &NetworkservicesProjectsLocationsLbEdgeExtensionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_edge_extensions_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_edge_extensions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb route extensions create.
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
    pub fn networkservices_projects_locations_lb_route_extensions_create(
        &self,
        args: &NetworkservicesProjectsLocationsLbRouteExtensionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_route_extensions_create_builder(
            &self.http_client,
            &args.parent,
            &args.lbRouteExtensionId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_route_extensions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb route extensions delete.
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
    pub fn networkservices_projects_locations_lb_route_extensions_delete(
        &self,
        args: &NetworkservicesProjectsLocationsLbRouteExtensionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_route_extensions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_route_extensions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb route extensions patch.
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
    pub fn networkservices_projects_locations_lb_route_extensions_patch(
        &self,
        args: &NetworkservicesProjectsLocationsLbRouteExtensionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_route_extensions_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_route_extensions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb traffic extensions create.
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
    pub fn networkservices_projects_locations_lb_traffic_extensions_create(
        &self,
        args: &NetworkservicesProjectsLocationsLbTrafficExtensionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_traffic_extensions_create_builder(
            &self.http_client,
            &args.parent,
            &args.lbTrafficExtensionId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_traffic_extensions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb traffic extensions delete.
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
    pub fn networkservices_projects_locations_lb_traffic_extensions_delete(
        &self,
        args: &NetworkservicesProjectsLocationsLbTrafficExtensionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_traffic_extensions_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_traffic_extensions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb traffic extensions patch.
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
    pub fn networkservices_projects_locations_lb_traffic_extensions_patch(
        &self,
        args: &NetworkservicesProjectsLocationsLbTrafficExtensionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_traffic_extensions_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_traffic_extensions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations meshes create.
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
    pub fn networkservices_projects_locations_meshes_create(
        &self,
        args: &NetworkservicesProjectsLocationsMeshesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_meshes_create_builder(
            &self.http_client,
            &args.parent,
            &args.meshId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_meshes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations meshes delete.
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
    pub fn networkservices_projects_locations_meshes_delete(
        &self,
        args: &NetworkservicesProjectsLocationsMeshesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_meshes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_meshes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations meshes patch.
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
    pub fn networkservices_projects_locations_meshes_patch(
        &self,
        args: &NetworkservicesProjectsLocationsMeshesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_meshes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_meshes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations operations cancel.
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
    pub fn networkservices_projects_locations_operations_cancel(
        &self,
        args: &NetworkservicesProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations operations delete.
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
    pub fn networkservices_projects_locations_operations_delete(
        &self,
        args: &NetworkservicesProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service bindings create.
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
    pub fn networkservices_projects_locations_service_bindings_create(
        &self,
        args: &NetworkservicesProjectsLocationsServiceBindingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_bindings_create_builder(
            &self.http_client,
            &args.parent,
            &args.serviceBindingId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_bindings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service bindings delete.
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
    pub fn networkservices_projects_locations_service_bindings_delete(
        &self,
        args: &NetworkservicesProjectsLocationsServiceBindingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_bindings_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_bindings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service bindings patch.
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
    pub fn networkservices_projects_locations_service_bindings_patch(
        &self,
        args: &NetworkservicesProjectsLocationsServiceBindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_bindings_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_bindings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service lb policies create.
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
    pub fn networkservices_projects_locations_service_lb_policies_create(
        &self,
        args: &NetworkservicesProjectsLocationsServiceLbPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_lb_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.serviceLbPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_lb_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service lb policies delete.
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
    pub fn networkservices_projects_locations_service_lb_policies_delete(
        &self,
        args: &NetworkservicesProjectsLocationsServiceLbPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_lb_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_lb_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service lb policies patch.
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
    pub fn networkservices_projects_locations_service_lb_policies_patch(
        &self,
        args: &NetworkservicesProjectsLocationsServiceLbPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_lb_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_lb_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tcp routes create.
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
    pub fn networkservices_projects_locations_tcp_routes_create(
        &self,
        args: &NetworkservicesProjectsLocationsTcpRoutesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tcp_routes_create_builder(
            &self.http_client,
            &args.parent,
            &args.tcpRouteId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tcp_routes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tcp routes delete.
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
    pub fn networkservices_projects_locations_tcp_routes_delete(
        &self,
        args: &NetworkservicesProjectsLocationsTcpRoutesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tcp_routes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tcp_routes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tcp routes patch.
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
    pub fn networkservices_projects_locations_tcp_routes_patch(
        &self,
        args: &NetworkservicesProjectsLocationsTcpRoutesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tcp_routes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tcp_routes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tls routes create.
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
    pub fn networkservices_projects_locations_tls_routes_create(
        &self,
        args: &NetworkservicesProjectsLocationsTlsRoutesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tls_routes_create_builder(
            &self.http_client,
            &args.parent,
            &args.tlsRouteId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tls_routes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tls routes delete.
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
    pub fn networkservices_projects_locations_tls_routes_delete(
        &self,
        args: &NetworkservicesProjectsLocationsTlsRoutesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tls_routes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tls_routes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tls routes patch.
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
    pub fn networkservices_projects_locations_tls_routes_patch(
        &self,
        args: &NetworkservicesProjectsLocationsTlsRoutesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tls_routes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tls_routes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations wasm plugins create.
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
    pub fn networkservices_projects_locations_wasm_plugins_create(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_create_builder(
            &self.http_client,
            &args.parent,
            &args.wasmPluginId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations wasm plugins delete.
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
    pub fn networkservices_projects_locations_wasm_plugins_delete(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations wasm plugins patch.
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
    pub fn networkservices_projects_locations_wasm_plugins_patch(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations wasm plugins versions create.
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
    pub fn networkservices_projects_locations_wasm_plugins_versions_create(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsVersionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_versions_create_builder(
            &self.http_client,
            &args.parent,
            &args.wasmPluginVersionId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_versions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations wasm plugins versions delete.
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
    pub fn networkservices_projects_locations_wasm_plugins_versions_delete(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsVersionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_versions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_versions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
