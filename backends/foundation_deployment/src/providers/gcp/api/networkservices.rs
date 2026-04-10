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
    networkservices_projects_locations_get_builder, networkservices_projects_locations_get_task,
    networkservices_projects_locations_list_builder, networkservices_projects_locations_list_task,
    networkservices_projects_locations_authz_extensions_create_builder, networkservices_projects_locations_authz_extensions_create_task,
    networkservices_projects_locations_authz_extensions_delete_builder, networkservices_projects_locations_authz_extensions_delete_task,
    networkservices_projects_locations_authz_extensions_get_builder, networkservices_projects_locations_authz_extensions_get_task,
    networkservices_projects_locations_authz_extensions_list_builder, networkservices_projects_locations_authz_extensions_list_task,
    networkservices_projects_locations_authz_extensions_patch_builder, networkservices_projects_locations_authz_extensions_patch_task,
    networkservices_projects_locations_edge_cache_keysets_get_iam_policy_builder, networkservices_projects_locations_edge_cache_keysets_get_iam_policy_task,
    networkservices_projects_locations_edge_cache_keysets_set_iam_policy_builder, networkservices_projects_locations_edge_cache_keysets_set_iam_policy_task,
    networkservices_projects_locations_edge_cache_keysets_test_iam_permissions_builder, networkservices_projects_locations_edge_cache_keysets_test_iam_permissions_task,
    networkservices_projects_locations_edge_cache_origins_get_iam_policy_builder, networkservices_projects_locations_edge_cache_origins_get_iam_policy_task,
    networkservices_projects_locations_edge_cache_origins_set_iam_policy_builder, networkservices_projects_locations_edge_cache_origins_set_iam_policy_task,
    networkservices_projects_locations_edge_cache_origins_test_iam_permissions_builder, networkservices_projects_locations_edge_cache_origins_test_iam_permissions_task,
    networkservices_projects_locations_edge_cache_services_get_iam_policy_builder, networkservices_projects_locations_edge_cache_services_get_iam_policy_task,
    networkservices_projects_locations_edge_cache_services_set_iam_policy_builder, networkservices_projects_locations_edge_cache_services_set_iam_policy_task,
    networkservices_projects_locations_edge_cache_services_test_iam_permissions_builder, networkservices_projects_locations_edge_cache_services_test_iam_permissions_task,
    networkservices_projects_locations_endpoint_policies_create_builder, networkservices_projects_locations_endpoint_policies_create_task,
    networkservices_projects_locations_endpoint_policies_delete_builder, networkservices_projects_locations_endpoint_policies_delete_task,
    networkservices_projects_locations_endpoint_policies_get_builder, networkservices_projects_locations_endpoint_policies_get_task,
    networkservices_projects_locations_endpoint_policies_list_builder, networkservices_projects_locations_endpoint_policies_list_task,
    networkservices_projects_locations_endpoint_policies_patch_builder, networkservices_projects_locations_endpoint_policies_patch_task,
    networkservices_projects_locations_gateways_create_builder, networkservices_projects_locations_gateways_create_task,
    networkservices_projects_locations_gateways_delete_builder, networkservices_projects_locations_gateways_delete_task,
    networkservices_projects_locations_gateways_get_builder, networkservices_projects_locations_gateways_get_task,
    networkservices_projects_locations_gateways_list_builder, networkservices_projects_locations_gateways_list_task,
    networkservices_projects_locations_gateways_patch_builder, networkservices_projects_locations_gateways_patch_task,
    networkservices_projects_locations_gateways_route_views_get_builder, networkservices_projects_locations_gateways_route_views_get_task,
    networkservices_projects_locations_gateways_route_views_list_builder, networkservices_projects_locations_gateways_route_views_list_task,
    networkservices_projects_locations_grpc_routes_create_builder, networkservices_projects_locations_grpc_routes_create_task,
    networkservices_projects_locations_grpc_routes_delete_builder, networkservices_projects_locations_grpc_routes_delete_task,
    networkservices_projects_locations_grpc_routes_get_builder, networkservices_projects_locations_grpc_routes_get_task,
    networkservices_projects_locations_grpc_routes_list_builder, networkservices_projects_locations_grpc_routes_list_task,
    networkservices_projects_locations_grpc_routes_patch_builder, networkservices_projects_locations_grpc_routes_patch_task,
    networkservices_projects_locations_http_routes_create_builder, networkservices_projects_locations_http_routes_create_task,
    networkservices_projects_locations_http_routes_delete_builder, networkservices_projects_locations_http_routes_delete_task,
    networkservices_projects_locations_http_routes_get_builder, networkservices_projects_locations_http_routes_get_task,
    networkservices_projects_locations_http_routes_list_builder, networkservices_projects_locations_http_routes_list_task,
    networkservices_projects_locations_http_routes_patch_builder, networkservices_projects_locations_http_routes_patch_task,
    networkservices_projects_locations_lb_edge_extensions_create_builder, networkservices_projects_locations_lb_edge_extensions_create_task,
    networkservices_projects_locations_lb_edge_extensions_delete_builder, networkservices_projects_locations_lb_edge_extensions_delete_task,
    networkservices_projects_locations_lb_edge_extensions_get_builder, networkservices_projects_locations_lb_edge_extensions_get_task,
    networkservices_projects_locations_lb_edge_extensions_list_builder, networkservices_projects_locations_lb_edge_extensions_list_task,
    networkservices_projects_locations_lb_edge_extensions_patch_builder, networkservices_projects_locations_lb_edge_extensions_patch_task,
    networkservices_projects_locations_lb_route_extensions_create_builder, networkservices_projects_locations_lb_route_extensions_create_task,
    networkservices_projects_locations_lb_route_extensions_delete_builder, networkservices_projects_locations_lb_route_extensions_delete_task,
    networkservices_projects_locations_lb_route_extensions_get_builder, networkservices_projects_locations_lb_route_extensions_get_task,
    networkservices_projects_locations_lb_route_extensions_list_builder, networkservices_projects_locations_lb_route_extensions_list_task,
    networkservices_projects_locations_lb_route_extensions_patch_builder, networkservices_projects_locations_lb_route_extensions_patch_task,
    networkservices_projects_locations_lb_traffic_extensions_create_builder, networkservices_projects_locations_lb_traffic_extensions_create_task,
    networkservices_projects_locations_lb_traffic_extensions_delete_builder, networkservices_projects_locations_lb_traffic_extensions_delete_task,
    networkservices_projects_locations_lb_traffic_extensions_get_builder, networkservices_projects_locations_lb_traffic_extensions_get_task,
    networkservices_projects_locations_lb_traffic_extensions_list_builder, networkservices_projects_locations_lb_traffic_extensions_list_task,
    networkservices_projects_locations_lb_traffic_extensions_patch_builder, networkservices_projects_locations_lb_traffic_extensions_patch_task,
    networkservices_projects_locations_meshes_create_builder, networkservices_projects_locations_meshes_create_task,
    networkservices_projects_locations_meshes_delete_builder, networkservices_projects_locations_meshes_delete_task,
    networkservices_projects_locations_meshes_get_builder, networkservices_projects_locations_meshes_get_task,
    networkservices_projects_locations_meshes_list_builder, networkservices_projects_locations_meshes_list_task,
    networkservices_projects_locations_meshes_patch_builder, networkservices_projects_locations_meshes_patch_task,
    networkservices_projects_locations_meshes_route_views_get_builder, networkservices_projects_locations_meshes_route_views_get_task,
    networkservices_projects_locations_meshes_route_views_list_builder, networkservices_projects_locations_meshes_route_views_list_task,
    networkservices_projects_locations_operations_cancel_builder, networkservices_projects_locations_operations_cancel_task,
    networkservices_projects_locations_operations_delete_builder, networkservices_projects_locations_operations_delete_task,
    networkservices_projects_locations_operations_get_builder, networkservices_projects_locations_operations_get_task,
    networkservices_projects_locations_operations_list_builder, networkservices_projects_locations_operations_list_task,
    networkservices_projects_locations_service_bindings_create_builder, networkservices_projects_locations_service_bindings_create_task,
    networkservices_projects_locations_service_bindings_delete_builder, networkservices_projects_locations_service_bindings_delete_task,
    networkservices_projects_locations_service_bindings_get_builder, networkservices_projects_locations_service_bindings_get_task,
    networkservices_projects_locations_service_bindings_list_builder, networkservices_projects_locations_service_bindings_list_task,
    networkservices_projects_locations_service_bindings_patch_builder, networkservices_projects_locations_service_bindings_patch_task,
    networkservices_projects_locations_service_lb_policies_create_builder, networkservices_projects_locations_service_lb_policies_create_task,
    networkservices_projects_locations_service_lb_policies_delete_builder, networkservices_projects_locations_service_lb_policies_delete_task,
    networkservices_projects_locations_service_lb_policies_get_builder, networkservices_projects_locations_service_lb_policies_get_task,
    networkservices_projects_locations_service_lb_policies_list_builder, networkservices_projects_locations_service_lb_policies_list_task,
    networkservices_projects_locations_service_lb_policies_patch_builder, networkservices_projects_locations_service_lb_policies_patch_task,
    networkservices_projects_locations_tcp_routes_create_builder, networkservices_projects_locations_tcp_routes_create_task,
    networkservices_projects_locations_tcp_routes_delete_builder, networkservices_projects_locations_tcp_routes_delete_task,
    networkservices_projects_locations_tcp_routes_get_builder, networkservices_projects_locations_tcp_routes_get_task,
    networkservices_projects_locations_tcp_routes_list_builder, networkservices_projects_locations_tcp_routes_list_task,
    networkservices_projects_locations_tcp_routes_patch_builder, networkservices_projects_locations_tcp_routes_patch_task,
    networkservices_projects_locations_tls_routes_create_builder, networkservices_projects_locations_tls_routes_create_task,
    networkservices_projects_locations_tls_routes_delete_builder, networkservices_projects_locations_tls_routes_delete_task,
    networkservices_projects_locations_tls_routes_get_builder, networkservices_projects_locations_tls_routes_get_task,
    networkservices_projects_locations_tls_routes_list_builder, networkservices_projects_locations_tls_routes_list_task,
    networkservices_projects_locations_tls_routes_patch_builder, networkservices_projects_locations_tls_routes_patch_task,
    networkservices_projects_locations_wasm_plugins_create_builder, networkservices_projects_locations_wasm_plugins_create_task,
    networkservices_projects_locations_wasm_plugins_delete_builder, networkservices_projects_locations_wasm_plugins_delete_task,
    networkservices_projects_locations_wasm_plugins_get_builder, networkservices_projects_locations_wasm_plugins_get_task,
    networkservices_projects_locations_wasm_plugins_list_builder, networkservices_projects_locations_wasm_plugins_list_task,
    networkservices_projects_locations_wasm_plugins_patch_builder, networkservices_projects_locations_wasm_plugins_patch_task,
    networkservices_projects_locations_wasm_plugins_versions_create_builder, networkservices_projects_locations_wasm_plugins_versions_create_task,
    networkservices_projects_locations_wasm_plugins_versions_delete_builder, networkservices_projects_locations_wasm_plugins_versions_delete_task,
    networkservices_projects_locations_wasm_plugins_versions_get_builder, networkservices_projects_locations_wasm_plugins_versions_get_task,
    networkservices_projects_locations_wasm_plugins_versions_list_builder, networkservices_projects_locations_wasm_plugins_versions_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::networkservices::AuthzExtension;
