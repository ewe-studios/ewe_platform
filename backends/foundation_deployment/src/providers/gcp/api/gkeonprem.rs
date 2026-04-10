//! GkeonpremProvider - State-aware gkeonprem API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       gkeonprem API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::gkeonprem::{
    gkeonprem_projects_locations_get_builder, gkeonprem_projects_locations_get_task,
    gkeonprem_projects_locations_list_builder, gkeonprem_projects_locations_list_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_create_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_create_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_get_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_get_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_get_iam_policy_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_get_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_list_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_list_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_patch_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_patch_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_operations_get_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_operations_get_task,
    gkeonprem_projects_locations_bare_metal_admin_clusters_operations_list_builder, gkeonprem_projects_locations_bare_metal_admin_clusters_operations_list_task,
    gkeonprem_projects_locations_bare_metal_clusters_create_builder, gkeonprem_projects_locations_bare_metal_clusters_create_task,
    gkeonprem_projects_locations_bare_metal_clusters_delete_builder, gkeonprem_projects_locations_bare_metal_clusters_delete_task,
    gkeonprem_projects_locations_bare_metal_clusters_enroll_builder, gkeonprem_projects_locations_bare_metal_clusters_enroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_get_builder, gkeonprem_projects_locations_bare_metal_clusters_get_task,
    gkeonprem_projects_locations_bare_metal_clusters_get_iam_policy_builder, gkeonprem_projects_locations_bare_metal_clusters_get_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_clusters_list_builder, gkeonprem_projects_locations_bare_metal_clusters_list_task,
    gkeonprem_projects_locations_bare_metal_clusters_patch_builder, gkeonprem_projects_locations_bare_metal_clusters_patch_task,
    gkeonprem_projects_locations_bare_metal_clusters_query_version_config_builder, gkeonprem_projects_locations_bare_metal_clusters_query_version_config_task,
    gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_builder, gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_bare_metal_clusters_unenroll_builder, gkeonprem_projects_locations_bare_metal_clusters_unenroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_iam_policy_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_list_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_list_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_get_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_get_task,
    gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_list_builder, gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_list_task,
    gkeonprem_projects_locations_bare_metal_clusters_operations_get_builder, gkeonprem_projects_locations_bare_metal_clusters_operations_get_task,
    gkeonprem_projects_locations_bare_metal_clusters_operations_list_builder, gkeonprem_projects_locations_bare_metal_clusters_operations_list_task,
    gkeonprem_projects_locations_operations_cancel_builder, gkeonprem_projects_locations_operations_cancel_task,
    gkeonprem_projects_locations_operations_delete_builder, gkeonprem_projects_locations_operations_delete_task,
    gkeonprem_projects_locations_operations_get_builder, gkeonprem_projects_locations_operations_get_task,
    gkeonprem_projects_locations_operations_list_builder, gkeonprem_projects_locations_operations_list_task,
    gkeonprem_projects_locations_vmware_admin_clusters_create_builder, gkeonprem_projects_locations_vmware_admin_clusters_create_task,
    gkeonprem_projects_locations_vmware_admin_clusters_enroll_builder, gkeonprem_projects_locations_vmware_admin_clusters_enroll_task,
    gkeonprem_projects_locations_vmware_admin_clusters_get_builder, gkeonprem_projects_locations_vmware_admin_clusters_get_task,
    gkeonprem_projects_locations_vmware_admin_clusters_get_iam_policy_builder, gkeonprem_projects_locations_vmware_admin_clusters_get_iam_policy_task,
    gkeonprem_projects_locations_vmware_admin_clusters_list_builder, gkeonprem_projects_locations_vmware_admin_clusters_list_task,
    gkeonprem_projects_locations_vmware_admin_clusters_patch_builder, gkeonprem_projects_locations_vmware_admin_clusters_patch_task,
    gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_builder, gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_vmware_admin_clusters_unenroll_builder, gkeonprem_projects_locations_vmware_admin_clusters_unenroll_task,
    gkeonprem_projects_locations_vmware_admin_clusters_operations_get_builder, gkeonprem_projects_locations_vmware_admin_clusters_operations_get_task,
    gkeonprem_projects_locations_vmware_admin_clusters_operations_list_builder, gkeonprem_projects_locations_vmware_admin_clusters_operations_list_task,
    gkeonprem_projects_locations_vmware_clusters_create_builder, gkeonprem_projects_locations_vmware_clusters_create_task,
    gkeonprem_projects_locations_vmware_clusters_delete_builder, gkeonprem_projects_locations_vmware_clusters_delete_task,
    gkeonprem_projects_locations_vmware_clusters_enroll_builder, gkeonprem_projects_locations_vmware_clusters_enroll_task,
    gkeonprem_projects_locations_vmware_clusters_get_builder, gkeonprem_projects_locations_vmware_clusters_get_task,
    gkeonprem_projects_locations_vmware_clusters_get_iam_policy_builder, gkeonprem_projects_locations_vmware_clusters_get_iam_policy_task,
    gkeonprem_projects_locations_vmware_clusters_list_builder, gkeonprem_projects_locations_vmware_clusters_list_task,
    gkeonprem_projects_locations_vmware_clusters_patch_builder, gkeonprem_projects_locations_vmware_clusters_patch_task,
    gkeonprem_projects_locations_vmware_clusters_query_version_config_builder, gkeonprem_projects_locations_vmware_clusters_query_version_config_task,
    gkeonprem_projects_locations_vmware_clusters_set_iam_policy_builder, gkeonprem_projects_locations_vmware_clusters_set_iam_policy_task,
    gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_builder, gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_task,
    gkeonprem_projects_locations_vmware_clusters_unenroll_builder, gkeonprem_projects_locations_vmware_clusters_unenroll_task,
    gkeonprem_projects_locations_vmware_clusters_operations_get_builder, gkeonprem_projects_locations_vmware_clusters_operations_get_task,
    gkeonprem_projects_locations_vmware_clusters_operations_list_builder, gkeonprem_projects_locations_vmware_clusters_operations_list_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_iam_policy_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_iam_policy_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_list_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_list_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_get_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_get_task,
    gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_list_builder, gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gkeonprem::BareMetalAdminCluster;
