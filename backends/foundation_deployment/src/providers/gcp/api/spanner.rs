//! SpannerProvider - State-aware spanner API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       spanner API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::spanner::{
    spanner_projects_instance_config_operations_list_builder, spanner_projects_instance_config_operations_list_task,
    spanner_projects_instance_configs_create_builder, spanner_projects_instance_configs_create_task,
    spanner_projects_instance_configs_delete_builder, spanner_projects_instance_configs_delete_task,
    spanner_projects_instance_configs_get_builder, spanner_projects_instance_configs_get_task,
    spanner_projects_instance_configs_list_builder, spanner_projects_instance_configs_list_task,
    spanner_projects_instance_configs_patch_builder, spanner_projects_instance_configs_patch_task,
    spanner_projects_instance_configs_operations_cancel_builder, spanner_projects_instance_configs_operations_cancel_task,
    spanner_projects_instance_configs_operations_delete_builder, spanner_projects_instance_configs_operations_delete_task,
    spanner_projects_instance_configs_operations_get_builder, spanner_projects_instance_configs_operations_get_task,
    spanner_projects_instance_configs_operations_list_builder, spanner_projects_instance_configs_operations_list_task,
    spanner_projects_instance_configs_ssd_caches_operations_cancel_builder, spanner_projects_instance_configs_ssd_caches_operations_cancel_task,
    spanner_projects_instance_configs_ssd_caches_operations_delete_builder, spanner_projects_instance_configs_ssd_caches_operations_delete_task,
    spanner_projects_instance_configs_ssd_caches_operations_get_builder, spanner_projects_instance_configs_ssd_caches_operations_get_task,
    spanner_projects_instance_configs_ssd_caches_operations_list_builder, spanner_projects_instance_configs_ssd_caches_operations_list_task,
    spanner_projects_instances_create_builder, spanner_projects_instances_create_task,
    spanner_projects_instances_delete_builder, spanner_projects_instances_delete_task,
    spanner_projects_instances_get_builder, spanner_projects_instances_get_task,
    spanner_projects_instances_get_iam_policy_builder, spanner_projects_instances_get_iam_policy_task,
    spanner_projects_instances_list_builder, spanner_projects_instances_list_task,
    spanner_projects_instances_move_builder, spanner_projects_instances_move_task,
    spanner_projects_instances_patch_builder, spanner_projects_instances_patch_task,
    spanner_projects_instances_set_iam_policy_builder, spanner_projects_instances_set_iam_policy_task,
    spanner_projects_instances_test_iam_permissions_builder, spanner_projects_instances_test_iam_permissions_task,
    spanner_projects_instances_backup_operations_list_builder, spanner_projects_instances_backup_operations_list_task,
    spanner_projects_instances_backups_copy_builder, spanner_projects_instances_backups_copy_task,
    spanner_projects_instances_backups_create_builder, spanner_projects_instances_backups_create_task,
    spanner_projects_instances_backups_delete_builder, spanner_projects_instances_backups_delete_task,
    spanner_projects_instances_backups_get_builder, spanner_projects_instances_backups_get_task,
    spanner_projects_instances_backups_get_iam_policy_builder, spanner_projects_instances_backups_get_iam_policy_task,
    spanner_projects_instances_backups_list_builder, spanner_projects_instances_backups_list_task,
    spanner_projects_instances_backups_patch_builder, spanner_projects_instances_backups_patch_task,
    spanner_projects_instances_backups_set_iam_policy_builder, spanner_projects_instances_backups_set_iam_policy_task,
    spanner_projects_instances_backups_test_iam_permissions_builder, spanner_projects_instances_backups_test_iam_permissions_task,
    spanner_projects_instances_backups_operations_cancel_builder, spanner_projects_instances_backups_operations_cancel_task,
    spanner_projects_instances_backups_operations_delete_builder, spanner_projects_instances_backups_operations_delete_task,
    spanner_projects_instances_backups_operations_get_builder, spanner_projects_instances_backups_operations_get_task,
    spanner_projects_instances_backups_operations_list_builder, spanner_projects_instances_backups_operations_list_task,
    spanner_projects_instances_database_operations_list_builder, spanner_projects_instances_database_operations_list_task,
    spanner_projects_instances_databases_add_split_points_builder, spanner_projects_instances_databases_add_split_points_task,
    spanner_projects_instances_databases_changequorum_builder, spanner_projects_instances_databases_changequorum_task,
    spanner_projects_instances_databases_create_builder, spanner_projects_instances_databases_create_task,
    spanner_projects_instances_databases_drop_database_builder, spanner_projects_instances_databases_drop_database_task,
    spanner_projects_instances_databases_get_builder, spanner_projects_instances_databases_get_task,
    spanner_projects_instances_databases_get_ddl_builder, spanner_projects_instances_databases_get_ddl_task,
    spanner_projects_instances_databases_get_iam_policy_builder, spanner_projects_instances_databases_get_iam_policy_task,
    spanner_projects_instances_databases_get_scans_builder, spanner_projects_instances_databases_get_scans_task,
    spanner_projects_instances_databases_list_builder, spanner_projects_instances_databases_list_task,
    spanner_projects_instances_databases_patch_builder, spanner_projects_instances_databases_patch_task,
    spanner_projects_instances_databases_restore_builder, spanner_projects_instances_databases_restore_task,
    spanner_projects_instances_databases_set_iam_policy_builder, spanner_projects_instances_databases_set_iam_policy_task,
    spanner_projects_instances_databases_test_iam_permissions_builder, spanner_projects_instances_databases_test_iam_permissions_task,
    spanner_projects_instances_databases_update_ddl_builder, spanner_projects_instances_databases_update_ddl_task,
    spanner_projects_instances_databases_backup_schedules_create_builder, spanner_projects_instances_databases_backup_schedules_create_task,
    spanner_projects_instances_databases_backup_schedules_delete_builder, spanner_projects_instances_databases_backup_schedules_delete_task,
    spanner_projects_instances_databases_backup_schedules_get_builder, spanner_projects_instances_databases_backup_schedules_get_task,
    spanner_projects_instances_databases_backup_schedules_get_iam_policy_builder, spanner_projects_instances_databases_backup_schedules_get_iam_policy_task,
    spanner_projects_instances_databases_backup_schedules_list_builder, spanner_projects_instances_databases_backup_schedules_list_task,
    spanner_projects_instances_databases_backup_schedules_patch_builder, spanner_projects_instances_databases_backup_schedules_patch_task,
    spanner_projects_instances_databases_backup_schedules_set_iam_policy_builder, spanner_projects_instances_databases_backup_schedules_set_iam_policy_task,
    spanner_projects_instances_databases_backup_schedules_test_iam_permissions_builder, spanner_projects_instances_databases_backup_schedules_test_iam_permissions_task,
    spanner_projects_instances_databases_database_roles_list_builder, spanner_projects_instances_databases_database_roles_list_task,
    spanner_projects_instances_databases_database_roles_test_iam_permissions_builder, spanner_projects_instances_databases_database_roles_test_iam_permissions_task,
    spanner_projects_instances_databases_operations_cancel_builder, spanner_projects_instances_databases_operations_cancel_task,
    spanner_projects_instances_databases_operations_delete_builder, spanner_projects_instances_databases_operations_delete_task,
    spanner_projects_instances_databases_operations_get_builder, spanner_projects_instances_databases_operations_get_task,
    spanner_projects_instances_databases_operations_list_builder, spanner_projects_instances_databases_operations_list_task,
    spanner_projects_instances_databases_sessions_adapt_message_builder, spanner_projects_instances_databases_sessions_adapt_message_task,
    spanner_projects_instances_databases_sessions_adapter_builder, spanner_projects_instances_databases_sessions_adapter_task,
    spanner_projects_instances_databases_sessions_batch_create_builder, spanner_projects_instances_databases_sessions_batch_create_task,
    spanner_projects_instances_databases_sessions_batch_write_builder, spanner_projects_instances_databases_sessions_batch_write_task,
    spanner_projects_instances_databases_sessions_begin_transaction_builder, spanner_projects_instances_databases_sessions_begin_transaction_task,
    spanner_projects_instances_databases_sessions_commit_builder, spanner_projects_instances_databases_sessions_commit_task,
    spanner_projects_instances_databases_sessions_create_builder, spanner_projects_instances_databases_sessions_create_task,
    spanner_projects_instances_databases_sessions_delete_builder, spanner_projects_instances_databases_sessions_delete_task,
    spanner_projects_instances_databases_sessions_execute_batch_dml_builder, spanner_projects_instances_databases_sessions_execute_batch_dml_task,
    spanner_projects_instances_databases_sessions_execute_sql_builder, spanner_projects_instances_databases_sessions_execute_sql_task,
    spanner_projects_instances_databases_sessions_execute_streaming_sql_builder, spanner_projects_instances_databases_sessions_execute_streaming_sql_task,
    spanner_projects_instances_databases_sessions_get_builder, spanner_projects_instances_databases_sessions_get_task,
    spanner_projects_instances_databases_sessions_list_builder, spanner_projects_instances_databases_sessions_list_task,
    spanner_projects_instances_databases_sessions_partition_query_builder, spanner_projects_instances_databases_sessions_partition_query_task,
    spanner_projects_instances_databases_sessions_partition_read_builder, spanner_projects_instances_databases_sessions_partition_read_task,
    spanner_projects_instances_databases_sessions_read_builder, spanner_projects_instances_databases_sessions_read_task,
    spanner_projects_instances_databases_sessions_rollback_builder, spanner_projects_instances_databases_sessions_rollback_task,
    spanner_projects_instances_databases_sessions_streaming_read_builder, spanner_projects_instances_databases_sessions_streaming_read_task,
    spanner_projects_instances_instance_partition_operations_list_builder, spanner_projects_instances_instance_partition_operations_list_task,
    spanner_projects_instances_instance_partitions_create_builder, spanner_projects_instances_instance_partitions_create_task,
    spanner_projects_instances_instance_partitions_delete_builder, spanner_projects_instances_instance_partitions_delete_task,
    spanner_projects_instances_instance_partitions_get_builder, spanner_projects_instances_instance_partitions_get_task,
    spanner_projects_instances_instance_partitions_list_builder, spanner_projects_instances_instance_partitions_list_task,
    spanner_projects_instances_instance_partitions_patch_builder, spanner_projects_instances_instance_partitions_patch_task,
    spanner_projects_instances_instance_partitions_operations_cancel_builder, spanner_projects_instances_instance_partitions_operations_cancel_task,
    spanner_projects_instances_instance_partitions_operations_delete_builder, spanner_projects_instances_instance_partitions_operations_delete_task,
    spanner_projects_instances_instance_partitions_operations_get_builder, spanner_projects_instances_instance_partitions_operations_get_task,
    spanner_projects_instances_instance_partitions_operations_list_builder, spanner_projects_instances_instance_partitions_operations_list_task,
    spanner_projects_instances_operations_cancel_builder, spanner_projects_instances_operations_cancel_task,
    spanner_projects_instances_operations_delete_builder, spanner_projects_instances_operations_delete_task,
    spanner_projects_instances_operations_get_builder, spanner_projects_instances_operations_get_task,
    spanner_projects_instances_operations_list_builder, spanner_projects_instances_operations_list_task,
    spanner_scans_list_builder, spanner_scans_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::spanner::AdaptMessageResponse;
