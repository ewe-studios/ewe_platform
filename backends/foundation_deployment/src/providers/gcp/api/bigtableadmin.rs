//! BigtableadminProvider - State-aware bigtableadmin API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       bigtableadmin API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::bigtableadmin::{
    bigtableadmin_operations_get_builder, bigtableadmin_operations_get_task,
    bigtableadmin_operations_projects_operations_list_builder, bigtableadmin_operations_projects_operations_list_task,
    bigtableadmin_projects_instances_create_builder, bigtableadmin_projects_instances_create_task,
    bigtableadmin_projects_instances_delete_builder, bigtableadmin_projects_instances_delete_task,
    bigtableadmin_projects_instances_get_builder, bigtableadmin_projects_instances_get_task,
    bigtableadmin_projects_instances_get_iam_policy_builder, bigtableadmin_projects_instances_get_iam_policy_task,
    bigtableadmin_projects_instances_list_builder, bigtableadmin_projects_instances_list_task,
    bigtableadmin_projects_instances_partial_update_instance_builder, bigtableadmin_projects_instances_partial_update_instance_task,
    bigtableadmin_projects_instances_set_iam_policy_builder, bigtableadmin_projects_instances_set_iam_policy_task,
    bigtableadmin_projects_instances_test_iam_permissions_builder, bigtableadmin_projects_instances_test_iam_permissions_task,
    bigtableadmin_projects_instances_update_builder, bigtableadmin_projects_instances_update_task,
    bigtableadmin_projects_instances_app_profiles_create_builder, bigtableadmin_projects_instances_app_profiles_create_task,
    bigtableadmin_projects_instances_app_profiles_delete_builder, bigtableadmin_projects_instances_app_profiles_delete_task,
    bigtableadmin_projects_instances_app_profiles_get_builder, bigtableadmin_projects_instances_app_profiles_get_task,
    bigtableadmin_projects_instances_app_profiles_list_builder, bigtableadmin_projects_instances_app_profiles_list_task,
    bigtableadmin_projects_instances_app_profiles_patch_builder, bigtableadmin_projects_instances_app_profiles_patch_task,
    bigtableadmin_projects_instances_clusters_create_builder, bigtableadmin_projects_instances_clusters_create_task,
    bigtableadmin_projects_instances_clusters_delete_builder, bigtableadmin_projects_instances_clusters_delete_task,
    bigtableadmin_projects_instances_clusters_get_builder, bigtableadmin_projects_instances_clusters_get_task,
    bigtableadmin_projects_instances_clusters_get_memory_layer_builder, bigtableadmin_projects_instances_clusters_get_memory_layer_task,
    bigtableadmin_projects_instances_clusters_list_builder, bigtableadmin_projects_instances_clusters_list_task,
    bigtableadmin_projects_instances_clusters_partial_update_cluster_builder, bigtableadmin_projects_instances_clusters_partial_update_cluster_task,
    bigtableadmin_projects_instances_clusters_update_builder, bigtableadmin_projects_instances_clusters_update_task,
    bigtableadmin_projects_instances_clusters_update_memory_layer_builder, bigtableadmin_projects_instances_clusters_update_memory_layer_task,
    bigtableadmin_projects_instances_clusters_backups_copy_builder, bigtableadmin_projects_instances_clusters_backups_copy_task,
    bigtableadmin_projects_instances_clusters_backups_create_builder, bigtableadmin_projects_instances_clusters_backups_create_task,
    bigtableadmin_projects_instances_clusters_backups_delete_builder, bigtableadmin_projects_instances_clusters_backups_delete_task,
    bigtableadmin_projects_instances_clusters_backups_get_builder, bigtableadmin_projects_instances_clusters_backups_get_task,
    bigtableadmin_projects_instances_clusters_backups_get_iam_policy_builder, bigtableadmin_projects_instances_clusters_backups_get_iam_policy_task,
    bigtableadmin_projects_instances_clusters_backups_list_builder, bigtableadmin_projects_instances_clusters_backups_list_task,
    bigtableadmin_projects_instances_clusters_backups_patch_builder, bigtableadmin_projects_instances_clusters_backups_patch_task,
    bigtableadmin_projects_instances_clusters_backups_set_iam_policy_builder, bigtableadmin_projects_instances_clusters_backups_set_iam_policy_task,
    bigtableadmin_projects_instances_clusters_backups_test_iam_permissions_builder, bigtableadmin_projects_instances_clusters_backups_test_iam_permissions_task,
    bigtableadmin_projects_instances_clusters_hot_tablets_list_builder, bigtableadmin_projects_instances_clusters_hot_tablets_list_task,
    bigtableadmin_projects_instances_clusters_memory_layers_list_builder, bigtableadmin_projects_instances_clusters_memory_layers_list_task,
    bigtableadmin_projects_instances_logical_views_create_builder, bigtableadmin_projects_instances_logical_views_create_task,
    bigtableadmin_projects_instances_logical_views_delete_builder, bigtableadmin_projects_instances_logical_views_delete_task,
    bigtableadmin_projects_instances_logical_views_get_builder, bigtableadmin_projects_instances_logical_views_get_task,
    bigtableadmin_projects_instances_logical_views_get_iam_policy_builder, bigtableadmin_projects_instances_logical_views_get_iam_policy_task,
    bigtableadmin_projects_instances_logical_views_list_builder, bigtableadmin_projects_instances_logical_views_list_task,
    bigtableadmin_projects_instances_logical_views_patch_builder, bigtableadmin_projects_instances_logical_views_patch_task,
    bigtableadmin_projects_instances_logical_views_set_iam_policy_builder, bigtableadmin_projects_instances_logical_views_set_iam_policy_task,
    bigtableadmin_projects_instances_logical_views_test_iam_permissions_builder, bigtableadmin_projects_instances_logical_views_test_iam_permissions_task,
    bigtableadmin_projects_instances_materialized_views_create_builder, bigtableadmin_projects_instances_materialized_views_create_task,
    bigtableadmin_projects_instances_materialized_views_delete_builder, bigtableadmin_projects_instances_materialized_views_delete_task,
    bigtableadmin_projects_instances_materialized_views_get_builder, bigtableadmin_projects_instances_materialized_views_get_task,
    bigtableadmin_projects_instances_materialized_views_get_iam_policy_builder, bigtableadmin_projects_instances_materialized_views_get_iam_policy_task,
    bigtableadmin_projects_instances_materialized_views_list_builder, bigtableadmin_projects_instances_materialized_views_list_task,
    bigtableadmin_projects_instances_materialized_views_patch_builder, bigtableadmin_projects_instances_materialized_views_patch_task,
    bigtableadmin_projects_instances_materialized_views_set_iam_policy_builder, bigtableadmin_projects_instances_materialized_views_set_iam_policy_task,
    bigtableadmin_projects_instances_materialized_views_test_iam_permissions_builder, bigtableadmin_projects_instances_materialized_views_test_iam_permissions_task,
    bigtableadmin_projects_instances_tables_check_consistency_builder, bigtableadmin_projects_instances_tables_check_consistency_task,
    bigtableadmin_projects_instances_tables_create_builder, bigtableadmin_projects_instances_tables_create_task,
    bigtableadmin_projects_instances_tables_delete_builder, bigtableadmin_projects_instances_tables_delete_task,
    bigtableadmin_projects_instances_tables_drop_row_range_builder, bigtableadmin_projects_instances_tables_drop_row_range_task,
    bigtableadmin_projects_instances_tables_generate_consistency_token_builder, bigtableadmin_projects_instances_tables_generate_consistency_token_task,
    bigtableadmin_projects_instances_tables_get_builder, bigtableadmin_projects_instances_tables_get_task,
    bigtableadmin_projects_instances_tables_get_iam_policy_builder, bigtableadmin_projects_instances_tables_get_iam_policy_task,
    bigtableadmin_projects_instances_tables_list_builder, bigtableadmin_projects_instances_tables_list_task,
    bigtableadmin_projects_instances_tables_modify_column_families_builder, bigtableadmin_projects_instances_tables_modify_column_families_task,
    bigtableadmin_projects_instances_tables_patch_builder, bigtableadmin_projects_instances_tables_patch_task,
    bigtableadmin_projects_instances_tables_restore_builder, bigtableadmin_projects_instances_tables_restore_task,
    bigtableadmin_projects_instances_tables_set_iam_policy_builder, bigtableadmin_projects_instances_tables_set_iam_policy_task,
    bigtableadmin_projects_instances_tables_test_iam_permissions_builder, bigtableadmin_projects_instances_tables_test_iam_permissions_task,
    bigtableadmin_projects_instances_tables_undelete_builder, bigtableadmin_projects_instances_tables_undelete_task,
    bigtableadmin_projects_instances_tables_authorized_views_create_builder, bigtableadmin_projects_instances_tables_authorized_views_create_task,
    bigtableadmin_projects_instances_tables_authorized_views_delete_builder, bigtableadmin_projects_instances_tables_authorized_views_delete_task,
    bigtableadmin_projects_instances_tables_authorized_views_get_builder, bigtableadmin_projects_instances_tables_authorized_views_get_task,
    bigtableadmin_projects_instances_tables_authorized_views_get_iam_policy_builder, bigtableadmin_projects_instances_tables_authorized_views_get_iam_policy_task,
    bigtableadmin_projects_instances_tables_authorized_views_list_builder, bigtableadmin_projects_instances_tables_authorized_views_list_task,
    bigtableadmin_projects_instances_tables_authorized_views_patch_builder, bigtableadmin_projects_instances_tables_authorized_views_patch_task,
    bigtableadmin_projects_instances_tables_authorized_views_set_iam_policy_builder, bigtableadmin_projects_instances_tables_authorized_views_set_iam_policy_task,
    bigtableadmin_projects_instances_tables_authorized_views_test_iam_permissions_builder, bigtableadmin_projects_instances_tables_authorized_views_test_iam_permissions_task,
    bigtableadmin_projects_instances_tables_schema_bundles_create_builder, bigtableadmin_projects_instances_tables_schema_bundles_create_task,
    bigtableadmin_projects_instances_tables_schema_bundles_delete_builder, bigtableadmin_projects_instances_tables_schema_bundles_delete_task,
    bigtableadmin_projects_instances_tables_schema_bundles_get_builder, bigtableadmin_projects_instances_tables_schema_bundles_get_task,
    bigtableadmin_projects_instances_tables_schema_bundles_get_iam_policy_builder, bigtableadmin_projects_instances_tables_schema_bundles_get_iam_policy_task,
    bigtableadmin_projects_instances_tables_schema_bundles_list_builder, bigtableadmin_projects_instances_tables_schema_bundles_list_task,
    bigtableadmin_projects_instances_tables_schema_bundles_patch_builder, bigtableadmin_projects_instances_tables_schema_bundles_patch_task,
    bigtableadmin_projects_instances_tables_schema_bundles_set_iam_policy_builder, bigtableadmin_projects_instances_tables_schema_bundles_set_iam_policy_task,
    bigtableadmin_projects_instances_tables_schema_bundles_test_iam_permissions_builder, bigtableadmin_projects_instances_tables_schema_bundles_test_iam_permissions_task,
    bigtableadmin_projects_locations_list_builder, bigtableadmin_projects_locations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::bigtableadmin::AppProfile;
