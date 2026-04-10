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
    container_projects_locations_clusters_complete_ip_rotation_builder, container_projects_locations_clusters_complete_ip_rotation_task,
    container_projects_locations_clusters_create_builder, container_projects_locations_clusters_create_task,
    container_projects_locations_clusters_delete_builder, container_projects_locations_clusters_delete_task,
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
    container_projects_locations_clusters_node_pools_rollback_builder, container_projects_locations_clusters_node_pools_rollback_task,
    container_projects_locations_clusters_node_pools_set_autoscaling_builder, container_projects_locations_clusters_node_pools_set_autoscaling_task,
    container_projects_locations_clusters_node_pools_set_management_builder, container_projects_locations_clusters_node_pools_set_management_task,
    container_projects_locations_clusters_node_pools_set_size_builder, container_projects_locations_clusters_node_pools_set_size_task,
    container_projects_locations_clusters_node_pools_update_builder, container_projects_locations_clusters_node_pools_update_task,
    container_projects_locations_operations_cancel_builder, container_projects_locations_operations_cancel_task,
    container_projects_zones_clusters_addons_builder, container_projects_zones_clusters_addons_task,
    container_projects_zones_clusters_complete_ip_rotation_builder, container_projects_zones_clusters_complete_ip_rotation_task,
    container_projects_zones_clusters_create_builder, container_projects_zones_clusters_create_task,
    container_projects_zones_clusters_delete_builder, container_projects_zones_clusters_delete_task,
    container_projects_zones_clusters_legacy_abac_builder, container_projects_zones_clusters_legacy_abac_task,
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
    container_projects_zones_clusters_node_pools_rollback_builder, container_projects_zones_clusters_node_pools_rollback_task,
    container_projects_zones_clusters_node_pools_set_management_builder, container_projects_zones_clusters_node_pools_set_management_task,
    container_projects_zones_clusters_node_pools_set_size_builder, container_projects_zones_clusters_node_pools_set_size_task,
    container_projects_zones_clusters_node_pools_update_builder, container_projects_zones_clusters_node_pools_update_task,
    container_projects_zones_operations_cancel_builder, container_projects_zones_operations_cancel_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::container::Empty;
use crate::providers::gcp::clients::container::Operation;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersCompleteIpRotationArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersDeleteArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsCompleteUpgradeArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsLocationsClustersNodePoolsDeleteArgs;
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
use crate::providers::gcp::clients::container::ContainerProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersAddonsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersCompleteIpRotationArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersDeleteArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersLegacyAbacArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersLocationsArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersLoggingArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersMasterArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersMonitoringArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsAutoscalingArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsCreateArgs;
use crate::providers::gcp::clients::container::ContainerProjectsZonesClustersNodePoolsDeleteArgs;
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
use crate::providers::gcp::clients::container::ContainerProjectsZonesOperationsCancelArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ContainerProvider with automatic state tracking.
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
/// let provider = ContainerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ContainerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ContainerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ContainerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

}
