//! MigrationcenterProvider - State-aware migrationcenter API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       migrationcenter API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::migrationcenter::{
    migrationcenter_projects_locations_update_settings_builder, migrationcenter_projects_locations_update_settings_task,
    migrationcenter_projects_locations_assets_aggregate_values_builder, migrationcenter_projects_locations_assets_aggregate_values_task,
    migrationcenter_projects_locations_assets_batch_delete_builder, migrationcenter_projects_locations_assets_batch_delete_task,
    migrationcenter_projects_locations_assets_batch_update_builder, migrationcenter_projects_locations_assets_batch_update_task,
    migrationcenter_projects_locations_assets_delete_builder, migrationcenter_projects_locations_assets_delete_task,
    migrationcenter_projects_locations_assets_patch_builder, migrationcenter_projects_locations_assets_patch_task,
    migrationcenter_projects_locations_assets_report_asset_frames_builder, migrationcenter_projects_locations_assets_report_asset_frames_task,
    migrationcenter_projects_locations_assets_export_jobs_create_builder, migrationcenter_projects_locations_assets_export_jobs_create_task,
    migrationcenter_projects_locations_assets_export_jobs_delete_builder, migrationcenter_projects_locations_assets_export_jobs_delete_task,
    migrationcenter_projects_locations_assets_export_jobs_run_builder, migrationcenter_projects_locations_assets_export_jobs_run_task,
    migrationcenter_projects_locations_discovery_clients_create_builder, migrationcenter_projects_locations_discovery_clients_create_task,
    migrationcenter_projects_locations_discovery_clients_delete_builder, migrationcenter_projects_locations_discovery_clients_delete_task,
    migrationcenter_projects_locations_discovery_clients_patch_builder, migrationcenter_projects_locations_discovery_clients_patch_task,
    migrationcenter_projects_locations_discovery_clients_send_heartbeat_builder, migrationcenter_projects_locations_discovery_clients_send_heartbeat_task,
    migrationcenter_projects_locations_groups_add_assets_builder, migrationcenter_projects_locations_groups_add_assets_task,
    migrationcenter_projects_locations_groups_create_builder, migrationcenter_projects_locations_groups_create_task,
    migrationcenter_projects_locations_groups_delete_builder, migrationcenter_projects_locations_groups_delete_task,
    migrationcenter_projects_locations_groups_patch_builder, migrationcenter_projects_locations_groups_patch_task,
    migrationcenter_projects_locations_groups_remove_assets_builder, migrationcenter_projects_locations_groups_remove_assets_task,
    migrationcenter_projects_locations_import_jobs_create_builder, migrationcenter_projects_locations_import_jobs_create_task,
    migrationcenter_projects_locations_import_jobs_delete_builder, migrationcenter_projects_locations_import_jobs_delete_task,
    migrationcenter_projects_locations_import_jobs_patch_builder, migrationcenter_projects_locations_import_jobs_patch_task,
    migrationcenter_projects_locations_import_jobs_run_builder, migrationcenter_projects_locations_import_jobs_run_task,
    migrationcenter_projects_locations_import_jobs_validate_builder, migrationcenter_projects_locations_import_jobs_validate_task,
    migrationcenter_projects_locations_import_jobs_import_data_files_create_builder, migrationcenter_projects_locations_import_jobs_import_data_files_create_task,
    migrationcenter_projects_locations_import_jobs_import_data_files_delete_builder, migrationcenter_projects_locations_import_jobs_import_data_files_delete_task,
    migrationcenter_projects_locations_operations_cancel_builder, migrationcenter_projects_locations_operations_cancel_task,
    migrationcenter_projects_locations_operations_delete_builder, migrationcenter_projects_locations_operations_delete_task,
    migrationcenter_projects_locations_preference_sets_create_builder, migrationcenter_projects_locations_preference_sets_create_task,
    migrationcenter_projects_locations_preference_sets_delete_builder, migrationcenter_projects_locations_preference_sets_delete_task,
    migrationcenter_projects_locations_preference_sets_patch_builder, migrationcenter_projects_locations_preference_sets_patch_task,
    migrationcenter_projects_locations_report_configs_create_builder, migrationcenter_projects_locations_report_configs_create_task,
    migrationcenter_projects_locations_report_configs_delete_builder, migrationcenter_projects_locations_report_configs_delete_task,
    migrationcenter_projects_locations_report_configs_reports_create_builder, migrationcenter_projects_locations_report_configs_reports_create_task,
    migrationcenter_projects_locations_report_configs_reports_delete_builder, migrationcenter_projects_locations_report_configs_reports_delete_task,
    migrationcenter_projects_locations_sources_create_builder, migrationcenter_projects_locations_sources_create_task,
    migrationcenter_projects_locations_sources_delete_builder, migrationcenter_projects_locations_sources_delete_task,
    migrationcenter_projects_locations_sources_patch_builder, migrationcenter_projects_locations_sources_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::migrationcenter::AggregateAssetsValuesResponse;