use crate::providers::gcp::clients::bigtableadmin::AuthorizedView;
use crate::providers::gcp::clients::bigtableadmin::Backup;
use crate::providers::gcp::clients::bigtableadmin::CheckConsistencyResponse;
use crate::providers::gcp::clients::bigtableadmin::Cluster;
use crate::providers::gcp::clients::bigtableadmin::Empty;
use crate::providers::gcp::clients::bigtableadmin::GenerateConsistencyTokenResponse;
use crate::providers::gcp::clients::bigtableadmin::Instance;
use crate::providers::gcp::clients::bigtableadmin::ListAppProfilesResponse;
use crate::providers::gcp::clients::bigtableadmin::ListAuthorizedViewsResponse;
use crate::providers::gcp::clients::bigtableadmin::ListBackupsResponse;
use crate::providers::gcp::clients::bigtableadmin::ListClustersResponse;
use crate::providers::gcp::clients::bigtableadmin::ListHotTabletsResponse;
use crate::providers::gcp::clients::bigtableadmin::ListInstancesResponse;
use crate::providers::gcp::clients::bigtableadmin::ListLocationsResponse;
use crate::providers::gcp::clients::bigtableadmin::ListLogicalViewsResponse;
use crate::providers::gcp::clients::bigtableadmin::ListMaterializedViewsResponse;
use crate::providers::gcp::clients::bigtableadmin::ListMemoryLayersResponse;
use crate::providers::gcp::clients::bigtableadmin::ListOperationsResponse;
use crate::providers::gcp::clients::bigtableadmin::ListSchemaBundlesResponse;
use crate::providers::gcp::clients::bigtableadmin::ListTablesResponse;
use crate::providers::gcp::clients::bigtableadmin::LogicalView;
use crate::providers::gcp::clients::bigtableadmin::MaterializedView;
use crate::providers::gcp::clients::bigtableadmin::MemoryLayer;
use crate::providers::gcp::clients::bigtableadmin::Operation;
use crate::providers::gcp::clients::bigtableadmin::Policy;
use crate::providers::gcp::clients::bigtableadmin::SchemaBundle;
use crate::providers::gcp::clients::bigtableadmin::Table;
use crate::providers::gcp::clients::bigtableadmin::TestIamPermissionsResponse;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminOperationsGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminOperationsProjectsOperationsListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesAppProfilesCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesAppProfilesDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesAppProfilesGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesAppProfilesListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesAppProfilesPatchArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsCopyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsGetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsPatchArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersBackupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersGetMemoryLayerArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersHotTabletsListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersMemoryLayersListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersPartialUpdateClusterArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersUpdateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesClustersUpdateMemoryLayerArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsGetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsPatchArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsSetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesLogicalViewsTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsGetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsPatchArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsSetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesMaterializedViewsTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesPartialUpdateInstanceArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsGetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsPatchArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsSetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesAuthorizedViewsTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesCheckConsistencyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesDropRowRangeArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesGenerateConsistencyTokenArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesModifyColumnFamiliesArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesPatchArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesRestoreArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesCreateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesDeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesGetArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesGetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesListArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesPatchArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSchemaBundlesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesSetIamPolicyArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTablesUndeleteArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesTestIamPermissionsArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsInstancesUpdateArgs;
use crate::providers::gcp::clients::bigtableadmin::BigtableadminProjectsLocationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BigtableadminProvider with automatic state tracking.
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
/// let provider = BigtableadminProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct BigtableadminProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> BigtableadminProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new BigtableadminProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new BigtableadminProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Bigtableadmin operations get.
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
    pub fn bigtableadmin_operations_get(
        &self,
        args: &BigtableadminOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin operations projects operations list.
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
    pub fn bigtableadmin_operations_projects_operations_list(
        &self,
        args: &BigtableadminOperationsProjectsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_operations_projects_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_operations_projects_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_create(
        &self,
        args: &BigtableadminProjectsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances delete.
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
    pub fn bigtableadmin_projects_instances_delete(
        &self,
        args: &BigtableadminProjectsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Instance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_get(
        &self,
        args: &BigtableadminProjectsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances get iam policy.
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
    pub fn bigtableadmin_projects_instances_get_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstancesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_list(
        &self,
        args: &BigtableadminProjectsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances partial update instance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_partial_update_instance(
        &self,
        args: &BigtableadminProjectsInstancesPartialUpdateInstanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_partial_update_instance_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_partial_update_instance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances set iam policy.
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
    pub fn bigtableadmin_projects_instances_set_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances test iam permissions.
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
    pub fn bigtableadmin_projects_instances_test_iam_permissions(
        &self,
        args: &BigtableadminProjectsInstancesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Instance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_update(
        &self,
        args: &BigtableadminProjectsInstancesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances app profiles create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_app_profiles_create(
        &self,
        args: &BigtableadminProjectsInstancesAppProfilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_app_profiles_create_builder(
            &self.http_client,
            &args.parent,
            &args.appProfileId,
            &args.ignoreWarnings,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_app_profiles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances app profiles delete.
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
    pub fn bigtableadmin_projects_instances_app_profiles_delete(
        &self,
        args: &BigtableadminProjectsInstancesAppProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_app_profiles_delete_builder(
            &self.http_client,
            &args.name,
            &args.ignoreWarnings,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_app_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances app profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_app_profiles_get(
        &self,
        args: &BigtableadminProjectsInstancesAppProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_app_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_app_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances app profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAppProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_app_profiles_list(
        &self,
        args: &BigtableadminProjectsInstancesAppProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAppProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_app_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_app_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances app profiles patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_app_profiles_patch(
        &self,
        args: &BigtableadminProjectsInstancesAppProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_app_profiles_patch_builder(
            &self.http_client,
            &args.name,
            &args.ignoreWarnings,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_app_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_clusters_create(
        &self,
        args: &BigtableadminProjectsInstancesClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters delete.
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
    pub fn bigtableadmin_projects_instances_clusters_delete(
        &self,
        args: &BigtableadminProjectsInstancesClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters get.
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
    pub fn bigtableadmin_projects_instances_clusters_get(
        &self,
        args: &BigtableadminProjectsInstancesClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters get memory layer.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MemoryLayer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_clusters_get_memory_layer(
        &self,
        args: &BigtableadminProjectsInstancesClustersGetMemoryLayerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MemoryLayer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_get_memory_layer_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_get_memory_layer_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters list.
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
    pub fn bigtableadmin_projects_instances_clusters_list(
        &self,
        args: &BigtableadminProjectsInstancesClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters partial update cluster.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_clusters_partial_update_cluster(
        &self,
        args: &BigtableadminProjectsInstancesClustersPartialUpdateClusterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_partial_update_cluster_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_partial_update_cluster_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_clusters_update(
        &self,
        args: &BigtableadminProjectsInstancesClustersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters update memory layer.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_clusters_update_memory_layer(
        &self,
        args: &BigtableadminProjectsInstancesClustersUpdateMemoryLayerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_update_memory_layer_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_update_memory_layer_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups copy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_clusters_backups_copy(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsCopyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_copy_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_copy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_clusters_backups_create(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups delete.
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
    pub fn bigtableadmin_projects_instances_clusters_backups_delete(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Backup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_clusters_backups_get(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups get iam policy.
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
    pub fn bigtableadmin_projects_instances_clusters_backups_get_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_clusters_backups_list(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Backup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_clusters_backups_patch(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups set iam policy.
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
    pub fn bigtableadmin_projects_instances_clusters_backups_set_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters backups test iam permissions.
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
    pub fn bigtableadmin_projects_instances_clusters_backups_test_iam_permissions(
        &self,
        args: &BigtableadminProjectsInstancesClustersBackupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_backups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_backups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters hot tablets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHotTabletsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_clusters_hot_tablets_list(
        &self,
        args: &BigtableadminProjectsInstancesClustersHotTabletsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHotTabletsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_hot_tablets_list_builder(
            &self.http_client,
            &args.parent,
            &args.endTime,
            &args.pageSize,
            &args.pageToken,
            &args.startTime,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_hot_tablets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances clusters memory layers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMemoryLayersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_clusters_memory_layers_list(
        &self,
        args: &BigtableadminProjectsInstancesClustersMemoryLayersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMemoryLayersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_clusters_memory_layers_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_clusters_memory_layers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_logical_views_create(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.logicalViewId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views delete.
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
    pub fn bigtableadmin_projects_instances_logical_views_delete(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LogicalView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_logical_views_get(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LogicalView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views get iam policy.
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
    pub fn bigtableadmin_projects_instances_logical_views_get_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLogicalViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_logical_views_list(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLogicalViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_logical_views_patch(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views set iam policy.
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
    pub fn bigtableadmin_projects_instances_logical_views_set_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances logical views test iam permissions.
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
    pub fn bigtableadmin_projects_instances_logical_views_test_iam_permissions(
        &self,
        args: &BigtableadminProjectsInstancesLogicalViewsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_logical_views_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_logical_views_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_materialized_views_create(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.materializedViewId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views delete.
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
    pub fn bigtableadmin_projects_instances_materialized_views_delete(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MaterializedView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_materialized_views_get(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MaterializedView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views get iam policy.
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
    pub fn bigtableadmin_projects_instances_materialized_views_get_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMaterializedViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_materialized_views_list(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMaterializedViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_materialized_views_patch(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views set iam policy.
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
    pub fn bigtableadmin_projects_instances_materialized_views_set_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances materialized views test iam permissions.
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
    pub fn bigtableadmin_projects_instances_materialized_views_test_iam_permissions(
        &self,
        args: &BigtableadminProjectsInstancesMaterializedViewsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_materialized_views_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_materialized_views_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables check consistency.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckConsistencyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_check_consistency(
        &self,
        args: &BigtableadminProjectsInstancesTablesCheckConsistencyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckConsistencyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_check_consistency_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_check_consistency_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_create(
        &self,
        args: &BigtableadminProjectsInstancesTablesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables delete.
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
    pub fn bigtableadmin_projects_instances_tables_delete(
        &self,
        args: &BigtableadminProjectsInstancesTablesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables drop row range.
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
    pub fn bigtableadmin_projects_instances_tables_drop_row_range(
        &self,
        args: &BigtableadminProjectsInstancesTablesDropRowRangeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_drop_row_range_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_drop_row_range_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables generate consistency token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateConsistencyTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_generate_consistency_token(
        &self,
        args: &BigtableadminProjectsInstancesTablesGenerateConsistencyTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateConsistencyTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_generate_consistency_token_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_generate_consistency_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_tables_get(
        &self,
        args: &BigtableadminProjectsInstancesTablesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables get iam policy.
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
    pub fn bigtableadmin_projects_instances_tables_get_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesTablesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTablesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_tables_list(
        &self,
        args: &BigtableadminProjectsInstancesTablesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTablesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables modify column families.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Table result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_modify_column_families(
        &self,
        args: &BigtableadminProjectsInstancesTablesModifyColumnFamiliesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Table, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_modify_column_families_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_modify_column_families_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_patch(
        &self,
        args: &BigtableadminProjectsInstancesTablesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_patch_builder(
            &self.http_client,
            &args.name,
            &args.ignoreWarnings,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_restore(
        &self,
        args: &BigtableadminProjectsInstancesTablesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_restore_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables set iam policy.
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
    pub fn bigtableadmin_projects_instances_tables_set_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesTablesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables test iam permissions.
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
    pub fn bigtableadmin_projects_instances_tables_test_iam_permissions(
        &self,
        args: &BigtableadminProjectsInstancesTablesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables undelete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_undelete(
        &self,
        args: &BigtableadminProjectsInstancesTablesUndeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_undelete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_undelete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_authorized_views_create(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_create_builder(
            &self.http_client,
            &args.parent,
            &args.authorizedViewId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views delete.
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
    pub fn bigtableadmin_projects_instances_tables_authorized_views_delete(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AuthorizedView result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_tables_authorized_views_get(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AuthorizedView, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views get iam policy.
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
    pub fn bigtableadmin_projects_instances_tables_authorized_views_get_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAuthorizedViewsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_tables_authorized_views_list(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAuthorizedViewsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_authorized_views_patch(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_patch_builder(
            &self.http_client,
            &args.name,
            &args.ignoreWarnings,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views set iam policy.
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
    pub fn bigtableadmin_projects_instances_tables_authorized_views_set_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables authorized views test iam permissions.
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
    pub fn bigtableadmin_projects_instances_tables_authorized_views_test_iam_permissions(
        &self,
        args: &BigtableadminProjectsInstancesTablesAuthorizedViewsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_authorized_views_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_authorized_views_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_create(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_create_builder(
            &self.http_client,
            &args.parent,
            &args.schemaBundleId,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles delete.
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
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_delete(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SchemaBundle result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_get(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SchemaBundle, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles get iam policy.
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
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_get_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSchemaBundlesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_list(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSchemaBundlesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_patch(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_patch_builder(
            &self.http_client,
            &args.name,
            &args.ignoreWarnings,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles set iam policy.
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
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_set_iam_policy(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects instances tables schema bundles test iam permissions.
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
    pub fn bigtableadmin_projects_instances_tables_schema_bundles_test_iam_permissions(
        &self,
        args: &BigtableadminProjectsInstancesTablesSchemaBundlesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_instances_tables_schema_bundles_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_instances_tables_schema_bundles_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Bigtableadmin projects locations list.
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
    pub fn bigtableadmin_projects_locations_list(
        &self,
        args: &BigtableadminProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = bigtableadmin_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = bigtableadmin_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
