//! OracledatabaseProvider - State-aware oracledatabase API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       oracledatabase API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::oracledatabase::{
    oracledatabase_projects_locations_get_builder, oracledatabase_projects_locations_get_task,
    oracledatabase_projects_locations_list_builder, oracledatabase_projects_locations_list_task,
    oracledatabase_projects_locations_autonomous_database_backups_list_builder, oracledatabase_projects_locations_autonomous_database_backups_list_task,
    oracledatabase_projects_locations_autonomous_database_character_sets_list_builder, oracledatabase_projects_locations_autonomous_database_character_sets_list_task,
    oracledatabase_projects_locations_autonomous_databases_create_builder, oracledatabase_projects_locations_autonomous_databases_create_task,
    oracledatabase_projects_locations_autonomous_databases_delete_builder, oracledatabase_projects_locations_autonomous_databases_delete_task,
    oracledatabase_projects_locations_autonomous_databases_failover_builder, oracledatabase_projects_locations_autonomous_databases_failover_task,
    oracledatabase_projects_locations_autonomous_databases_generate_wallet_builder, oracledatabase_projects_locations_autonomous_databases_generate_wallet_task,
    oracledatabase_projects_locations_autonomous_databases_get_builder, oracledatabase_projects_locations_autonomous_databases_get_task,
    oracledatabase_projects_locations_autonomous_databases_list_builder, oracledatabase_projects_locations_autonomous_databases_list_task,
    oracledatabase_projects_locations_autonomous_databases_patch_builder, oracledatabase_projects_locations_autonomous_databases_patch_task,
    oracledatabase_projects_locations_autonomous_databases_restart_builder, oracledatabase_projects_locations_autonomous_databases_restart_task,
    oracledatabase_projects_locations_autonomous_databases_restore_builder, oracledatabase_projects_locations_autonomous_databases_restore_task,
    oracledatabase_projects_locations_autonomous_databases_start_builder, oracledatabase_projects_locations_autonomous_databases_start_task,
    oracledatabase_projects_locations_autonomous_databases_stop_builder, oracledatabase_projects_locations_autonomous_databases_stop_task,
    oracledatabase_projects_locations_autonomous_databases_switchover_builder, oracledatabase_projects_locations_autonomous_databases_switchover_task,
    oracledatabase_projects_locations_autonomous_db_versions_list_builder, oracledatabase_projects_locations_autonomous_db_versions_list_task,
    oracledatabase_projects_locations_cloud_exadata_infrastructures_create_builder, oracledatabase_projects_locations_cloud_exadata_infrastructures_create_task,
    oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_builder, oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_task,
    oracledatabase_projects_locations_cloud_exadata_infrastructures_get_builder, oracledatabase_projects_locations_cloud_exadata_infrastructures_get_task,
    oracledatabase_projects_locations_cloud_exadata_infrastructures_list_builder, oracledatabase_projects_locations_cloud_exadata_infrastructures_list_task,
    oracledatabase_projects_locations_cloud_exadata_infrastructures_db_servers_list_builder, oracledatabase_projects_locations_cloud_exadata_infrastructures_db_servers_list_task,
    oracledatabase_projects_locations_cloud_vm_clusters_create_builder, oracledatabase_projects_locations_cloud_vm_clusters_create_task,
    oracledatabase_projects_locations_cloud_vm_clusters_delete_builder, oracledatabase_projects_locations_cloud_vm_clusters_delete_task,
    oracledatabase_projects_locations_cloud_vm_clusters_get_builder, oracledatabase_projects_locations_cloud_vm_clusters_get_task,
    oracledatabase_projects_locations_cloud_vm_clusters_list_builder, oracledatabase_projects_locations_cloud_vm_clusters_list_task,
    oracledatabase_projects_locations_cloud_vm_clusters_db_nodes_list_builder, oracledatabase_projects_locations_cloud_vm_clusters_db_nodes_list_task,
    oracledatabase_projects_locations_database_character_sets_list_builder, oracledatabase_projects_locations_database_character_sets_list_task,
    oracledatabase_projects_locations_databases_get_builder, oracledatabase_projects_locations_databases_get_task,
    oracledatabase_projects_locations_databases_list_builder, oracledatabase_projects_locations_databases_list_task,
    oracledatabase_projects_locations_db_system_initial_storage_sizes_list_builder, oracledatabase_projects_locations_db_system_initial_storage_sizes_list_task,
    oracledatabase_projects_locations_db_system_shapes_list_builder, oracledatabase_projects_locations_db_system_shapes_list_task,
    oracledatabase_projects_locations_db_systems_create_builder, oracledatabase_projects_locations_db_systems_create_task,
    oracledatabase_projects_locations_db_systems_delete_builder, oracledatabase_projects_locations_db_systems_delete_task,
    oracledatabase_projects_locations_db_systems_get_builder, oracledatabase_projects_locations_db_systems_get_task,
    oracledatabase_projects_locations_db_systems_list_builder, oracledatabase_projects_locations_db_systems_list_task,
    oracledatabase_projects_locations_db_versions_list_builder, oracledatabase_projects_locations_db_versions_list_task,
    oracledatabase_projects_locations_entitlements_list_builder, oracledatabase_projects_locations_entitlements_list_task,
    oracledatabase_projects_locations_exadb_vm_clusters_create_builder, oracledatabase_projects_locations_exadb_vm_clusters_create_task,
    oracledatabase_projects_locations_exadb_vm_clusters_delete_builder, oracledatabase_projects_locations_exadb_vm_clusters_delete_task,
    oracledatabase_projects_locations_exadb_vm_clusters_get_builder, oracledatabase_projects_locations_exadb_vm_clusters_get_task,
    oracledatabase_projects_locations_exadb_vm_clusters_list_builder, oracledatabase_projects_locations_exadb_vm_clusters_list_task,
    oracledatabase_projects_locations_exadb_vm_clusters_patch_builder, oracledatabase_projects_locations_exadb_vm_clusters_patch_task,
    oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_builder, oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_task,
    oracledatabase_projects_locations_exadb_vm_clusters_db_nodes_list_builder, oracledatabase_projects_locations_exadb_vm_clusters_db_nodes_list_task,
    oracledatabase_projects_locations_exascale_db_storage_vaults_create_builder, oracledatabase_projects_locations_exascale_db_storage_vaults_create_task,
    oracledatabase_projects_locations_exascale_db_storage_vaults_delete_builder, oracledatabase_projects_locations_exascale_db_storage_vaults_delete_task,
    oracledatabase_projects_locations_exascale_db_storage_vaults_get_builder, oracledatabase_projects_locations_exascale_db_storage_vaults_get_task,
    oracledatabase_projects_locations_exascale_db_storage_vaults_list_builder, oracledatabase_projects_locations_exascale_db_storage_vaults_list_task,
    oracledatabase_projects_locations_gi_versions_list_builder, oracledatabase_projects_locations_gi_versions_list_task,
    oracledatabase_projects_locations_gi_versions_minor_versions_list_builder, oracledatabase_projects_locations_gi_versions_minor_versions_list_task,
    oracledatabase_projects_locations_odb_networks_create_builder, oracledatabase_projects_locations_odb_networks_create_task,
    oracledatabase_projects_locations_odb_networks_delete_builder, oracledatabase_projects_locations_odb_networks_delete_task,
    oracledatabase_projects_locations_odb_networks_get_builder, oracledatabase_projects_locations_odb_networks_get_task,
    oracledatabase_projects_locations_odb_networks_list_builder, oracledatabase_projects_locations_odb_networks_list_task,
    oracledatabase_projects_locations_odb_networks_odb_subnets_create_builder, oracledatabase_projects_locations_odb_networks_odb_subnets_create_task,
    oracledatabase_projects_locations_odb_networks_odb_subnets_delete_builder, oracledatabase_projects_locations_odb_networks_odb_subnets_delete_task,
    oracledatabase_projects_locations_odb_networks_odb_subnets_get_builder, oracledatabase_projects_locations_odb_networks_odb_subnets_get_task,
    oracledatabase_projects_locations_odb_networks_odb_subnets_list_builder, oracledatabase_projects_locations_odb_networks_odb_subnets_list_task,
    oracledatabase_projects_locations_operations_cancel_builder, oracledatabase_projects_locations_operations_cancel_task,
    oracledatabase_projects_locations_operations_delete_builder, oracledatabase_projects_locations_operations_delete_task,
    oracledatabase_projects_locations_operations_get_builder, oracledatabase_projects_locations_operations_get_task,
    oracledatabase_projects_locations_operations_list_builder, oracledatabase_projects_locations_operations_list_task,
    oracledatabase_projects_locations_pluggable_databases_get_builder, oracledatabase_projects_locations_pluggable_databases_get_task,
    oracledatabase_projects_locations_pluggable_databases_list_builder, oracledatabase_projects_locations_pluggable_databases_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::oracledatabase::AutonomousDatabase;
