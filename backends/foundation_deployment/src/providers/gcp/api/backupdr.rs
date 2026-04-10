//! BackupdrProvider - State-aware backupdr API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       backupdr API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::backupdr::{
    backupdr_projects_locations_backup_plan_associations_create_builder, backupdr_projects_locations_backup_plan_associations_create_task,
    backupdr_projects_locations_backup_plan_associations_delete_builder, backupdr_projects_locations_backup_plan_associations_delete_task,
    backupdr_projects_locations_backup_plan_associations_patch_builder, backupdr_projects_locations_backup_plan_associations_patch_task,
    backupdr_projects_locations_backup_plan_associations_trigger_backup_builder, backupdr_projects_locations_backup_plan_associations_trigger_backup_task,
    backupdr_projects_locations_backup_plans_create_builder, backupdr_projects_locations_backup_plans_create_task,
    backupdr_projects_locations_backup_plans_delete_builder, backupdr_projects_locations_backup_plans_delete_task,
    backupdr_projects_locations_backup_plans_patch_builder, backupdr_projects_locations_backup_plans_patch_task,
    backupdr_projects_locations_backup_vaults_create_builder, backupdr_projects_locations_backup_vaults_create_task,
    backupdr_projects_locations_backup_vaults_delete_builder, backupdr_projects_locations_backup_vaults_delete_task,
    backupdr_projects_locations_backup_vaults_patch_builder, backupdr_projects_locations_backup_vaults_patch_task,
    backupdr_projects_locations_backup_vaults_test_iam_permissions_builder, backupdr_projects_locations_backup_vaults_test_iam_permissions_task,
    backupdr_projects_locations_backup_vaults_data_sources_abandon_backup_builder, backupdr_projects_locations_backup_vaults_data_sources_abandon_backup_task,
    backupdr_projects_locations_backup_vaults_data_sources_fetch_access_token_builder, backupdr_projects_locations_backup_vaults_data_sources_fetch_access_token_task,
    backupdr_projects_locations_backup_vaults_data_sources_finalize_backup_builder, backupdr_projects_locations_backup_vaults_data_sources_finalize_backup_task,
    backupdr_projects_locations_backup_vaults_data_sources_initiate_backup_builder, backupdr_projects_locations_backup_vaults_data_sources_initiate_backup_task,
    backupdr_projects_locations_backup_vaults_data_sources_patch_builder, backupdr_projects_locations_backup_vaults_data_sources_patch_task,
    backupdr_projects_locations_backup_vaults_data_sources_remove_builder, backupdr_projects_locations_backup_vaults_data_sources_remove_task,
    backupdr_projects_locations_backup_vaults_data_sources_set_internal_status_builder, backupdr_projects_locations_backup_vaults_data_sources_set_internal_status_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_delete_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_delete_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_patch_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_patch_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_restore_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_restore_task,
    backupdr_projects_locations_management_servers_create_builder, backupdr_projects_locations_management_servers_create_task,
    backupdr_projects_locations_management_servers_delete_builder, backupdr_projects_locations_management_servers_delete_task,
    backupdr_projects_locations_management_servers_ms_compliance_metadata_builder, backupdr_projects_locations_management_servers_ms_compliance_metadata_task,
    backupdr_projects_locations_management_servers_set_iam_policy_builder, backupdr_projects_locations_management_servers_set_iam_policy_task,
    backupdr_projects_locations_management_servers_test_iam_permissions_builder, backupdr_projects_locations_management_servers_test_iam_permissions_task,
    backupdr_projects_locations_operations_cancel_builder, backupdr_projects_locations_operations_cancel_task,
    backupdr_projects_locations_operations_delete_builder, backupdr_projects_locations_operations_delete_task,
    backupdr_projects_locations_service_config_initialize_builder, backupdr_projects_locations_service_config_initialize_task,
    backupdr_projects_locations_trial_end_builder, backupdr_projects_locations_trial_end_task,
    backupdr_projects_locations_trial_subscribe_builder, backupdr_projects_locations_trial_subscribe_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::backupdr::Empty;
