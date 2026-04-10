//! NetworkconnectivityProvider - State-aware networkconnectivity API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       networkconnectivity API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::networkconnectivity::{
    networkconnectivity_projects_locations_check_consumer_config_builder, networkconnectivity_projects_locations_check_consumer_config_task,
    networkconnectivity_projects_locations_get_builder, networkconnectivity_projects_locations_get_task,
    networkconnectivity_projects_locations_list_builder, networkconnectivity_projects_locations_list_task,
    networkconnectivity_projects_locations_automated_dns_records_create_builder, networkconnectivity_projects_locations_automated_dns_records_create_task,
    networkconnectivity_projects_locations_automated_dns_records_delete_builder, networkconnectivity_projects_locations_automated_dns_records_delete_task,
    networkconnectivity_projects_locations_automated_dns_records_get_builder, networkconnectivity_projects_locations_automated_dns_records_get_task,
    networkconnectivity_projects_locations_automated_dns_records_list_builder, networkconnectivity_projects_locations_automated_dns_records_list_task,
    networkconnectivity_projects_locations_global_hubs_accept_spoke_builder, networkconnectivity_projects_locations_global_hubs_accept_spoke_task,
    networkconnectivity_projects_locations_global_hubs_accept_spoke_update_builder, networkconnectivity_projects_locations_global_hubs_accept_spoke_update_task,
    networkconnectivity_projects_locations_global_hubs_create_builder, networkconnectivity_projects_locations_global_hubs_create_task,
    networkconnectivity_projects_locations_global_hubs_delete_builder, networkconnectivity_projects_locations_global_hubs_delete_task,
    networkconnectivity_projects_locations_global_hubs_get_builder, networkconnectivity_projects_locations_global_hubs_get_task,
    networkconnectivity_projects_locations_global_hubs_get_iam_policy_builder, networkconnectivity_projects_locations_global_hubs_get_iam_policy_task,
    networkconnectivity_projects_locations_global_hubs_list_builder, networkconnectivity_projects_locations_global_hubs_list_task,
    networkconnectivity_projects_locations_global_hubs_list_spokes_builder, networkconnectivity_projects_locations_global_hubs_list_spokes_task,
    networkconnectivity_projects_locations_global_hubs_patch_builder, networkconnectivity_projects_locations_global_hubs_patch_task,
    networkconnectivity_projects_locations_global_hubs_query_status_builder, networkconnectivity_projects_locations_global_hubs_query_status_task,
    networkconnectivity_projects_locations_global_hubs_reject_spoke_builder, networkconnectivity_projects_locations_global_hubs_reject_spoke_task,
    networkconnectivity_projects_locations_global_hubs_reject_spoke_update_builder, networkconnectivity_projects_locations_global_hubs_reject_spoke_update_task,
    networkconnectivity_projects_locations_global_hubs_set_iam_policy_builder, networkconnectivity_projects_locations_global_hubs_set_iam_policy_task,
    networkconnectivity_projects_locations_global_hubs_test_iam_permissions_builder, networkconnectivity_projects_locations_global_hubs_test_iam_permissions_task,
    networkconnectivity_projects_locations_global_hubs_groups_get_builder, networkconnectivity_projects_locations_global_hubs_groups_get_task,
    networkconnectivity_projects_locations_global_hubs_groups_get_iam_policy_builder, networkconnectivity_projects_locations_global_hubs_groups_get_iam_policy_task,
    networkconnectivity_projects_locations_global_hubs_groups_list_builder, networkconnectivity_projects_locations_global_hubs_groups_list_task,
    networkconnectivity_projects_locations_global_hubs_groups_patch_builder, networkconnectivity_projects_locations_global_hubs_groups_patch_task,
    networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_builder, networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_task,
    networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_builder, networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_task,
    networkconnectivity_projects_locations_global_hubs_route_tables_get_builder, networkconnectivity_projects_locations_global_hubs_route_tables_get_task,
    networkconnectivity_projects_locations_global_hubs_route_tables_list_builder, networkconnectivity_projects_locations_global_hubs_route_tables_list_task,
    networkconnectivity_projects_locations_global_hubs_route_tables_routes_get_builder, networkconnectivity_projects_locations_global_hubs_route_tables_routes_get_task,
    networkconnectivity_projects_locations_global_hubs_route_tables_routes_list_builder, networkconnectivity_projects_locations_global_hubs_route_tables_routes_list_task,
    networkconnectivity_projects_locations_global_policy_based_routes_create_builder, networkconnectivity_projects_locations_global_policy_based_routes_create_task,
    networkconnectivity_projects_locations_global_policy_based_routes_delete_builder, networkconnectivity_projects_locations_global_policy_based_routes_delete_task,
    networkconnectivity_projects_locations_global_policy_based_routes_get_builder, networkconnectivity_projects_locations_global_policy_based_routes_get_task,
    networkconnectivity_projects_locations_global_policy_based_routes_get_iam_policy_builder, networkconnectivity_projects_locations_global_policy_based_routes_get_iam_policy_task,
    networkconnectivity_projects_locations_global_policy_based_routes_list_builder, networkconnectivity_projects_locations_global_policy_based_routes_list_task,
    networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_builder, networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_task,
    networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_builder, networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_task,
    networkconnectivity_projects_locations_internal_ranges_create_builder, networkconnectivity_projects_locations_internal_ranges_create_task,
    networkconnectivity_projects_locations_internal_ranges_delete_builder, networkconnectivity_projects_locations_internal_ranges_delete_task,
    networkconnectivity_projects_locations_internal_ranges_get_builder, networkconnectivity_projects_locations_internal_ranges_get_task,
    networkconnectivity_projects_locations_internal_ranges_get_iam_policy_builder, networkconnectivity_projects_locations_internal_ranges_get_iam_policy_task,
    networkconnectivity_projects_locations_internal_ranges_list_builder, networkconnectivity_projects_locations_internal_ranges_list_task,
    networkconnectivity_projects_locations_internal_ranges_patch_builder, networkconnectivity_projects_locations_internal_ranges_patch_task,
    networkconnectivity_projects_locations_internal_ranges_set_iam_policy_builder, networkconnectivity_projects_locations_internal_ranges_set_iam_policy_task,
    networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_builder, networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_get_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_get_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_list_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_list_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_get_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_get_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_list_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_list_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_get_builder, networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_get_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_list_builder, networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_list_task,
    networkconnectivity_projects_locations_operations_cancel_builder, networkconnectivity_projects_locations_operations_cancel_task,
    networkconnectivity_projects_locations_operations_delete_builder, networkconnectivity_projects_locations_operations_delete_task,
    networkconnectivity_projects_locations_operations_get_builder, networkconnectivity_projects_locations_operations_get_task,
    networkconnectivity_projects_locations_operations_list_builder, networkconnectivity_projects_locations_operations_list_task,
    networkconnectivity_projects_locations_regional_endpoints_create_builder, networkconnectivity_projects_locations_regional_endpoints_create_task,
    networkconnectivity_projects_locations_regional_endpoints_delete_builder, networkconnectivity_projects_locations_regional_endpoints_delete_task,
    networkconnectivity_projects_locations_regional_endpoints_get_builder, networkconnectivity_projects_locations_regional_endpoints_get_task,
    networkconnectivity_projects_locations_regional_endpoints_list_builder, networkconnectivity_projects_locations_regional_endpoints_list_task,
    networkconnectivity_projects_locations_remote_transport_profiles_get_builder, networkconnectivity_projects_locations_remote_transport_profiles_get_task,
    networkconnectivity_projects_locations_remote_transport_profiles_list_builder, networkconnectivity_projects_locations_remote_transport_profiles_list_task,
    networkconnectivity_projects_locations_service_classes_delete_builder, networkconnectivity_projects_locations_service_classes_delete_task,
    networkconnectivity_projects_locations_service_classes_get_builder, networkconnectivity_projects_locations_service_classes_get_task,
    networkconnectivity_projects_locations_service_classes_list_builder, networkconnectivity_projects_locations_service_classes_list_task,
    networkconnectivity_projects_locations_service_classes_patch_builder, networkconnectivity_projects_locations_service_classes_patch_task,
    networkconnectivity_projects_locations_service_connection_maps_create_builder, networkconnectivity_projects_locations_service_connection_maps_create_task,
    networkconnectivity_projects_locations_service_connection_maps_delete_builder, networkconnectivity_projects_locations_service_connection_maps_delete_task,
    networkconnectivity_projects_locations_service_connection_maps_get_builder, networkconnectivity_projects_locations_service_connection_maps_get_task,
    networkconnectivity_projects_locations_service_connection_maps_list_builder, networkconnectivity_projects_locations_service_connection_maps_list_task,
    networkconnectivity_projects_locations_service_connection_maps_patch_builder, networkconnectivity_projects_locations_service_connection_maps_patch_task,
    networkconnectivity_projects_locations_service_connection_policies_create_builder, networkconnectivity_projects_locations_service_connection_policies_create_task,
    networkconnectivity_projects_locations_service_connection_policies_delete_builder, networkconnectivity_projects_locations_service_connection_policies_delete_task,
    networkconnectivity_projects_locations_service_connection_policies_get_builder, networkconnectivity_projects_locations_service_connection_policies_get_task,
    networkconnectivity_projects_locations_service_connection_policies_list_builder, networkconnectivity_projects_locations_service_connection_policies_list_task,
    networkconnectivity_projects_locations_service_connection_policies_patch_builder, networkconnectivity_projects_locations_service_connection_policies_patch_task,
    networkconnectivity_projects_locations_service_connection_tokens_create_builder, networkconnectivity_projects_locations_service_connection_tokens_create_task,
    networkconnectivity_projects_locations_service_connection_tokens_delete_builder, networkconnectivity_projects_locations_service_connection_tokens_delete_task,
    networkconnectivity_projects_locations_service_connection_tokens_get_builder, networkconnectivity_projects_locations_service_connection_tokens_get_task,
    networkconnectivity_projects_locations_service_connection_tokens_list_builder, networkconnectivity_projects_locations_service_connection_tokens_list_task,
    networkconnectivity_projects_locations_spokes_create_builder, networkconnectivity_projects_locations_spokes_create_task,
    networkconnectivity_projects_locations_spokes_delete_builder, networkconnectivity_projects_locations_spokes_delete_task,
    networkconnectivity_projects_locations_spokes_get_builder, networkconnectivity_projects_locations_spokes_get_task,
    networkconnectivity_projects_locations_spokes_get_iam_policy_builder, networkconnectivity_projects_locations_spokes_get_iam_policy_task,
    networkconnectivity_projects_locations_spokes_list_builder, networkconnectivity_projects_locations_spokes_list_task,
    networkconnectivity_projects_locations_spokes_patch_builder, networkconnectivity_projects_locations_spokes_patch_task,
    networkconnectivity_projects_locations_spokes_set_iam_policy_builder, networkconnectivity_projects_locations_spokes_set_iam_policy_task,
    networkconnectivity_projects_locations_spokes_test_iam_permissions_builder, networkconnectivity_projects_locations_spokes_test_iam_permissions_task,
    networkconnectivity_projects_locations_transports_create_builder, networkconnectivity_projects_locations_transports_create_task,
    networkconnectivity_projects_locations_transports_delete_builder, networkconnectivity_projects_locations_transports_delete_task,
    networkconnectivity_projects_locations_transports_get_builder, networkconnectivity_projects_locations_transports_get_task,
    networkconnectivity_projects_locations_transports_list_builder, networkconnectivity_projects_locations_transports_list_task,
    networkconnectivity_projects_locations_transports_patch_builder, networkconnectivity_projects_locations_transports_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::networkconnectivity::AutomatedDnsRecord;