use crate::providers::gcp::clients::oracledatabase::CloudExadataInfrastructure;
use crate::providers::gcp::clients::oracledatabase::CloudVmCluster;
use crate::providers::gcp::clients::oracledatabase::Database;
use crate::providers::gcp::clients::oracledatabase::DbSystem;
use crate::providers::gcp::clients::oracledatabase::Empty;
use crate::providers::gcp::clients::oracledatabase::ExadbVmCluster;
use crate::providers::gcp::clients::oracledatabase::ExascaleDbStorageVault;
use crate::providers::gcp::clients::oracledatabase::GenerateAutonomousDatabaseWalletResponse;
use crate::providers::gcp::clients::oracledatabase::ListAutonomousDatabaseBackupsResponse;
use crate::providers::gcp::clients::oracledatabase::ListAutonomousDatabaseCharacterSetsResponse;
use crate::providers::gcp::clients::oracledatabase::ListAutonomousDatabasesResponse;
use crate::providers::gcp::clients::oracledatabase::ListAutonomousDbVersionsResponse;
use crate::providers::gcp::clients::oracledatabase::ListCloudExadataInfrastructuresResponse;
use crate::providers::gcp::clients::oracledatabase::ListCloudVmClustersResponse;
use crate::providers::gcp::clients::oracledatabase::ListDatabaseCharacterSetsResponse;
use crate::providers::gcp::clients::oracledatabase::ListDatabasesResponse;
use crate::providers::gcp::clients::oracledatabase::ListDbNodesResponse;
use crate::providers::gcp::clients::oracledatabase::ListDbServersResponse;
use crate::providers::gcp::clients::oracledatabase::ListDbSystemInitialStorageSizesResponse;
use crate::providers::gcp::clients::oracledatabase::ListDbSystemShapesResponse;
use crate::providers::gcp::clients::oracledatabase::ListDbSystemsResponse;
use crate::providers::gcp::clients::oracledatabase::ListDbVersionsResponse;
use crate::providers::gcp::clients::oracledatabase::ListEntitlementsResponse;
use crate::providers::gcp::clients::oracledatabase::ListExadbVmClustersResponse;
use crate::providers::gcp::clients::oracledatabase::ListExascaleDbStorageVaultsResponse;
use crate::providers::gcp::clients::oracledatabase::ListGiVersionsResponse;
use crate::providers::gcp::clients::oracledatabase::ListLocationsResponse;
use crate::providers::gcp::clients::oracledatabase::ListMinorVersionsResponse;
use crate::providers::gcp::clients::oracledatabase::ListOdbNetworksResponse;
use crate::providers::gcp::clients::oracledatabase::ListOdbSubnetsResponse;
use crate::providers::gcp::clients::oracledatabase::ListOperationsResponse;
use crate::providers::gcp::clients::oracledatabase::ListPluggableDatabasesResponse;
use crate::providers::gcp::clients::oracledatabase::Location;
use crate::providers::gcp::clients::oracledatabase::OdbNetwork;
use crate::providers::gcp::clients::oracledatabase::OdbSubnet;
use crate::providers::gcp::clients::oracledatabase::Operation;
use crate::providers::gcp::clients::oracledatabase::PluggableDatabase;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabaseBackupsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabaseCharacterSetsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesFailoverArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesGenerateWalletArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesPatchArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesRestartArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesRestoreArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesStartArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesStopArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesSwitchoverArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDbVersionsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudExadataInfrastructuresCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudExadataInfrastructuresDbServersListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudExadataInfrastructuresDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudExadataInfrastructuresGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudExadataInfrastructuresListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudVmClustersCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudVmClustersDbNodesListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudVmClustersDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudVmClustersGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudVmClustersListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDatabaseCharacterSetsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDatabasesGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDatabasesListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemInitialStorageSizesListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemShapesListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemsCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemsDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemsGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbVersionsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsEntitlementsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersDbNodesListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersPatchArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersRemoveVirtualMachineArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExascaleDbStorageVaultsCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExascaleDbStorageVaultsDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExascaleDbStorageVaultsGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExascaleDbStorageVaultsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsGiVersionsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsGiVersionsMinorVersionsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsPluggableDatabasesGetArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsPluggableDatabasesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// OracledatabaseProvider with automatic state tracking.
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
/// let provider = OracledatabaseProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct OracledatabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> OracledatabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new OracledatabaseProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Oracledatabase projects locations get.
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
    pub fn oracledatabase_projects_locations_get(
        &self,
        args: &OracledatabaseProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations list.
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
    pub fn oracledatabase_projects_locations_list(
        &self,
        args: &OracledatabaseProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous database backups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutonomousDatabaseBackupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_autonomous_database_backups_list(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabaseBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutonomousDatabaseBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_database_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_database_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous database character sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutonomousDatabaseCharacterSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_autonomous_database_character_sets_list(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabaseCharacterSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutonomousDatabaseCharacterSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_database_character_sets_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_database_character_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_create(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_create_builder(
            &self.http_client,
            &args.parent,
            &args.autonomousDatabaseId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_delete(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases failover.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_failover(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesFailoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_failover_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_failover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases generate wallet.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateAutonomousDatabaseWalletResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_generate_wallet(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesGenerateWalletArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateAutonomousDatabaseWalletResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_generate_wallet_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_generate_wallet_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AutonomousDatabase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_get(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AutonomousDatabase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutonomousDatabasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_list(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutonomousDatabasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_patch(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases restart.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_restart(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_restart_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_restore(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases start.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_start(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_stop(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases switchover.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_switchover(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesSwitchoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_switchover_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_switchover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous db versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAutonomousDbVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_autonomous_db_versions_list(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDbVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAutonomousDbVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_db_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_db_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud exadata infrastructures create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_cloud_exadata_infrastructures_create(
        &self,
        args: &OracledatabaseProjectsLocationsCloudExadataInfrastructuresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_exadata_infrastructures_create_builder(
            &self.http_client,
            &args.parent,
            &args.cloudExadataInfrastructureId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_exadata_infrastructures_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud exadata infrastructures delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_cloud_exadata_infrastructures_delete(
        &self,
        args: &OracledatabaseProjectsLocationsCloudExadataInfrastructuresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud exadata infrastructures get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CloudExadataInfrastructure result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_cloud_exadata_infrastructures_get(
        &self,
        args: &OracledatabaseProjectsLocationsCloudExadataInfrastructuresGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CloudExadataInfrastructure, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_exadata_infrastructures_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_exadata_infrastructures_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud exadata infrastructures list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCloudExadataInfrastructuresResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_cloud_exadata_infrastructures_list(
        &self,
        args: &OracledatabaseProjectsLocationsCloudExadataInfrastructuresListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCloudExadataInfrastructuresResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_exadata_infrastructures_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_exadata_infrastructures_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud exadata infrastructures db servers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDbServersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_cloud_exadata_infrastructures_db_servers_list(
        &self,
        args: &OracledatabaseProjectsLocationsCloudExadataInfrastructuresDbServersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDbServersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_exadata_infrastructures_db_servers_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_exadata_infrastructures_db_servers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud vm clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_cloud_vm_clusters_create(
        &self,
        args: &OracledatabaseProjectsLocationsCloudVmClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_vm_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.cloudVmClusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_vm_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud vm clusters delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_cloud_vm_clusters_delete(
        &self,
        args: &OracledatabaseProjectsLocationsCloudVmClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_vm_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_vm_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud vm clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CloudVmCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_cloud_vm_clusters_get(
        &self,
        args: &OracledatabaseProjectsLocationsCloudVmClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CloudVmCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_vm_clusters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_vm_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud vm clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCloudVmClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_cloud_vm_clusters_list(
        &self,
        args: &OracledatabaseProjectsLocationsCloudVmClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCloudVmClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_vm_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_vm_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud vm clusters db nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDbNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_cloud_vm_clusters_db_nodes_list(
        &self,
        args: &OracledatabaseProjectsLocationsCloudVmClustersDbNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDbNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_vm_clusters_db_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_vm_clusters_db_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations database character sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatabaseCharacterSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_database_character_sets_list(
        &self,
        args: &OracledatabaseProjectsLocationsDatabaseCharacterSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatabaseCharacterSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_database_character_sets_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_database_character_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations databases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Database result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_databases_get(
        &self,
        args: &OracledatabaseProjectsLocationsDatabasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_databases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_databases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations databases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatabasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_databases_list(
        &self,
        args: &OracledatabaseProjectsLocationsDatabasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatabasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_databases_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_databases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db system initial storage sizes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDbSystemInitialStorageSizesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_db_system_initial_storage_sizes_list(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemInitialStorageSizesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDbSystemInitialStorageSizesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_system_initial_storage_sizes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_system_initial_storage_sizes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db system shapes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDbSystemShapesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_db_system_shapes_list(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemShapesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDbSystemShapesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_system_shapes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_system_shapes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db systems create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_db_systems_create(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_systems_create_builder(
            &self.http_client,
            &args.parent,
            &args.dbSystemId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_systems_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db systems delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_db_systems_delete(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_systems_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_systems_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db systems get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DbSystem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_db_systems_get(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DbSystem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_systems_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_systems_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db systems list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDbSystemsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_db_systems_list(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDbSystemsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_systems_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_systems_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDbVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_db_versions_list(
        &self,
        args: &OracledatabaseProjectsLocationsDbVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDbVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations entitlements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEntitlementsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_entitlements_list(
        &self,
        args: &OracledatabaseProjectsLocationsEntitlementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEntitlementsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_entitlements_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_entitlements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_create(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.exadbVmClusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_delete(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExadbVmCluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_get(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExadbVmCluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExadbVmClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_list(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExadbVmClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_patch(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters remove virtual machine.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersRemoveVirtualMachineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters db nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDbNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_db_nodes_list(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersDbNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDbNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_db_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_db_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exascale db storage vaults create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_exascale_db_storage_vaults_create(
        &self,
        args: &OracledatabaseProjectsLocationsExascaleDbStorageVaultsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exascale_db_storage_vaults_create_builder(
            &self.http_client,
            &args.parent,
            &args.exascaleDbStorageVaultId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exascale_db_storage_vaults_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exascale db storage vaults delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_exascale_db_storage_vaults_delete(
        &self,
        args: &OracledatabaseProjectsLocationsExascaleDbStorageVaultsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exascale_db_storage_vaults_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exascale_db_storage_vaults_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exascale db storage vaults get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExascaleDbStorageVault result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_exascale_db_storage_vaults_get(
        &self,
        args: &OracledatabaseProjectsLocationsExascaleDbStorageVaultsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExascaleDbStorageVault, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exascale_db_storage_vaults_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exascale_db_storage_vaults_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exascale db storage vaults list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListExascaleDbStorageVaultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_exascale_db_storage_vaults_list(
        &self,
        args: &OracledatabaseProjectsLocationsExascaleDbStorageVaultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListExascaleDbStorageVaultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exascale_db_storage_vaults_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exascale_db_storage_vaults_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations gi versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListGiVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_gi_versions_list(
        &self,
        args: &OracledatabaseProjectsLocationsGiVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGiVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_gi_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_gi_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations gi versions minor versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMinorVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_gi_versions_minor_versions_list(
        &self,
        args: &OracledatabaseProjectsLocationsGiVersionsMinorVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMinorVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_gi_versions_minor_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_gi_versions_minor_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_odb_networks_create(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_create_builder(
            &self.http_client,
            &args.parent,
            &args.odbNetworkId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_odb_networks_delete(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OdbNetwork result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_odb_networks_get(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OdbNetwork, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOdbNetworksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_odb_networks_list(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOdbNetworksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks odb subnets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_odb_networks_odb_subnets_create(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_odb_subnets_create_builder(
            &self.http_client,
            &args.parent,
            &args.odbSubnetId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_odb_subnets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks odb subnets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_odb_networks_odb_subnets_delete(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_odb_subnets_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_odb_subnets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks odb subnets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OdbSubnet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_odb_networks_odb_subnets_get(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OdbSubnet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_odb_subnets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_odb_subnets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks odb subnets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOdbSubnetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_odb_networks_odb_subnets_list(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOdbSubnetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_odb_subnets_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_odb_subnets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations operations cancel.
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
    pub fn oracledatabase_projects_locations_operations_cancel(
        &self,
        args: &OracledatabaseProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations operations delete.
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
    pub fn oracledatabase_projects_locations_operations_delete(
        &self,
        args: &OracledatabaseProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations operations get.
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
    pub fn oracledatabase_projects_locations_operations_get(
        &self,
        args: &OracledatabaseProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations operations list.
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
    pub fn oracledatabase_projects_locations_operations_list(
        &self,
        args: &OracledatabaseProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations pluggable databases get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PluggableDatabase result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_pluggable_databases_get(
        &self,
        args: &OracledatabaseProjectsLocationsPluggableDatabasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PluggableDatabase, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_pluggable_databases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_pluggable_databases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations pluggable databases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPluggableDatabasesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn oracledatabase_projects_locations_pluggable_databases_list(
        &self,
        args: &OracledatabaseProjectsLocationsPluggableDatabasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPluggableDatabasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_pluggable_databases_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_pluggable_databases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
