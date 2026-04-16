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
    backupdr_projects_locations_get_builder, backupdr_projects_locations_get_task,
    backupdr_projects_locations_get_trial_builder, backupdr_projects_locations_get_trial_task,
    backupdr_projects_locations_list_builder, backupdr_projects_locations_list_task,
    backupdr_projects_locations_backup_plan_associations_create_builder, backupdr_projects_locations_backup_plan_associations_create_task,
    backupdr_projects_locations_backup_plan_associations_delete_builder, backupdr_projects_locations_backup_plan_associations_delete_task,
    backupdr_projects_locations_backup_plan_associations_fetch_for_resource_type_builder, backupdr_projects_locations_backup_plan_associations_fetch_for_resource_type_task,
    backupdr_projects_locations_backup_plan_associations_get_builder, backupdr_projects_locations_backup_plan_associations_get_task,
    backupdr_projects_locations_backup_plan_associations_list_builder, backupdr_projects_locations_backup_plan_associations_list_task,
    backupdr_projects_locations_backup_plan_associations_patch_builder, backupdr_projects_locations_backup_plan_associations_patch_task,
    backupdr_projects_locations_backup_plan_associations_trigger_backup_builder, backupdr_projects_locations_backup_plan_associations_trigger_backup_task,
    backupdr_projects_locations_backup_plans_create_builder, backupdr_projects_locations_backup_plans_create_task,
    backupdr_projects_locations_backup_plans_delete_builder, backupdr_projects_locations_backup_plans_delete_task,
    backupdr_projects_locations_backup_plans_get_builder, backupdr_projects_locations_backup_plans_get_task,
    backupdr_projects_locations_backup_plans_list_builder, backupdr_projects_locations_backup_plans_list_task,
    backupdr_projects_locations_backup_plans_patch_builder, backupdr_projects_locations_backup_plans_patch_task,
    backupdr_projects_locations_backup_plans_revisions_get_builder, backupdr_projects_locations_backup_plans_revisions_get_task,
    backupdr_projects_locations_backup_plans_revisions_list_builder, backupdr_projects_locations_backup_plans_revisions_list_task,
    backupdr_projects_locations_backup_vaults_create_builder, backupdr_projects_locations_backup_vaults_create_task,
    backupdr_projects_locations_backup_vaults_delete_builder, backupdr_projects_locations_backup_vaults_delete_task,
    backupdr_projects_locations_backup_vaults_fetch_usable_builder, backupdr_projects_locations_backup_vaults_fetch_usable_task,
    backupdr_projects_locations_backup_vaults_get_builder, backupdr_projects_locations_backup_vaults_get_task,
    backupdr_projects_locations_backup_vaults_list_builder, backupdr_projects_locations_backup_vaults_list_task,
    backupdr_projects_locations_backup_vaults_patch_builder, backupdr_projects_locations_backup_vaults_patch_task,
    backupdr_projects_locations_backup_vaults_test_iam_permissions_builder, backupdr_projects_locations_backup_vaults_test_iam_permissions_task,
    backupdr_projects_locations_backup_vaults_data_sources_abandon_backup_builder, backupdr_projects_locations_backup_vaults_data_sources_abandon_backup_task,
    backupdr_projects_locations_backup_vaults_data_sources_fetch_access_token_builder, backupdr_projects_locations_backup_vaults_data_sources_fetch_access_token_task,
    backupdr_projects_locations_backup_vaults_data_sources_finalize_backup_builder, backupdr_projects_locations_backup_vaults_data_sources_finalize_backup_task,
    backupdr_projects_locations_backup_vaults_data_sources_get_builder, backupdr_projects_locations_backup_vaults_data_sources_get_task,
    backupdr_projects_locations_backup_vaults_data_sources_initiate_backup_builder, backupdr_projects_locations_backup_vaults_data_sources_initiate_backup_task,
    backupdr_projects_locations_backup_vaults_data_sources_list_builder, backupdr_projects_locations_backup_vaults_data_sources_list_task,
    backupdr_projects_locations_backup_vaults_data_sources_patch_builder, backupdr_projects_locations_backup_vaults_data_sources_patch_task,
    backupdr_projects_locations_backup_vaults_data_sources_remove_builder, backupdr_projects_locations_backup_vaults_data_sources_remove_task,
    backupdr_projects_locations_backup_vaults_data_sources_set_internal_status_builder, backupdr_projects_locations_backup_vaults_data_sources_set_internal_status_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_delete_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_delete_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_fetch_for_resource_type_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_fetch_for_resource_type_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_get_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_get_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_list_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_list_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_patch_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_patch_task,
    backupdr_projects_locations_backup_vaults_data_sources_backups_restore_builder, backupdr_projects_locations_backup_vaults_data_sources_backups_restore_task,
    backupdr_projects_locations_data_source_references_fetch_for_resource_type_builder, backupdr_projects_locations_data_source_references_fetch_for_resource_type_task,
    backupdr_projects_locations_data_source_references_get_builder, backupdr_projects_locations_data_source_references_get_task,
    backupdr_projects_locations_data_source_references_list_builder, backupdr_projects_locations_data_source_references_list_task,
    backupdr_projects_locations_management_servers_create_builder, backupdr_projects_locations_management_servers_create_task,
    backupdr_projects_locations_management_servers_delete_builder, backupdr_projects_locations_management_servers_delete_task,
    backupdr_projects_locations_management_servers_get_builder, backupdr_projects_locations_management_servers_get_task,
    backupdr_projects_locations_management_servers_get_iam_policy_builder, backupdr_projects_locations_management_servers_get_iam_policy_task,
    backupdr_projects_locations_management_servers_list_builder, backupdr_projects_locations_management_servers_list_task,
    backupdr_projects_locations_management_servers_ms_compliance_metadata_builder, backupdr_projects_locations_management_servers_ms_compliance_metadata_task,
    backupdr_projects_locations_management_servers_set_iam_policy_builder, backupdr_projects_locations_management_servers_set_iam_policy_task,
    backupdr_projects_locations_management_servers_test_iam_permissions_builder, backupdr_projects_locations_management_servers_test_iam_permissions_task,
    backupdr_projects_locations_operations_cancel_builder, backupdr_projects_locations_operations_cancel_task,
    backupdr_projects_locations_operations_delete_builder, backupdr_projects_locations_operations_delete_task,
    backupdr_projects_locations_operations_get_builder, backupdr_projects_locations_operations_get_task,
    backupdr_projects_locations_operations_list_builder, backupdr_projects_locations_operations_list_task,
    backupdr_projects_locations_resource_backup_configs_list_builder, backupdr_projects_locations_resource_backup_configs_list_task,
    backupdr_projects_locations_service_config_initialize_builder, backupdr_projects_locations_service_config_initialize_task,
    backupdr_projects_locations_trial_end_builder, backupdr_projects_locations_trial_end_task,
    backupdr_projects_locations_trial_subscribe_builder, backupdr_projects_locations_trial_subscribe_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::backupdr::Backup;
