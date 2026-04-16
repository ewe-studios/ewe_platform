//! GkebackupProvider - State-aware gkebackup API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       gkebackup API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::gkebackup::{
    gkebackup_projects_locations_get_builder, gkebackup_projects_locations_get_task,
    gkebackup_projects_locations_list_builder, gkebackup_projects_locations_list_task,
    gkebackup_projects_locations_backup_channels_create_builder, gkebackup_projects_locations_backup_channels_create_task,
    gkebackup_projects_locations_backup_channels_delete_builder, gkebackup_projects_locations_backup_channels_delete_task,
    gkebackup_projects_locations_backup_channels_get_builder, gkebackup_projects_locations_backup_channels_get_task,
    gkebackup_projects_locations_backup_channels_list_builder, gkebackup_projects_locations_backup_channels_list_task,
    gkebackup_projects_locations_backup_channels_patch_builder, gkebackup_projects_locations_backup_channels_patch_task,
    gkebackup_projects_locations_backup_channels_backup_plan_bindings_get_builder, gkebackup_projects_locations_backup_channels_backup_plan_bindings_get_task,
    gkebackup_projects_locations_backup_channels_backup_plan_bindings_list_builder, gkebackup_projects_locations_backup_channels_backup_plan_bindings_list_task,
    gkebackup_projects_locations_backup_plans_create_builder, gkebackup_projects_locations_backup_plans_create_task,
    gkebackup_projects_locations_backup_plans_delete_builder, gkebackup_projects_locations_backup_plans_delete_task,
    gkebackup_projects_locations_backup_plans_get_builder, gkebackup_projects_locations_backup_plans_get_task,
    gkebackup_projects_locations_backup_plans_get_iam_policy_builder, gkebackup_projects_locations_backup_plans_get_iam_policy_task,
    gkebackup_projects_locations_backup_plans_get_tags_builder, gkebackup_projects_locations_backup_plans_get_tags_task,
    gkebackup_projects_locations_backup_plans_list_builder, gkebackup_projects_locations_backup_plans_list_task,
    gkebackup_projects_locations_backup_plans_patch_builder, gkebackup_projects_locations_backup_plans_patch_task,
    gkebackup_projects_locations_backup_plans_set_iam_policy_builder, gkebackup_projects_locations_backup_plans_set_iam_policy_task,
    gkebackup_projects_locations_backup_plans_set_tags_builder, gkebackup_projects_locations_backup_plans_set_tags_task,
    gkebackup_projects_locations_backup_plans_test_iam_permissions_builder, gkebackup_projects_locations_backup_plans_test_iam_permissions_task,
    gkebackup_projects_locations_backup_plans_backups_create_builder, gkebackup_projects_locations_backup_plans_backups_create_task,
    gkebackup_projects_locations_backup_plans_backups_delete_builder, gkebackup_projects_locations_backup_plans_backups_delete_task,
    gkebackup_projects_locations_backup_plans_backups_get_builder, gkebackup_projects_locations_backup_plans_backups_get_task,
    gkebackup_projects_locations_backup_plans_backups_get_backup_index_download_url_builder, gkebackup_projects_locations_backup_plans_backups_get_backup_index_download_url_task,
    gkebackup_projects_locations_backup_plans_backups_get_iam_policy_builder, gkebackup_projects_locations_backup_plans_backups_get_iam_policy_task,
    gkebackup_projects_locations_backup_plans_backups_list_builder, gkebackup_projects_locations_backup_plans_backups_list_task,
    gkebackup_projects_locations_backup_plans_backups_patch_builder, gkebackup_projects_locations_backup_plans_backups_patch_task,
    gkebackup_projects_locations_backup_plans_backups_set_iam_policy_builder, gkebackup_projects_locations_backup_plans_backups_set_iam_policy_task,
    gkebackup_projects_locations_backup_plans_backups_test_iam_permissions_builder, gkebackup_projects_locations_backup_plans_backups_test_iam_permissions_task,
    gkebackup_projects_locations_backup_plans_backups_volume_backups_get_builder, gkebackup_projects_locations_backup_plans_backups_volume_backups_get_task,
    gkebackup_projects_locations_backup_plans_backups_volume_backups_get_iam_policy_builder, gkebackup_projects_locations_backup_plans_backups_volume_backups_get_iam_policy_task,
    gkebackup_projects_locations_backup_plans_backups_volume_backups_list_builder, gkebackup_projects_locations_backup_plans_backups_volume_backups_list_task,
    gkebackup_projects_locations_backup_plans_backups_volume_backups_set_iam_policy_builder, gkebackup_projects_locations_backup_plans_backups_volume_backups_set_iam_policy_task,
    gkebackup_projects_locations_backup_plans_backups_volume_backups_test_iam_permissions_builder, gkebackup_projects_locations_backup_plans_backups_volume_backups_test_iam_permissions_task,
    gkebackup_projects_locations_operations_cancel_builder, gkebackup_projects_locations_operations_cancel_task,
    gkebackup_projects_locations_operations_delete_builder, gkebackup_projects_locations_operations_delete_task,
    gkebackup_projects_locations_operations_get_builder, gkebackup_projects_locations_operations_get_task,
    gkebackup_projects_locations_operations_list_builder, gkebackup_projects_locations_operations_list_task,
    gkebackup_projects_locations_restore_channels_create_builder, gkebackup_projects_locations_restore_channels_create_task,
    gkebackup_projects_locations_restore_channels_delete_builder, gkebackup_projects_locations_restore_channels_delete_task,
    gkebackup_projects_locations_restore_channels_get_builder, gkebackup_projects_locations_restore_channels_get_task,
    gkebackup_projects_locations_restore_channels_list_builder, gkebackup_projects_locations_restore_channels_list_task,
    gkebackup_projects_locations_restore_channels_patch_builder, gkebackup_projects_locations_restore_channels_patch_task,
    gkebackup_projects_locations_restore_channels_restore_plan_bindings_get_builder, gkebackup_projects_locations_restore_channels_restore_plan_bindings_get_task,
    gkebackup_projects_locations_restore_channels_restore_plan_bindings_list_builder, gkebackup_projects_locations_restore_channels_restore_plan_bindings_list_task,
    gkebackup_projects_locations_restore_plans_create_builder, gkebackup_projects_locations_restore_plans_create_task,
    gkebackup_projects_locations_restore_plans_delete_builder, gkebackup_projects_locations_restore_plans_delete_task,
    gkebackup_projects_locations_restore_plans_get_builder, gkebackup_projects_locations_restore_plans_get_task,
    gkebackup_projects_locations_restore_plans_get_iam_policy_builder, gkebackup_projects_locations_restore_plans_get_iam_policy_task,
    gkebackup_projects_locations_restore_plans_get_tags_builder, gkebackup_projects_locations_restore_plans_get_tags_task,
    gkebackup_projects_locations_restore_plans_list_builder, gkebackup_projects_locations_restore_plans_list_task,
    gkebackup_projects_locations_restore_plans_patch_builder, gkebackup_projects_locations_restore_plans_patch_task,
    gkebackup_projects_locations_restore_plans_set_iam_policy_builder, gkebackup_projects_locations_restore_plans_set_iam_policy_task,
    gkebackup_projects_locations_restore_plans_set_tags_builder, gkebackup_projects_locations_restore_plans_set_tags_task,
    gkebackup_projects_locations_restore_plans_test_iam_permissions_builder, gkebackup_projects_locations_restore_plans_test_iam_permissions_task,
    gkebackup_projects_locations_restore_plans_restores_create_builder, gkebackup_projects_locations_restore_plans_restores_create_task,
    gkebackup_projects_locations_restore_plans_restores_delete_builder, gkebackup_projects_locations_restore_plans_restores_delete_task,
    gkebackup_projects_locations_restore_plans_restores_get_builder, gkebackup_projects_locations_restore_plans_restores_get_task,
    gkebackup_projects_locations_restore_plans_restores_get_iam_policy_builder, gkebackup_projects_locations_restore_plans_restores_get_iam_policy_task,
    gkebackup_projects_locations_restore_plans_restores_list_builder, gkebackup_projects_locations_restore_plans_restores_list_task,
    gkebackup_projects_locations_restore_plans_restores_patch_builder, gkebackup_projects_locations_restore_plans_restores_patch_task,
    gkebackup_projects_locations_restore_plans_restores_set_iam_policy_builder, gkebackup_projects_locations_restore_plans_restores_set_iam_policy_task,
    gkebackup_projects_locations_restore_plans_restores_test_iam_permissions_builder, gkebackup_projects_locations_restore_plans_restores_test_iam_permissions_task,
    gkebackup_projects_locations_restore_plans_restores_volume_restores_get_builder, gkebackup_projects_locations_restore_plans_restores_volume_restores_get_task,
    gkebackup_projects_locations_restore_plans_restores_volume_restores_get_iam_policy_builder, gkebackup_projects_locations_restore_plans_restores_volume_restores_get_iam_policy_task,
    gkebackup_projects_locations_restore_plans_restores_volume_restores_list_builder, gkebackup_projects_locations_restore_plans_restores_volume_restores_list_task,
    gkebackup_projects_locations_restore_plans_restores_volume_restores_set_iam_policy_builder, gkebackup_projects_locations_restore_plans_restores_volume_restores_set_iam_policy_task,
    gkebackup_projects_locations_restore_plans_restores_volume_restores_test_iam_permissions_builder, gkebackup_projects_locations_restore_plans_restores_volume_restores_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gkebackup::Backup;
