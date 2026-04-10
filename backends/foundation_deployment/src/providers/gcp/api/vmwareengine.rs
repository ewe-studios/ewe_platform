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
    vmwareengine_projects_locations_get_builder, vmwareengine_projects_locations_get_task,
    vmwareengine_projects_locations_get_dns_bind_permission_builder, vmwareengine_projects_locations_get_dns_bind_permission_task,
    vmwareengine_projects_locations_list_builder, vmwareengine_projects_locations_list_task,
    vmwareengine_projects_locations_announcements_get_builder, vmwareengine_projects_locations_announcements_get_task,
    vmwareengine_projects_locations_announcements_list_builder, vmwareengine_projects_locations_announcements_list_task,
    vmwareengine_projects_locations_datastores_create_builder, vmwareengine_projects_locations_datastores_create_task,
    vmwareengine_projects_locations_datastores_delete_builder, vmwareengine_projects_locations_datastores_delete_task,
    vmwareengine_projects_locations_datastores_get_builder, vmwareengine_projects_locations_datastores_get_task,
    vmwareengine_projects_locations_datastores_list_builder, vmwareengine_projects_locations_datastores_list_task,
    vmwareengine_projects_locations_datastores_patch_builder, vmwareengine_projects_locations_datastores_patch_task,
    vmwareengine_projects_locations_dns_bind_permission_grant_builder, vmwareengine_projects_locations_dns_bind_permission_grant_task,
    vmwareengine_projects_locations_dns_bind_permission_revoke_builder, vmwareengine_projects_locations_dns_bind_permission_revoke_task,
    vmwareengine_projects_locations_network_peerings_create_builder, vmwareengine_projects_locations_network_peerings_create_task,
    vmwareengine_projects_locations_network_peerings_delete_builder, vmwareengine_projects_locations_network_peerings_delete_task,
    vmwareengine_projects_locations_network_peerings_get_builder, vmwareengine_projects_locations_network_peerings_get_task,
    vmwareengine_projects_locations_network_peerings_list_builder, vmwareengine_projects_locations_network_peerings_list_task,
    vmwareengine_projects_locations_network_peerings_patch_builder, vmwareengine_projects_locations_network_peerings_patch_task,
    vmwareengine_projects_locations_network_peerings_peering_routes_list_builder, vmwareengine_projects_locations_network_peerings_peering_routes_list_task,
    vmwareengine_projects_locations_network_policies_create_builder, vmwareengine_projects_locations_network_policies_create_task,
    vmwareengine_projects_locations_network_policies_delete_builder, vmwareengine_projects_locations_network_policies_delete_task,
    vmwareengine_projects_locations_network_policies_fetch_external_addresses_builder, vmwareengine_projects_locations_network_policies_fetch_external_addresses_task,
    vmwareengine_projects_locations_network_policies_get_builder, vmwareengine_projects_locations_network_policies_get_task,
    vmwareengine_projects_locations_network_policies_list_builder, vmwareengine_projects_locations_network_policies_list_task,
    vmwareengine_projects_locations_network_policies_patch_builder, vmwareengine_projects_locations_network_policies_patch_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_create_builder, vmwareengine_projects_locations_network_policies_external_access_rules_create_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_delete_builder, vmwareengine_projects_locations_network_policies_external_access_rules_delete_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_get_builder, vmwareengine_projects_locations_network_policies_external_access_rules_get_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_list_builder, vmwareengine_projects_locations_network_policies_external_access_rules_list_task,
    vmwareengine_projects_locations_network_policies_external_access_rules_patch_builder, vmwareengine_projects_locations_network_policies_external_access_rules_patch_task,
    vmwareengine_projects_locations_node_types_get_builder, vmwareengine_projects_locations_node_types_get_task,
    vmwareengine_projects_locations_node_types_list_builder, vmwareengine_projects_locations_node_types_list_task,
    vmwareengine_projects_locations_operations_delete_builder, vmwareengine_projects_locations_operations_delete_task,
    vmwareengine_projects_locations_operations_get_builder, vmwareengine_projects_locations_operations_get_task,
    vmwareengine_projects_locations_operations_list_builder, vmwareengine_projects_locations_operations_list_task,
    vmwareengine_projects_locations_private_clouds_create_builder, vmwareengine_projects_locations_private_clouds_create_task,
    vmwareengine_projects_locations_private_clouds_delete_builder, vmwareengine_projects_locations_private_clouds_delete_task,
    vmwareengine_projects_locations_private_clouds_get_builder, vmwareengine_projects_locations_private_clouds_get_task,
    vmwareengine_projects_locations_private_clouds_get_dns_forwarding_builder, vmwareengine_projects_locations_private_clouds_get_dns_forwarding_task,
    vmwareengine_projects_locations_private_clouds_get_iam_policy_builder, vmwareengine_projects_locations_private_clouds_get_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_list_builder, vmwareengine_projects_locations_private_clouds_list_task,
    vmwareengine_projects_locations_private_clouds_patch_builder, vmwareengine_projects_locations_private_clouds_patch_task,
    vmwareengine_projects_locations_private_clouds_private_cloud_deletion_now_builder, vmwareengine_projects_locations_private_clouds_private_cloud_deletion_now_task,
    vmwareengine_projects_locations_private_clouds_reset_nsx_credentials_builder, vmwareengine_projects_locations_private_clouds_reset_nsx_credentials_task,
    vmwareengine_projects_locations_private_clouds_reset_vcenter_credentials_builder, vmwareengine_projects_locations_private_clouds_reset_vcenter_credentials_task,
    vmwareengine_projects_locations_private_clouds_set_iam_policy_builder, vmwareengine_projects_locations_private_clouds_set_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_show_nsx_credentials_builder, vmwareengine_projects_locations_private_clouds_show_nsx_credentials_task,
    vmwareengine_projects_locations_private_clouds_show_vcenter_credentials_builder, vmwareengine_projects_locations_private_clouds_show_vcenter_credentials_task,
    vmwareengine_projects_locations_private_clouds_test_iam_permissions_builder, vmwareengine_projects_locations_private_clouds_test_iam_permissions_task,
    vmwareengine_projects_locations_private_clouds_undelete_builder, vmwareengine_projects_locations_private_clouds_undelete_task,
    vmwareengine_projects_locations_private_clouds_update_dns_forwarding_builder, vmwareengine_projects_locations_private_clouds_update_dns_forwarding_task,
    vmwareengine_projects_locations_private_clouds_clusters_create_builder, vmwareengine_projects_locations_private_clouds_clusters_create_task,
    vmwareengine_projects_locations_private_clouds_clusters_delete_builder, vmwareengine_projects_locations_private_clouds_clusters_delete_task,
    vmwareengine_projects_locations_private_clouds_clusters_get_builder, vmwareengine_projects_locations_private_clouds_clusters_get_task,
    vmwareengine_projects_locations_private_clouds_clusters_get_iam_policy_builder, vmwareengine_projects_locations_private_clouds_clusters_get_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_clusters_list_builder, vmwareengine_projects_locations_private_clouds_clusters_list_task,
    vmwareengine_projects_locations_private_clouds_clusters_mount_datastore_builder, vmwareengine_projects_locations_private_clouds_clusters_mount_datastore_task,
    vmwareengine_projects_locations_private_clouds_clusters_patch_builder, vmwareengine_projects_locations_private_clouds_clusters_patch_task,
    vmwareengine_projects_locations_private_clouds_clusters_set_iam_policy_builder, vmwareengine_projects_locations_private_clouds_clusters_set_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_clusters_test_iam_permissions_builder, vmwareengine_projects_locations_private_clouds_clusters_test_iam_permissions_task,
    vmwareengine_projects_locations_private_clouds_clusters_unmount_datastore_builder, vmwareengine_projects_locations_private_clouds_clusters_unmount_datastore_task,
    vmwareengine_projects_locations_private_clouds_clusters_nodes_get_builder, vmwareengine_projects_locations_private_clouds_clusters_nodes_get_task,
    vmwareengine_projects_locations_private_clouds_clusters_nodes_list_builder, vmwareengine_projects_locations_private_clouds_clusters_nodes_list_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_create_builder, vmwareengine_projects_locations_private_clouds_external_addresses_create_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_delete_builder, vmwareengine_projects_locations_private_clouds_external_addresses_delete_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_get_builder, vmwareengine_projects_locations_private_clouds_external_addresses_get_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_list_builder, vmwareengine_projects_locations_private_clouds_external_addresses_list_task,
    vmwareengine_projects_locations_private_clouds_external_addresses_patch_builder, vmwareengine_projects_locations_private_clouds_external_addresses_patch_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_create_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_create_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_iam_policy_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_list_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_list_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_set_iam_policy_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_set_iam_policy_task,
    vmwareengine_projects_locations_private_clouds_hcx_activation_keys_test_iam_permissions_builder, vmwareengine_projects_locations_private_clouds_hcx_activation_keys_test_iam_permissions_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_create_builder, vmwareengine_projects_locations_private_clouds_logging_servers_create_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_delete_builder, vmwareengine_projects_locations_private_clouds_logging_servers_delete_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_get_builder, vmwareengine_projects_locations_private_clouds_logging_servers_get_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_list_builder, vmwareengine_projects_locations_private_clouds_logging_servers_list_task,
    vmwareengine_projects_locations_private_clouds_logging_servers_patch_builder, vmwareengine_projects_locations_private_clouds_logging_servers_patch_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_create_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_create_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_delete_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_delete_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_get_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_get_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_list_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_list_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_patch_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_patch_task,
    vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_repair_builder, vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_repair_task,
    vmwareengine_projects_locations_private_clouds_subnets_get_builder, vmwareengine_projects_locations_private_clouds_subnets_get_task,
    vmwareengine_projects_locations_private_clouds_subnets_list_builder, vmwareengine_projects_locations_private_clouds_subnets_list_task,
    vmwareengine_projects_locations_private_clouds_subnets_patch_builder, vmwareengine_projects_locations_private_clouds_subnets_patch_task,
    vmwareengine_projects_locations_private_clouds_upgrades_get_builder, vmwareengine_projects_locations_private_clouds_upgrades_get_task,
    vmwareengine_projects_locations_private_clouds_upgrades_list_builder, vmwareengine_projects_locations_private_clouds_upgrades_list_task,
    vmwareengine_projects_locations_private_clouds_upgrades_patch_builder, vmwareengine_projects_locations_private_clouds_upgrades_patch_task,
    vmwareengine_projects_locations_private_connections_create_builder, vmwareengine_projects_locations_private_connections_create_task,
    vmwareengine_projects_locations_private_connections_delete_builder, vmwareengine_projects_locations_private_connections_delete_task,
    vmwareengine_projects_locations_private_connections_get_builder, vmwareengine_projects_locations_private_connections_get_task,
    vmwareengine_projects_locations_private_connections_list_builder, vmwareengine_projects_locations_private_connections_list_task,
    vmwareengine_projects_locations_private_connections_patch_builder, vmwareengine_projects_locations_private_connections_patch_task,
    vmwareengine_projects_locations_private_connections_peering_routes_list_builder, vmwareengine_projects_locations_private_connections_peering_routes_list_task,
    vmwareengine_projects_locations_vmware_engine_networks_create_builder, vmwareengine_projects_locations_vmware_engine_networks_create_task,
    vmwareengine_projects_locations_vmware_engine_networks_delete_builder, vmwareengine_projects_locations_vmware_engine_networks_delete_task,
    vmwareengine_projects_locations_vmware_engine_networks_get_builder, vmwareengine_projects_locations_vmware_engine_networks_get_task,
    vmwareengine_projects_locations_vmware_engine_networks_list_builder, vmwareengine_projects_locations_vmware_engine_networks_list_task,
    vmwareengine_projects_locations_vmware_engine_networks_patch_builder, vmwareengine_projects_locations_vmware_engine_networks_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::vmwareengine::Announcement;