use crate::providers::gcp::clients::gkeonprem::BareMetalCluster;
use crate::providers::gcp::clients::gkeonprem::BareMetalNodePool;
use crate::providers::gcp::clients::gkeonprem::Empty;
use crate::providers::gcp::clients::gkeonprem::ListBareMetalAdminClustersResponse;
use crate::providers::gcp::clients::gkeonprem::ListBareMetalClustersResponse;
use crate::providers::gcp::clients::gkeonprem::ListBareMetalNodePoolsResponse;
use crate::providers::gcp::clients::gkeonprem::ListLocationsResponse;
use crate::providers::gcp::clients::gkeonprem::ListOperationsResponse;
use crate::providers::gcp::clients::gkeonprem::ListVmwareAdminClustersResponse;
use crate::providers::gcp::clients::gkeonprem::ListVmwareClustersResponse;
use crate::providers::gcp::clients::gkeonprem::ListVmwareNodePoolsResponse;
use crate::providers::gcp::clients::gkeonprem::Location;
use crate::providers::gcp::clients::gkeonprem::Operation;
use crate::providers::gcp::clients::gkeonprem::Policy;
use crate::providers::gcp::clients::gkeonprem::QueryBareMetalAdminVersionConfigResponse;
use crate::providers::gcp::clients::gkeonprem::QueryBareMetalVersionConfigResponse;
use crate::providers::gcp::clients::gkeonprem::QueryVmwareVersionConfigResponse;
use crate::providers::gcp::clients::gkeonprem::TestIamPermissionsResponse;
use crate::providers::gcp::clients::gkeonprem::VmwareAdminCluster;
use crate::providers::gcp::clients::gkeonprem::VmwareCluster;
use crate::providers::gcp::clients::gkeonprem::VmwareNodePool;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersGetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersOperationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersOperationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersQueryVersionConfigArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalAdminClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsGetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsOperationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsOperationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersGetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersOperationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersOperationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersQueryVersionConfigArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsBareMetalClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersGetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersOperationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersOperationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareAdminClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersGetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersOperationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersOperationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersQueryVersionConfigArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersUnenrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsCreateArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsDeleteArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsEnrollArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsGetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsOperationsGetArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsOperationsListArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsPatchArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkeonprem::GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsUnenrollArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GkeonpremProvider with automatic state tracking.
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
/// let provider = GkeonpremProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GkeonpremProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GkeonpremProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GkeonpremProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Gkeonprem projects locations get.
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
    pub fn gkeonprem_projects_locations_get(
        &self,
        args: &GkeonpremProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations list.
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
    pub fn gkeonprem_projects_locations_list(
        &self,
        args: &GkeonpremProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.bareMetalAdminClusterId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters enroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BareMetalAdminCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_get(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BareMetalAdminCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_get_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters get iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_get_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBareMetalAdminClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_list(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBareMetalAdminClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.allowMissing,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters query version config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryBareMetalAdminVersionConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersQueryVersionConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryBareMetalAdminVersionConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_builder(
            &self.http_client,
            &args.parent,
            &args.upgradeConfig.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_query_version_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters unenroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters operations get.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_operations_get(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal admin clusters operations list.
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
    pub fn gkeonprem_projects_locations_bare_metal_admin_clusters_operations_list(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalAdminClustersOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_admin_clusters_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_admin_clusters_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.bareMetalClusterId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_delete(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters enroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BareMetalCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_get(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BareMetalCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_get_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters get iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_get_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBareMetalClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_list(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBareMetalClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.allowMissing,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters query version config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryBareMetalVersionConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_query_version_config(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersQueryVersionConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryBareMetalVersionConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_query_version_config_builder(
            &self.http_client,
            &args.parent,
            &args.createConfig.adminClusterMembership,
            &args.createConfig.adminClusterName,
            &args.upgradeConfig.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_query_version_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters unenroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_builder(
            &self.http_client,
            &args.parent,
            &args.bareMetalNodePoolId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools enroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BareMetalNodePool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BareMetalNodePool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools get iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBareMetalNodePoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_list(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBareMetalNodePoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools set iam policy.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools test iam permissions.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools unenroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools operations get.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_get(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters bare metal node pools operations list.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_list(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersBareMetalNodePoolsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_bare_metal_node_pools_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters operations get.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_operations_get(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations bare metal clusters operations list.
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
    pub fn gkeonprem_projects_locations_bare_metal_clusters_operations_list(
        &self,
        args: &GkeonpremProjectsLocationsBareMetalClustersOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_bare_metal_clusters_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_bare_metal_clusters_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations operations cancel.
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
    pub fn gkeonprem_projects_locations_operations_cancel(
        &self,
        args: &GkeonpremProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations operations delete.
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
    pub fn gkeonprem_projects_locations_operations_delete(
        &self,
        args: &GkeonpremProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations operations get.
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
    pub fn gkeonprem_projects_locations_operations_get(
        &self,
        args: &GkeonpremProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations operations list.
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
    pub fn gkeonprem_projects_locations_operations_list(
        &self,
        args: &GkeonpremProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.skipValidations,
            &args.validateOnly,
            &args.vmwareAdminClusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters enroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VmwareAdminCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_get(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VmwareAdminCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_get_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters get iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_get_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVmwareAdminClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_list(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVmwareAdminClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.allowMissing,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.skipValidations,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters unenroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters operations get.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_operations_get(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware admin clusters operations list.
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
    pub fn gkeonprem_projects_locations_vmware_admin_clusters_operations_list(
        &self,
        args: &GkeonpremProjectsLocationsVmwareAdminClustersOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_admin_clusters_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_admin_clusters_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_create(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.allowPreflightFailure,
            &args.skipValidations,
            &args.validateOnly,
            &args.vmwareClusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_delete(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters enroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_enroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VmwareCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_get(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VmwareCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_get_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters get iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_get_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVmwareClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_list(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVmwareClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.allowMissing,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_patch(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.skipValidations,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters query version config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryVmwareVersionConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_query_version_config(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersQueryVersionConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryVmwareVersionConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_query_version_config_builder(
            &self.http_client,
            &args.parent,
            &args.createConfig.adminClusterMembership,
            &args.createConfig.adminClusterName,
            &args.upgradeConfig.clusterName,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_query_version_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters set iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters test iam permissions.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters unenroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters operations get.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_operations_get(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters operations list.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_operations_list(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_builder(
            &self.http_client,
            &args.parent,
            &args.validateOnly,
            &args.vmwareNodePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.ignoreErrors,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools enroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsEnrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_enroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VmwareNodePool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VmwareNodePool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools get iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVmwareNodePoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_list(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVmwareNodePoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools set iam policy.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools test iam permissions.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools unenroll.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsUnenrollArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_unenroll_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools operations get.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_get(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkeonprem projects locations vmware clusters vmware node pools operations list.
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
    pub fn gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_list(
        &self,
        args: &GkeonpremProjectsLocationsVmwareClustersVmwareNodePoolsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkeonprem_projects_locations_vmware_clusters_vmware_node_pools_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
