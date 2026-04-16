//! NetappProvider - State-aware netapp API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       netapp API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::netapp::{
    netapp_projects_locations_get_builder, netapp_projects_locations_get_task,
    netapp_projects_locations_list_builder, netapp_projects_locations_list_task,
    netapp_projects_locations_active_directories_create_builder, netapp_projects_locations_active_directories_create_task,
    netapp_projects_locations_active_directories_delete_builder, netapp_projects_locations_active_directories_delete_task,
    netapp_projects_locations_active_directories_get_builder, netapp_projects_locations_active_directories_get_task,
    netapp_projects_locations_active_directories_list_builder, netapp_projects_locations_active_directories_list_task,
    netapp_projects_locations_active_directories_patch_builder, netapp_projects_locations_active_directories_patch_task,
    netapp_projects_locations_backup_policies_create_builder, netapp_projects_locations_backup_policies_create_task,
    netapp_projects_locations_backup_policies_delete_builder, netapp_projects_locations_backup_policies_delete_task,
    netapp_projects_locations_backup_policies_get_builder, netapp_projects_locations_backup_policies_get_task,
    netapp_projects_locations_backup_policies_list_builder, netapp_projects_locations_backup_policies_list_task,
    netapp_projects_locations_backup_policies_patch_builder, netapp_projects_locations_backup_policies_patch_task,
    netapp_projects_locations_backup_vaults_create_builder, netapp_projects_locations_backup_vaults_create_task,
    netapp_projects_locations_backup_vaults_delete_builder, netapp_projects_locations_backup_vaults_delete_task,
    netapp_projects_locations_backup_vaults_get_builder, netapp_projects_locations_backup_vaults_get_task,
    netapp_projects_locations_backup_vaults_list_builder, netapp_projects_locations_backup_vaults_list_task,
    netapp_projects_locations_backup_vaults_patch_builder, netapp_projects_locations_backup_vaults_patch_task,
    netapp_projects_locations_backup_vaults_backups_create_builder, netapp_projects_locations_backup_vaults_backups_create_task,
    netapp_projects_locations_backup_vaults_backups_delete_builder, netapp_projects_locations_backup_vaults_backups_delete_task,
    netapp_projects_locations_backup_vaults_backups_get_builder, netapp_projects_locations_backup_vaults_backups_get_task,
    netapp_projects_locations_backup_vaults_backups_list_builder, netapp_projects_locations_backup_vaults_backups_list_task,
    netapp_projects_locations_backup_vaults_backups_patch_builder, netapp_projects_locations_backup_vaults_backups_patch_task,
    netapp_projects_locations_host_groups_create_builder, netapp_projects_locations_host_groups_create_task,
    netapp_projects_locations_host_groups_delete_builder, netapp_projects_locations_host_groups_delete_task,
    netapp_projects_locations_host_groups_get_builder, netapp_projects_locations_host_groups_get_task,
    netapp_projects_locations_host_groups_list_builder, netapp_projects_locations_host_groups_list_task,
    netapp_projects_locations_host_groups_patch_builder, netapp_projects_locations_host_groups_patch_task,
    netapp_projects_locations_kms_configs_create_builder, netapp_projects_locations_kms_configs_create_task,
    netapp_projects_locations_kms_configs_delete_builder, netapp_projects_locations_kms_configs_delete_task,
    netapp_projects_locations_kms_configs_encrypt_builder, netapp_projects_locations_kms_configs_encrypt_task,
    netapp_projects_locations_kms_configs_get_builder, netapp_projects_locations_kms_configs_get_task,
    netapp_projects_locations_kms_configs_list_builder, netapp_projects_locations_kms_configs_list_task,
    netapp_projects_locations_kms_configs_patch_builder, netapp_projects_locations_kms_configs_patch_task,
    netapp_projects_locations_kms_configs_verify_builder, netapp_projects_locations_kms_configs_verify_task,
    netapp_projects_locations_operations_cancel_builder, netapp_projects_locations_operations_cancel_task,
    netapp_projects_locations_operations_delete_builder, netapp_projects_locations_operations_delete_task,
    netapp_projects_locations_operations_get_builder, netapp_projects_locations_operations_get_task,
    netapp_projects_locations_operations_list_builder, netapp_projects_locations_operations_list_task,
    netapp_projects_locations_storage_pools_create_builder, netapp_projects_locations_storage_pools_create_task,
    netapp_projects_locations_storage_pools_delete_builder, netapp_projects_locations_storage_pools_delete_task,
    netapp_projects_locations_storage_pools_get_builder, netapp_projects_locations_storage_pools_get_task,
    netapp_projects_locations_storage_pools_list_builder, netapp_projects_locations_storage_pools_list_task,
    netapp_projects_locations_storage_pools_patch_builder, netapp_projects_locations_storage_pools_patch_task,
    netapp_projects_locations_storage_pools_switch_builder, netapp_projects_locations_storage_pools_switch_task,
    netapp_projects_locations_storage_pools_validate_directory_service_builder, netapp_projects_locations_storage_pools_validate_directory_service_task,
    netapp_projects_locations_storage_pools_ontap_execute_ontap_delete_builder, netapp_projects_locations_storage_pools_ontap_execute_ontap_delete_task,
    netapp_projects_locations_storage_pools_ontap_execute_ontap_get_builder, netapp_projects_locations_storage_pools_ontap_execute_ontap_get_task,
    netapp_projects_locations_storage_pools_ontap_execute_ontap_patch_builder, netapp_projects_locations_storage_pools_ontap_execute_ontap_patch_task,
    netapp_projects_locations_storage_pools_ontap_execute_ontap_post_builder, netapp_projects_locations_storage_pools_ontap_execute_ontap_post_task,
    netapp_projects_locations_volumes_create_builder, netapp_projects_locations_volumes_create_task,
    netapp_projects_locations_volumes_delete_builder, netapp_projects_locations_volumes_delete_task,
    netapp_projects_locations_volumes_establish_peering_builder, netapp_projects_locations_volumes_establish_peering_task,
    netapp_projects_locations_volumes_get_builder, netapp_projects_locations_volumes_get_task,
    netapp_projects_locations_volumes_list_builder, netapp_projects_locations_volumes_list_task,
    netapp_projects_locations_volumes_patch_builder, netapp_projects_locations_volumes_patch_task,
    netapp_projects_locations_volumes_restore_builder, netapp_projects_locations_volumes_restore_task,
    netapp_projects_locations_volumes_revert_builder, netapp_projects_locations_volumes_revert_task,
    netapp_projects_locations_volumes_quota_rules_create_builder, netapp_projects_locations_volumes_quota_rules_create_task,
    netapp_projects_locations_volumes_quota_rules_delete_builder, netapp_projects_locations_volumes_quota_rules_delete_task,
    netapp_projects_locations_volumes_quota_rules_get_builder, netapp_projects_locations_volumes_quota_rules_get_task,
    netapp_projects_locations_volumes_quota_rules_list_builder, netapp_projects_locations_volumes_quota_rules_list_task,
    netapp_projects_locations_volumes_quota_rules_patch_builder, netapp_projects_locations_volumes_quota_rules_patch_task,
    netapp_projects_locations_volumes_replications_create_builder, netapp_projects_locations_volumes_replications_create_task,
    netapp_projects_locations_volumes_replications_delete_builder, netapp_projects_locations_volumes_replications_delete_task,
    netapp_projects_locations_volumes_replications_establish_peering_builder, netapp_projects_locations_volumes_replications_establish_peering_task,
    netapp_projects_locations_volumes_replications_get_builder, netapp_projects_locations_volumes_replications_get_task,
    netapp_projects_locations_volumes_replications_list_builder, netapp_projects_locations_volumes_replications_list_task,
    netapp_projects_locations_volumes_replications_patch_builder, netapp_projects_locations_volumes_replications_patch_task,
    netapp_projects_locations_volumes_replications_resume_builder, netapp_projects_locations_volumes_replications_resume_task,
    netapp_projects_locations_volumes_replications_reverse_direction_builder, netapp_projects_locations_volumes_replications_reverse_direction_task,
    netapp_projects_locations_volumes_replications_stop_builder, netapp_projects_locations_volumes_replications_stop_task,
    netapp_projects_locations_volumes_replications_sync_builder, netapp_projects_locations_volumes_replications_sync_task,
    netapp_projects_locations_volumes_snapshots_create_builder, netapp_projects_locations_volumes_snapshots_create_task,
    netapp_projects_locations_volumes_snapshots_delete_builder, netapp_projects_locations_volumes_snapshots_delete_task,
    netapp_projects_locations_volumes_snapshots_get_builder, netapp_projects_locations_volumes_snapshots_get_task,
    netapp_projects_locations_volumes_snapshots_list_builder, netapp_projects_locations_volumes_snapshots_list_task,
    netapp_projects_locations_volumes_snapshots_patch_builder, netapp_projects_locations_volumes_snapshots_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::netapp::ActiveDirectory;