use crate::providers::gcp::clients::gkebackup::BackupChannel;
use crate::providers::gcp::clients::gkebackup::BackupPlan;
use crate::providers::gcp::clients::gkebackup::BackupPlanBinding;
use crate::providers::gcp::clients::gkebackup::Empty;
use crate::providers::gcp::clients::gkebackup::GetBackupIndexDownloadUrlResponse;
use crate::providers::gcp::clients::gkebackup::GetTagsResponse;
use crate::providers::gcp::clients::gkebackup::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::gkebackup::GoogleLongrunningOperation;
use crate::providers::gcp::clients::gkebackup::ListBackupChannelsResponse;
use crate::providers::gcp::clients::gkebackup::ListBackupPlanBindingsResponse;
use crate::providers::gcp::clients::gkebackup::ListBackupPlansResponse;
use crate::providers::gcp::clients::gkebackup::ListBackupsResponse;
use crate::providers::gcp::clients::gkebackup::ListLocationsResponse;
use crate::providers::gcp::clients::gkebackup::ListRestoreChannelsResponse;
use crate::providers::gcp::clients::gkebackup::ListRestorePlanBindingsResponse;
use crate::providers::gcp::clients::gkebackup::ListRestorePlansResponse;
use crate::providers::gcp::clients::gkebackup::ListRestoresResponse;
use crate::providers::gcp::clients::gkebackup::ListVolumeBackupsResponse;
use crate::providers::gcp::clients::gkebackup::ListVolumeRestoresResponse;
use crate::providers::gcp::clients::gkebackup::Location;
use crate::providers::gcp::clients::gkebackup::Policy;
use crate::providers::gcp::clients::gkebackup::Restore;
use crate::providers::gcp::clients::gkebackup::RestoreChannel;
use crate::providers::gcp::clients::gkebackup::RestorePlan;
use crate::providers::gcp::clients::gkebackup::RestorePlanBinding;
use crate::providers::gcp::clients::gkebackup::SetTagsResponse;
use crate::providers::gcp::clients::gkebackup::TestIamPermissionsResponse;
use crate::providers::gcp::clients::gkebackup::VolumeBackup;
use crate::providers::gcp::clients::gkebackup::VolumeRestore;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsBackupPlanBindingsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsBackupPlanBindingsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsGetBackupIndexDownloadUrlArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsGetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsGetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansGetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansGetTagsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansSetTagsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsRestorePlanBindingsGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsRestorePlanBindingsListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansGetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansGetTagsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresGetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresGetArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresGetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresListArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansSetTagsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GkebackupProvider with automatic state tracking.
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
/// let provider = GkebackupProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct GkebackupProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> GkebackupProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new GkebackupProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new GkebackupProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Gkebackup projects locations get.
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
    pub fn gkebackup_projects_locations_get(
        &self,
        args: &GkebackupProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations list.
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
    pub fn gkebackup_projects_locations_list(
        &self,
        args: &GkebackupProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup channels create.
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
    pub fn gkebackup_projects_locations_backup_channels_create(
        &self,
        args: &GkebackupProjectsLocationsBackupChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_channels_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupChannelId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup channels delete.
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
    pub fn gkebackup_projects_locations_backup_channels_delete(
        &self,
        args: &GkebackupProjectsLocationsBackupChannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_channels_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_channels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup channels get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_channels_get(
        &self,
        args: &GkebackupProjectsLocationsBackupChannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_channels_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_channels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_channels_list(
        &self,
        args: &GkebackupProjectsLocationsBackupChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_channels_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup channels patch.
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
    pub fn gkebackup_projects_locations_backup_channels_patch(
        &self,
        args: &GkebackupProjectsLocationsBackupChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_channels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup channels backup plan bindings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupPlanBinding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_channels_backup_plan_bindings_get(
        &self,
        args: &GkebackupProjectsLocationsBackupChannelsBackupPlanBindingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupPlanBinding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_channels_backup_plan_bindings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_channels_backup_plan_bindings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup channels backup plan bindings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupPlanBindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_channels_backup_plan_bindings_list(
        &self,
        args: &GkebackupProjectsLocationsBackupChannelsBackupPlanBindingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupPlanBindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_channels_backup_plan_bindings_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_channels_backup_plan_bindings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans create.
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
    pub fn gkebackup_projects_locations_backup_plans_create(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupPlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans delete.
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
    pub fn gkebackup_projects_locations_backup_plans_delete(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupPlan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_plans_get(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupPlan, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans get iam policy.
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
    pub fn gkebackup_projects_locations_backup_plans_get_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans get tags.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_plans_get_tags(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansGetTagsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_get_tags_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_get_tags_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupPlansResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_plans_list(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupPlansResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans patch.
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
    pub fn gkebackup_projects_locations_backup_plans_patch(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans set iam policy.
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
    pub fn gkebackup_projects_locations_backup_plans_set_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans set tags.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkebackup_projects_locations_backup_plans_set_tags(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansSetTagsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_set_tags_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_set_tags_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans test iam permissions.
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
    pub fn gkebackup_projects_locations_backup_plans_test_iam_permissions(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups create.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_create(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups delete.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_delete(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups get.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_get(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups get backup index download url.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetBackupIndexDownloadUrlResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_plans_backups_get_backup_index_download_url(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsGetBackupIndexDownloadUrlArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetBackupIndexDownloadUrlResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_get_backup_index_download_url_builder(
            &self.http_client,
            &args.backup,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_get_backup_index_download_url_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups get iam policy.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_get_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups list.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_list(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups patch.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_patch(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups set iam policy.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_set_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups test iam permissions.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_test_iam_permissions(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups volume backups get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VolumeBackup result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_plans_backups_volume_backups_get(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VolumeBackup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_volume_backups_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_volume_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups volume backups get iam policy.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_volume_backups_get_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_volume_backups_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_volume_backups_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups volume backups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVolumeBackupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_backup_plans_backups_volume_backups_list(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVolumeBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_volume_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_volume_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups volume backups set iam policy.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_volume_backups_set_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_volume_backups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_volume_backups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations backup plans backups volume backups test iam permissions.
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
    pub fn gkebackup_projects_locations_backup_plans_backups_volume_backups_test_iam_permissions(
        &self,
        args: &GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_backup_plans_backups_volume_backups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_backup_plans_backups_volume_backups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations operations cancel.
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
    pub fn gkebackup_projects_locations_operations_cancel(
        &self,
        args: &GkebackupProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations operations delete.
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
    pub fn gkebackup_projects_locations_operations_delete(
        &self,
        args: &GkebackupProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations operations get.
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
    pub fn gkebackup_projects_locations_operations_get(
        &self,
        args: &GkebackupProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations operations list.
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
    pub fn gkebackup_projects_locations_operations_list(
        &self,
        args: &GkebackupProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore channels create.
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
    pub fn gkebackup_projects_locations_restore_channels_create(
        &self,
        args: &GkebackupProjectsLocationsRestoreChannelsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_channels_create_builder(
            &self.http_client,
            &args.parent,
            &args.restoreChannelId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_channels_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore channels delete.
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
    pub fn gkebackup_projects_locations_restore_channels_delete(
        &self,
        args: &GkebackupProjectsLocationsRestoreChannelsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_channels_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_channels_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore channels get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RestoreChannel result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_channels_get(
        &self,
        args: &GkebackupProjectsLocationsRestoreChannelsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RestoreChannel, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_channels_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_channels_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore channels list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRestoreChannelsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_channels_list(
        &self,
        args: &GkebackupProjectsLocationsRestoreChannelsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRestoreChannelsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_channels_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_channels_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore channels patch.
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
    pub fn gkebackup_projects_locations_restore_channels_patch(
        &self,
        args: &GkebackupProjectsLocationsRestoreChannelsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_channels_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_channels_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore channels restore plan bindings get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RestorePlanBinding result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_channels_restore_plan_bindings_get(
        &self,
        args: &GkebackupProjectsLocationsRestoreChannelsRestorePlanBindingsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RestorePlanBinding, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_channels_restore_plan_bindings_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_channels_restore_plan_bindings_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore channels restore plan bindings list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRestorePlanBindingsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_channels_restore_plan_bindings_list(
        &self,
        args: &GkebackupProjectsLocationsRestoreChannelsRestorePlanBindingsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRestorePlanBindingsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_channels_restore_plan_bindings_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_channels_restore_plan_bindings_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans create.
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
    pub fn gkebackup_projects_locations_restore_plans_create(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_create_builder(
            &self.http_client,
            &args.parent,
            &args.restorePlanId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans delete.
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
    pub fn gkebackup_projects_locations_restore_plans_delete(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RestorePlan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_plans_get(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RestorePlan, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans get iam policy.
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
    pub fn gkebackup_projects_locations_restore_plans_get_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans get tags.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_plans_get_tags(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansGetTagsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_get_tags_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_get_tags_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRestorePlansResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_plans_list(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRestorePlansResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans patch.
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
    pub fn gkebackup_projects_locations_restore_plans_patch(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans set iam policy.
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
    pub fn gkebackup_projects_locations_restore_plans_set_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans set tags.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SetTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn gkebackup_projects_locations_restore_plans_set_tags(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansSetTagsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SetTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_set_tags_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_set_tags_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans test iam permissions.
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
    pub fn gkebackup_projects_locations_restore_plans_test_iam_permissions(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores create.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_create(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_create_builder(
            &self.http_client,
            &args.parent,
            &args.restoreId,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores delete.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_delete(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Restore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_plans_restores_get(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Restore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores get iam policy.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_get_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRestoresResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_plans_restores_list(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRestoresResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores patch.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_patch(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores set iam policy.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_set_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores test iam permissions.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_test_iam_permissions(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores volume restores get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VolumeRestore result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_plans_restores_volume_restores_get(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VolumeRestore, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_volume_restores_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_volume_restores_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores volume restores get iam policy.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_volume_restores_get_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_volume_restores_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_volume_restores_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores volume restores list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVolumeRestoresResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn gkebackup_projects_locations_restore_plans_restores_volume_restores_list(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVolumeRestoresResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_volume_restores_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_volume_restores_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores volume restores set iam policy.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_volume_restores_set_iam_policy(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_volume_restores_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_volume_restores_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Gkebackup projects locations restore plans restores volume restores test iam permissions.
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
    pub fn gkebackup_projects_locations_restore_plans_restores_volume_restores_test_iam_permissions(
        &self,
        args: &GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = gkebackup_projects_locations_restore_plans_restores_volume_restores_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = gkebackup_projects_locations_restore_plans_restores_volume_restores_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