use crate::providers::gcp::clients::vmwareengine::Cluster;
use crate::providers::gcp::clients::vmwareengine::Credentials;
use crate::providers::gcp::clients::vmwareengine::Datastore;
use crate::providers::gcp::clients::vmwareengine::DnsBindPermission;
use crate::providers::gcp::clients::vmwareengine::DnsForwarding;
use crate::providers::gcp::clients::vmwareengine::Empty;
use crate::providers::gcp::clients::vmwareengine::ExternalAccessRule;
use crate::providers::gcp::clients::vmwareengine::ExternalAddress;
use crate::providers::gcp::clients::vmwareengine::FetchNetworkPolicyExternalAddressesResponse;
use crate::providers::gcp::clients::vmwareengine::HcxActivationKey;
use crate::providers::gcp::clients::vmwareengine::ListAnnouncementsResponse;
use crate::providers::gcp::clients::vmwareengine::ListClustersResponse;
use crate::providers::gcp::clients::vmwareengine::ListDatastoresResponse;
use crate::providers::gcp::clients::vmwareengine::ListExternalAccessRulesResponse;
use crate::providers::gcp::clients::vmwareengine::ListExternalAddressesResponse;
use crate::providers::gcp::clients::vmwareengine::ListHcxActivationKeysResponse;
use crate::providers::gcp::clients::vmwareengine::ListLocationsResponse;
use crate::providers::gcp::clients::vmwareengine::ListLoggingServersResponse;
use crate::providers::gcp::clients::vmwareengine::ListManagementDnsZoneBindingsResponse;
use crate::providers::gcp::clients::vmwareengine::ListNetworkPeeringsResponse;
use crate::providers::gcp::clients::vmwareengine::ListNetworkPoliciesResponse;
use crate::providers::gcp::clients::vmwareengine::ListNodeTypesResponse;
use crate::providers::gcp::clients::vmwareengine::ListNodesResponse;
use crate::providers::gcp::clients::vmwareengine::ListOperationsResponse;
use crate::providers::gcp::clients::vmwareengine::ListPeeringRoutesResponse;
use crate::providers::gcp::clients::vmwareengine::ListPrivateCloudsResponse;
use crate::providers::gcp::clients::vmwareengine::ListPrivateConnectionPeeringRoutesResponse;
use crate::providers::gcp::clients::vmwareengine::ListPrivateConnectionsResponse;
use crate::providers::gcp::clients::vmwareengine::ListSubnetsResponse;
use crate::providers::gcp::clients::vmwareengine::ListUpgradesResponse;
use crate::providers::gcp::clients::vmwareengine::ListVmwareEngineNetworksResponse;
use crate::providers::gcp::clients::vmwareengine::Location;
use crate::providers::gcp::clients::vmwareengine::LoggingServer;
use crate::providers::gcp::clients::vmwareengine::ManagementDnsZoneBinding;
use crate::providers::gcp::clients::vmwareengine::NetworkPeering;
use crate::providers::gcp::clients::vmwareengine::NetworkPolicy;
use crate::providers::gcp::clients::vmwareengine::Node;
use crate::providers::gcp::clients::vmwareengine::NodeType;
use crate::providers::gcp::clients::vmwareengine::Operation;
use crate::providers::gcp::clients::vmwareengine::Policy;
use crate::providers::gcp::clients::vmwareengine::PrivateCloud;
use crate::providers::gcp::clients::vmwareengine::PrivateConnection;
use crate::providers::gcp::clients::vmwareengine::Subnet;
use crate::providers::gcp::clients::vmwareengine::TestIamPermissionsResponse;
use crate::providers::gcp::clients::vmwareengine::Upgrade;
use crate::providers::gcp::clients::vmwareengine::VmwareEngineNetwork;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsAnnouncementsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsAnnouncementsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDatastoresPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDnsBindPermissionGrantArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsDnsBindPermissionRevokeArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsGetDnsBindPermissionArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPeeringsPeeringRoutesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesFetchExternalAddressesArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNetworkPoliciesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNodeTypesGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsNodeTypesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersGetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersMountDatastoreArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersNodesGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersNodesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsClustersUnmountDatastoreArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsExternalAddressesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsGetDnsForwardingArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsGetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysGetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysSetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysTestIamPermissionsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsLoggingServersPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsRepairArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsPrivateCloudDeletionNowArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsResetNsxCredentialsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsResetVcenterCredentialsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsSetIamPolicyArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsShowNsxCredentialsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsShowVcenterCredentialsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsSubnetsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsSubnetsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsSubnetsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsTestIamPermissionsArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUndeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUpdateDnsForwardingArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUpgradesGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUpgradesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateCloudsUpgradesPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsPatchArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsPrivateConnectionsPeeringRoutesListArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsVmwareEngineNetworksCreateArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsVmwareEngineNetworksDeleteArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsVmwareEngineNetworksGetArgs;
use crate::providers::gcp::clients::vmwareengine::VmwareengineProjectsLocationsVmwareEngineNetworksListArgs;
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

    /// Vmwareengine projects locations get.
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
    pub fn vmwareengine_projects_locations_get(
        &self,
        args: &VmwareengineProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations get dns bind permission.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DnsBindPermission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_get_dns_bind_permission(
        &self,
        args: &VmwareengineProjectsLocationsGetDnsBindPermissionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DnsBindPermission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_get_dns_bind_permission_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_get_dns_bind_permission_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations list.
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
    pub fn vmwareengine_projects_locations_list(
        &self,
        args: &VmwareengineProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations announcements get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Announcement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_announcements_get(
        &self,
        args: &VmwareengineProjectsLocationsAnnouncementsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Announcement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_announcements_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_announcements_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations announcements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAnnouncementsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_announcements_list(
        &self,
        args: &VmwareengineProjectsLocationsAnnouncementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAnnouncementsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_announcements_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_announcements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations datastores get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Datastore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_datastores_get(
        &self,
        args: &VmwareengineProjectsLocationsDatastoresGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Datastore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_datastores_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_datastores_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations datastores list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatastoresResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_datastores_list(
        &self,
        args: &VmwareengineProjectsLocationsDatastoresListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatastoresResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_datastores_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_datastores_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations network peerings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NetworkPeering result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_network_peerings_get(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPeeringsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NetworkPeering, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_peerings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_peerings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network peerings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNetworkPeeringsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_network_peerings_list(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPeeringsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNetworkPeeringsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_peerings_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_peerings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations network peerings peering routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPeeringRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_network_peerings_peering_routes_list(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPeeringsPeeringRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPeeringRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_peerings_peering_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_peerings_peering_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations network policies fetch external addresses.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchNetworkPolicyExternalAddressesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmwareengine_projects_locations_network_policies_fetch_external_addresses(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesFetchExternalAddressesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchNetworkPolicyExternalAddressesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_fetch_external_addresses_builder(
            &self.http_client,
            &args.networkPolicy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_fetch_external_addresses_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NetworkPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_network_policies_get(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NetworkPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNetworkPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_network_policies_list(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNetworkPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations network policies external access rules get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExternalAccessRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_network_policies_external_access_rules_get(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAccessRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_external_access_rules_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_external_access_rules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations network policies external access rules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExternalAccessRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_network_policies_external_access_rules_list(
        &self,
        args: &VmwareengineProjectsLocationsNetworkPoliciesExternalAccessRulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExternalAccessRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_network_policies_external_access_rules_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_network_policies_external_access_rules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations node types get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NodeType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_node_types_get(
        &self,
        args: &VmwareengineProjectsLocationsNodeTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NodeType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_node_types_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_node_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations node types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNodeTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_node_types_list(
        &self,
        args: &VmwareengineProjectsLocationsNodeTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNodeTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_node_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_node_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations operations get.
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
    pub fn vmwareengine_projects_locations_operations_get(
        &self,
        args: &VmwareengineProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations operations list.
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
    pub fn vmwareengine_projects_locations_operations_list(
        &self,
        args: &VmwareengineProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PrivateCloud result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PrivateCloud, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds get dns forwarding.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DnsForwarding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_get_dns_forwarding(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsGetDnsForwardingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DnsForwarding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_get_dns_forwarding_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_get_dns_forwarding_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds get iam policy.
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
    pub fn vmwareengine_projects_locations_private_clouds_get_iam_policy(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPrivateCloudsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPrivateCloudsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds show nsx credentials.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Credentials result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_show_nsx_credentials(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsShowNsxCredentialsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Credentials, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_show_nsx_credentials_builder(
            &self.http_client,
            &args.privateCloud,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_show_nsx_credentials_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds show vcenter credentials.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Credentials result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_show_vcenter_credentials(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsShowVcenterCredentialsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Credentials, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_show_vcenter_credentials_builder(
            &self.http_client,
            &args.privateCloud,
            &args.username,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_show_vcenter_credentials_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds test iam permissions.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds clusters get.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters get iam policy.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_get_iam_policy(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters list.
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
    pub fn vmwareengine_projects_locations_private_clouds_clusters_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds clusters nodes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Node result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_clusters_nodes_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Node, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds clusters nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_clusters_nodes_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsClustersNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_clusters_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_clusters_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds external addresses get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExternalAddress result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmwareengine_projects_locations_private_clouds_external_addresses_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsExternalAddressesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExternalAddress, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_external_addresses_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_external_addresses_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds external addresses list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExternalAddressesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmwareengine_projects_locations_private_clouds_external_addresses_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsExternalAddressesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExternalAddressesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_external_addresses_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_external_addresses_list_task(builder)
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

    /// Vmwareengine projects locations private clouds hcx activation keys get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HcxActivationKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HcxActivationKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds hcx activation keys get iam policy.
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
    pub fn vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_iam_policy(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds hcx activation keys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHcxActivationKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_hcx_activation_keys_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsHcxActivationKeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHcxActivationKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_hcx_activation_keys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds logging servers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoggingServer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_logging_servers_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsLoggingServersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoggingServer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_logging_servers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_logging_servers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds logging servers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLoggingServersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_logging_servers_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsLoggingServersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLoggingServersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_logging_servers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_logging_servers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds management dns zone bindings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagementDnsZoneBinding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagementDnsZoneBinding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds management dns zone bindings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListManagementDnsZoneBindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsManagementDnsZoneBindingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListManagementDnsZoneBindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_management_dns_zone_bindings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds subnets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subnet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_subnets_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsSubnetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subnet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_subnets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_subnets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds subnets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSubnetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_subnets_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsSubnetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSubnetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_subnets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_subnets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private clouds upgrades get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Upgrade result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_upgrades_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsUpgradesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Upgrade, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_upgrades_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_upgrades_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private clouds upgrades list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUpgradesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_clouds_upgrades_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateCloudsUpgradesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUpgradesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_clouds_upgrades_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_clouds_upgrades_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private connections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PrivateConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_connections_get(
        &self,
        args: &VmwareengineProjectsLocationsPrivateConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PrivateConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations private connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPrivateConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_connections_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPrivateConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations private connections peering routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPrivateConnectionPeeringRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_private_connections_peering_routes_list(
        &self,
        args: &VmwareengineProjectsLocationsPrivateConnectionsPeeringRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPrivateConnectionPeeringRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_private_connections_peering_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_private_connections_peering_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Vmwareengine projects locations vmware engine networks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VmwareEngineNetwork result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_vmware_engine_networks_get(
        &self,
        args: &VmwareengineProjectsLocationsVmwareEngineNetworksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VmwareEngineNetwork, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_vmware_engine_networks_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_vmware_engine_networks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmwareengine projects locations vmware engine networks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVmwareEngineNetworksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmwareengine_projects_locations_vmware_engine_networks_list(
        &self,
        args: &VmwareengineProjectsLocationsVmwareEngineNetworksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVmwareEngineNetworksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmwareengine_projects_locations_vmware_engine_networks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmwareengine_projects_locations_vmware_engine_networks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
