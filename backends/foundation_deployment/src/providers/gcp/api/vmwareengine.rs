//! VmwareengineProvider - State-aware vmwareengine API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       vmwareengine API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::vmwareengine::{
    vmwareengine_projects_locations_datastores_create_builder, vmwareengine_projects_locations_datastores_create_task,
    vmwareengine_projects_locations_datastores_delete_builder, vmwareengine_projects_locations_datastores_delete_task,
    vmwareengine_projects_locations_datastores_patch_builder, vmwareengine_projects_locations_datastores_patch_task,
    vmwareengine_projects_locations_dns_bind_permission_grant_builder, vmwareengine_projects_locations_dns_bind_permission_grant_task,
    vmwareengine_projects_locations_dns_bind_permission_revoke_builder, vmwareengine_projects_locations_dns_bind_permission_revoke_task,
    vmwareengine_projects_locations_network_peerings_create_builder, vmwareengine_projects_locations_network_peerings_create_task,
    vmwareengine_projects_locations_network_peerings_delete_builder, vmwareengine_projects_locations_network_peerings_delete_task,
    vmwareengine_projects_locations_network_peerings_patch_builder, vmwareengine_projects_locations_network_peerings_patch_task,
    vmwareengine_projects_locations_network_policies_create_builder, vmwareengine_projects_locations_network_policies_create_task,
    vmwareengine_projects_locations_network_policies_delete_builder, vmwareengine_projects_locations_network_policies_delete_task,
    vmwareengine_projects_locations_network_policies_patch_builder, vmwareengine_projects_locations_network_policies_patch_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_create_builder, vmwareengine_projects_locations_network_policies_external_access_rules_create_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_delete_builder, vmwareengine_projects_locations_network_policies_external_access_rules_delete_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_patch_builder, vmwareengine_projects_locations_network_policies_external_access_rules_patch_task,
    vmwareengine_projects_locations_operations_delete_builder, vmwareengine_projects_locations_operations_delete_task,
    vmwareengine_projects_locations_private_clouds_create_builder, vmwareengine_projects_locations_private_clouds_create_task,
    vmwareengine_projects_locations_private_clouds_delete_builder, vmwareengine_projects_locations_private_clouds_delete_task,
    vmwareengine_projects_locations_private_clouds_patch_builder, vmwareengine_projects_locations_private_clouds_patch_task,
    vmwareengine_projects_locations_private_clouds_private_cloud_deletion_now_builder, vmwareengine_projects_locations_private_clouds_private_cloud_deletion_now_task,
    vmwareengine_projects_locations_private_clouds_reset_nsx_credentials_builder, vmwareengine_projects_locations_private_clouds_reset_nsx_credentials_task,
    vmwareengine_projects_locations_private_clouds_reset_vcenter_credentials_builder, vmwareengine_projects_locations_private_clouds_reset_vcenter_credentials_task,
    vmwareengine_projects_locations_private_clouds_set_iam_policy_builder, vmwareengine_projects_locations_private_clouds_set_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_test_iam_permissions_builder, vmwareengine_projects_locations_private_clouds_test_iam_permissions_task,
    vmwareengine_projects_locations_private_clouds_undelete_builder, vmwareengine_projects_locations_private_clouds_undelete_task,
    vmwareengine_projects_locations_private_clouds_update_dns_forwarding_builder, vmwareengine_projects_locations_private_clouds_update_dns_forwarding_task,
    vmwareengine_projects_locations_private_clouds_clusters_create_builder, vmwareengine_projects_locations_private_clouds_clusters_create_task,
    vmwareengine_projects_locations_private_clouds_clusters_delete_builder, vmwareengine_projects_locations_private_clouds_clusters_delete_task,
    vmwareengine_projects_locations_private_clouds_clusters_mount_datastore_builder, vmwareengine_projects_locations_private_clouds_clusters_mount_datastore_task,
    vmwareengine_projects_locations_private_clouds_clusters_patch_builder, vmwareengine_projects_locations_private_clouds_clusters_patch_task,
    vmwareengine_projects_locations_private_clouds_clusters_set_iam_policy_builder, vmwareengine_projects_locations_private_clouds_clusters_set_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_clusters_test_iam_permissions_builder, vmwareengine_projects_locations_private_clouds_clusters_test_iam_permissions_task,
    vmwareengine_projects_locations_private_clouds_clusters_unmount_datastore_builder, vmwareengine_projects_locations_private_clouds_clusters_unmount_datastore_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_create_builder, vmwareengine_projects_locations_private_clouds_external_addresses_create_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_delete_builder, vmwareengine_projects_locations_private_clouds_external_addresses_delete_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_patch_builder, vmwareengine_projects_locations_private_clouds_external_addresses_patch_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_create_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_create_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_set_iam_policy_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_set_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_test_iam_permissions_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_test_iam_permissions_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_create_builder, vmwareengine_projects_locations_private_clouds_logging_servers_create_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_delete_builder, vmwareengine_projects_locations_private_clouds_logging_servers_delete_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_patch_builder, vmwareengine_projects_locations_private_clouds_logging_servers_patch_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_create_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_create_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_delete_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_delete_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_patch_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_patch_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_repair_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_repair_task,
    vmwareengine_projects_locations_private_clouds_subnets_patch_builder, vmwareengine_projects_locations_private_clouds_subnets_patch_task,
    vmwareengine_projects_locations_private_clouds_upgrades_patch_builder, vmwareengine_projects_locations_private_clouds_upgrades_patch_task,
    vmwareengine_projects_locations_private_connections_create_builder, vmwareengine_projects_locations_private_connections_create_task,
    vmwareengine_projects_locations_private_connections_delete_builder, vmwareengine_projects_locations_private_connections_delete_task,
    vmwareengine_projects_locations_private_connections_patch_builder, vmwareengine_projects_locations_private_connections_patch_task,
    vmwareengine_projects_locations_vmware_engine_networks_create_builder, vmwareengine_projects_locations_vmware_engine_networks_create_task,
    vmwareengine_projects_locations_vmware_engine_networks_delete_builder, vmwareengine_projects_locations_vmware_engine_networks_delete_task,
    vmwareengine_projects_locations_vmware_engine_networks_patch_builder, vmwareengine_projects_locations_vmware_engine_networks_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::vmwareengine::Empty;