use crate::providers::gcp::clients::networkservices::Empty;
use crate::providers::gcp::clients::networkservices::EndpointPolicy;
use crate::providers::gcp::clients::networkservices::Gateway;
use crate::providers::gcp::clients::networkservices::GatewayRouteView;
use crate::providers::gcp::clients::networkservices::GrpcRoute;
use crate::providers::gcp::clients::networkservices::HttpRoute;
use crate::providers::gcp::clients::networkservices::LbEdgeExtension;
use crate::providers::gcp::clients::networkservices::LbRouteExtension;
use crate::providers::gcp::clients::networkservices::LbTrafficExtension;
use crate::providers::gcp::clients::networkservices::ListAuthzExtensionsResponse;
use crate::providers::gcp::clients::networkservices::ListEndpointPoliciesResponse;
use crate::providers::gcp::clients::networkservices::ListGatewayRouteViewsResponse;
use crate::providers::gcp::clients::networkservices::ListGatewaysResponse;
use crate::providers::gcp::clients::networkservices::ListGrpcRoutesResponse;
use crate::providers::gcp::clients::networkservices::ListHttpRoutesResponse;
use crate::providers::gcp::clients::networkservices::ListLbEdgeExtensionsResponse;
use crate::providers::gcp::clients::networkservices::ListLbRouteExtensionsResponse;
use crate::providers::gcp::clients::networkservices::ListLbTrafficExtensionsResponse;
use crate::providers::gcp::clients::networkservices::ListLocationsResponse;
use crate::providers::gcp::clients::networkservices::ListMeshRouteViewsResponse;
use crate::providers::gcp::clients::networkservices::ListMeshesResponse;
use crate::providers::gcp::clients::networkservices::ListOperationsResponse;
use crate::providers::gcp::clients::networkservices::ListServiceBindingsResponse;
use crate::providers::gcp::clients::networkservices::ListServiceLbPoliciesResponse;
use crate::providers::gcp::clients::networkservices::ListTcpRoutesResponse;
use crate::providers::gcp::clients::networkservices::ListTlsRoutesResponse;
use crate::providers::gcp::clients::networkservices::ListWasmPluginVersionsResponse;
use crate::providers::gcp::clients::networkservices::ListWasmPluginsResponse;
use crate::providers::gcp::clients::networkservices::Location;
use crate::providers::gcp::clients::networkservices::Mesh;
use crate::providers::gcp::clients::networkservices::MeshRouteView;
use crate::providers::gcp::clients::networkservices::Operation;
use crate::providers::gcp::clients::networkservices::Policy;
use crate::providers::gcp::clients::networkservices::ServiceBinding;
use crate::providers::gcp::clients::networkservices::ServiceLbPolicy;
use crate::providers::gcp::clients::networkservices::TcpRoute;
use crate::providers::gcp::clients::networkservices::TestIamPermissionsResponse;
use crate::providers::gcp::clients::networkservices::TlsRoute;
use crate::providers::gcp::clients::networkservices::WasmPlugin;
use crate::providers::gcp::clients::networkservices::WasmPluginVersion;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsAuthzExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheKeysetsGetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheKeysetsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheKeysetsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheOriginsGetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheOriginsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheOriginsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheServicesGetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheServicesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEdgeCacheServicesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsEndpointPoliciesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysRouteViewsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGatewaysRouteViewsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsGrpcRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsHttpRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbEdgeExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbRouteExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsLbTrafficExtensionsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesRouteViewsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsMeshesRouteViewsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceBindingsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsServiceLbPoliciesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTcpRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsTlsRoutesPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsListArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsPatchArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsVersionsCreateArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsVersionsDeleteArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsVersionsGetArgs;
use crate::providers::gcp::clients::networkservices::NetworkservicesProjectsLocationsWasmPluginsVersionsListArgs;
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

    /// Networkservices projects locations get.
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
    pub fn networkservices_projects_locations_get(
        &self,
        args: &NetworkservicesProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations list.
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
    pub fn networkservices_projects_locations_list(
        &self,
        args: &NetworkservicesProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations authz extensions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthzExtension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_authz_extensions_get(
        &self,
        args: &NetworkservicesProjectsLocationsAuthzExtensionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthzExtension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_authz_extensions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_authz_extensions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations authz extensions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAuthzExtensionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_authz_extensions_list(
        &self,
        args: &NetworkservicesProjectsLocationsAuthzExtensionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAuthzExtensionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_authz_extensions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_authz_extensions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations edge cache keysets get iam policy.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_edge_cache_keysets_get_iam_policy(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheKeysetsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_keysets_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_keysets_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations edge cache origins get iam policy.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_edge_cache_origins_get_iam_policy(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheOriginsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_origins_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_origins_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations edge cache services get iam policy.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_edge_cache_services_get_iam_policy(
        &self,
        args: &NetworkservicesProjectsLocationsEdgeCacheServicesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_edge_cache_services_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_edge_cache_services_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations endpoint policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EndpointPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_endpoint_policies_get(
        &self,
        args: &NetworkservicesProjectsLocationsEndpointPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EndpointPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_endpoint_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_endpoint_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations endpoint policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEndpointPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_endpoint_policies_list(
        &self,
        args: &NetworkservicesProjectsLocationsEndpointPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEndpointPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_endpoint_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_endpoint_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations gateways get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Gateway result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_gateways_get(
        &self,
        args: &NetworkservicesProjectsLocationsGatewaysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Gateway, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_gateways_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_gateways_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations gateways list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGatewaysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_gateways_list(
        &self,
        args: &NetworkservicesProjectsLocationsGatewaysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGatewaysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_gateways_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_gateways_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations gateways route views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GatewayRouteView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_gateways_route_views_get(
        &self,
        args: &NetworkservicesProjectsLocationsGatewaysRouteViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GatewayRouteView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_gateways_route_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_gateways_route_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations gateways route views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGatewayRouteViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_gateways_route_views_list(
        &self,
        args: &NetworkservicesProjectsLocationsGatewaysRouteViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGatewayRouteViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_gateways_route_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_gateways_route_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations grpc routes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GrpcRoute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_grpc_routes_get(
        &self,
        args: &NetworkservicesProjectsLocationsGrpcRoutesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GrpcRoute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_grpc_routes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_grpc_routes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations grpc routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGrpcRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_grpc_routes_list(
        &self,
        args: &NetworkservicesProjectsLocationsGrpcRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGrpcRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_grpc_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_grpc_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations http routes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpRoute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_http_routes_get(
        &self,
        args: &NetworkservicesProjectsLocationsHttpRoutesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpRoute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_http_routes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_http_routes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations http routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHttpRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_http_routes_list(
        &self,
        args: &NetworkservicesProjectsLocationsHttpRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHttpRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_http_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_http_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations lb edge extensions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LbEdgeExtension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_lb_edge_extensions_get(
        &self,
        args: &NetworkservicesProjectsLocationsLbEdgeExtensionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LbEdgeExtension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_edge_extensions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_edge_extensions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb edge extensions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLbEdgeExtensionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_lb_edge_extensions_list(
        &self,
        args: &NetworkservicesProjectsLocationsLbEdgeExtensionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLbEdgeExtensionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_edge_extensions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_edge_extensions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations lb route extensions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LbRouteExtension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_lb_route_extensions_get(
        &self,
        args: &NetworkservicesProjectsLocationsLbRouteExtensionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LbRouteExtension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_route_extensions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_route_extensions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb route extensions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLbRouteExtensionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_lb_route_extensions_list(
        &self,
        args: &NetworkservicesProjectsLocationsLbRouteExtensionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLbRouteExtensionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_route_extensions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_route_extensions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations lb traffic extensions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LbTrafficExtension result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_lb_traffic_extensions_get(
        &self,
        args: &NetworkservicesProjectsLocationsLbTrafficExtensionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LbTrafficExtension, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_traffic_extensions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_traffic_extensions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations lb traffic extensions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLbTrafficExtensionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_lb_traffic_extensions_list(
        &self,
        args: &NetworkservicesProjectsLocationsLbTrafficExtensionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLbTrafficExtensionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_lb_traffic_extensions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_lb_traffic_extensions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations meshes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Mesh result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_meshes_get(
        &self,
        args: &NetworkservicesProjectsLocationsMeshesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Mesh, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_meshes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_meshes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations meshes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMeshesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_meshes_list(
        &self,
        args: &NetworkservicesProjectsLocationsMeshesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMeshesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_meshes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_meshes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations meshes route views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MeshRouteView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_meshes_route_views_get(
        &self,
        args: &NetworkservicesProjectsLocationsMeshesRouteViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MeshRouteView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_meshes_route_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_meshes_route_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations meshes route views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMeshRouteViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_meshes_route_views_list(
        &self,
        args: &NetworkservicesProjectsLocationsMeshesRouteViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMeshRouteViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_meshes_route_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_meshes_route_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations operations get.
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
    pub fn networkservices_projects_locations_operations_get(
        &self,
        args: &NetworkservicesProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations operations list.
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
    pub fn networkservices_projects_locations_operations_list(
        &self,
        args: &NetworkservicesProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations service bindings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceBinding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_service_bindings_get(
        &self,
        args: &NetworkservicesProjectsLocationsServiceBindingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceBinding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_bindings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_bindings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service bindings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceBindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_service_bindings_list(
        &self,
        args: &NetworkservicesProjectsLocationsServiceBindingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceBindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_bindings_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_bindings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations service lb policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceLbPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_service_lb_policies_get(
        &self,
        args: &NetworkservicesProjectsLocationsServiceLbPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceLbPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_lb_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_lb_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations service lb policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceLbPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_service_lb_policies_list(
        &self,
        args: &NetworkservicesProjectsLocationsServiceLbPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceLbPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_service_lb_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_service_lb_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations tcp routes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TcpRoute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_tcp_routes_get(
        &self,
        args: &NetworkservicesProjectsLocationsTcpRoutesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TcpRoute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tcp_routes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tcp_routes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tcp routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTcpRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_tcp_routes_list(
        &self,
        args: &NetworkservicesProjectsLocationsTcpRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTcpRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tcp_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tcp_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations tls routes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TlsRoute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_tls_routes_get(
        &self,
        args: &NetworkservicesProjectsLocationsTlsRoutesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TlsRoute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tls_routes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tls_routes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations tls routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTlsRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_tls_routes_list(
        &self,
        args: &NetworkservicesProjectsLocationsTlsRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTlsRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_tls_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_tls_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations wasm plugins get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WasmPlugin result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_wasm_plugins_get(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WasmPlugin, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations wasm plugins list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWasmPluginsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_wasm_plugins_list(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWasmPluginsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Networkservices projects locations wasm plugins versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WasmPluginVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_wasm_plugins_versions_get(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WasmPluginVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkservices projects locations wasm plugins versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListWasmPluginVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkservices_projects_locations_wasm_plugins_versions_list(
        &self,
        args: &NetworkservicesProjectsLocationsWasmPluginsVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListWasmPluginVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkservices_projects_locations_wasm_plugins_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkservices_projects_locations_wasm_plugins_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
