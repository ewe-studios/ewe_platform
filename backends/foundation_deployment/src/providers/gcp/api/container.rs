//! ContainerProvider - State-aware container API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       container API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::container::{
    container_projects_aggregated_usable_subnetworks_list_builder, container_projects_aggregated_usable_subnetworks_list_task,
    container_projects_locations_get_server_config_builder, container_projects_locations_get_server_config_task,
    container_projects_locations_clusters_check_autopilot_compatibility_builder, container_projects_locations_clusters_check_autopilot_compatibility_task,
    container_projects_locations_clusters_complete_ip_rotation_builder, container_projects_locations_clusters_complete_ip_rotation_task,
    container_projects_locations_clusters_create_builder, container_projects_locations_clusters_create_task,
    container_projects_locations_clusters_delete_builder, container_projects_locations_clusters_delete_task,
    container_projects_locations_clusters_fetch_cluster_upgrade_info_builder, container_projects_locations_clusters_fetch_cluster_upgrade_info_task,
    container_projects_locations_clusters_get_builder, container_projects_locations_clusters_get_task,
    container_projects_locations_clusters_get_jwks_builder, container_projects_locations_clusters_get_jwks_task,
    container_projects_locations_clusters_list_builder, container_projects_locations_clusters_list_task,
    container_projects_locations_clusters_set_addons_builder, container_projects_locations_clusters_set_addons_task,
    container_projects_locations_clusters_set_legacy_abac_builder, container_projects_locations_clusters_set_legacy_abac_task,
    container_projects_locations_clusters_set_locations_builder, container_projects_locations_clusters_set_locations_task,
    container_projects_locations_clusters_set_logging_builder, container_projects_locations_clusters_set_logging_task,
    container_projects_locations_clusters_set_maintenance_policy_builder, container_projects_locations_clusters_set_maintenance_policy_task,
    container_projects_locations_clusters_set_master_auth_builder, container_projects_locations_clusters_set_master_auth_task,
    container_projects_locations_clusters_set_monitoring_builder, container_projects_locations_clusters_set_monitoring_task,
    container_projects_locations_clusters_set_network_policy_builder, container_projects_locations_clusters_set_network_policy_task,
    container_projects_locations_clusters_set_resource_labels_builder, container_projects_locations_clusters_set_resource_labels_task,
    container_projects_locations_clusters_start_ip_rotation_builder, container_projects_locations_clusters_start_ip_rotation_task,
    container_projects_locations_clusters_update_builder, container_projects_locations_clusters_update_task,
    container_projects_locations_clusters_update_master_builder, container_projects_locations_clusters_update_master_task,
    container_projects_locations_clusters_node_pools_complete_upgrade_builder, container_projects_locations_clusters_node_pools_complete_upgrade_task,
    container_projects_locations_clusters_node_pools_create_builder, container_projects_locations_clusters_node_pools_create_task,
    container_projects_locations_clusters_node_pools_delete_builder, container_projects_locations_clusters_node_pools_delete_task,
    container_projects_locations_clusters_node_pools_fetch_node_pool_upgrade_info_builder, container_projects_locations_clusters_node_pools_fetch_node_pool_upgrade_info_task,
    container_projects_locations_clusters_node_pools_get_builder, container_projects_locations_clusters_node_pools_get_task,
    container_projects_locations_clusters_node_pools_list_builder, container_projects_locations_clusters_node_pools_list_task,
    container_projects_locations_clusters_node_pools_rollback_builder, container_projects_locations_clusters_node_pools_rollback_task,
    container_projects_locations_clusters_node_pools_set_autoscaling_builder, container_projects_locations_clusters_node_pools_set_autoscaling_task,
    container_projects_locations_clusters_node_pools_set_management_builder, container_projects_locations_clusters_node_pools_set_management_task,
    container_projects_locations_clusters_node_pools_set_size_builder, container_projects_locations_clusters_node_pools_set_size_task,
    container_projects_locations_clusters_node_pools_update_builder, container_projects_locations_clusters_node_pools_update_task,
    container_projects_locations_clusters_well_known_get_openid_configuration_builder, container_projects_locations_clusters_well_known_get_openid_configuration_task,
    container_projects_locations_operations_cancel_builder, container_projects_locations_operations_cancel_task,
    container_projects_locations_operations_get_builder, container_projects_locations_operations_get_task,
    container_projects_locations_operations_list_builder, container_projects_locations_operations_list_task,
    container_projects_zones_get_serverconfig_builder, container_projects_zones_get_serverconfig_task,
    container_projects_zones_clusters_addons_builder, container_projects_zones_clusters_addons_task,
    container_projects_zones_clusters_complete_ip_rotation_builder, container_projects_zones_clusters_complete_ip_rotation_task,
    container_projects_zones_clusters_create_builder, container_projects_zones_clusters_create_task,
    container_projects_zones_clusters_delete_builder, container_projects_zones_clusters_delete_task,
    container_projects_zones_clusters_fetch_cluster_upgrade_info_builder, container_projects_zones_clusters_fetch_cluster_upgrade_info_task,
    container_projects_zones_clusters_get_builder, container_projects_zones_clusters_get_task,
    container_projects_zones_clusters_legacy_abac_builder, container_projects_zones_clusters_legacy_abac_task,
    container_projects_zones_clusters_list_builder, container_projects_zones_clusters_list_task,
    container_projects_zones_clusters_locations_builder, container_projects_zones_clusters_locations_task,
    container_projects_zones_clusters_logging_builder, container_projects_zones_clusters_logging_task,
    container_projects_zones_clusters_master_builder, container_projects_zones_clusters_master_task,
    container_projects_zones_clusters_monitoring_builder, container_projects_zones_clusters_monitoring_task,
    container_projects_zones_clusters_resource_labels_builder, container_projects_zones_clusters_resource_labels_task,
    container_projects_zones_clusters_set_maintenance_policy_builder, container_projects_zones_clusters_set_maintenance_policy_task,
    container_projects_zones_clusters_set_master_auth_builder, container_projects_zones_clusters_set_master_auth_task,
    container_projects_zones_clusters_set_network_policy_builder, container_projects_zones_clusters_set_network_policy_task,
    container_projects_zones_clusters_start_ip_rotation_builder, container_projects_zones_clusters_start_ip_rotation_task,
    container_projects_zones_clusters_update_builder, container_projects_zones_clusters_update_task,
    container_projects_zones_clusters_node_pools_autoscaling_builder, container_projects_zones_clusters_node_pools_autoscaling_task,
    container_projects_zones_clusters_node_pools_create_builder, container_projects_zones_clusters_node_pools_create_task,
    container_projects_zones_clusters_node_pools_delete_builder, container_projects_zones_clusters_node_pools_delete_task,
    container_projects_zones_clusters_node_pools_fetch_node_pool_upgrade_info_builder, container_projects_zones_clusters_node_pools_fetch_node_pool_upgrade_info_task,
    container_projects_zones_clusters_node_pools_get_builder, container_projects_zones_clusters_node_pools_get_task,
    container_projects_zones_clusters_node_pools_list_builder, container_projects_zones_clusters_node_pools_list_task,
    container_projects_zones_clusters_node_pools_rollback_builder, container_projects_zones_clusters_node_pools_rollback_task,
    container_projects_zones_clusters_node_pools_set_management_builder, container_projects_zones_clusters_node_pools_set_management_task,
    container_projects_zones_clusters_node_pools_set_size_builder, container_projects_zones_clusters_node_pools_set_size_task,
    container_projects_zones_clusters_node_pools_update_builder, container_projects_zones_clusters_node_pools_update_task,
    container_projects_zones_operations_cancel_builder, container_projects_zones_operations_cancel_task,
    container_projects_zones_operations_get_builder, container_projects_zones_operations_get_task,
    container_projects_zones_operations_list_builder, container_projects_zones_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::container::CheckAutopilotCompatibilityResponse;
