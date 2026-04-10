//! VmmigrationProvider - State-aware vmmigration API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       vmmigration API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::vmmigration::{
    vmmigration_projects_locations_get_builder, vmmigration_projects_locations_get_task,
    vmmigration_projects_locations_list_builder, vmmigration_projects_locations_list_task,
    vmmigration_projects_locations_groups_add_group_migration_builder, vmmigration_projects_locations_groups_add_group_migration_task,
    vmmigration_projects_locations_groups_create_builder, vmmigration_projects_locations_groups_create_task,
    vmmigration_projects_locations_groups_delete_builder, vmmigration_projects_locations_groups_delete_task,
    vmmigration_projects_locations_groups_get_builder, vmmigration_projects_locations_groups_get_task,
    vmmigration_projects_locations_groups_list_builder, vmmigration_projects_locations_groups_list_task,
    vmmigration_projects_locations_groups_patch_builder, vmmigration_projects_locations_groups_patch_task,
    vmmigration_projects_locations_groups_remove_group_migration_builder, vmmigration_projects_locations_groups_remove_group_migration_task,
    vmmigration_projects_locations_image_imports_create_builder, vmmigration_projects_locations_image_imports_create_task,
    vmmigration_projects_locations_image_imports_delete_builder, vmmigration_projects_locations_image_imports_delete_task,
    vmmigration_projects_locations_image_imports_get_builder, vmmigration_projects_locations_image_imports_get_task,
    vmmigration_projects_locations_image_imports_list_builder, vmmigration_projects_locations_image_imports_list_task,
    vmmigration_projects_locations_image_imports_image_import_jobs_cancel_builder, vmmigration_projects_locations_image_imports_image_import_jobs_cancel_task,
    vmmigration_projects_locations_image_imports_image_import_jobs_get_builder, vmmigration_projects_locations_image_imports_image_import_jobs_get_task,
    vmmigration_projects_locations_image_imports_image_import_jobs_list_builder, vmmigration_projects_locations_image_imports_image_import_jobs_list_task,
    vmmigration_projects_locations_operations_cancel_builder, vmmigration_projects_locations_operations_cancel_task,
    vmmigration_projects_locations_operations_delete_builder, vmmigration_projects_locations_operations_delete_task,
    vmmigration_projects_locations_operations_get_builder, vmmigration_projects_locations_operations_get_task,
    vmmigration_projects_locations_operations_list_builder, vmmigration_projects_locations_operations_list_task,
    vmmigration_projects_locations_sources_create_builder, vmmigration_projects_locations_sources_create_task,
    vmmigration_projects_locations_sources_delete_builder, vmmigration_projects_locations_sources_delete_task,
    vmmigration_projects_locations_sources_fetch_inventory_builder, vmmigration_projects_locations_sources_fetch_inventory_task,
    vmmigration_projects_locations_sources_fetch_storage_inventory_builder, vmmigration_projects_locations_sources_fetch_storage_inventory_task,
    vmmigration_projects_locations_sources_get_builder, vmmigration_projects_locations_sources_get_task,
    vmmigration_projects_locations_sources_list_builder, vmmigration_projects_locations_sources_list_task,
    vmmigration_projects_locations_sources_patch_builder, vmmigration_projects_locations_sources_patch_task,
    vmmigration_projects_locations_sources_datacenter_connectors_create_builder, vmmigration_projects_locations_sources_datacenter_connectors_create_task,
    vmmigration_projects_locations_sources_datacenter_connectors_delete_builder, vmmigration_projects_locations_sources_datacenter_connectors_delete_task,
    vmmigration_projects_locations_sources_datacenter_connectors_get_builder, vmmigration_projects_locations_sources_datacenter_connectors_get_task,
    vmmigration_projects_locations_sources_datacenter_connectors_list_builder, vmmigration_projects_locations_sources_datacenter_connectors_list_task,
    vmmigration_projects_locations_sources_datacenter_connectors_upgrade_appliance_builder, vmmigration_projects_locations_sources_datacenter_connectors_upgrade_appliance_task,
    vmmigration_projects_locations_sources_disk_migration_jobs_cancel_builder, vmmigration_projects_locations_sources_disk_migration_jobs_cancel_task,
    vmmigration_projects_locations_sources_disk_migration_jobs_create_builder, vmmigration_projects_locations_sources_disk_migration_jobs_create_task,
    vmmigration_projects_locations_sources_disk_migration_jobs_delete_builder, vmmigration_projects_locations_sources_disk_migration_jobs_delete_task,
    vmmigration_projects_locations_sources_disk_migration_jobs_get_builder, vmmigration_projects_locations_sources_disk_migration_jobs_get_task,
    vmmigration_projects_locations_sources_disk_migration_jobs_list_builder, vmmigration_projects_locations_sources_disk_migration_jobs_list_task,
    vmmigration_projects_locations_sources_disk_migration_jobs_patch_builder, vmmigration_projects_locations_sources_disk_migration_jobs_patch_task,
    vmmigration_projects_locations_sources_disk_migration_jobs_run_builder, vmmigration_projects_locations_sources_disk_migration_jobs_run_task,
    vmmigration_projects_locations_sources_migrating_vms_create_builder, vmmigration_projects_locations_sources_migrating_vms_create_task,
    vmmigration_projects_locations_sources_migrating_vms_delete_builder, vmmigration_projects_locations_sources_migrating_vms_delete_task,
    vmmigration_projects_locations_sources_migrating_vms_extend_migration_builder, vmmigration_projects_locations_sources_migrating_vms_extend_migration_task,
    vmmigration_projects_locations_sources_migrating_vms_finalize_migration_builder, vmmigration_projects_locations_sources_migrating_vms_finalize_migration_task,
    vmmigration_projects_locations_sources_migrating_vms_get_builder, vmmigration_projects_locations_sources_migrating_vms_get_task,
    vmmigration_projects_locations_sources_migrating_vms_list_builder, vmmigration_projects_locations_sources_migrating_vms_list_task,
    vmmigration_projects_locations_sources_migrating_vms_patch_builder, vmmigration_projects_locations_sources_migrating_vms_patch_task,
    vmmigration_projects_locations_sources_migrating_vms_pause_migration_builder, vmmigration_projects_locations_sources_migrating_vms_pause_migration_task,
    vmmigration_projects_locations_sources_migrating_vms_resume_migration_builder, vmmigration_projects_locations_sources_migrating_vms_resume_migration_task,
    vmmigration_projects_locations_sources_migrating_vms_start_migration_builder, vmmigration_projects_locations_sources_migrating_vms_start_migration_task,
    vmmigration_projects_locations_sources_migrating_vms_clone_jobs_cancel_builder, vmmigration_projects_locations_sources_migrating_vms_clone_jobs_cancel_task,
    vmmigration_projects_locations_sources_migrating_vms_clone_jobs_create_builder, vmmigration_projects_locations_sources_migrating_vms_clone_jobs_create_task,
    vmmigration_projects_locations_sources_migrating_vms_clone_jobs_get_builder, vmmigration_projects_locations_sources_migrating_vms_clone_jobs_get_task,
    vmmigration_projects_locations_sources_migrating_vms_clone_jobs_list_builder, vmmigration_projects_locations_sources_migrating_vms_clone_jobs_list_task,
    vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_cancel_builder, vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_cancel_task,
    vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_create_builder, vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_create_task,
    vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_get_builder, vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_get_task,
    vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_list_builder, vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_list_task,
    vmmigration_projects_locations_sources_migrating_vms_replication_cycles_get_builder, vmmigration_projects_locations_sources_migrating_vms_replication_cycles_get_task,
    vmmigration_projects_locations_sources_migrating_vms_replication_cycles_list_builder, vmmigration_projects_locations_sources_migrating_vms_replication_cycles_list_task,
    vmmigration_projects_locations_sources_utilization_reports_create_builder, vmmigration_projects_locations_sources_utilization_reports_create_task,
    vmmigration_projects_locations_sources_utilization_reports_delete_builder, vmmigration_projects_locations_sources_utilization_reports_delete_task,
    vmmigration_projects_locations_sources_utilization_reports_get_builder, vmmigration_projects_locations_sources_utilization_reports_get_task,
    vmmigration_projects_locations_sources_utilization_reports_list_builder, vmmigration_projects_locations_sources_utilization_reports_list_task,
    vmmigration_projects_locations_target_projects_create_builder, vmmigration_projects_locations_target_projects_create_task,
    vmmigration_projects_locations_target_projects_delete_builder, vmmigration_projects_locations_target_projects_delete_task,
    vmmigration_projects_locations_target_projects_get_builder, vmmigration_projects_locations_target_projects_get_task,
    vmmigration_projects_locations_target_projects_list_builder, vmmigration_projects_locations_target_projects_list_task,
    vmmigration_projects_locations_target_projects_patch_builder, vmmigration_projects_locations_target_projects_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::vmmigration::CloneJob;