use crate::providers::gcp::clients::vmwareengine::Operation;
use crate::providers::gcp::clients::vmwareengine::Policy;
use crate::providers::gcp::clients::vmwareengine::TestIamPermissionsResponse;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDnsBindPermissionGrantArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDnsBindPermissionRevokeArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersMountDatastoreArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersUnmountDatastoreArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysSetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysTestIamPermissionsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsRepairArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsPrivateCloudDeletionNowArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsResetNsxCredentialsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsResetVcenterCredentialsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsSetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsSubnetsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsTestIamPermissionsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUndeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUpdateDnsForwardingArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUpgradesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsVmwareEngineNetworksCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsVmwareEngineNetworksDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsVmwareEngineNetworksPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// VmwareengineProvider with automatic state tracking.
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
/// let provider = VmwareengineProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct VmwareengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> VmwareengineProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new VmwareengineProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Vmwareengine projects locations datastores create.
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
    pub fn vmwareengine_projects_locations_datastores_create(
        &self,
        args: &VmwareengineProjectsLocationsDatastoresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_datastores_create_builder(
            &self.http_client,
            &args.parent,
            &args.datastoreId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_datastores_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations datastores delete.
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
    pub fn vmwareengine_projects_locations_datastores_delete(
        &self,
        args: &VmwareengineProjectsLocationsDatastoresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_datastores_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_datastores_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations datastores patch.
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
    pub fn vmwareengine_projects_locations_datastores_patch(
        &self,
        args: &VmwareengineProjectsLocationsDatastoresPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_datastores_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_datastores_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations dns bind permission grant.
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
    pub fn vmwareengine_projects_locations_dns_bind_permission_grant(
        &self,
        args: &VmwareengineProjectsLocationsDnsBindPermissionGrantArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_dns_bind_permission_grant_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_dns_bind_permission_grant_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations dns bind permission revoke.
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
    pub fn vmwareengine_projects_locations_dns_bind_permission_revoke(
        &self,
        args: &VmwareengineProjectsLocationsDnsBindPermissionRevokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_dns_bind_permission_revoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_dns_bind_permission_revoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network peerings create.
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
    pub fn vmwareengine_projects_locations_network_peerings_create(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPeeringsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_peerings_create_builder(
            &self.http_client,
            &args.parent,
            &args.networkPeeringId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_peerings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network peerings delete.
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
    pub fn vmwareengine_projects_locations_network_peerings_delete(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPeeringsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_peerings_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_peerings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network peerings patch.
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
    pub fn vmwareengine_projects_locations_network_peerings_patch(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPeeringsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_peerings_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_peerings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies create.
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
    pub fn vmwareengine_projects_locations_network_policies_create(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.networkPolicyId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies delete.
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
    pub fn vmwareengine_projects_locations_network_policies_delete(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies patch.
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
    pub fn vmwareengine_projects_locations_network_policies_patch(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies external access rules create.
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
    pub fn vmwareengine_projects_locations_network_policies_external_access_rules_create(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_external_access_rules_create_builder(
            &self.http_client,
            &args.parent,
            &args.externalAccessRuleId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_external_access_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies external access rules delete.
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
    pub fn vmwareengine_projects_locations_network_policies_external_access_rules_delete(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_external_access_rules_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_external_access_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies external access rules patch.
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
    pub fn vmwareengine_projects_locations_network_policies_external_access_rules_patch(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_external_access_rules_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_external_access_rules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations operations delete.
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
    pub fn vmwareengine_projects_locations_operations_delete(
        &self,
        args: &VmwareengineProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds create.
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
    pub fn vmwareengine_projects_locations_private_clouds_create(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_create_builder(
            &self.http_client,
            &args.parent,
            &args.privateCloudId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds delete.
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
    pub fn vmwareengine_projects_locations_private_clouds_delete(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_delete_builder(
            &self.http_client,
            &args.name,
            &args.delayHours,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds patch.
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
    pub fn vmwareengine_projects_locations_private_clouds_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds private cloud deletion now.
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
    pub fn vmwareengine_projects_locations_private_clouds_private_cloud_deletion_now(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsPrivateCloudDeletionNowArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_private_cloud_deletion_now_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_private_cloud_deletion_now_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds reset nsx credentials.
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
    pub fn vmwareengine_projects_locations_private_clouds_reset_nsx_credentials(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsResetNsxCredentialsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_reset_nsx_credentials_builder(
            &self.http_client,
            &args.privateCloud,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_reset_nsx_credentials_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds reset vcenter credentials.
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
    pub fn vmwareengine_projects_locations_private_clouds_reset_vcenter_credentials(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsResetVcenterCredentialsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_reset_vcenter_credentials_builder(
            &self.http_client,
            &args.privateCloud,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_reset_vcenter_credentials_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds set iam policy.
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
    pub fn vmwareengine_projects_locations_private_clouds_set_iam_policy(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds test iam permissions.
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
    pub fn vmwareengine_projects_locations_private_clouds_test_iam_permissions(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds undelete.
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
    pub fn vmwareengine_projects_locations_private_clouds_undelete(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds update dns forwarding.
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
    pub fn vmwareengine_projects_locations_private_clouds_update_dns_forwarding(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsUpdateDnsForwardingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_update_dns_forwarding_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_update_dns_forwarding_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters create.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_create(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters delete.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_delete(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters mount datastore.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_mount_datastore(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersMountDatastoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_mount_datastore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_mount_datastore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters patch.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters set iam policy.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_set_iam_policy(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters test iam permissions.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_test_iam_permissions(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters unmount datastore.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_unmount_datastore(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersUnmountDatastoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_unmount_datastore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_unmount_datastore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds external addresses create.
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
    pub fn vmwareengine_projects_locations_private_clouds_external_addresses_create(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsExternalAddressesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_external_addresses_create_builder(
            &self.http_client,
            &args.parent,
            &args.externalAddressId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_external_addresses_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds external addresses delete.
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
    pub fn vmwareengine_projects_locations_private_clouds_external_addresses_delete(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsExternalAddressesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_external_addresses_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_external_addresses_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds external addresses patch.
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
    pub fn vmwareengine_projects_locations_private_clouds_external_addresses_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsExternalAddressesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_external_addresses_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_external_addresses_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds hcx activation keys create.
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
    pub fn vmwareengine_projects_locations_private_clouds_hcx_activation_keys_create(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_create_builder(
            &self.http_client,
            &args.parent,
            &args.hcxActivationKeyId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds hcx activation keys set iam policy.
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
    pub fn vmwareengine_projects_locations_private_clouds_hcx_activation_keys_set_iam_policy(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds hcx activation keys test iam permissions.
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
    pub fn vmwareengine_projects_locations_private_clouds_hcx_activation_keys_test_iam_permissions(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds logging servers create.
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
    pub fn vmwareengine_projects_locations_private_clouds_logging_servers_create(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsLoggingServersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_logging_servers_create_builder(
            &self.http_client,
            &args.parent,
            &args.loggingServerId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_logging_servers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds logging servers delete.
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
    pub fn vmwareengine_projects_locations_private_clouds_logging_servers_delete(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsLoggingServersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_logging_servers_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_logging_servers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds logging servers patch.
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
    pub fn vmwareengine_projects_locations_private_clouds_logging_servers_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsLoggingServersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_logging_servers_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_logging_servers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds management dns zone bindings create.
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
    pub fn vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_create(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_create_builder(
            &self.http_client,
            &args.parent,
            &args.managementDnsZoneBindingId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds management dns zone bindings delete.
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
    pub fn vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_delete(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds management dns zone bindings patch.
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
    pub fn vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds management dns zone bindings repair.
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
    pub fn vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_repair(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsRepairArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_repair_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_repair_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds subnets patch.
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
    pub fn vmwareengine_projects_locations_private_clouds_subnets_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsSubnetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_subnets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_subnets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds upgrades patch.
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
    pub fn vmwareengine_projects_locations_private_clouds_upgrades_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsUpgradesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_upgrades_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_upgrades_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private connections create.
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
    pub fn vmwareengine_projects_locations_private_connections_create(
        &self,
        args: &VmwareengineProjectsLocationsPrivateConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.privateConnectionId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private connections delete.
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
    pub fn vmwareengine_projects_locations_private_connections_delete(
        &self,
        args: &VmwareengineProjectsLocationsPrivateConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_connections_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private connections patch.
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
    pub fn vmwareengine_projects_locations_private_connections_patch(
        &self,
        args: &VmwareengineProjectsLocationsPrivateConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_connections_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations vmware engine networks create.
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
    pub fn vmwareengine_projects_locations_vmware_engine_networks_create(
        &self,
        args: &VmwareengineProjectsLocationsVmwareEngineNetworksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_vmware_engine_networks_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.validateOnly,
            &args.vmwareEngineNetworkId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_vmware_engine_networks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations vmware engine networks delete.
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
    pub fn vmwareengine_projects_locations_vmware_engine_networks_delete(
        &self,
        args: &VmwareengineProjectsLocationsVmwareEngineNetworksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_vmware_engine_networks_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_vmware_engine_networks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations vmware engine networks patch.
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
    pub fn vmwareengine_projects_locations_vmware_engine_networks_patch(
        &self,
        args: &VmwareengineProjectsLocationsVmwareEngineNetworksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_vmware_engine_networks_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_vmware_engine_networks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