use crate::providers::gcp::clients::container::Cluster;
use crate::providers::gcp::clients::container::ClusterUpgradeInfo;
use crate::providers::gcp::clients::container::Empty;
use crate::providers::gcp::clients::container::GetJSONWebKeysResponse;
use crate::providers::gcp::clients::container::GetOpenIDConfigResponse;
use crate::providers::gcp::clients::container::ListClustersResponse;
use crate::providers::gcp::clients::container::ListNodePoolsResponse;
use crate::providers::gcp::clients::container::ListOperationsResponse;
use crate::providers::gcp::clients::container::ListUsableSubnetworksResponse;
use crate::providers::gcp::clients::container::NodePool;
use crate::providers::gcp::clients::container::NodePoolUpgradeInfo;
use crate::providers::gcp::clients::container::Operation;
use crate::providers::gcp::clients::container::ServerConfig;
use crate::providers::gcp::clients::container::ContainerProjectsAggregatedUsableSubnetworksListArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersCheckAutopilotCompatibilityArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersCompleteIpRotationArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersDeleteArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersFetchClusterUpgradeInfoArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersGetArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersGetJwksArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersListArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsCompleteUpgradeArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsDeleteArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsFetchNodePoolUpgradeInfoArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsGetArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsListArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsRollbackArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsSetAutoscalingArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsSetManagementArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsSetSizeArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsUpdateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetAddonsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetLegacyAbacArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetLocationsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetLoggingArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetMaintenancePolicyArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetMasterAuthArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetMonitoringArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetNetworkPolicyArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersSetResourceLabelsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersStartIpRotationArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersUpdateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersUpdateMasterArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersWellKnownGetOpenidConfigurationArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsGetServerConfigArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersAddonsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersCompleteIpRotationArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersDeleteArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersFetchClusterUpgradeInfoArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersGetArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersLegacyAbacArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersListArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersLocationsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersLoggingArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersMasterArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersMonitoringArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsAutoscalingArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsDeleteArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsFetchNodePoolUpgradeInfoArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsGetArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsListArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsRollbackArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsSetManagementArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsSetSizeArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsUpdateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersResourceLabelsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersSetMaintenancePolicyArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersSetMasterAuthArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersSetNetworkPolicyArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersStartIpRotationArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersUpdateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesGetServerconfigArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesOperationsCancelArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesOperationsGetArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ContainerProvider with automatic state tracking.
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
/// let provider = ContainerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ContainerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ContainerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ContainerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ContainerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Container projects aggregated usable subnetworks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUsableSubnetworksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_aggregated_usable_subnetworks_list(
        &self,
        args: &ContainerProjectsAggregatedUsableSubnetworksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUsableSubnetworksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_aggregated_usable_subnetworks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_aggregated_usable_subnetworks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations get server config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServerConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_get_server_config(
        &self,
        args: &ContainerProjectsLocationsGetServerConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServerConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_get_server_config_builder(
            &self.http_client,
            &args.name,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_get_server_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters check autopilot compatibility.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckAutopilotCompatibilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_check_autopilot_compatibility(
        &self,
        args: &ContainerProjectsLocationsClustersCheckAutopilotCompatibilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckAutopilotCompatibilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_check_autopilot_compatibility_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_check_autopilot_compatibility_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters complete ip rotation.
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
    pub fn container_projects_locations_clusters_complete_ip_rotation(
        &self,
        args: &ContainerProjectsLocationsClustersCompleteIpRotationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_complete_ip_rotation_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_complete_ip_rotation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters create.
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
    pub fn container_projects_locations_clusters_create(
        &self,
        args: &ContainerProjectsLocationsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters delete.
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
    pub fn container_projects_locations_clusters_delete(
        &self,
        args: &ContainerProjectsLocationsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.clusterId,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters fetch cluster upgrade info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClusterUpgradeInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_fetch_cluster_upgrade_info(
        &self,
        args: &ContainerProjectsLocationsClustersFetchClusterUpgradeInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClusterUpgradeInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_fetch_cluster_upgrade_info_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_fetch_cluster_upgrade_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Cluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_get(
        &self,
        args: &ContainerProjectsLocationsClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_get_builder(
            &self.http_client,
            &args.name,
            &args.clusterId,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters get jwks.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetJSONWebKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_get_jwks(
        &self,
        args: &ContainerProjectsLocationsClustersGetJwksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetJSONWebKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_get_jwks_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_get_jwks_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_list(
        &self,
        args: &ContainerProjectsLocationsClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set addons.
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
    pub fn container_projects_locations_clusters_set_addons(
        &self,
        args: &ContainerProjectsLocationsClustersSetAddonsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_addons_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_addons_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set legacy abac.
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
    pub fn container_projects_locations_clusters_set_legacy_abac(
        &self,
        args: &ContainerProjectsLocationsClustersSetLegacyAbacArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_legacy_abac_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_legacy_abac_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set locations.
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
    pub fn container_projects_locations_clusters_set_locations(
        &self,
        args: &ContainerProjectsLocationsClustersSetLocationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_locations_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_locations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set logging.
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
    pub fn container_projects_locations_clusters_set_logging(
        &self,
        args: &ContainerProjectsLocationsClustersSetLoggingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_logging_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_logging_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set maintenance policy.
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
    pub fn container_projects_locations_clusters_set_maintenance_policy(
        &self,
        args: &ContainerProjectsLocationsClustersSetMaintenancePolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_maintenance_policy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_maintenance_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set master auth.
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
    pub fn container_projects_locations_clusters_set_master_auth(
        &self,
        args: &ContainerProjectsLocationsClustersSetMasterAuthArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_master_auth_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_master_auth_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set monitoring.
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
    pub fn container_projects_locations_clusters_set_monitoring(
        &self,
        args: &ContainerProjectsLocationsClustersSetMonitoringArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_monitoring_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_monitoring_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set network policy.
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
    pub fn container_projects_locations_clusters_set_network_policy(
        &self,
        args: &ContainerProjectsLocationsClustersSetNetworkPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_network_policy_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_network_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters set resource labels.
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
    pub fn container_projects_locations_clusters_set_resource_labels(
        &self,
        args: &ContainerProjectsLocationsClustersSetResourceLabelsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_set_resource_labels_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_set_resource_labels_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters start ip rotation.
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
    pub fn container_projects_locations_clusters_start_ip_rotation(
        &self,
        args: &ContainerProjectsLocationsClustersStartIpRotationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_start_ip_rotation_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_start_ip_rotation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters update.
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
    pub fn container_projects_locations_clusters_update(
        &self,
        args: &ContainerProjectsLocationsClustersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters update master.
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
    pub fn container_projects_locations_clusters_update_master(
        &self,
        args: &ContainerProjectsLocationsClustersUpdateMasterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_update_master_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_update_master_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools complete upgrade.
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
    pub fn container_projects_locations_clusters_node_pools_complete_upgrade(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsCompleteUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_complete_upgrade_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_complete_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools create.
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
    pub fn container_projects_locations_clusters_node_pools_create(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools delete.
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
    pub fn container_projects_locations_clusters_node_pools_delete(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_delete_builder(
            &self.http_client,
            &args.name,
            &args.clusterId,
            &args.nodePoolId,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools fetch node pool upgrade info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NodePoolUpgradeInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_node_pools_fetch_node_pool_upgrade_info(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsFetchNodePoolUpgradeInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NodePoolUpgradeInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_fetch_node_pool_upgrade_info_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_fetch_node_pool_upgrade_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NodePool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_node_pools_get(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NodePool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_get_builder(
            &self.http_client,
            &args.name,
            &args.clusterId,
            &args.nodePoolId,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNodePoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_node_pools_list(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNodePoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_list_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools rollback.
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
    pub fn container_projects_locations_clusters_node_pools_rollback(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_rollback_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools set autoscaling.
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
    pub fn container_projects_locations_clusters_node_pools_set_autoscaling(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsSetAutoscalingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_set_autoscaling_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_set_autoscaling_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools set management.
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
    pub fn container_projects_locations_clusters_node_pools_set_management(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsSetManagementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_set_management_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_set_management_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools set size.
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
    pub fn container_projects_locations_clusters_node_pools_set_size(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsSetSizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_set_size_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_set_size_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters node pools update.
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
    pub fn container_projects_locations_clusters_node_pools_update(
        &self,
        args: &ContainerProjectsLocationsClustersNodePoolsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_node_pools_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_node_pools_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations clusters well known get openid configuration.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetOpenIDConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_locations_clusters_well_known_get_openid_configuration(
        &self,
        args: &ContainerProjectsLocationsClustersWellKnownGetOpenidConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetOpenIDConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_clusters_well_known_get_openid_configuration_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_clusters_well_known_get_openid_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations operations cancel.
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
    pub fn container_projects_locations_operations_cancel(
        &self,
        args: &ContainerProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations operations get.
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
    pub fn container_projects_locations_operations_get(
        &self,
        args: &ContainerProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
            &args.operationId,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects locations operations list.
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
    pub fn container_projects_locations_operations_list(
        &self,
        args: &ContainerProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_locations_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones get serverconfig.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServerConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_zones_get_serverconfig(
        &self,
        args: &ContainerProjectsZonesGetServerconfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServerConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_get_serverconfig_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_get_serverconfig_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters addons.
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
    pub fn container_projects_zones_clusters_addons(
        &self,
        args: &ContainerProjectsZonesClustersAddonsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_addons_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_addons_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters complete ip rotation.
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
    pub fn container_projects_zones_clusters_complete_ip_rotation(
        &self,
        args: &ContainerProjectsZonesClustersCompleteIpRotationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_complete_ip_rotation_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_complete_ip_rotation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters create.
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
    pub fn container_projects_zones_clusters_create(
        &self,
        args: &ContainerProjectsZonesClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_create_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters delete.
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
    pub fn container_projects_zones_clusters_delete(
        &self,
        args: &ContainerProjectsZonesClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters fetch cluster upgrade info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClusterUpgradeInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_zones_clusters_fetch_cluster_upgrade_info(
        &self,
        args: &ContainerProjectsZonesClustersFetchClusterUpgradeInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClusterUpgradeInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_fetch_cluster_upgrade_info_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_fetch_cluster_upgrade_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Cluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_zones_clusters_get(
        &self,
        args: &ContainerProjectsZonesClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_get_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters legacy abac.
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
    pub fn container_projects_zones_clusters_legacy_abac(
        &self,
        args: &ContainerProjectsZonesClustersLegacyAbacArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_legacy_abac_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_legacy_abac_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_zones_clusters_list(
        &self,
        args: &ContainerProjectsZonesClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_list_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters locations.
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
    pub fn container_projects_zones_clusters_locations(
        &self,
        args: &ContainerProjectsZonesClustersLocationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_locations_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_locations_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters logging.
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
    pub fn container_projects_zones_clusters_logging(
        &self,
        args: &ContainerProjectsZonesClustersLoggingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_logging_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_logging_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters master.
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
    pub fn container_projects_zones_clusters_master(
        &self,
        args: &ContainerProjectsZonesClustersMasterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_master_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_master_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters monitoring.
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
    pub fn container_projects_zones_clusters_monitoring(
        &self,
        args: &ContainerProjectsZonesClustersMonitoringArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_monitoring_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_monitoring_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters resource labels.
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
    pub fn container_projects_zones_clusters_resource_labels(
        &self,
        args: &ContainerProjectsZonesClustersResourceLabelsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_resource_labels_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_resource_labels_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters set maintenance policy.
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
    pub fn container_projects_zones_clusters_set_maintenance_policy(
        &self,
        args: &ContainerProjectsZonesClustersSetMaintenancePolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_set_maintenance_policy_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_set_maintenance_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters set master auth.
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
    pub fn container_projects_zones_clusters_set_master_auth(
        &self,
        args: &ContainerProjectsZonesClustersSetMasterAuthArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_set_master_auth_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_set_master_auth_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters set network policy.
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
    pub fn container_projects_zones_clusters_set_network_policy(
        &self,
        args: &ContainerProjectsZonesClustersSetNetworkPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_set_network_policy_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_set_network_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters start ip rotation.
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
    pub fn container_projects_zones_clusters_start_ip_rotation(
        &self,
        args: &ContainerProjectsZonesClustersStartIpRotationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_start_ip_rotation_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_start_ip_rotation_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters update.
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
    pub fn container_projects_zones_clusters_update(
        &self,
        args: &ContainerProjectsZonesClustersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_update_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools autoscaling.
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
    pub fn container_projects_zones_clusters_node_pools_autoscaling(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsAutoscalingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_autoscaling_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.nodePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_autoscaling_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools create.
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
    pub fn container_projects_zones_clusters_node_pools_create(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_create_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools delete.
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
    pub fn container_projects_zones_clusters_node_pools_delete(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_delete_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.nodePoolId,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools fetch node pool upgrade info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NodePoolUpgradeInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_zones_clusters_node_pools_fetch_node_pool_upgrade_info(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsFetchNodePoolUpgradeInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NodePoolUpgradeInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_fetch_node_pool_upgrade_info_builder(
            &self.http_client,
            &args.name,
            &args.version,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_fetch_node_pool_upgrade_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NodePool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_zones_clusters_node_pools_get(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NodePool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_get_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.nodePoolId,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNodePoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn container_projects_zones_clusters_node_pools_list(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNodePoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_list_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools rollback.
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
    pub fn container_projects_zones_clusters_node_pools_rollback(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_rollback_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.nodePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools set management.
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
    pub fn container_projects_zones_clusters_node_pools_set_management(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsSetManagementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_set_management_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.nodePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_set_management_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools set size.
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
    pub fn container_projects_zones_clusters_node_pools_set_size(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsSetSizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_set_size_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.nodePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_set_size_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones clusters node pools update.
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
    pub fn container_projects_zones_clusters_node_pools_update(
        &self,
        args: &ContainerProjectsZonesClustersNodePoolsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_clusters_node_pools_update_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.clusterId,
            &args.nodePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_clusters_node_pools_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones operations cancel.
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
    pub fn container_projects_zones_operations_cancel(
        &self,
        args: &ContainerProjectsZonesOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_operations_cancel_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.operationId,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones operations get.
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
    pub fn container_projects_zones_operations_get(
        &self,
        args: &ContainerProjectsZonesOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_operations_get_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.operationId,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Container projects zones operations list.
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
    pub fn container_projects_zones_operations_list(
        &self,
        args: &ContainerProjectsZonesOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = container_projects_zones_operations_list_builder(
            &self.http_client,
            &args.projectId,
            &args.zone,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = container_projects_zones_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