use crate::providers::gcp::clients::netapp::Backup;
use crate::providers::gcp::clients::netapp::BackupPolicy;
use crate::providers::gcp::clients::netapp::BackupVault;
use crate::providers::gcp::clients::netapp::ExecuteOntapDeleteResponse;
use crate::providers::gcp::clients::netapp::ExecuteOntapGetResponse;
use crate::providers::gcp::clients::netapp::ExecuteOntapPatchResponse;
use crate::providers::gcp::clients::netapp::ExecuteOntapPostResponse;
use crate::providers::gcp::clients::netapp::GoogleProtobufEmpty;
use crate::providers::gcp::clients::netapp::HostGroup;
use crate::providers::gcp::clients::netapp::KmsConfig;
use crate::providers::gcp::clients::netapp::ListActiveDirectoriesResponse;
use crate::providers::gcp::clients::netapp::ListBackupPoliciesResponse;
use crate::providers::gcp::clients::netapp::ListBackupVaultsResponse;
use crate::providers::gcp::clients::netapp::ListBackupsResponse;
use crate::providers::gcp::clients::netapp::ListHostGroupsResponse;
use crate::providers::gcp::clients::netapp::ListKmsConfigsResponse;
use crate::providers::gcp::clients::netapp::ListLocationsResponse;
use crate::providers::gcp::clients::netapp::ListOperationsResponse;
use crate::providers::gcp::clients::netapp::ListQuotaRulesResponse;
use crate::providers::gcp::clients::netapp::ListReplicationsResponse;
use crate::providers::gcp::clients::netapp::ListSnapshotsResponse;
use crate::providers::gcp::clients::netapp::ListStoragePoolsResponse;
use crate::providers::gcp::clients::netapp::ListVolumesResponse;
use crate::providers::gcp::clients::netapp::Location;
use crate::providers::gcp::clients::netapp::Operation;
use crate::providers::gcp::clients::netapp::QuotaRule;
use crate::providers::gcp::clients::netapp::Replication;
use crate::providers::gcp::clients::netapp::Snapshot;
use crate::providers::gcp::clients::netapp::StoragePool;
use crate::providers::gcp::clients::netapp::VerifyKmsConfigResponse;
use crate::providers::gcp::clients::netapp::Volume;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsEncryptArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsVerifyArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsOntapExecuteOntapDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsOntapExecuteOntapGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsOntapExecuteOntapPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsOntapExecuteOntapPostArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsSwitchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsValidateDirectoryServiceArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesEstablishPeeringArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsEstablishPeeringArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsResumeArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsReverseDirectionArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsStopArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsSyncArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesRestoreArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesRevertArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsGetArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsListArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NetappProvider with automatic state tracking.
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
/// let provider = NetappProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct NetappProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> NetappProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new NetappProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new NetappProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Netapp projects locations get.
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
    pub fn netapp_projects_locations_get(
        &self,
        args: &NetappProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations list.
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
    pub fn netapp_projects_locations_list(
        &self,
        args: &NetappProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations active directories create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_active_directories_create(
        &self,
        args: &NetappProjectsLocationsActiveDirectoriesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_active_directories_create_builder(
            &self.http_client,
            &args.parent,
            &args.activeDirectoryId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_active_directories_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations active directories delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_active_directories_delete(
        &self,
        args: &NetappProjectsLocationsActiveDirectoriesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_active_directories_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_active_directories_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations active directories get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ActiveDirectory result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_active_directories_get(
        &self,
        args: &NetappProjectsLocationsActiveDirectoriesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ActiveDirectory, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_active_directories_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_active_directories_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations active directories list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListActiveDirectoriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_active_directories_list(
        &self,
        args: &NetappProjectsLocationsActiveDirectoriesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListActiveDirectoriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_active_directories_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_active_directories_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations active directories patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_active_directories_patch(
        &self,
        args: &NetappProjectsLocationsActiveDirectoriesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_active_directories_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_active_directories_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_policies_create(
        &self,
        args: &NetappProjectsLocationsBackupPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupPolicyId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_policies_delete(
        &self,
        args: &NetappProjectsLocationsBackupPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_policies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_backup_policies_get(
        &self,
        args: &NetappProjectsLocationsBackupPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_policies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup policies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_backup_policies_list(
        &self,
        args: &NetappProjectsLocationsBackupPoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_policies_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_policies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_policies_patch(
        &self,
        args: &NetappProjectsLocationsBackupPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_vaults_create(
        &self,
        args: &NetappProjectsLocationsBackupVaultsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupVaultId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_vaults_delete(
        &self,
        args: &NetappProjectsLocationsBackupVaultsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupVault result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_backup_vaults_get(
        &self,
        args: &NetappProjectsLocationsBackupVaultsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupVault, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupVaultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_backup_vaults_list(
        &self,
        args: &NetappProjectsLocationsBackupVaultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupVaultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_vaults_patch(
        &self,
        args: &NetappProjectsLocationsBackupVaultsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults backups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_vaults_backups_create(
        &self,
        args: &NetappProjectsLocationsBackupVaultsBackupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_backups_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_backups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults backups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_vaults_backups_delete(
        &self,
        args: &NetappProjectsLocationsBackupVaultsBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_backups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults backups get.
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
    pub fn netapp_projects_locations_backup_vaults_backups_get(
        &self,
        args: &NetappProjectsLocationsBackupVaultsBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults backups list.
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
    pub fn netapp_projects_locations_backup_vaults_backups_list(
        &self,
        args: &NetappProjectsLocationsBackupVaultsBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations backup vaults backups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_backup_vaults_backups_patch(
        &self,
        args: &NetappProjectsLocationsBackupVaultsBackupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_backup_vaults_backups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_backup_vaults_backups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations host groups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_host_groups_create(
        &self,
        args: &NetappProjectsLocationsHostGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_host_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.hostGroupId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_host_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations host groups delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_host_groups_delete(
        &self,
        args: &NetappProjectsLocationsHostGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_host_groups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_host_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations host groups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HostGroup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_host_groups_get(
        &self,
        args: &NetappProjectsLocationsHostGroupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HostGroup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_host_groups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_host_groups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations host groups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListHostGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_host_groups_list(
        &self,
        args: &NetappProjectsLocationsHostGroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListHostGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_host_groups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_host_groups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations host groups patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_host_groups_patch(
        &self,
        args: &NetappProjectsLocationsHostGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_host_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_host_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations kms configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_kms_configs_create(
        &self,
        args: &NetappProjectsLocationsKmsConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_kms_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.kmsConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_kms_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations kms configs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_kms_configs_delete(
        &self,
        args: &NetappProjectsLocationsKmsConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_kms_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_kms_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations kms configs encrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_kms_configs_encrypt(
        &self,
        args: &NetappProjectsLocationsKmsConfigsEncryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_kms_configs_encrypt_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_kms_configs_encrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations kms configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the KmsConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_kms_configs_get(
        &self,
        args: &NetappProjectsLocationsKmsConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<KmsConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_kms_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_kms_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations kms configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListKmsConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_kms_configs_list(
        &self,
        args: &NetappProjectsLocationsKmsConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListKmsConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_kms_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_kms_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations kms configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_kms_configs_patch(
        &self,
        args: &NetappProjectsLocationsKmsConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_kms_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_kms_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations kms configs verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VerifyKmsConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_kms_configs_verify(
        &self,
        args: &NetappProjectsLocationsKmsConfigsVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VerifyKmsConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_kms_configs_verify_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_kms_configs_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_operations_cancel(
        &self,
        args: &NetappProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations operations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_operations_delete(
        &self,
        args: &NetappProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations operations get.
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
    pub fn netapp_projects_locations_operations_get(
        &self,
        args: &NetappProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations operations list.
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
    pub fn netapp_projects_locations_operations_list(
        &self,
        args: &NetappProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_storage_pools_create(
        &self,
        args: &NetappProjectsLocationsStoragePoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_create_builder(
            &self.http_client,
            &args.parent,
            &args.storagePoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_storage_pools_delete(
        &self,
        args: &NetappProjectsLocationsStoragePoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StoragePool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_storage_pools_get(
        &self,
        args: &NetappProjectsLocationsStoragePoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StoragePool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStoragePoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_storage_pools_list(
        &self,
        args: &NetappProjectsLocationsStoragePoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStoragePoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_storage_pools_patch(
        &self,
        args: &NetappProjectsLocationsStoragePoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools switch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_storage_pools_switch(
        &self,
        args: &NetappProjectsLocationsStoragePoolsSwitchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_switch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_switch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools validate directory service.
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
    pub fn netapp_projects_locations_storage_pools_validate_directory_service(
        &self,
        args: &NetappProjectsLocationsStoragePoolsValidateDirectoryServiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_validate_directory_service_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_validate_directory_service_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools ontap execute ontap delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteOntapDeleteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_storage_pools_ontap_execute_ontap_delete(
        &self,
        args: &NetappProjectsLocationsStoragePoolsOntapExecuteOntapDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteOntapDeleteResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_ontap_execute_ontap_delete_builder(
            &self.http_client,
            &args.ontapPath,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_ontap_execute_ontap_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools ontap execute ontap get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteOntapGetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_storage_pools_ontap_execute_ontap_get(
        &self,
        args: &NetappProjectsLocationsStoragePoolsOntapExecuteOntapGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteOntapGetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_ontap_execute_ontap_get_builder(
            &self.http_client,
            &args.ontapPath,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_ontap_execute_ontap_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools ontap execute ontap patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteOntapPatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_storage_pools_ontap_execute_ontap_patch(
        &self,
        args: &NetappProjectsLocationsStoragePoolsOntapExecuteOntapPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteOntapPatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_ontap_execute_ontap_patch_builder(
            &self.http_client,
            &args.ontapPath,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_ontap_execute_ontap_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations storage pools ontap execute ontap post.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteOntapPostResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_storage_pools_ontap_execute_ontap_post(
        &self,
        args: &NetappProjectsLocationsStoragePoolsOntapExecuteOntapPostArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteOntapPostResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_storage_pools_ontap_execute_ontap_post_builder(
            &self.http_client,
            &args.ontapPath,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_storage_pools_ontap_execute_ontap_post_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_create(
        &self,
        args: &NetappProjectsLocationsVolumesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_create_builder(
            &self.http_client,
            &args.parent,
            &args.volumeId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_delete(
        &self,
        args: &NetappProjectsLocationsVolumesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes establish peering.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_establish_peering(
        &self,
        args: &NetappProjectsLocationsVolumesEstablishPeeringArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_establish_peering_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_establish_peering_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Volume result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_get(
        &self,
        args: &NetappProjectsLocationsVolumesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVolumesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_list(
        &self,
        args: &NetappProjectsLocationsVolumesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVolumesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_patch(
        &self,
        args: &NetappProjectsLocationsVolumesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_restore(
        &self,
        args: &NetappProjectsLocationsVolumesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes revert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_revert(
        &self,
        args: &NetappProjectsLocationsVolumesRevertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_revert_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_revert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes quota rules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_quota_rules_create(
        &self,
        args: &NetappProjectsLocationsVolumesQuotaRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_quota_rules_create_builder(
            &self.http_client,
            &args.parent,
            &args.quotaRuleId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_quota_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes quota rules delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_quota_rules_delete(
        &self,
        args: &NetappProjectsLocationsVolumesQuotaRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_quota_rules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_quota_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes quota rules get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QuotaRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_quota_rules_get(
        &self,
        args: &NetappProjectsLocationsVolumesQuotaRulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QuotaRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_quota_rules_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_quota_rules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes quota rules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListQuotaRulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_quota_rules_list(
        &self,
        args: &NetappProjectsLocationsVolumesQuotaRulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListQuotaRulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_quota_rules_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_quota_rules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes quota rules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_quota_rules_patch(
        &self,
        args: &NetappProjectsLocationsVolumesQuotaRulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_quota_rules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_quota_rules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_create(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_create_builder(
            &self.http_client,
            &args.parent,
            &args.replicationId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_delete(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications establish peering.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_establish_peering(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsEstablishPeeringArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_establish_peering_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_establish_peering_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Replication result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_replications_get(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Replication, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_replications_list(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_patch(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_resume(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications reverse direction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_reverse_direction(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsReverseDirectionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_reverse_direction_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_reverse_direction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications stop.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_stop(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes replications sync.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_replications_sync(
        &self,
        args: &NetappProjectsLocationsVolumesReplicationsSyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_replications_sync_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_replications_sync_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes snapshots create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_snapshots_create(
        &self,
        args: &NetappProjectsLocationsVolumesSnapshotsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_snapshots_create_builder(
            &self.http_client,
            &args.parent,
            &args.snapshotId,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_snapshots_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes snapshots delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_snapshots_delete(
        &self,
        args: &NetappProjectsLocationsVolumesSnapshotsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_snapshots_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_snapshots_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes snapshots get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Snapshot result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_snapshots_get(
        &self,
        args: &NetappProjectsLocationsVolumesSnapshotsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Snapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_snapshots_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_snapshots_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes snapshots list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSnapshotsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn netapp_projects_locations_volumes_snapshots_list(
        &self,
        args: &NetappProjectsLocationsVolumesSnapshotsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSnapshotsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_snapshots_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_snapshots_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Netapp projects locations volumes snapshots patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn netapp_projects_locations_volumes_snapshots_patch(
        &self,
        args: &NetappProjectsLocationsVolumesSnapshotsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = netapp_projects_locations_volumes_snapshots_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = netapp_projects_locations_volumes_snapshots_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
