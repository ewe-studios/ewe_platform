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
    netapp_projects_locations_active_directories_create_builder, netapp_projects_locations_active_directories_create_task,
    netapp_projects_locations_active_directories_delete_builder, netapp_projects_locations_active_directories_delete_task,
    netapp_projects_locations_active_directories_patch_builder, netapp_projects_locations_active_directories_patch_task,
    netapp_projects_locations_backup_policies_create_builder, netapp_projects_locations_backup_policies_create_task,
    netapp_projects_locations_backup_policies_delete_builder, netapp_projects_locations_backup_policies_delete_task,
    netapp_projects_locations_backup_policies_patch_builder, netapp_projects_locations_backup_policies_patch_task,
    netapp_projects_locations_backup_vaults_create_builder, netapp_projects_locations_backup_vaults_create_task,
    netapp_projects_locations_backup_vaults_delete_builder, netapp_projects_locations_backup_vaults_delete_task,
    netapp_projects_locations_backup_vaults_patch_builder, netapp_projects_locations_backup_vaults_patch_task,
    netapp_projects_locations_backup_vaults_backups_create_builder, netapp_projects_locations_backup_vaults_backups_create_task,
    netapp_projects_locations_backup_vaults_backups_delete_builder, netapp_projects_locations_backup_vaults_backups_delete_task,
    netapp_projects_locations_backup_vaults_backups_patch_builder, netapp_projects_locations_backup_vaults_backups_patch_task,
    netapp_projects_locations_host_groups_create_builder, netapp_projects_locations_host_groups_create_task,
    netapp_projects_locations_host_groups_delete_builder, netapp_projects_locations_host_groups_delete_task,
    netapp_projects_locations_host_groups_patch_builder, netapp_projects_locations_host_groups_patch_task,
    netapp_projects_locations_kms_configs_create_builder, netapp_projects_locations_kms_configs_create_task,
    netapp_projects_locations_kms_configs_delete_builder, netapp_projects_locations_kms_configs_delete_task,
    netapp_projects_locations_kms_configs_encrypt_builder, netapp_projects_locations_kms_configs_encrypt_task,
    netapp_projects_locations_kms_configs_patch_builder, netapp_projects_locations_kms_configs_patch_task,
    netapp_projects_locations_kms_configs_verify_builder, netapp_projects_locations_kms_configs_verify_task,
    netapp_projects_locations_operations_cancel_builder, netapp_projects_locations_operations_cancel_task,
    netapp_projects_locations_operations_delete_builder, netapp_projects_locations_operations_delete_task,
    netapp_projects_locations_storage_pools_create_builder, netapp_projects_locations_storage_pools_create_task,
    netapp_projects_locations_storage_pools_delete_builder, netapp_projects_locations_storage_pools_delete_task,
    netapp_projects_locations_storage_pools_patch_builder, netapp_projects_locations_storage_pools_patch_task,
    netapp_projects_locations_storage_pools_switch_builder, netapp_projects_locations_storage_pools_switch_task,
    netapp_projects_locations_storage_pools_validate_directory_service_builder, netapp_projects_locations_storage_pools_validate_directory_service_task,
    netapp_projects_locations_storage_pools_ontap_execute_ontap_delete_builder, netapp_projects_locations_storage_pools_ontap_execute_ontap_delete_task,
    netapp_projects_locations_storage_pools_ontap_execute_ontap_patch_builder, netapp_projects_locations_storage_pools_ontap_execute_ontap_patch_task,
    netapp_projects_locations_storage_pools_ontap_execute_ontap_post_builder, netapp_projects_locations_storage_pools_ontap_execute_ontap_post_task,
    netapp_projects_locations_volumes_create_builder, netapp_projects_locations_volumes_create_task,
    netapp_projects_locations_volumes_delete_builder, netapp_projects_locations_volumes_delete_task,
    netapp_projects_locations_volumes_establish_peering_builder, netapp_projects_locations_volumes_establish_peering_task,
    netapp_projects_locations_volumes_patch_builder, netapp_projects_locations_volumes_patch_task,
    netapp_projects_locations_volumes_restore_builder, netapp_projects_locations_volumes_restore_task,
    netapp_projects_locations_volumes_revert_builder, netapp_projects_locations_volumes_revert_task,
    netapp_projects_locations_volumes_quota_rules_create_builder, netapp_projects_locations_volumes_quota_rules_create_task,
    netapp_projects_locations_volumes_quota_rules_delete_builder, netapp_projects_locations_volumes_quota_rules_delete_task,
    netapp_projects_locations_volumes_quota_rules_patch_builder, netapp_projects_locations_volumes_quota_rules_patch_task,
    netapp_projects_locations_volumes_replications_create_builder, netapp_projects_locations_volumes_replications_create_task,
    netapp_projects_locations_volumes_replications_delete_builder, netapp_projects_locations_volumes_replications_delete_task,
    netapp_projects_locations_volumes_replications_establish_peering_builder, netapp_projects_locations_volumes_replications_establish_peering_task,
    netapp_projects_locations_volumes_replications_patch_builder, netapp_projects_locations_volumes_replications_patch_task,
    netapp_projects_locations_volumes_replications_resume_builder, netapp_projects_locations_volumes_replications_resume_task,
    netapp_projects_locations_volumes_replications_reverse_direction_builder, netapp_projects_locations_volumes_replications_reverse_direction_task,
    netapp_projects_locations_volumes_replications_stop_builder, netapp_projects_locations_volumes_replications_stop_task,
    netapp_projects_locations_volumes_replications_sync_builder, netapp_projects_locations_volumes_replications_sync_task,
    netapp_projects_locations_volumes_snapshots_create_builder, netapp_projects_locations_volumes_snapshots_create_task,
    netapp_projects_locations_volumes_snapshots_delete_builder, netapp_projects_locations_volumes_snapshots_delete_task,
    netapp_projects_locations_volumes_snapshots_patch_builder, netapp_projects_locations_volumes_snapshots_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::netapp::ExecuteOntapDeleteResponse;
use crate::providers::gcp::clients::netapp::ExecuteOntapPatchResponse;
use crate::providers::gcp::clients::netapp::ExecuteOntapPostResponse;
use crate::providers::gcp::clients::netapp::GoogleProtobufEmpty;
use crate::providers::gcp::clients::netapp::Operation;
use crate::providers::gcp::clients::netapp::VerifyKmsConfigResponse;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsActiveDirectoriesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupPoliciesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsBackupsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsBackupVaultsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsHostGroupsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsEncryptArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsKmsConfigsVerifyArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsOntapExecuteOntapDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsOntapExecuteOntapPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsOntapExecuteOntapPostArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsSwitchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsStoragePoolsValidateDirectoryServiceArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesEstablishPeeringArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesQuotaRulesPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsEstablishPeeringArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsPatchArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsResumeArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsReverseDirectionArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsStopArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesReplicationsSyncArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesRestoreArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesRevertArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsCreateArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsDeleteArgs;
use crate::providers::gcp::clients::netapp::NetappProjectsLocationsVolumesSnapshotsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NetappProvider with automatic state tracking.
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
/// let provider = NetappProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct NetappProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> NetappProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new NetappProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