use crate::providers::gcp::clients::backupdr::BackupPlan;
use crate::providers::gcp::clients::backupdr::BackupPlanAssociation;
use crate::providers::gcp::clients::backupdr::BackupPlanRevision;
use crate::providers::gcp::clients::backupdr::BackupVault;
use crate::providers::gcp::clients::backupdr::DataSource;
use crate::providers::gcp::clients::backupdr::DataSourceReference;
use crate::providers::gcp::clients::backupdr::Empty;
use crate::providers::gcp::clients::backupdr::FetchAccessTokenResponse;
use crate::providers::gcp::clients::backupdr::FetchBackupPlanAssociationsForResourceTypeResponse;
use crate::providers::gcp::clients::backupdr::FetchBackupsForResourceTypeResponse;
use crate::providers::gcp::clients::backupdr::FetchDataSourceReferencesForResourceTypeResponse;
use crate::providers::gcp::clients::backupdr::FetchMsComplianceMetadataResponse;
use crate::providers::gcp::clients::backupdr::FetchUsableBackupVaultsResponse;
use crate::providers::gcp::clients::backupdr::InitiateBackupResponse;
use crate::providers::gcp::clients::backupdr::ListBackupPlanAssociationsResponse;
use crate::providers::gcp::clients::backupdr::ListBackupPlanRevisionsResponse;
use crate::providers::gcp::clients::backupdr::ListBackupPlansResponse;
use crate::providers::gcp::clients::backupdr::ListBackupVaultsResponse;
use crate::providers::gcp::clients::backupdr::ListBackupsResponse;
use crate::providers::gcp::clients::backupdr::ListDataSourceReferencesResponse;
use crate::providers::gcp::clients::backupdr::ListDataSourcesResponse;
use crate::providers::gcp::clients::backupdr::ListLocationsResponse;
use crate::providers::gcp::clients::backupdr::ListManagementServersResponse;
use crate::providers::gcp::clients::backupdr::ListOperationsResponse;
use crate::providers::gcp::clients::backupdr::ListResourceBackupConfigsResponse;
use crate::providers::gcp::clients::backupdr::Location;
use crate::providers::gcp::clients::backupdr::ManagementServer;
use crate::providers::gcp::clients::backupdr::Operation;
use crate::providers::gcp::clients::backupdr::Policy;
use crate::providers::gcp::clients::backupdr::TestIamPermissionsResponse;
use crate::providers::gcp::clients::backupdr::Trial;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsFetchForResourceTypeArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlanAssociationsTriggerBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansRevisionsGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupPlansRevisionsListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesAbandonBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsFetchForResourceTypeArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsRestoreArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesFetchAccessTokenArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesFinalizeBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesInitiateBackupArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesRemoveArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDataSourcesSetInternalStatusArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsFetchUsableArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsPatchArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsBackupVaultsTestIamPermissionsArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsDataSourceReferencesFetchForResourceTypeArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsDataSourceReferencesGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsDataSourceReferencesListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsGetTrialArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersCreateArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersGetIamPolicyArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersMsComplianceMetadataArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersSetIamPolicyArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsManagementServersTestIamPermissionsArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsResourceBackupConfigsListArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsServiceConfigInitializeArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsTrialEndArgs;
use crate::providers::gcp::clients::backupdr::BackupdrProjectsLocationsTrialSubscribeArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BackupdrProvider with automatic state tracking.
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
/// let provider = BackupdrProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct BackupdrProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> BackupdrProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new BackupdrProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new BackupdrProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Backupdr projects locations get.
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
    pub fn backupdr_projects_locations_get(
        &self,
        args: &BackupdrProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations get trial.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_get_trial(
        &self,
        args: &BackupdrProjectsLocationsGetTrialArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Trial, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_get_trial_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_get_trial_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations list.
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
    pub fn backupdr_projects_locations_list(
        &self,
        args: &BackupdrProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations backup plan associations fetch for resource type.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchBackupPlanAssociationsForResourceTypeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_plan_associations_fetch_for_resource_type(
        &self,
        args: &BackupdrProjectsLocationsBackupPlanAssociationsFetchForResourceTypeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchBackupPlanAssociationsForResourceTypeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plan_associations_fetch_for_resource_type_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.resourceType,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plan_associations_fetch_for_resource_type_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plan associations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupPlanAssociation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_plan_associations_get(
        &self,
        args: &BackupdrProjectsLocationsBackupPlanAssociationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupPlanAssociation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plan_associations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plan_associations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plan associations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupPlanAssociationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_plan_associations_list(
        &self,
        args: &BackupdrProjectsLocationsBackupPlanAssociationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupPlanAssociationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plan_associations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plan_associations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations backup plans get.
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
    pub fn backupdr_projects_locations_backup_plans_get(
        &self,
        args: &BackupdrProjectsLocationsBackupPlansGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupPlan, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plans_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plans_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plans list.
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
    pub fn backupdr_projects_locations_backup_plans_list(
        &self,
        args: &BackupdrProjectsLocationsBackupPlansListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupPlansResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plans_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plans_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations backup plans revisions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BackupPlanRevision result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_plans_revisions_get(
        &self,
        args: &BackupdrProjectsLocationsBackupPlansRevisionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupPlanRevision, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plans_revisions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plans_revisions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup plans revisions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBackupPlanRevisionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_plans_revisions_list(
        &self,
        args: &BackupdrProjectsLocationsBackupPlansRevisionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupPlanRevisionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_plans_revisions_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_plans_revisions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations backup vaults fetch usable.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchUsableBackupVaultsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_vaults_fetch_usable(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsFetchUsableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchUsableBackupVaultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_fetch_usable_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_fetch_usable_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults get.
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
    pub fn backupdr_projects_locations_backup_vaults_get(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BackupVault, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults list.
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
    pub fn backupdr_projects_locations_backup_vaults_list(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupVaultsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations backup vaults data sources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_vaults_data_sources_get(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations backup vaults data sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataSourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_vaults_data_sources_list(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataSourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations backup vaults data sources backups fetch for resource type.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchBackupsForResourceTypeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_backup_vaults_data_sources_backups_fetch_for_resource_type(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsFetchForResourceTypeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchBackupsForResourceTypeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_backups_fetch_for_resource_type_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.resourceType,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_backups_fetch_for_resource_type_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources backups get.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_backups_get(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_backups_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations backup vaults data sources backups list.
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
    pub fn backupdr_projects_locations_backup_vaults_data_sources_backups_list(
        &self,
        args: &BackupdrProjectsLocationsBackupVaultsDataSourcesBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_backup_vaults_data_sources_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_backup_vaults_data_sources_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations data source references fetch for resource type.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchDataSourceReferencesForResourceTypeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_data_source_references_fetch_for_resource_type(
        &self,
        args: &BackupdrProjectsLocationsDataSourceReferencesFetchForResourceTypeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchDataSourceReferencesForResourceTypeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_data_source_references_fetch_for_resource_type_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.resourceType,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_data_source_references_fetch_for_resource_type_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations data source references get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataSourceReference result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_data_source_references_get(
        &self,
        args: &BackupdrProjectsLocationsDataSourceReferencesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataSourceReference, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_data_source_references_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_data_source_references_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations data source references list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDataSourceReferencesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_data_source_references_list(
        &self,
        args: &BackupdrProjectsLocationsDataSourceReferencesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDataSourceReferencesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_data_source_references_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_data_source_references_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations management servers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagementServer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_management_servers_get(
        &self,
        args: &BackupdrProjectsLocationsManagementServersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagementServer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations management servers get iam policy.
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
    pub fn backupdr_projects_locations_management_servers_get_iam_policy(
        &self,
        args: &BackupdrProjectsLocationsManagementServersGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations management servers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListManagementServersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_management_servers_list(
        &self,
        args: &BackupdrProjectsLocationsManagementServersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListManagementServersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_management_servers_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_management_servers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Backupdr projects locations operations get.
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
    pub fn backupdr_projects_locations_operations_get(
        &self,
        args: &BackupdrProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations operations list.
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
    pub fn backupdr_projects_locations_operations_list(
        &self,
        args: &BackupdrProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Backupdr projects locations resource backup configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListResourceBackupConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn backupdr_projects_locations_resource_backup_configs_list(
        &self,
        args: &BackupdrProjectsLocationsResourceBackupConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListResourceBackupConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = backupdr_projects_locations_resource_backup_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = backupdr_projects_locations_resource_backup_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