use crate::providers::gcp::clients::backupdr::FetchAccessTokenResponse;
use crate::providers::gcp::clients::backupdr::FetchMsComplianceMetadataResponse;
use crate::providers::gcp::clients::backupdr::InitiateBackupResponse;
use crate::providers::gcp::clients::backupdr::Operation;
use crate::providers::gcp::clients::backupdr::Policy;
use crate::providers::gcp::clients::backupdr::TestIamPermissionsResponse;
use crate::providers::gcp::clients::backupdr::Trial;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsTriggerBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesAbandonBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsRestoreArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesFetchAccessTokenArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesFinalizeBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesInitiateBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesRemoveArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesSetInternalStatusArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsTestIamPermissionsArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersMsComplianceMetadataArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersSetIamPolicyArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersTestIamPermissionsArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsServiceConfigInitializeArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsTrialEndArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsTrialSubscribeArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BackupdrProvider with automatic state tracking.
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
/// let provider = BackupdrProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BackupdrProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BackupdrProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BackupdrProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Backupdr projects locations backup plan associations create.
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
    pub fn backupdr_projects_locations_backup_plan_associations_create(
        &self,
        args: &BackupdrProjectsLocationsBackupPlanAssociationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plan_associations_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupPlanAssociationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plan_associations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plan associations delete.
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
    pub fn backupdr_projects_locations_backup_plan_associations_delete(
        &self,
        args: &BackupdrProjectsLocationsBackupPlanAssociationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plan_associations_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plan_associations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plan associations patch.
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
    pub fn backupdr_projects_locations_backup_plan_associations_patch(
        &self,
        args: &BackupdrProjectsLocationsBackupPlanAssociationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plan_associations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plan_associations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plan associations trigger backup.
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
    pub fn backupdr_projects_locations_backup_plan_associations_trigger_backup(
        &self,
        args: &BackupdrProjectsLocationsBackupPlanAssociationsTriggerBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plan_associations_trigger_backup_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plan_associations_trigger_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plans create.
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
    pub fn backupdr_projects_locations_backup_plans_create(
        &self,
        args: &BackupdrProjectsLocationsBackupPlansCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plans_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupPlanId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plans_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plans delete.
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
    pub fn backupdr_projects_locations_backup_plans_delete(
        &self,
        args: &BackupdrProjectsLocationsBackupPlansDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plans_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plans_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plans patch.
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
    pub fn backupdr_projects_locations_backup_plans_patch(
        &self,
        args: &BackupdrProjectsLocationsBackupPlansPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plans_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plans_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults create.
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
    pub fn backupdr_projects_locations_backup_vaults_create(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupVaultId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults delete.
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
    pub fn backupdr_projects_locations_backup_vaults_delete(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.etag,
            &args.force,
            &args.ignoreBackupPlanReferences,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults patch.
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
    pub fn backupdr_projects_locations_backup_vaults_patch(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_patch_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.forceUpdateAccessRestriction,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults test iam permissions.
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
    pub fn backupdr_projects_locations_backup_vaults_test_iam_permissions(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources abandon backup.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_abandon_backup(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesAbandonBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_abandon_backup_builder(
            &self.http_client,
            &args.dataSource,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_abandon_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources fetch access token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchAccessTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn backupdr_projects_locations_backup_vaults_data_sources_fetch_access_token(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesFetchAccessTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchAccessTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_fetch_access_token_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_fetch_access_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources finalize backup.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_finalize_backup(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesFinalizeBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_finalize_backup_builder(
            &self.http_client,
            &args.dataSource,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_finalize_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources initiate backup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InitiateBackupResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn backupdr_projects_locations_backup_vaults_data_sources_initiate_backup(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesInitiateBackupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InitiateBackupResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_initiate_backup_builder(
            &self.http_client,
            &args.dataSource,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_initiate_backup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources patch.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_patch(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources remove.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_remove(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_remove_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources set internal status.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_set_internal_status(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesSetInternalStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_set_internal_status_builder(
            &self.http_client,
            &args.dataSource,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_set_internal_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources backups delete.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_backups_delete(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_backups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources backups patch.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_backups_patch(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_backups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_backups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources backups restore.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_backups_restore(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_backups_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_backups_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations management servers create.
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
    pub fn backupdr_projects_locations_management_servers_create(
        &self,
        args: &BackupdrProjectsLocationsManagementServersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_create_builder(
            &self.http_client,
            &args.parent,
            &args.managementServerId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations management servers delete.
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
    pub fn backupdr_projects_locations_management_servers_delete(
        &self,
        args: &BackupdrProjectsLocationsManagementServersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations management servers ms compliance metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchMsComplianceMetadataResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn backupdr_projects_locations_management_servers_ms_compliance_metadata(
        &self,
        args: &BackupdrProjectsLocationsManagementServersMsComplianceMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchMsComplianceMetadataResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_ms_compliance_metadata_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_ms_compliance_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations management servers set iam policy.
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
    pub fn backupdr_projects_locations_management_servers_set_iam_policy(
        &self,
        args: &BackupdrProjectsLocationsManagementServersSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations management servers test iam permissions.
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
    pub fn backupdr_projects_locations_management_servers_test_iam_permissions(
        &self,
        args: &BackupdrProjectsLocationsManagementServersTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations operations cancel.
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
    pub fn backupdr_projects_locations_operations_cancel(
        &self,
        args: &BackupdrProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations operations delete.
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
    pub fn backupdr_projects_locations_operations_delete(
        &self,
        args: &BackupdrProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations service config initialize.
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
    pub fn backupdr_projects_locations_service_config_initialize(
        &self,
        args: &BackupdrProjectsLocationsServiceConfigInitializeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_service_config_initialize_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_service_config_initialize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations trial end.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Trial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn backupdr_projects_locations_trial_end(
        &self,
        args: &BackupdrProjectsLocationsTrialEndArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_trial_end_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_trial_end_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations trial subscribe.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Trial result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn backupdr_projects_locations_trial_subscribe(
        &self,
        args: &BackupdrProjectsLocationsTrialSubscribeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_trial_subscribe_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_trial_subscribe_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