use crate::providers::gcp::clients::spanner::AdapterSession;
use crate::providers::gcp::clients::spanner::AddSplitPointsResponse;
use crate::providers::gcp::clients::spanner::Backup;
use crate::providers::gcp::clients::spanner::BackupSchedule;
use crate::providers::gcp::clients::spanner::BatchCreateSessionsResponse;
use crate::providers::gcp::clients::spanner::BatchWriteResponse;
use crate::providers::gcp::clients::spanner::CommitResponse;
use crate::providers::gcp::clients::spanner::Database;
use crate::providers::gcp::clients::spanner::Empty;
use crate::providers::gcp::clients::spanner::ExecuteBatchDmlResponse;
use crate::providers::gcp::clients::spanner::GetDatabaseDdlResponse;
use crate::providers::gcp::clients::spanner::Instance;
use crate::providers::gcp::clients::spanner::InstanceConfig;
use crate::providers::gcp::clients::spanner::InstancePartition;
use crate::providers::gcp::clients::spanner::ListBackupOperationsResponse;
use crate::providers::gcp::clients::spanner::ListBackupSchedulesResponse;
use crate::providers::gcp::clients::spanner::ListBackupsResponse;
use crate::providers::gcp::clients::spanner::ListDatabaseOperationsResponse;
use crate::providers::gcp::clients::spanner::ListDatabaseRolesResponse;
use crate::providers::gcp::clients::spanner::ListDatabasesResponse;
use crate::providers::gcp::clients::spanner::ListInstanceConfigOperationsResponse;
use crate::providers::gcp::clients::spanner::ListInstanceConfigsResponse;
use crate::providers::gcp::clients::spanner::ListInstancePartitionOperationsResponse;
use crate::providers::gcp::clients::spanner::ListInstancePartitionsResponse;
use crate::providers::gcp::clients::spanner::ListInstancesResponse;
use crate::providers::gcp::clients::spanner::ListOperationsResponse;
use crate::providers::gcp::clients::spanner::ListScansResponse;
use crate::providers::gcp::clients::spanner::ListSessionsResponse;
use crate::providers::gcp::clients::spanner::Operation;
use crate::providers::gcp::clients::spanner::PartialResultSet;
use crate::providers::gcp::clients::spanner::PartitionResponse;
use crate::providers::gcp::clients::spanner::Policy;
use crate::providers::gcp::clients::spanner::ResultSet;
use crate::providers::gcp::clients::spanner::Scan;
use crate::providers::gcp::clients::spanner::Session;
use crate::providers::gcp::clients::spanner::TestIamPermissionsResponse;
use crate::providers::gcp::clients::spanner::Transaction;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsOperationsCancelArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsOperationsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsOperationsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsPatchArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsSsdCachesOperationsCancelArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsSsdCachesOperationsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsSsdCachesOperationsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstanceConfigsSsdCachesOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsCopyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsGetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsOperationsCancelArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsOperationsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsOperationsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsPatchArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesBackupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabaseOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesAddSplitPointsArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesGetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesPatchArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesSetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesBackupSchedulesTestIamPermissionsArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesChangequorumArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesDatabaseRolesListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesDatabaseRolesTestIamPermissionsArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesDropDatabaseArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesGetDdlArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesGetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesGetScansArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesOperationsCancelArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesOperationsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesOperationsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesPatchArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesRestoreArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsAdaptMessageArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsAdapterArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsBatchCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsBatchWriteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsBeginTransactionArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsCommitArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsExecuteBatchDmlArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsExecuteSqlArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsExecuteStreamingSqlArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsPartitionQueryArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsPartitionReadArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsReadArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsRollbackArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSessionsStreamingReadArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesSetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesTestIamPermissionsArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDatabasesUpdateDdlArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesGetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsCreateArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsOperationsCancelArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsOperationsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsOperationsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesInstancePartitionsPatchArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesMoveArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesOperationsCancelArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesOperationsDeleteArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesOperationsGetArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesOperationsListArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesPatchArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesSetIamPolicyArgs;
use crate::providers::gcp::clients::spanner::SpannerProjectsInstancesTestIamPermissionsArgs;
use crate::providers::gcp::clients::spanner::SpannerScansListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SpannerProvider with automatic state tracking.
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
/// let provider = SpannerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SpannerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SpannerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SpannerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Spanner projects instance config operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstanceConfigOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instance_config_operations_list(
        &self,
        args: &SpannerProjectsInstanceConfigOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstanceConfigOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_config_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_config_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instance_configs_create(
        &self,
        args: &SpannerProjectsInstanceConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs delete.
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
    pub fn spanner_projects_instance_configs_delete(
        &self,
        args: &SpannerProjectsInstanceConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstanceConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instance_configs_get(
        &self,
        args: &SpannerProjectsInstanceConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstanceConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstanceConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instance_configs_list(
        &self,
        args: &SpannerProjectsInstanceConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstanceConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instance_configs_patch(
        &self,
        args: &SpannerProjectsInstanceConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs operations cancel.
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
    pub fn spanner_projects_instance_configs_operations_cancel(
        &self,
        args: &SpannerProjectsInstanceConfigsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs operations delete.
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
    pub fn spanner_projects_instance_configs_operations_delete(
        &self,
        args: &SpannerProjectsInstanceConfigsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs operations get.
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
    pub fn spanner_projects_instance_configs_operations_get(
        &self,
        args: &SpannerProjectsInstanceConfigsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs operations list.
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
    pub fn spanner_projects_instance_configs_operations_list(
        &self,
        args: &SpannerProjectsInstanceConfigsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs ssd caches operations cancel.
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
    pub fn spanner_projects_instance_configs_ssd_caches_operations_cancel(
        &self,
        args: &SpannerProjectsInstanceConfigsSsdCachesOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_ssd_caches_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_ssd_caches_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs ssd caches operations delete.
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
    pub fn spanner_projects_instance_configs_ssd_caches_operations_delete(
        &self,
        args: &SpannerProjectsInstanceConfigsSsdCachesOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_ssd_caches_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_ssd_caches_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs ssd caches operations get.
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
    pub fn spanner_projects_instance_configs_ssd_caches_operations_get(
        &self,
        args: &SpannerProjectsInstanceConfigsSsdCachesOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_ssd_caches_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_ssd_caches_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instance configs ssd caches operations list.
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
    pub fn spanner_projects_instance_configs_ssd_caches_operations_list(
        &self,
        args: &SpannerProjectsInstanceConfigsSsdCachesOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instance_configs_ssd_caches_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instance_configs_ssd_caches_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_create(
        &self,
        args: &SpannerProjectsInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances delete.
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
    pub fn spanner_projects_instances_delete(
        &self,
        args: &SpannerProjectsInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances get.
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
    pub fn spanner_projects_instances_get(
        &self,
        args: &SpannerProjectsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_get_builder(
            &self.http_client,
            &args.name,
            &args.fieldMask,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances get iam policy.
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
    pub fn spanner_projects_instances_get_iam_policy(
        &self,
        args: &SpannerProjectsInstancesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances list.
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
    pub fn spanner_projects_instances_list(
        &self,
        args: &SpannerProjectsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.instanceDeadline,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_move(
        &self,
        args: &SpannerProjectsInstancesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_patch(
        &self,
        args: &SpannerProjectsInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances set iam policy.
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
    pub fn spanner_projects_instances_set_iam_policy(
        &self,
        args: &SpannerProjectsInstancesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances test iam permissions.
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
    pub fn spanner_projects_instances_test_iam_permissions(
        &self,
        args: &SpannerProjectsInstancesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backup operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_backup_operations_list(
        &self,
        args: &SpannerProjectsInstancesBackupOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backup_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backup_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups copy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_backups_copy(
        &self,
        args: &SpannerProjectsInstancesBackupsCopyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_copy_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_copy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_backups_create(
        &self,
        args: &SpannerProjectsInstancesBackupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupId,
            &args.encryptionConfig.encryptionType,
            &args.encryptionConfig.kmsKeyName,
            &args.encryptionConfig.kmsKeyNames,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups delete.
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
    pub fn spanner_projects_instances_backups_delete(
        &self,
        args: &SpannerProjectsInstancesBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups get.
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
    pub fn spanner_projects_instances_backups_get(
        &self,
        args: &SpannerProjectsInstancesBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups get iam policy.
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
    pub fn spanner_projects_instances_backups_get_iam_policy(
        &self,
        args: &SpannerProjectsInstancesBackupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups list.
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
    pub fn spanner_projects_instances_backups_list(
        &self,
        args: &SpannerProjectsInstancesBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups patch.
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
    pub fn spanner_projects_instances_backups_patch(
        &self,
        args: &SpannerProjectsInstancesBackupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups set iam policy.
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
    pub fn spanner_projects_instances_backups_set_iam_policy(
        &self,
        args: &SpannerProjectsInstancesBackupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups test iam permissions.
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
    pub fn spanner_projects_instances_backups_test_iam_permissions(
        &self,
        args: &SpannerProjectsInstancesBackupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups operations cancel.
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
    pub fn spanner_projects_instances_backups_operations_cancel(
        &self,
        args: &SpannerProjectsInstancesBackupsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups operations delete.
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
    pub fn spanner_projects_instances_backups_operations_delete(
        &self,
        args: &SpannerProjectsInstancesBackupsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups operations get.
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
    pub fn spanner_projects_instances_backups_operations_get(
        &self,
        args: &SpannerProjectsInstancesBackupsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances backups operations list.
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
    pub fn spanner_projects_instances_backups_operations_list(
        &self,
        args: &SpannerProjectsInstancesBackupsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_backups_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_backups_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances database operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatabaseOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_database_operations_list(
        &self,
        args: &SpannerProjectsInstancesDatabaseOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatabaseOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_database_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_database_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases add split points.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AddSplitPointsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_add_split_points(
        &self,
        args: &SpannerProjectsInstancesDatabasesAddSplitPointsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AddSplitPointsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_add_split_points_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_add_split_points_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases changequorum.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_changequorum(
        &self,
        args: &SpannerProjectsInstancesDatabasesChangequorumArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_changequorum_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_changequorum_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_create(
        &self,
        args: &SpannerProjectsInstancesDatabasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases drop database.
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
    pub fn spanner_projects_instances_databases_drop_database(
        &self,
        args: &SpannerProjectsInstancesDatabasesDropDatabaseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_drop_database_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_drop_database_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases get.
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
    pub fn spanner_projects_instances_databases_get(
        &self,
        args: &SpannerProjectsInstancesDatabasesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Database, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases get ddl.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetDatabaseDdlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_get_ddl(
        &self,
        args: &SpannerProjectsInstancesDatabasesGetDdlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetDatabaseDdlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_get_ddl_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_get_ddl_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases get iam policy.
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
    pub fn spanner_projects_instances_databases_get_iam_policy(
        &self,
        args: &SpannerProjectsInstancesDatabasesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases get scans.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Scan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_get_scans(
        &self,
        args: &SpannerProjectsInstancesDatabasesGetScansArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Scan, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_get_scans_builder(
            &self.http_client,
            &args.name,
            &args.endTime,
            &args.startTime,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_get_scans_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases list.
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
    pub fn spanner_projects_instances_databases_list(
        &self,
        args: &SpannerProjectsInstancesDatabasesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatabasesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_patch(
        &self,
        args: &SpannerProjectsInstancesDatabasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases restore.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_restore(
        &self,
        args: &SpannerProjectsInstancesDatabasesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_restore_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases set iam policy.
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
    pub fn spanner_projects_instances_databases_set_iam_policy(
        &self,
        args: &SpannerProjectsInstancesDatabasesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases test iam permissions.
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
    pub fn spanner_projects_instances_databases_test_iam_permissions(
        &self,
        args: &SpannerProjectsInstancesDatabasesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases update ddl.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_update_ddl(
        &self,
        args: &SpannerProjectsInstancesDatabasesUpdateDdlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_update_ddl_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_update_ddl_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_backup_schedules_create(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupSchedule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupScheduleId,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules delete.
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
    pub fn spanner_projects_instances_databases_backup_schedules_delete(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_backup_schedules_get(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupSchedule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules get iam policy.
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
    pub fn spanner_projects_instances_databases_backup_schedules_get_iam_policy(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupSchedulesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_backup_schedules_list(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupSchedulesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupSchedule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_backup_schedules_patch(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupSchedule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules set iam policy.
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
    pub fn spanner_projects_instances_databases_backup_schedules_set_iam_policy(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases backup schedules test iam permissions.
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
    pub fn spanner_projects_instances_databases_backup_schedules_test_iam_permissions(
        &self,
        args: &SpannerProjectsInstancesDatabasesBackupSchedulesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_backup_schedules_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_backup_schedules_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases database roles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDatabaseRolesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_database_roles_list(
        &self,
        args: &SpannerProjectsInstancesDatabasesDatabaseRolesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDatabaseRolesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_database_roles_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_database_roles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases database roles test iam permissions.
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
    pub fn spanner_projects_instances_databases_database_roles_test_iam_permissions(
        &self,
        args: &SpannerProjectsInstancesDatabasesDatabaseRolesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_database_roles_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_database_roles_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases operations cancel.
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
    pub fn spanner_projects_instances_databases_operations_cancel(
        &self,
        args: &SpannerProjectsInstancesDatabasesOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases operations delete.
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
    pub fn spanner_projects_instances_databases_operations_delete(
        &self,
        args: &SpannerProjectsInstancesDatabasesOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases operations get.
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
    pub fn spanner_projects_instances_databases_operations_get(
        &self,
        args: &SpannerProjectsInstancesDatabasesOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases operations list.
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
    pub fn spanner_projects_instances_databases_operations_list(
        &self,
        args: &SpannerProjectsInstancesDatabasesOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions adapt message.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdaptMessageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_adapt_message(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsAdaptMessageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdaptMessageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_adapt_message_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_adapt_message_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions adapter.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AdapterSession result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_adapter(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsAdapterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AdapterSession, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_adapter_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_adapter_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions batch create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchCreateSessionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_batch_create(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsBatchCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchCreateSessionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_batch_create_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_batch_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions batch write.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchWriteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_batch_write(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsBatchWriteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchWriteResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_batch_write_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_batch_write_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions begin transaction.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Transaction result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_begin_transaction(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsBeginTransactionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Transaction, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_begin_transaction_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_begin_transaction_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions commit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CommitResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_commit(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsCommitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CommitResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_commit_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_commit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Session result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_create(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Session, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_create_builder(
            &self.http_client,
            &args.database,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions delete.
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
    pub fn spanner_projects_instances_databases_sessions_delete(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions execute batch dml.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExecuteBatchDmlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_execute_batch_dml(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsExecuteBatchDmlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExecuteBatchDmlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_execute_batch_dml_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_execute_batch_dml_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions execute sql.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResultSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_execute_sql(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsExecuteSqlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResultSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_execute_sql_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_execute_sql_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions execute streaming sql.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PartialResultSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_execute_streaming_sql(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsExecuteStreamingSqlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PartialResultSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_execute_streaming_sql_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_execute_streaming_sql_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Session result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_sessions_get(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Session, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSessionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_sessions_list(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSessionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_list_builder(
            &self.http_client,
            &args.database,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions partition query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PartitionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_databases_sessions_partition_query(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsPartitionQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PartitionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_partition_query_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_partition_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions partition read.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PartitionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_partition_read(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsPartitionReadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PartitionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_partition_read_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_partition_read_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions read.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResultSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_read(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsReadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResultSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_read_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_read_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions rollback.
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
    pub fn spanner_projects_instances_databases_sessions_rollback(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsRollbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_rollback_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_rollback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances databases sessions streaming read.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PartialResultSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_databases_sessions_streaming_read(
        &self,
        args: &SpannerProjectsInstancesDatabasesSessionsStreamingReadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PartialResultSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_databases_sessions_streaming_read_builder(
            &self.http_client,
            &args.session,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_databases_sessions_streaming_read_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partition operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstancePartitionOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_instance_partition_operations_list(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancePartitionOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partition_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.instancePartitionDeadline,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partition_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_instance_partitions_create(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions delete.
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
    pub fn spanner_projects_instances_instance_partitions_delete(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InstancePartition result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_instance_partitions_get(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InstancePartition, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListInstancePartitionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_projects_instances_instance_partitions_list(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancePartitionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_list_builder(
            &self.http_client,
            &args.parent,
            &args.instancePartitionDeadline,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn spanner_projects_instances_instance_partitions_patch(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions operations cancel.
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
    pub fn spanner_projects_instances_instance_partitions_operations_cancel(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions operations delete.
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
    pub fn spanner_projects_instances_instance_partitions_operations_delete(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions operations get.
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
    pub fn spanner_projects_instances_instance_partitions_operations_get(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances instance partitions operations list.
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
    pub fn spanner_projects_instances_instance_partitions_operations_list(
        &self,
        args: &SpannerProjectsInstancesInstancePartitionsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_instance_partitions_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_instance_partitions_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances operations cancel.
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
    pub fn spanner_projects_instances_operations_cancel(
        &self,
        args: &SpannerProjectsInstancesOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances operations delete.
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
    pub fn spanner_projects_instances_operations_delete(
        &self,
        args: &SpannerProjectsInstancesOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances operations get.
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
    pub fn spanner_projects_instances_operations_get(
        &self,
        args: &SpannerProjectsInstancesOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner projects instances operations list.
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
    pub fn spanner_projects_instances_operations_list(
        &self,
        args: &SpannerProjectsInstancesOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_projects_instances_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_projects_instances_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Spanner scans list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListScansResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn spanner_scans_list(
        &self,
        args: &SpannerScansListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListScansResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = spanner_scans_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = spanner_scans_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
