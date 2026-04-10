//! SqladminProvider - State-aware sqladmin API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       sqladmin API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::sqladmin::{
    sql_backups_create_backup_builder, sql_backups_create_backup_task,
    sql_backups_delete_backup_builder, sql_backups_delete_backup_task,
    sql_backups_get_backup_builder, sql_backups_get_backup_task,
    sql_backups_list_backups_builder, sql_backups_list_backups_task,
    sql_backups_update_backup_builder, sql_backups_update_backup_task,
    sql_backup_runs_delete_builder, sql_backup_runs_delete_task,
    sql_backup_runs_get_builder, sql_backup_runs_get_task,
    sql_backup_runs_insert_builder, sql_backup_runs_insert_task,
    sql_backup_runs_list_builder, sql_backup_runs_list_task,
    sql_connect_generate_ephemeral_builder, sql_connect_generate_ephemeral_task,
    sql_connect_get_builder, sql_connect_get_task,
    sql_databases_delete_builder, sql_databases_delete_task,
    sql_databases_get_builder, sql_databases_get_task,
    sql_databases_insert_builder, sql_databases_insert_task,
    sql_databases_list_builder, sql_databases_list_task,
    sql_databases_patch_builder, sql_databases_patch_task,
    sql_databases_update_builder, sql_databases_update_task,
    sql_flags_list_builder, sql_flags_list_task,
    sql_instances_list_entra_id_certificates_builder, sql_instances_list_entra_id_certificates_task,
    sql_instances_list_server_certificates_builder, sql_instances_list_server_certificates_task,
    sql_instances_rotate_entra_id_certificate_builder, sql_instances_rotate_entra_id_certificate_task,
    sql_instances_rotate_server_certificate_builder, sql_instances_rotate_server_certificate_task,
    sql_instances_acquire_ssrs_lease_builder, sql_instances_acquire_ssrs_lease_task,
    sql_instances_add_entra_id_certificate_builder, sql_instances_add_entra_id_certificate_task,
    sql_instances_add_server_ca_builder, sql_instances_add_server_ca_task,
    sql_instances_add_server_certificate_builder, sql_instances_add_server_certificate_task,
    sql_instances_clone_builder, sql_instances_clone_task,
    sql_instances_delete_builder, sql_instances_delete_task,
    sql_instances_demote_builder, sql_instances_demote_task,
    sql_instances_demote_master_builder, sql_instances_demote_master_task,
    sql_instances_execute_sql_builder, sql_instances_execute_sql_task,
    sql_instances_export_builder, sql_instances_export_task,
    sql_instances_failover_builder, sql_instances_failover_task,
    sql_instances_get_builder, sql_instances_get_task,
    sql_instances_import_builder, sql_instances_import_task,
    sql_instances_insert_builder, sql_instances_insert_task,
    sql_instances_list_builder, sql_instances_list_task,
    sql_instances_list_server_cas_builder, sql_instances_list_server_cas_task,
    sql_instances_patch_builder, sql_instances_patch_task,
    sql_instances_point_in_time_restore_builder, sql_instances_point_in_time_restore_task,
    sql_instances_pre_check_major_version_upgrade_builder, sql_instances_pre_check_major_version_upgrade_task,
    sql_instances_promote_replica_builder, sql_instances_promote_replica_task,
    sql_instances_reencrypt_builder, sql_instances_reencrypt_task,
    sql_instances_release_ssrs_lease_builder, sql_instances_release_ssrs_lease_task,
    sql_instances_reset_ssl_config_builder, sql_instances_reset_ssl_config_task,
    sql_instances_restart_builder, sql_instances_restart_task,
    sql_instances_restore_backup_builder, sql_instances_restore_backup_task,
    sql_instances_rotate_server_ca_builder, sql_instances_rotate_server_ca_task,
    sql_instances_start_replica_builder, sql_instances_start_replica_task,
    sql_instances_stop_replica_builder, sql_instances_stop_replica_task,
    sql_instances_switchover_builder, sql_instances_switchover_task,
    sql_instances_truncate_log_builder, sql_instances_truncate_log_task,
    sql_instances_update_builder, sql_instances_update_task,
    sql_operations_cancel_builder, sql_operations_cancel_task,
    sql_operations_get_builder, sql_operations_get_task,
    sql_operations_list_builder, sql_operations_list_task,
    sql_projects_instances_get_disk_shrink_config_builder, sql_projects_instances_get_disk_shrink_config_task,
    sql_projects_instances_get_latest_recovery_time_builder, sql_projects_instances_get_latest_recovery_time_task,
    sql_projects_instances_perform_disk_shrink_builder, sql_projects_instances_perform_disk_shrink_task,
    sql_projects_instances_reschedule_maintenance_builder, sql_projects_instances_reschedule_maintenance_task,
    sql_projects_instances_reset_replica_size_builder, sql_projects_instances_reset_replica_size_task,
    sql_projects_instances_start_external_sync_builder, sql_projects_instances_start_external_sync_task,
    sql_projects_instances_verify_external_sync_settings_builder, sql_projects_instances_verify_external_sync_settings_task,
    sql_ssl_certs_create_ephemeral_builder, sql_ssl_certs_create_ephemeral_task,
    sql_ssl_certs_delete_builder, sql_ssl_certs_delete_task,
    sql_ssl_certs_get_builder, sql_ssl_certs_get_task,
    sql_ssl_certs_insert_builder, sql_ssl_certs_insert_task,
    sql_ssl_certs_list_builder, sql_ssl_certs_list_task,
    sql_tiers_list_builder, sql_tiers_list_task,
    sql_users_delete_builder, sql_users_delete_task,
    sql_users_get_builder, sql_users_get_task,
    sql_users_insert_builder, sql_users_insert_task,
    sql_users_list_builder, sql_users_list_task,
    sql_users_update_builder, sql_users_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::sqladmin::Backup;