use crate::providers::gcp::clients::networkconnectivity::CheckConsumerConfigResponse;
use crate::providers::gcp::clients::networkconnectivity::Destination;
use crate::providers::gcp::clients::networkconnectivity::Empty;
use crate::providers::gcp::clients::networkconnectivity::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::networkconnectivity::GoogleLongrunningOperation;
use crate::providers::gcp::clients::networkconnectivity::Group;
use crate::providers::gcp::clients::networkconnectivity::Hub;
use crate::providers::gcp::clients::networkconnectivity::InternalRange;
use crate::providers::gcp::clients::networkconnectivity::ListAutomatedDnsRecordsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListDestinationsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListGroupsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListHubSpokesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListHubsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListInternalRangesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListLocationsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListMulticloudDataTransferConfigsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListMulticloudDataTransferSupportedServicesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListPolicyBasedRoutesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListRegionalEndpointsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListRemoteTransportProfilesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListRouteTablesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListRoutesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListServiceClassesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListServiceConnectionMapsResponse;
use crate::providers::gcp::clients::networkconnectivity::ListServiceConnectionPoliciesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListServiceConnectionTokensResponse;
use crate::providers::gcp::clients::networkconnectivity::ListSpokesResponse;
use crate::providers::gcp::clients::networkconnectivity::ListTransportsResponse;
use crate::providers::gcp::clients::networkconnectivity::Location;
use crate::providers::gcp::clients::networkconnectivity::MulticloudDataTransferConfig;
use crate::providers::gcp::clients::networkconnectivity::MulticloudDataTransferSupportedService;
use crate::providers::gcp::clients::networkconnectivity::Policy;
use crate::providers::gcp::clients::networkconnectivity::PolicyBasedRoute;
use crate::providers::gcp::clients::networkconnectivity::QueryHubStatusResponse;
use crate::providers::gcp::clients::networkconnectivity::RegionalEndpoint;
use crate::providers::gcp::clients::networkconnectivity::RemoteTransportProfile;
use crate::providers::gcp::clients::networkconnectivity::Route;
use crate::providers::gcp::clients::networkconnectivity::RouteTable;
use crate::providers::gcp::clients::networkconnectivity::ServiceClass;
use crate::providers::gcp::clients::networkconnectivity::ServiceConnectionMap;
use crate::providers::gcp::clients::networkconnectivity::ServiceConnectionPolicy;
use crate::providers::gcp::clients::networkconnectivity::ServiceConnectionToken;
use crate::providers::gcp::clients::networkconnectivity::Spoke;
use crate::providers::gcp::clients::networkconnectivity::TestIamPermissionsResponse;
use crate::providers::gcp::clients::networkconnectivity::Transport;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsAutomatedDnsRecordsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsAutomatedDnsRecordsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsAutomatedDnsRecordsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsAutomatedDnsRecordsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsCheckConsumerConfigArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeUpdateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsGetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsListSpokesArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsQueryStatusArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeUpdateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesRoutesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesRoutesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesGetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesGetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferSupportedServicesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferSupportedServicesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRegionalEndpointsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRegionalEndpointsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRegionalEndpointsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRegionalEndpointsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRemoteTransportProfilesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRemoteTransportProfilesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceClassesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceClassesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceClassesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceClassesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionTokensCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionTokensDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionTokensGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionTokensListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesGetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsGetArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsListArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NetworkconnectivityProvider with automatic state tracking.
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
/// let provider = NetworkconnectivityProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct NetworkconnectivityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> NetworkconnectivityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new NetworkconnectivityProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Networkconnectivity projects locations check consumer config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckConsumerConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_check_consumer_config(
        &self,
        args: &NetworkconnectivityProjectsLocationsCheckConsumerConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckConsumerConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_check_consumer_config_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_check_consumer_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations get.
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
    pub fn networkconnectivity_projects_locations_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations list.
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
    pub fn networkconnectivity_projects_locations_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations automated dns records create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_automated_dns_records_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsAutomatedDnsRecordsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_automated_dns_records_create_builder(
            &self.http_client,
            &args.parent,
            &args.automatedDnsRecordId,
            &args.insertMode,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_automated_dns_records_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations automated dns records delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_automated_dns_records_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsAutomatedDnsRecordsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_automated_dns_records_delete_builder(
            &self.http_client,
            &args.name,
            &args.deleteMode,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_automated_dns_records_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations automated dns records get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutomatedDnsRecord result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_automated_dns_records_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsAutomatedDnsRecordsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutomatedDnsRecord, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_automated_dns_records_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_automated_dns_records_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations automated dns records list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutomatedDnsRecordsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_automated_dns_records_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsAutomatedDnsRecordsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutomatedDnsRecordsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_automated_dns_records_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_automated_dns_records_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs accept spoke.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_accept_spoke(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_accept_spoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_accept_spoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs accept spoke update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_accept_spoke_update(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_accept_spoke_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_accept_spoke_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_create_builder(
            &self.http_client,
            &args.parent,
            &args.hubId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Hub result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Hub, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs get iam policy.
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
    pub fn networkconnectivity_projects_locations_global_hubs_get_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHubsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHubsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs list spokes.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHubSpokesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_list_spokes(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsListSpokesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHubSpokesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_list_spokes_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.spokeLocations,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_list_spokes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs query status.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryHubStatusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_query_status(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsQueryStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryHubStatusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_query_status_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.groupBy,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_query_status_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs reject spoke.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_reject_spoke(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_reject_spoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_reject_spoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs reject spoke update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_reject_spoke_update(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_reject_spoke_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_reject_spoke_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs set iam policy.
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
    pub fn networkconnectivity_projects_locations_global_hubs_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs test iam permissions.
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
    pub fn networkconnectivity_projects_locations_global_hubs_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Group result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_groups_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups get iam policy.
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
    pub fn networkconnectivity_projects_locations_global_hubs_groups_get_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_groups_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_groups_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups set iam policy.
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
    pub fn networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups test iam permissions.
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
    pub fn networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs route tables get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RouteTable result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_route_tables_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RouteTable, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_route_tables_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_route_tables_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs route tables list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRouteTablesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_route_tables_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRouteTablesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_route_tables_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_route_tables_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs route tables routes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Route result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_route_tables_routes_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesRoutesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Route, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_route_tables_routes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_route_tables_routes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs route tables routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_hubs_route_tables_routes_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRouteTablesRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_route_tables_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_route_tables_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_create_builder(
            &self.http_client,
            &args.parent,
            &args.policyBasedRouteId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PolicyBasedRoute result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PolicyBasedRoute, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes get iam policy.
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
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_get_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPolicyBasedRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPolicyBasedRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes set iam policy.
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
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes test iam permissions.
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
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_internal_ranges_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_create_builder(
            &self.http_client,
            &args.parent,
            &args.internalRangeId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_internal_ranges_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InternalRange result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_internal_ranges_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InternalRange, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges get iam policy.
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
    pub fn networkconnectivity_projects_locations_internal_ranges_get_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInternalRangesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_internal_ranges_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInternalRangesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_internal_ranges_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges set iam policy.
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
    pub fn networkconnectivity_projects_locations_internal_ranges_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges test iam permissions.
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
    pub fn networkconnectivity_projects_locations_internal_ranges_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.multicloudDataTransferConfigId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MulticloudDataTransferConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MulticloudDataTransferConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMulticloudDataTransferConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMulticloudDataTransferConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_builder(
            &self.http_client,
            &args.parent,
            &args.destinationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Destination result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Destination, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDestinationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDestinationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer supported services get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MulticloudDataTransferSupportedService result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferSupportedServicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MulticloudDataTransferSupportedService, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer supported services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMulticloudDataTransferSupportedServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferSupportedServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMulticloudDataTransferSupportedServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_supported_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations operations cancel.
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
    pub fn networkconnectivity_projects_locations_operations_cancel(
        &self,
        args: &NetworkconnectivityProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations operations delete.
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
    pub fn networkconnectivity_projects_locations_operations_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_operations_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_operations_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations regional endpoints create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_regional_endpoints_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsRegionalEndpointsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_regional_endpoints_create_builder(
            &self.http_client,
            &args.parent,
            &args.regionalEndpointId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_regional_endpoints_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations regional endpoints delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_regional_endpoints_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsRegionalEndpointsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_regional_endpoints_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_regional_endpoints_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations regional endpoints get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RegionalEndpoint result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_regional_endpoints_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsRegionalEndpointsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RegionalEndpoint, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_regional_endpoints_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_regional_endpoints_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations regional endpoints list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRegionalEndpointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_regional_endpoints_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsRegionalEndpointsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRegionalEndpointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_regional_endpoints_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_regional_endpoints_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations remote transport profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RemoteTransportProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_remote_transport_profiles_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsRemoteTransportProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RemoteTransportProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_remote_transport_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_remote_transport_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations remote transport profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRemoteTransportProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_remote_transport_profiles_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsRemoteTransportProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRemoteTransportProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_remote_transport_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_remote_transport_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service classes delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_classes_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceClassesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_classes_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_classes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service classes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceClass result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_classes_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceClassesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceClass, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_classes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_classes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service classes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceClassesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_classes_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceClassesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceClassesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_classes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_classes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service classes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_classes_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceClassesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_classes_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_classes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_maps_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceConnectionMapId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_maps_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceConnectionMap result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_connection_maps_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceConnectionMap, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceConnectionMapsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_connection_maps_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceConnectionMapsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_maps_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_policies_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.autoSubnetworkConfig.allocRangeSpace,
            &args.autoSubnetworkConfig.ipStack,
            &args.autoSubnetworkConfig.prefixLength,
            &args.requestId,
            &args.serviceConnectionPolicyId,
            &args.subnetworkMode,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_policies_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceConnectionPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_connection_policies_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceConnectionPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceConnectionPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_connection_policies_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceConnectionPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_policies_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection tokens create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_tokens_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionTokensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_tokens_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceConnectionTokenId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_tokens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection tokens delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_service_connection_tokens_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionTokensDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_tokens_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_tokens_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection tokens get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceConnectionToken result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_connection_tokens_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionTokensGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceConnectionToken, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_tokens_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_tokens_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection tokens list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServiceConnectionTokensResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_service_connection_tokens_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionTokensListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServiceConnectionTokensResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_tokens_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_tokens_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_spokes_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.spokeId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_spokes_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Spoke result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_spokes_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Spoke, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes get iam policy.
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
    pub fn networkconnectivity_projects_locations_spokes_get_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSpokesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_spokes_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSpokesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_spokes_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes set iam policy.
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
    pub fn networkconnectivity_projects_locations_spokes_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes test iam permissions.
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
    pub fn networkconnectivity_projects_locations_spokes_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_transports_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.transportId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_transports_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_transports_get(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTransportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkconnectivity_projects_locations_transports_list(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTransportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_transports_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