use crate::providers::gcp::clients::migrationcenter::Asset;
use crate::providers::gcp::clients::migrationcenter::BatchUpdateAssetsResponse;
use crate::providers::gcp::clients::migrationcenter::Empty;
use crate::providers::gcp::clients::migrationcenter::Operation;
use crate::providers::gcp::clients::migrationcenter::ReportAssetFramesResponse;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsAggregateValuesArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsBatchDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsBatchUpdateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsExportJobsCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsExportJobsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsExportJobsRunArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsPatchArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsAssetsReportAssetFramesArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsDiscoveryClientsCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsDiscoveryClientsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsDiscoveryClientsPatchArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsDiscoveryClientsSendHeartbeatArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsGroupsAddAssetsArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsGroupsCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsGroupsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsGroupsPatchArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsGroupsRemoveAssetsArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsImportJobsCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsImportJobsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsImportJobsImportDataFilesCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsImportJobsImportDataFilesDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsImportJobsPatchArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsImportJobsRunArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsImportJobsValidateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsPreferenceSetsCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsPreferenceSetsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsPreferenceSetsPatchArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsReportConfigsCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsReportConfigsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsReportConfigsReportsCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsReportConfigsReportsDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsSourcesCreateArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsSourcesDeleteArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsSourcesPatchArgs;
use crate::providers::gcp::clients::migrationcenter::MigrationcenterProjectsLocationsUpdateSettingsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MigrationcenterProvider with automatic state tracking.
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
/// let provider = MigrationcenterProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MigrationcenterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MigrationcenterProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MigrationcenterProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Migrationcenter projects locations update settings.
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
    pub fn migrationcenter_projects_locations_update_settings(
        &self,
        args: &MigrationcenterProjectsLocationsUpdateSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_update_settings_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_update_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets aggregate values.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AggregateAssetsValuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn migrationcenter_projects_locations_assets_aggregate_values(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsAggregateValuesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AggregateAssetsValuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_aggregate_values_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_aggregate_values_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets batch delete.
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
    pub fn migrationcenter_projects_locations_assets_batch_delete(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_batch_delete_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets batch update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchUpdateAssetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn migrationcenter_projects_locations_assets_batch_update(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsBatchUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchUpdateAssetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_batch_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_batch_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets delete.
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
    pub fn migrationcenter_projects_locations_assets_delete(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Asset result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn migrationcenter_projects_locations_assets_patch(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Asset, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets report asset frames.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ReportAssetFramesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn migrationcenter_projects_locations_assets_report_asset_frames(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsReportAssetFramesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ReportAssetFramesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_report_asset_frames_builder(
            &self.http_client,
            &args.parent,
            &args.source,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_report_asset_frames_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets export jobs create.
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
    pub fn migrationcenter_projects_locations_assets_export_jobs_create(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsExportJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_export_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.assetsExportJobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_export_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets export jobs delete.
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
    pub fn migrationcenter_projects_locations_assets_export_jobs_delete(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsExportJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_export_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_export_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations assets export jobs run.
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
    pub fn migrationcenter_projects_locations_assets_export_jobs_run(
        &self,
        args: &MigrationcenterProjectsLocationsAssetsExportJobsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_assets_export_jobs_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_assets_export_jobs_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations discovery clients create.
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
    pub fn migrationcenter_projects_locations_discovery_clients_create(
        &self,
        args: &MigrationcenterProjectsLocationsDiscoveryClientsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_discovery_clients_create_builder(
            &self.http_client,
            &args.parent,
            &args.discoveryClientId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_discovery_clients_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations discovery clients delete.
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
    pub fn migrationcenter_projects_locations_discovery_clients_delete(
        &self,
        args: &MigrationcenterProjectsLocationsDiscoveryClientsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_discovery_clients_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_discovery_clients_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations discovery clients patch.
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
    pub fn migrationcenter_projects_locations_discovery_clients_patch(
        &self,
        args: &MigrationcenterProjectsLocationsDiscoveryClientsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_discovery_clients_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_discovery_clients_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations discovery clients send heartbeat.
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
    pub fn migrationcenter_projects_locations_discovery_clients_send_heartbeat(
        &self,
        args: &MigrationcenterProjectsLocationsDiscoveryClientsSendHeartbeatArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_discovery_clients_send_heartbeat_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_discovery_clients_send_heartbeat_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations groups add assets.
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
    pub fn migrationcenter_projects_locations_groups_add_assets(
        &self,
        args: &MigrationcenterProjectsLocationsGroupsAddAssetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_groups_add_assets_builder(
            &self.http_client,
            &args.group,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_groups_add_assets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations groups create.
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
    pub fn migrationcenter_projects_locations_groups_create(
        &self,
        args: &MigrationcenterProjectsLocationsGroupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_groups_create_builder(
            &self.http_client,
            &args.parent,
            &args.groupId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_groups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations groups delete.
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
    pub fn migrationcenter_projects_locations_groups_delete(
        &self,
        args: &MigrationcenterProjectsLocationsGroupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_groups_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_groups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations groups patch.
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
    pub fn migrationcenter_projects_locations_groups_patch(
        &self,
        args: &MigrationcenterProjectsLocationsGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations groups remove assets.
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
    pub fn migrationcenter_projects_locations_groups_remove_assets(
        &self,
        args: &MigrationcenterProjectsLocationsGroupsRemoveAssetsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_groups_remove_assets_builder(
            &self.http_client,
            &args.group,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_groups_remove_assets_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations import jobs create.
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
    pub fn migrationcenter_projects_locations_import_jobs_create(
        &self,
        args: &MigrationcenterProjectsLocationsImportJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_import_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.importJobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_import_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations import jobs delete.
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
    pub fn migrationcenter_projects_locations_import_jobs_delete(
        &self,
        args: &MigrationcenterProjectsLocationsImportJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_import_jobs_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_import_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations import jobs patch.
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
    pub fn migrationcenter_projects_locations_import_jobs_patch(
        &self,
        args: &MigrationcenterProjectsLocationsImportJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_import_jobs_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_import_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations import jobs run.
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
    pub fn migrationcenter_projects_locations_import_jobs_run(
        &self,
        args: &MigrationcenterProjectsLocationsImportJobsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_import_jobs_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_import_jobs_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations import jobs validate.
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
    pub fn migrationcenter_projects_locations_import_jobs_validate(
        &self,
        args: &MigrationcenterProjectsLocationsImportJobsValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_import_jobs_validate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_import_jobs_validate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations import jobs import data files create.
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
    pub fn migrationcenter_projects_locations_import_jobs_import_data_files_create(
        &self,
        args: &MigrationcenterProjectsLocationsImportJobsImportDataFilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_import_jobs_import_data_files_create_builder(
            &self.http_client,
            &args.parent,
            &args.importDataFileId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_import_jobs_import_data_files_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations import jobs import data files delete.
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
    pub fn migrationcenter_projects_locations_import_jobs_import_data_files_delete(
        &self,
        args: &MigrationcenterProjectsLocationsImportJobsImportDataFilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_import_jobs_import_data_files_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_import_jobs_import_data_files_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations operations cancel.
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
    pub fn migrationcenter_projects_locations_operations_cancel(
        &self,
        args: &MigrationcenterProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations operations delete.
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
    pub fn migrationcenter_projects_locations_operations_delete(
        &self,
        args: &MigrationcenterProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations preference sets create.
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
    pub fn migrationcenter_projects_locations_preference_sets_create(
        &self,
        args: &MigrationcenterProjectsLocationsPreferenceSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_preference_sets_create_builder(
            &self.http_client,
            &args.parent,
            &args.preferenceSetId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_preference_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations preference sets delete.
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
    pub fn migrationcenter_projects_locations_preference_sets_delete(
        &self,
        args: &MigrationcenterProjectsLocationsPreferenceSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_preference_sets_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_preference_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations preference sets patch.
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
    pub fn migrationcenter_projects_locations_preference_sets_patch(
        &self,
        args: &MigrationcenterProjectsLocationsPreferenceSetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_preference_sets_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_preference_sets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations report configs create.
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
    pub fn migrationcenter_projects_locations_report_configs_create(
        &self,
        args: &MigrationcenterProjectsLocationsReportConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_report_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.reportConfigId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_report_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations report configs delete.
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
    pub fn migrationcenter_projects_locations_report_configs_delete(
        &self,
        args: &MigrationcenterProjectsLocationsReportConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_report_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_report_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations report configs reports create.
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
    pub fn migrationcenter_projects_locations_report_configs_reports_create(
        &self,
        args: &MigrationcenterProjectsLocationsReportConfigsReportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_report_configs_reports_create_builder(
            &self.http_client,
            &args.parent,
            &args.reportId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_report_configs_reports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations report configs reports delete.
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
    pub fn migrationcenter_projects_locations_report_configs_reports_delete(
        &self,
        args: &MigrationcenterProjectsLocationsReportConfigsReportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_report_configs_reports_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_report_configs_reports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations sources create.
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
    pub fn migrationcenter_projects_locations_sources_create(
        &self,
        args: &MigrationcenterProjectsLocationsSourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_sources_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.sourceId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations sources delete.
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
    pub fn migrationcenter_projects_locations_sources_delete(
        &self,
        args: &MigrationcenterProjectsLocationsSourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_sources_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_sources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Migrationcenter projects locations sources patch.
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
    pub fn migrationcenter_projects_locations_sources_patch(
        &self,
        args: &MigrationcenterProjectsLocationsSourcesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = migrationcenter_projects_locations_sources_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = migrationcenter_projects_locations_sources_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
