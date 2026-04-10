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
    gkebackup_projects_locations_backup_channels_create_builder, gkebackup_projects_locations_backup_channels_create_task,
    gkebackup_projects_locations_backup_channels_delete_builder, gkebackup_projects_locations_backup_channels_delete_task,
    gkebackup_projects_locations_backup_channels_patch_builder, gkebackup_projects_locations_backup_channels_patch_task,
    gkebackup_projects_locations_backup_plans_create_builder, gkebackup_projects_locations_backup_plans_create_task,
    gkebackup_projects_locations_backup_plans_delete_builder, gkebackup_projects_locations_backup_plans_delete_task,
    gkebackup_projects_locations_backup_plans_patch_builder, gkebackup_projects_locations_backup_plans_patch_task,
    gkebackup_projects_locations_backup_plans_set_iam_policy_builder, gkebackup_projects_locations_backup_plans_set_iam_policy_task,
    gkebackup_projects_locations_backup_plans_set_tags_builder, gkebackup_projects_locations_backup_plans_set_tags_task,
    gkebackup_projects_locations_backup_plans_test_iam_permissions_builder, gkebackup_projects_locations_backup_plans_test_iam_permissions_task,
    gkebackup_projects_locations_backup_plans_backups_create_builder, gkebackup_projects_locations_backup_plans_backups_create_task,
    gkebackup_projects_locations_backup_plans_backups_delete_builder, gkebackup_projects_locations_backup_plans_backups_delete_task,
    gkebackup_projects_locations_backup_plans_backups_patch_builder, gkebackup_projects_locations_backup_plans_backups_patch_task,
    gkebackup_projects_locations_backup_plans_backups_set_iam_policy_builder, gkebackup_projects_locations_backup_plans_backups_set_iam_policy_task,
    gkebackup_projects_locations_backup_plans_backups_test_iam_permissions_builder, gkebackup_projects_locations_backup_plans_backups_test_iam_permissions_task,
    gkebackup_projects_locations_backup_plans_backups_volume_backups_set_iam_policy_builder, gkebackup_projects_locations_backup_plans_backups_volume_backups_set_iam_policy_task,
    gkebackup_projects_locations_backup_plans_backups_volume_backups_test_iam_permissions_builder, gkebackup_projects_locations_backup_plans_backups_volume_backups_test_iam_permissions_task,
    gkebackup_projects_locations_operations_cancel_builder, gkebackup_projects_locations_operations_cancel_task,
    gkebackup_projects_locations_operations_delete_builder, gkebackup_projects_locations_operations_delete_task,
    gkebackup_projects_locations_restore_channels_create_builder, gkebackup_projects_locations_restore_channels_create_task,
    gkebackup_projects_locations_restore_channels_delete_builder, gkebackup_projects_locations_restore_channels_delete_task,
    gkebackup_projects_locations_restore_channels_patch_builder, gkebackup_projects_locations_restore_channels_patch_task,
    gkebackup_projects_locations_restore_plans_create_builder, gkebackup_projects_locations_restore_plans_create_task,
    gkebackup_projects_locations_restore_plans_delete_builder, gkebackup_projects_locations_restore_plans_delete_task,
    gkebackup_projects_locations_restore_plans_patch_builder, gkebackup_projects_locations_restore_plans_patch_task,
    gkebackup_projects_locations_restore_plans_set_iam_policy_builder, gkebackup_projects_locations_restore_plans_set_iam_policy_task,
    gkebackup_projects_locations_restore_plans_set_tags_builder, gkebackup_projects_locations_restore_plans_set_tags_task,
    gkebackup_projects_locations_restore_plans_test_iam_permissions_builder, gkebackup_projects_locations_restore_plans_test_iam_permissions_task,
    gkebackup_projects_locations_restore_plans_restores_create_builder, gkebackup_projects_locations_restore_plans_restores_create_task,
    gkebackup_projects_locations_restore_plans_restores_delete_builder, gkebackup_projects_locations_restore_plans_restores_delete_task,
    gkebackup_projects_locations_restore_plans_restores_patch_builder, gkebackup_projects_locations_restore_plans_restores_patch_task,
    gkebackup_projects_locations_restore_plans_restores_set_iam_policy_builder, gkebackup_projects_locations_restore_plans_restores_set_iam_policy_task,
    gkebackup_projects_locations_restore_plans_restores_test_iam_permissions_builder, gkebackup_projects_locations_restore_plans_restores_test_iam_permissions_task,
    gkebackup_projects_locations_restore_plans_restores_volume_restores_set_iam_policy_builder, gkebackup_projects_locations_restore_plans_restores_volume_restores_set_iam_policy_task,
    gkebackup_projects_locations_restore_plans_restores_volume_restores_test_iam_permissions_builder, gkebackup_projects_locations_restore_plans_restores_volume_restores_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::gkebackup::Empty;
use crate::providers::gcp::clients::gkebackup::GoogleLongrunningOperation;
use crate::providers::gcp::clients::gkebackup::Policy;
use crate::providers::gcp::clients::gkebackup::SetTagsResponse;
use crate::providers::gcp::clients::gkebackup::TestIamPermissionsResponse;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupChannelsPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansBackupsVolumeBackupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansSetTagsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsBackupPlansTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestoreChannelsPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresCreateArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresDeleteArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresPatchArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansRestoresVolumeRestoresTestIamPermissionsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansSetIamPolicyArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansSetTagsArgs;
use crate::providers::gcp::clients::gkebackup::GkebackupProjectsLocationsRestorePlansTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// GkebackupProvider with automatic state tracking.
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
/// let provider = GkebackupProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct GkebackupProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> GkebackupProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new GkebackupProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