use crate::providers::gcp::clients::vmmigration::CutoverJob;
use crate::providers::gcp::clients::vmmigration::DatacenterConnector;
use crate::providers::gcp::clients::vmmigration::DiskMigrationJob;
use crate::providers::gcp::clients::vmmigration::Empty;
use crate::providers::gcp::clients::vmmigration::FetchInventoryResponse;
use crate::providers::gcp::clients::vmmigration::FetchStorageInventoryResponse;
use crate::providers::gcp::clients::vmmigration::Group;
use crate::providers::gcp::clients::vmmigration::ImageImport;
use crate::providers::gcp::clients::vmmigration::ImageImportJob;
use crate::providers::gcp::clients::vmmigration::ListCloneJobsResponse;
use crate::providers::gcp::clients::vmmigration::ListCutoverJobsResponse;
use crate::providers::gcp::clients::vmmigration::ListDatacenterConnectorsResponse;
use crate::providers::gcp::clients::vmmigration::ListDiskMigrationJobsResponse;
use crate::providers::gcp::clients::vmmigration::ListGroupsResponse;
use crate::providers::gcp::clients::vmmigration::ListImageImportJobsResponse;
use crate::providers::gcp::clients::vmmigration::ListImageImportsResponse;
use crate::providers::gcp::clients::vmmigration::ListLocationsResponse;
use crate::providers::gcp::clients::vmmigration::ListMigratingVmsResponse;
use crate::providers::gcp::clients::vmmigration::ListOperationsResponse;
use crate::providers::gcp::clients::vmmigration::ListReplicationCyclesResponse;
use crate::providers::gcp::clients::vmmigration::ListSourcesResponse;
use crate::providers::gcp::clients::vmmigration::ListTargetProjectsResponse;
use crate::providers::gcp::clients::vmmigration::ListUtilizationReportsResponse;
use crate::providers::gcp::clients::vmmigration::Location;
use crate::providers::gcp::clients::vmmigration::MigratingVm;
use crate::providers::gcp::clients::vmmigration::Operation;
use crate::providers::gcp::clients::vmmigration::ReplicationCycle;
use crate::providers::gcp::clients::vmmigration::Source;
use crate::providers::gcp::clients::vmmigration::TargetProject;
use crate::providers::gcp::clients::vmmigration::UtilizationReport;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGroupsAddGroupMigrationArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGroupsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGroupsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGroupsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGroupsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGroupsPatchArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsGroupsRemoveGroupMigrationArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsImageImportsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsImageImportsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsImageImportsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsImageImportsImageImportJobsCancelArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsImageImportsImageImportJobsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsImageImportsImageImportJobsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsImageImportsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDatacenterConnectorsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDatacenterConnectorsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDatacenterConnectorsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDatacenterConnectorsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDatacenterConnectorsUpgradeApplianceArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDiskMigrationJobsCancelArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDiskMigrationJobsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDiskMigrationJobsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDiskMigrationJobsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDiskMigrationJobsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDiskMigrationJobsPatchArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesDiskMigrationJobsRunArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesFetchInventoryArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesFetchStorageInventoryArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsCancelArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsCancelArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsExtendMigrationArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsFinalizeMigrationArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsPatchArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsPauseMigrationArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsReplicationCyclesGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsReplicationCyclesListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsResumeMigrationArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesMigratingVmsStartMigrationArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesPatchArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesUtilizationReportsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesUtilizationReportsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesUtilizationReportsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsSourcesUtilizationReportsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsTargetProjectsCreateArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsTargetProjectsDeleteArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsTargetProjectsGetArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsTargetProjectsListArgs;
use crate::providers::gcp::clients::vmmigration::VmmigrationProjectsLocationsTargetProjectsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// VmmigrationProvider with automatic state tracking.
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
/// let provider = VmmigrationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct VmmigrationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> VmmigrationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new VmmigrationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Vmmigration projects locations get.
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
    pub fn vmmigration_projects_locations_get(
        &self,
        args: &VmmigrationProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations list.
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
    pub fn vmmigration_projects_locations_list(
        &self,
        args: &VmmigrationProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations groups add group migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_groups_add_group_migration(
        &self,
        args: &VmmigrationProjectsLocationsGroupsAddGroupMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_groups_add_group_migration_builder(
            &self.http_client,
            &args.group,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_groups_add_group_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_groups_create(
        &self,
        args: &VmmigrationProjectsLocationsGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.groupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations groups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_groups_delete(
        &self,
        args: &VmmigrationProjectsLocationsGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations groups get.
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
    pub fn vmmigration_projects_locations_groups_get(
        &self,
        args: &VmmigrationProjectsLocationsGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Group, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations groups list.
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
    pub fn vmmigration_projects_locations_groups_list(
        &self,
        args: &VmmigrationProjectsLocationsGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_groups_patch(
        &self,
        args: &VmmigrationProjectsLocationsGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations groups remove group migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_groups_remove_group_migration(
        &self,
        args: &VmmigrationProjectsLocationsGroupsRemoveGroupMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_groups_remove_group_migration_builder(
            &self.http_client,
            &args.group,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_groups_remove_group_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations image imports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_image_imports_create(
        &self,
        args: &VmmigrationProjectsLocationsImageImportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_image_imports_create_builder(
            &self.http_client,
            &args.parent,
            &args.imageImportId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_image_imports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations image imports delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_image_imports_delete(
        &self,
        args: &VmmigrationProjectsLocationsImageImportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_image_imports_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_image_imports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations image imports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImageImport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_image_imports_get(
        &self,
        args: &VmmigrationProjectsLocationsImageImportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImageImport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_image_imports_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_image_imports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations image imports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListImageImportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_image_imports_list(
        &self,
        args: &VmmigrationProjectsLocationsImageImportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListImageImportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_image_imports_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_image_imports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations image imports image import jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_image_imports_image_import_jobs_cancel(
        &self,
        args: &VmmigrationProjectsLocationsImageImportsImageImportJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_image_imports_image_import_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_image_imports_image_import_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations image imports image import jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ImageImportJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_image_imports_image_import_jobs_get(
        &self,
        args: &VmmigrationProjectsLocationsImageImportsImageImportJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ImageImportJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_image_imports_image_import_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_image_imports_image_import_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations image imports image import jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListImageImportJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_image_imports_image_import_jobs_list(
        &self,
        args: &VmmigrationProjectsLocationsImageImportsImageImportJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListImageImportJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_image_imports_image_import_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_image_imports_image_import_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations operations cancel.
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
    pub fn vmmigration_projects_locations_operations_cancel(
        &self,
        args: &VmmigrationProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations operations delete.
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
    pub fn vmmigration_projects_locations_operations_delete(
        &self,
        args: &VmmigrationProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations operations get.
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
    pub fn vmmigration_projects_locations_operations_get(
        &self,
        args: &VmmigrationProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations operations list.
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
    pub fn vmmigration_projects_locations_operations_list(
        &self,
        args: &VmmigrationProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_create(
        &self,
        args: &VmmigrationProjectsLocationsSourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.sourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_delete(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources fetch inventory.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchInventoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_fetch_inventory(
        &self,
        args: &VmmigrationProjectsLocationsSourcesFetchInventoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchInventoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_fetch_inventory_builder(
            &self.http_client,
            &args.source,
            &args.forceRefresh,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_fetch_inventory_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources fetch storage inventory.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchStorageInventoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_fetch_storage_inventory(
        &self,
        args: &VmmigrationProjectsLocationsSourcesFetchStorageInventoryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchStorageInventoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_fetch_storage_inventory_builder(
            &self.http_client,
            &args.source,
            &args.forceRefresh,
            &args.pageSize,
            &args.pageToken,
            &args.type_rs,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_fetch_storage_inventory_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Source result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Source, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_patch(
        &self,
        args: &VmmigrationProjectsLocationsSourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources datacenter connectors create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_datacenter_connectors_create(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDatacenterConnectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_datacenter_connectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.datacenterConnectorId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_datacenter_connectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources datacenter connectors delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_datacenter_connectors_delete(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDatacenterConnectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_datacenter_connectors_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_datacenter_connectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources datacenter connectors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatacenterConnector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_datacenter_connectors_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDatacenterConnectorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatacenterConnector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_datacenter_connectors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_datacenter_connectors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources datacenter connectors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatacenterConnectorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_datacenter_connectors_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDatacenterConnectorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatacenterConnectorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_datacenter_connectors_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_datacenter_connectors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources datacenter connectors upgrade appliance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_datacenter_connectors_upgrade_appliance(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDatacenterConnectorsUpgradeApplianceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_datacenter_connectors_upgrade_appliance_builder(
            &self.http_client,
            &args.datacenterConnector,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_datacenter_connectors_upgrade_appliance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources disk migration jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_disk_migration_jobs_cancel(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDiskMigrationJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_disk_migration_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_disk_migration_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources disk migration jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_disk_migration_jobs_create(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDiskMigrationJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_disk_migration_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.diskMigrationJobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_disk_migration_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources disk migration jobs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_disk_migration_jobs_delete(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDiskMigrationJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_disk_migration_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_disk_migration_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources disk migration jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DiskMigrationJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_disk_migration_jobs_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDiskMigrationJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DiskMigrationJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_disk_migration_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_disk_migration_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources disk migration jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDiskMigrationJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_disk_migration_jobs_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDiskMigrationJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDiskMigrationJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_disk_migration_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_disk_migration_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources disk migration jobs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_disk_migration_jobs_patch(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDiskMigrationJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_disk_migration_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_disk_migration_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources disk migration jobs run.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_disk_migration_jobs_run(
        &self,
        args: &VmmigrationProjectsLocationsSourcesDiskMigrationJobsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_disk_migration_jobs_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_disk_migration_jobs_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_create(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_create_builder(
            &self.http_client,
            &args.parent,
            &args.migratingVmId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_delete(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms extend migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_extend_migration(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsExtendMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_extend_migration_builder(
            &self.http_client,
            &args.migratingVm,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_extend_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms finalize migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_finalize_migration(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsFinalizeMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_finalize_migration_builder(
            &self.http_client,
            &args.migratingVm,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_finalize_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the MigratingVm result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<MigratingVm, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListMigratingVmsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListMigratingVmsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_patch(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms pause migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_pause_migration(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsPauseMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_pause_migration_builder(
            &self.http_client,
            &args.migratingVm,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_pause_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms resume migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_resume_migration(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsResumeMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_resume_migration_builder(
            &self.http_client,
            &args.migratingVm,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_resume_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms start migration.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_start_migration(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsStartMigrationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_start_migration_builder(
            &self.http_client,
            &args.migratingVm,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_start_migration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms clone jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_clone_jobs_cancel(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms clone jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_clone_jobs_create(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.cloneJobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms clone jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CloneJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_clone_jobs_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CloneJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms clone jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCloneJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_clone_jobs_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCloneJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCloneJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_clone_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms cutover jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_cancel(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms cutover jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_create(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.cutoverJobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms cutover jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CutoverJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CutoverJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms cutover jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCutoverJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsCutoverJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCutoverJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_cutover_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms replication cycles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReplicationCycle result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_replication_cycles_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsReplicationCyclesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReplicationCycle, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_replication_cycles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_replication_cycles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources migrating vms replication cycles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReplicationCyclesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_migrating_vms_replication_cycles_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesMigratingVmsReplicationCyclesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReplicationCyclesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_migrating_vms_replication_cycles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_migrating_vms_replication_cycles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources utilization reports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_utilization_reports_create(
        &self,
        args: &VmmigrationProjectsLocationsSourcesUtilizationReportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_utilization_reports_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.utilizationReportId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_utilization_reports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources utilization reports delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_sources_utilization_reports_delete(
        &self,
        args: &VmmigrationProjectsLocationsSourcesUtilizationReportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_utilization_reports_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_utilization_reports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources utilization reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UtilizationReport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_utilization_reports_get(
        &self,
        args: &VmmigrationProjectsLocationsSourcesUtilizationReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UtilizationReport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_utilization_reports_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_utilization_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations sources utilization reports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUtilizationReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_sources_utilization_reports_list(
        &self,
        args: &VmmigrationProjectsLocationsSourcesUtilizationReportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUtilizationReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_sources_utilization_reports_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_sources_utilization_reports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations target projects create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn vmmigration_projects_locations_target_projects_create(
        &self,
        args: &VmmigrationProjectsLocationsTargetProjectsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_target_projects_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.targetProjectId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_target_projects_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations target projects delete.
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
    pub fn vmmigration_projects_locations_target_projects_delete(
        &self,
        args: &VmmigrationProjectsLocationsTargetProjectsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_target_projects_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_target_projects_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations target projects get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TargetProject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_target_projects_get(
        &self,
        args: &VmmigrationProjectsLocationsTargetProjectsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TargetProject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_target_projects_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_target_projects_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations target projects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTargetProjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vmmigration_projects_locations_target_projects_list(
        &self,
        args: &VmmigrationProjectsLocationsTargetProjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTargetProjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_target_projects_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_target_projects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vmmigration projects locations target projects patch.
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
    pub fn vmmigration_projects_locations_target_projects_patch(
        &self,
        args: &VmmigrationProjectsLocationsTargetProjectsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vmmigration_projects_locations_target_projects_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vmmigration_projects_locations_target_projects_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