use crate::providers::gcp::clients::sqladmin::BackupRun;
use crate::providers::gcp::clients::sqladmin::BackupRunsListResponse;
use crate::providers::gcp::clients::sqladmin::ConnectSettings;
use crate::providers::gcp::clients::sqladmin::Database;
use crate::providers::gcp::clients::sqladmin::DatabaseInstance;
use crate::providers::gcp::clients::sqladmin::DatabasesListResponse;
use crate::providers::gcp::clients::sqladmin::Empty;
use crate::providers::gcp::clients::sqladmin::FlagsListResponse;
use crate::providers::gcp::clients::sqladmin::GenerateEphemeralCertResponse;
use crate::providers::gcp::clients::sqladmin::InstancesListEntraIdCertificatesResponse;
use crate::providers::gcp::clients::sqladmin::InstancesListResponse;
use crate::providers::gcp::clients::sqladmin::InstancesListServerCasResponse;
use crate::providers::gcp::clients::sqladmin::InstancesListServerCertificatesResponse;
use crate::providers::gcp::clients::sqladmin::ListBackupsResponse;
use crate::providers::gcp::clients::sqladmin::Operation;
use crate::providers::gcp::clients::sqladmin::OperationsListResponse;
use crate::providers::gcp::clients::sqladmin::SqlInstancesAcquireSsrsLeaseResponse;
use crate::providers::gcp::clients::sqladmin::SqlInstancesExecuteSqlResponse;
use crate::providers::gcp::clients::sqladmin::SqlInstancesGetDiskShrinkConfigResponse;
use crate::providers::gcp::clients::sqladmin::SqlInstancesGetLatestRecoveryTimeResponse;
use crate::providers::gcp::clients::sqladmin::SqlInstancesReleaseSsrsLeaseResponse;
use crate::providers::gcp::clients::sqladmin::SqlInstancesVerifyExternalSyncSettingsResponse;
use crate::providers::gcp::clients::sqladmin::SslCert;
use crate::providers::gcp::clients::sqladmin::SslCertsInsertResponse;
use crate::providers::gcp::clients::sqladmin::SslCertsListResponse;
use crate::providers::gcp::clients::sqladmin::TiersListResponse;
use crate::providers::gcp::clients::sqladmin::User;
use crate::providers::gcp::clients::sqladmin::UsersListResponse;
use crate::providers::gcp::clients::sqladmin::SqlBackupRunsDeleteArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupRunsGetArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupRunsInsertArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupRunsListArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupsCreateBackupArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupsDeleteBackupArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupsGetBackupArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupsListBackupsArgs;
use crate::providers::gcp::clients::sqladmin::SqlBackupsUpdateBackupArgs;
use crate::providers::gcp::clients::sqladmin::SqlConnectGenerateEphemeralArgs;
use crate::providers::gcp::clients::sqladmin::SqlConnectGetArgs;
use crate::providers::gcp::clients::sqladmin::SqlDatabasesDeleteArgs;
use crate::providers::gcp::clients::sqladmin::SqlDatabasesGetArgs;
use crate::providers::gcp::clients::sqladmin::SqlDatabasesInsertArgs;
use crate::providers::gcp::clients::sqladmin::SqlDatabasesListArgs;
use crate::providers::gcp::clients::sqladmin::SqlDatabasesPatchArgs;
use crate::providers::gcp::clients::sqladmin::SqlDatabasesUpdateArgs;
use crate::providers::gcp::clients::sqladmin::SqlFlagsListArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesAcquireSsrsLeaseArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesAddEntraIdCertificateArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesAddServerCaArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesAddServerCertificateArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesCloneArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesDeleteArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesDemoteArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesDemoteMasterArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesExecuteSqlArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesExportArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesFailoverArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesGetArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesImportArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesInsertArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesListArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesListEntraIdCertificatesArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesListServerCasArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesListServerCertificatesArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesPatchArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesPointInTimeRestoreArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesPreCheckMajorVersionUpgradeArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesPromoteReplicaArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesReencryptArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesReleaseSsrsLeaseArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesResetSslConfigArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesRestartArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesRestoreBackupArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesRotateEntraIdCertificateArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesRotateServerCaArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesRotateServerCertificateArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesStartReplicaArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesStopReplicaArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesSwitchoverArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesTruncateLogArgs;
use crate::providers::gcp::clients::sqladmin::SqlInstancesUpdateArgs;
use crate::providers::gcp::clients::sqladmin::SqlOperationsCancelArgs;
use crate::providers::gcp::clients::sqladmin::SqlOperationsGetArgs;
use crate::providers::gcp::clients::sqladmin::SqlOperationsListArgs;
use crate::providers::gcp::clients::sqladmin::SqlProjectsInstancesGetDiskShrinkConfigArgs;
use crate::providers::gcp::clients::sqladmin::SqlProjectsInstancesGetLatestRecoveryTimeArgs;
use crate::providers::gcp::clients::sqladmin::SqlProjectsInstancesPerformDiskShrinkArgs;
use crate::providers::gcp::clients::sqladmin::SqlProjectsInstancesRescheduleMaintenanceArgs;
use crate::providers::gcp::clients::sqladmin::SqlProjectsInstancesResetReplicaSizeArgs;
use crate::providers::gcp::clients::sqladmin::SqlProjectsInstancesStartExternalSyncArgs;
use crate::providers::gcp::clients::sqladmin::SqlProjectsInstancesVerifyExternalSyncSettingsArgs;
use crate::providers::gcp::clients::sqladmin::SqlSslCertsCreateEphemeralArgs;
use crate::providers::gcp::clients::sqladmin::SqlSslCertsDeleteArgs;
use crate::providers::gcp::clients::sqladmin::SqlSslCertsGetArgs;
use crate::providers::gcp::clients::sqladmin::SqlSslCertsInsertArgs;
use crate::providers::gcp::clients::sqladmin::SqlSslCertsListArgs;
use crate::providers::gcp::clients::sqladmin::SqlTiersListArgs;
use crate::providers::gcp::clients::sqladmin::SqlUsersDeleteArgs;
use crate::providers::gcp::clients::sqladmin::SqlUsersGetArgs;
use crate::providers::gcp::clients::sqladmin::SqlUsersInsertArgs;
use crate::providers::gcp::clients::sqladmin::SqlUsersListArgs;
use crate::providers::gcp::clients::sqladmin::SqlUsersUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SqladminProvider with automatic state tracking.
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
/// let provider = SqladminProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SqladminProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SqladminProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SqladminProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Sql backups create backup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_backups_create_backup(
        &self,
        args: &SqlBackupsCreateBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backups_create_backup_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backups_create_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backups delete backup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_backups_delete_backup(
        &self,
        args: &SqlBackupsDeleteBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backups_delete_backup_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backups_delete_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backups get backup.
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
    pub fn sql_backups_get_backup(
        &self,
        args: &SqlBackupsGetBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backups_get_backup_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backups_get_backup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backups list backups.
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
    pub fn sql_backups_list_backups(
        &self,
        args: &SqlBackupsListBackupsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backups_list_backups_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backups_list_backups_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backups update backup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_backups_update_backup(
        &self,
        args: &SqlBackupsUpdateBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backups_update_backup_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backups_update_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backup runs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_backup_runs_delete(
        &self,
        args: &SqlBackupRunsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backup_runs_delete_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backup_runs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backup runs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupRun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_backup_runs_get(
        &self,
        args: &SqlBackupRunsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupRun, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backup_runs_get_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backup_runs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backup runs insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_backup_runs_insert(
        &self,
        args: &SqlBackupRunsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backup_runs_insert_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backup_runs_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql backup runs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupRunsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_backup_runs_list(
        &self,
        args: &SqlBackupRunsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupRunsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_backup_runs_list_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_backup_runs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql connect generate ephemeral.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateEphemeralCertResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_connect_generate_ephemeral(
        &self,
        args: &SqlConnectGenerateEphemeralArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateEphemeralCertResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_connect_generate_ephemeral_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_connect_generate_ephemeral_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql connect get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectSettings result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_connect_get(
        &self,
        args: &SqlConnectGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectSettings, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_connect_get_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.readTime,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_connect_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql databases delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_databases_delete(
        &self,
        args: &SqlDatabasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_databases_delete_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_databases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql databases get.
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
    pub fn sql_databases_get(
        &self,
        args: &SqlDatabasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_databases_get_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_databases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql databases insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_databases_insert(
        &self,
        args: &SqlDatabasesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_databases_insert_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_databases_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql databases list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabasesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_databases_list(
        &self,
        args: &SqlDatabasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabasesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_databases_list_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_databases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql databases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_databases_patch(
        &self,
        args: &SqlDatabasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_databases_patch_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_databases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql databases update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_databases_update(
        &self,
        args: &SqlDatabasesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_databases_update_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_databases_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql flags list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FlagsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_flags_list(
        &self,
        args: &SqlFlagsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FlagsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_flags_list_builder(
            &self.http_client,
            &args.databaseVersion,
            &args.flagScope,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_flags_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances list entra id certificates.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstancesListEntraIdCertificatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_instances_list_entra_id_certificates(
        &self,
        args: &SqlInstancesListEntraIdCertificatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstancesListEntraIdCertificatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_list_entra_id_certificates_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_list_entra_id_certificates_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances list server certificates.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstancesListServerCertificatesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_instances_list_server_certificates(
        &self,
        args: &SqlInstancesListServerCertificatesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstancesListServerCertificatesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_list_server_certificates_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_list_server_certificates_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances rotate entra id certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_rotate_entra_id_certificate(
        &self,
        args: &SqlInstancesRotateEntraIdCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_rotate_entra_id_certificate_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_rotate_entra_id_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances rotate server certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_rotate_server_certificate(
        &self,
        args: &SqlInstancesRotateServerCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_rotate_server_certificate_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_rotate_server_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances acquire ssrs lease.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SqlInstancesAcquireSsrsLeaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_acquire_ssrs_lease(
        &self,
        args: &SqlInstancesAcquireSsrsLeaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SqlInstancesAcquireSsrsLeaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_acquire_ssrs_lease_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_acquire_ssrs_lease_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances add entra id certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_add_entra_id_certificate(
        &self,
        args: &SqlInstancesAddEntraIdCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_add_entra_id_certificate_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_add_entra_id_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances add server ca.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_add_server_ca(
        &self,
        args: &SqlInstancesAddServerCaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_add_server_ca_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_add_server_ca_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances add server certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_add_server_certificate(
        &self,
        args: &SqlInstancesAddServerCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_add_server_certificate_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_add_server_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances clone.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_clone(
        &self,
        args: &SqlInstancesCloneArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_clone_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_clone_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_delete(
        &self,
        args: &SqlInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_delete_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.enableFinalBackup,
            &args.finalBackupDescription,
            &args.finalBackupExpiryTime,
            &args.finalBackupTtlDays,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances demote.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_demote(
        &self,
        args: &SqlInstancesDemoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_demote_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_demote_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances demote master.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_demote_master(
        &self,
        args: &SqlInstancesDemoteMasterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_demote_master_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_demote_master_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances execute sql.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SqlInstancesExecuteSqlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_execute_sql(
        &self,
        args: &SqlInstancesExecuteSqlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SqlInstancesExecuteSqlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_execute_sql_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_execute_sql_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances export.
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
    pub fn sql_instances_export(
        &self,
        args: &SqlInstancesExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_export_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_export_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances failover.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_failover(
        &self,
        args: &SqlInstancesFailoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_failover_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_failover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DatabaseInstance result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_instances_get(
        &self,
        args: &SqlInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DatabaseInstance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_get_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances import.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_import(
        &self,
        args: &SqlInstancesImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_import_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_insert(
        &self,
        args: &SqlInstancesInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_insert_builder(
            &self.http_client,
            &args.project,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstancesListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_instances_list(
        &self,
        args: &SqlInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstancesListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_list_builder(
            &self.http_client,
            &args.project,
            &args.filter,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances list server cas.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstancesListServerCasResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_instances_list_server_cas(
        &self,
        args: &SqlInstancesListServerCasArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstancesListServerCasResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_list_server_cas_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_list_server_cas_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_patch(
        &self,
        args: &SqlInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_patch_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances point in time restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_point_in_time_restore(
        &self,
        args: &SqlInstancesPointInTimeRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_point_in_time_restore_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_point_in_time_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances pre check major version upgrade.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_pre_check_major_version_upgrade(
        &self,
        args: &SqlInstancesPreCheckMajorVersionUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_pre_check_major_version_upgrade_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_pre_check_major_version_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances promote replica.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_promote_replica(
        &self,
        args: &SqlInstancesPromoteReplicaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_promote_replica_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.failover,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_promote_replica_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances reencrypt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_reencrypt(
        &self,
        args: &SqlInstancesReencryptArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_reencrypt_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_reencrypt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances release ssrs lease.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SqlInstancesReleaseSsrsLeaseResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_release_ssrs_lease(
        &self,
        args: &SqlInstancesReleaseSsrsLeaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SqlInstancesReleaseSsrsLeaseResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_release_ssrs_lease_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_release_ssrs_lease_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances reset ssl config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_reset_ssl_config(
        &self,
        args: &SqlInstancesResetSslConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_reset_ssl_config_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.mode,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_reset_ssl_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances restart.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_restart(
        &self,
        args: &SqlInstancesRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_restart_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances restore backup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_restore_backup(
        &self,
        args: &SqlInstancesRestoreBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_restore_backup_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_restore_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances rotate server ca.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_rotate_server_ca(
        &self,
        args: &SqlInstancesRotateServerCaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_rotate_server_ca_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_rotate_server_ca_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances start replica.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_start_replica(
        &self,
        args: &SqlInstancesStartReplicaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_start_replica_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_start_replica_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances stop replica.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_stop_replica(
        &self,
        args: &SqlInstancesStopReplicaArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_stop_replica_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_stop_replica_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances switchover.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_switchover(
        &self,
        args: &SqlInstancesSwitchoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_switchover_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.dbTimeout,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_switchover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances truncate log.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_truncate_log(
        &self,
        args: &SqlInstancesTruncateLogArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_truncate_log_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_truncate_log_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql instances update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_instances_update(
        &self,
        args: &SqlInstancesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_instances_update_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_instances_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql operations cancel.
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
    pub fn sql_operations_cancel(
        &self,
        args: &SqlOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_operations_cancel_builder(
            &self.http_client,
            &args.project,
            &args.operation,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql operations get.
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
    pub fn sql_operations_get(
        &self,
        args: &SqlOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_operations_get_builder(
            &self.http_client,
            &args.project,
            &args.operation,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OperationsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_operations_list(
        &self,
        args: &SqlOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OperationsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_operations_list_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql projects instances get disk shrink config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SqlInstancesGetDiskShrinkConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_projects_instances_get_disk_shrink_config(
        &self,
        args: &SqlProjectsInstancesGetDiskShrinkConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SqlInstancesGetDiskShrinkConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_projects_instances_get_disk_shrink_config_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_projects_instances_get_disk_shrink_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql projects instances get latest recovery time.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SqlInstancesGetLatestRecoveryTimeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_projects_instances_get_latest_recovery_time(
        &self,
        args: &SqlProjectsInstancesGetLatestRecoveryTimeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SqlInstancesGetLatestRecoveryTimeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_projects_instances_get_latest_recovery_time_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.sourceInstanceDeletionTime,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_projects_instances_get_latest_recovery_time_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql projects instances perform disk shrink.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_projects_instances_perform_disk_shrink(
        &self,
        args: &SqlProjectsInstancesPerformDiskShrinkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_projects_instances_perform_disk_shrink_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_projects_instances_perform_disk_shrink_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql projects instances reschedule maintenance.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_projects_instances_reschedule_maintenance(
        &self,
        args: &SqlProjectsInstancesRescheduleMaintenanceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_projects_instances_reschedule_maintenance_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_projects_instances_reschedule_maintenance_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql projects instances reset replica size.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_projects_instances_reset_replica_size(
        &self,
        args: &SqlProjectsInstancesResetReplicaSizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_projects_instances_reset_replica_size_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_projects_instances_reset_replica_size_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql projects instances start external sync.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_projects_instances_start_external_sync(
        &self,
        args: &SqlProjectsInstancesStartExternalSyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_projects_instances_start_external_sync_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_projects_instances_start_external_sync_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql projects instances verify external sync settings.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SqlInstancesVerifyExternalSyncSettingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_projects_instances_verify_external_sync_settings(
        &self,
        args: &SqlProjectsInstancesVerifyExternalSyncSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SqlInstancesVerifyExternalSyncSettingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_projects_instances_verify_external_sync_settings_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_projects_instances_verify_external_sync_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql ssl certs create ephemeral.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SslCert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_ssl_certs_create_ephemeral(
        &self,
        args: &SqlSslCertsCreateEphemeralArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SslCert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_ssl_certs_create_ephemeral_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_ssl_certs_create_ephemeral_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql ssl certs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_ssl_certs_delete(
        &self,
        args: &SqlSslCertsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_ssl_certs_delete_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.sha1Fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_ssl_certs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql ssl certs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SslCert result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_ssl_certs_get(
        &self,
        args: &SqlSslCertsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SslCert, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_ssl_certs_get_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.sha1Fingerprint,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_ssl_certs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql ssl certs insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SslCertsInsertResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_ssl_certs_insert(
        &self,
        args: &SqlSslCertsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SslCertsInsertResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_ssl_certs_insert_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_ssl_certs_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql ssl certs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SslCertsListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_ssl_certs_list(
        &self,
        args: &SqlSslCertsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SslCertsListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_ssl_certs_list_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_ssl_certs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql tiers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TiersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_tiers_list(
        &self,
        args: &SqlTiersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TiersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_tiers_list_builder(
            &self.http_client,
            &args.project,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_tiers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql users delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_users_delete(
        &self,
        args: &SqlUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_users_delete_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.host,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql users get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the User result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_users_get(
        &self,
        args: &SqlUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_users_get_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.name,
            &args.host,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql users insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_users_insert(
        &self,
        args: &SqlUsersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_users_insert_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_users_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UsersListResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn sql_users_list(
        &self,
        args: &SqlUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UsersListResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_users_list_builder(
            &self.http_client,
            &args.project,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sql users update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn sql_users_update(
        &self,
        args: &SqlUsersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sql_users_update_builder(
            &self.http_client,
            &args.project,
            &args.instance,
            &args.databaseRoles,
            &args.host,
            &args.name,
            &args.revokeExistingRoles,
        )
        .map_err(ProviderError::Api)?;

        let task = sql_users_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
