//! BaremetalsolutionProvider - State-aware baremetalsolution API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       baremetalsolution API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::baremetalsolution::{
    baremetalsolution_projects_locations_get_builder, baremetalsolution_projects_locations_get_task,
    baremetalsolution_projects_locations_list_builder, baremetalsolution_projects_locations_list_task,
    baremetalsolution_projects_locations_instances_detach_lun_builder, baremetalsolution_projects_locations_instances_detach_lun_task,
    baremetalsolution_projects_locations_instances_disable_hyperthreading_builder, baremetalsolution_projects_locations_instances_disable_hyperthreading_task,
    baremetalsolution_projects_locations_instances_disable_interactive_serial_console_builder, baremetalsolution_projects_locations_instances_disable_interactive_serial_console_task,
    baremetalsolution_projects_locations_instances_enable_hyperthreading_builder, baremetalsolution_projects_locations_instances_enable_hyperthreading_task,
    baremetalsolution_projects_locations_instances_enable_interactive_serial_console_builder, baremetalsolution_projects_locations_instances_enable_interactive_serial_console_task,
    baremetalsolution_projects_locations_instances_get_builder, baremetalsolution_projects_locations_instances_get_task,
    baremetalsolution_projects_locations_instances_list_builder, baremetalsolution_projects_locations_instances_list_task,
    baremetalsolution_projects_locations_instances_load_auth_info_builder, baremetalsolution_projects_locations_instances_load_auth_info_task,
    baremetalsolution_projects_locations_instances_patch_builder, baremetalsolution_projects_locations_instances_patch_task,
    baremetalsolution_projects_locations_instances_reimage_builder, baremetalsolution_projects_locations_instances_reimage_task,
    baremetalsolution_projects_locations_instances_rename_builder, baremetalsolution_projects_locations_instances_rename_task,
    baremetalsolution_projects_locations_instances_reset_builder, baremetalsolution_projects_locations_instances_reset_task,
    baremetalsolution_projects_locations_instances_start_builder, baremetalsolution_projects_locations_instances_start_task,
    baremetalsolution_projects_locations_instances_stop_builder, baremetalsolution_projects_locations_instances_stop_task,
    baremetalsolution_projects_locations_networks_get_builder, baremetalsolution_projects_locations_networks_get_task,
    baremetalsolution_projects_locations_networks_list_builder, baremetalsolution_projects_locations_networks_list_task,
    baremetalsolution_projects_locations_networks_list_network_usage_builder, baremetalsolution_projects_locations_networks_list_network_usage_task,
    baremetalsolution_projects_locations_networks_patch_builder, baremetalsolution_projects_locations_networks_patch_task,
    baremetalsolution_projects_locations_networks_rename_builder, baremetalsolution_projects_locations_networks_rename_task,
    baremetalsolution_projects_locations_nfs_shares_create_builder, baremetalsolution_projects_locations_nfs_shares_create_task,
    baremetalsolution_projects_locations_nfs_shares_delete_builder, baremetalsolution_projects_locations_nfs_shares_delete_task,
    baremetalsolution_projects_locations_nfs_shares_get_builder, baremetalsolution_projects_locations_nfs_shares_get_task,
    baremetalsolution_projects_locations_nfs_shares_list_builder, baremetalsolution_projects_locations_nfs_shares_list_task,
    baremetalsolution_projects_locations_nfs_shares_patch_builder, baremetalsolution_projects_locations_nfs_shares_patch_task,
    baremetalsolution_projects_locations_nfs_shares_rename_builder, baremetalsolution_projects_locations_nfs_shares_rename_task,
    baremetalsolution_projects_locations_operations_get_builder, baremetalsolution_projects_locations_operations_get_task,
    baremetalsolution_projects_locations_os_images_get_builder, baremetalsolution_projects_locations_os_images_get_task,
    baremetalsolution_projects_locations_os_images_list_builder, baremetalsolution_projects_locations_os_images_list_task,
    baremetalsolution_projects_locations_provisioning_configs_create_builder, baremetalsolution_projects_locations_provisioning_configs_create_task,
    baremetalsolution_projects_locations_provisioning_configs_get_builder, baremetalsolution_projects_locations_provisioning_configs_get_task,
    baremetalsolution_projects_locations_provisioning_configs_patch_builder, baremetalsolution_projects_locations_provisioning_configs_patch_task,
    baremetalsolution_projects_locations_provisioning_configs_submit_builder, baremetalsolution_projects_locations_provisioning_configs_submit_task,
    baremetalsolution_projects_locations_provisioning_quotas_list_builder, baremetalsolution_projects_locations_provisioning_quotas_list_task,
    baremetalsolution_projects_locations_ssh_keys_create_builder, baremetalsolution_projects_locations_ssh_keys_create_task,
    baremetalsolution_projects_locations_ssh_keys_delete_builder, baremetalsolution_projects_locations_ssh_keys_delete_task,
    baremetalsolution_projects_locations_ssh_keys_list_builder, baremetalsolution_projects_locations_ssh_keys_list_task,
    baremetalsolution_projects_locations_volumes_evict_builder, baremetalsolution_projects_locations_volumes_evict_task,
    baremetalsolution_projects_locations_volumes_get_builder, baremetalsolution_projects_locations_volumes_get_task,
    baremetalsolution_projects_locations_volumes_list_builder, baremetalsolution_projects_locations_volumes_list_task,
    baremetalsolution_projects_locations_volumes_patch_builder, baremetalsolution_projects_locations_volumes_patch_task,
    baremetalsolution_projects_locations_volumes_rename_builder, baremetalsolution_projects_locations_volumes_rename_task,
    baremetalsolution_projects_locations_volumes_resize_builder, baremetalsolution_projects_locations_volumes_resize_task,
    baremetalsolution_projects_locations_volumes_luns_evict_builder, baremetalsolution_projects_locations_volumes_luns_evict_task,
    baremetalsolution_projects_locations_volumes_luns_get_builder, baremetalsolution_projects_locations_volumes_luns_get_task,
    baremetalsolution_projects_locations_volumes_luns_list_builder, baremetalsolution_projects_locations_volumes_luns_list_task,
    baremetalsolution_projects_locations_volumes_snapshots_create_builder, baremetalsolution_projects_locations_volumes_snapshots_create_task,
    baremetalsolution_projects_locations_volumes_snapshots_delete_builder, baremetalsolution_projects_locations_volumes_snapshots_delete_task,
    baremetalsolution_projects_locations_volumes_snapshots_get_builder, baremetalsolution_projects_locations_volumes_snapshots_get_task,
    baremetalsolution_projects_locations_volumes_snapshots_list_builder, baremetalsolution_projects_locations_volumes_snapshots_list_task,
    baremetalsolution_projects_locations_volumes_snapshots_restore_volume_snapshot_builder, baremetalsolution_projects_locations_volumes_snapshots_restore_volume_snapshot_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::baremetalsolution::Empty;
use crate::providers::gcp::clients::baremetalsolution::Instance;
use crate::providers::gcp::clients::baremetalsolution::ListInstancesResponse;
use crate::providers::gcp::clients::baremetalsolution::ListLocationsResponse;
use crate::providers::gcp::clients::baremetalsolution::ListLunsResponse;
use crate::providers::gcp::clients::baremetalsolution::ListNetworkUsageResponse;
use crate::providers::gcp::clients::baremetalsolution::ListNetworksResponse;
use crate::providers::gcp::clients::baremetalsolution::ListNfsSharesResponse;
use crate::providers::gcp::clients::baremetalsolution::ListOSImagesResponse;
use crate::providers::gcp::clients::baremetalsolution::ListProvisioningQuotasResponse;
use crate::providers::gcp::clients::baremetalsolution::ListSSHKeysResponse;
use crate::providers::gcp::clients::baremetalsolution::ListVolumeSnapshotsResponse;
use crate::providers::gcp::clients::baremetalsolution::ListVolumesResponse;
use crate::providers::gcp::clients::baremetalsolution::LoadInstanceAuthInfoResponse;
use crate::providers::gcp::clients::baremetalsolution::Location;
use crate::providers::gcp::clients::baremetalsolution::Lun;
use crate::providers::gcp::clients::baremetalsolution::Network;
use crate::providers::gcp::clients::baremetalsolution::NfsShare;
use crate::providers::gcp::clients::baremetalsolution::OSImage;
use crate::providers::gcp::clients::baremetalsolution::Operation;
use crate::providers::gcp::clients::baremetalsolution::ProvisioningConfig;
use crate::providers::gcp::clients::baremetalsolution::SSHKey;
use crate::providers::gcp::clients::baremetalsolution::SubmitProvisioningConfigResponse;
use crate::providers::gcp::clients::baremetalsolution::Volume;
use crate::providers::gcp::clients::baremetalsolution::VolumeSnapshot;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesDetachLunArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesDisableHyperthreadingArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesDisableInteractiveSerialConsoleArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesEnableHyperthreadingArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesEnableInteractiveSerialConsoleArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesLoadAuthInfoArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesPatchArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesReimageArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesRenameArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesResetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesStartArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsInstancesStopArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNetworksGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNetworksListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNetworksListNetworkUsageArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNetworksPatchArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNetworksRenameArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNfsSharesCreateArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNfsSharesDeleteArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNfsSharesGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNfsSharesListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNfsSharesPatchArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsNfsSharesRenameArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsOsImagesGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsOsImagesListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsProvisioningConfigsCreateArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsProvisioningConfigsGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsProvisioningConfigsPatchArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsProvisioningConfigsSubmitArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsProvisioningQuotasListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsSshKeysCreateArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsSshKeysDeleteArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsSshKeysListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesEvictArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesLunsEvictArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesLunsGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesLunsListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesPatchArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesRenameArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesResizeArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesSnapshotsCreateArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesSnapshotsDeleteArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesSnapshotsGetArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesSnapshotsListArgs;
use crate::providers::gcp::clients::baremetalsolution::BaremetalsolutionProjectsLocationsVolumesSnapshotsRestoreVolumeSnapshotArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BaremetalsolutionProvider with automatic state tracking.
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
/// let provider = BaremetalsolutionProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BaremetalsolutionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BaremetalsolutionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BaremetalsolutionProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Baremetalsolution projects locations get.
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
    pub fn baremetalsolution_projects_locations_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations list.
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
    pub fn baremetalsolution_projects_locations_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances detach lun.
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
    pub fn baremetalsolution_projects_locations_instances_detach_lun(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesDetachLunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_detach_lun_builder(
            &self.http_client,
            &args.instance,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_detach_lun_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances disable hyperthreading.
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
    pub fn baremetalsolution_projects_locations_instances_disable_hyperthreading(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesDisableHyperthreadingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_disable_hyperthreading_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_disable_hyperthreading_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances disable interactive serial console.
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
    pub fn baremetalsolution_projects_locations_instances_disable_interactive_serial_console(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesDisableInteractiveSerialConsoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_disable_interactive_serial_console_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_disable_interactive_serial_console_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances enable hyperthreading.
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
    pub fn baremetalsolution_projects_locations_instances_enable_hyperthreading(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesEnableHyperthreadingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_enable_hyperthreading_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_enable_hyperthreading_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances enable interactive serial console.
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
    pub fn baremetalsolution_projects_locations_instances_enable_interactive_serial_console(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesEnableInteractiveSerialConsoleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_enable_interactive_serial_console_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_enable_interactive_serial_console_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances get.
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
    pub fn baremetalsolution_projects_locations_instances_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances list.
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
    pub fn baremetalsolution_projects_locations_instances_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances load auth info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LoadInstanceAuthInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_instances_load_auth_info(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesLoadAuthInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LoadInstanceAuthInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_load_auth_info_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_load_auth_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances patch.
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
    pub fn baremetalsolution_projects_locations_instances_patch(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances reimage.
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
    pub fn baremetalsolution_projects_locations_instances_reimage(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesReimageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_reimage_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_reimage_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances rename.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_instances_rename(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_rename_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances reset.
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
    pub fn baremetalsolution_projects_locations_instances_reset(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_reset_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances start.
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
    pub fn baremetalsolution_projects_locations_instances_start(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations instances stop.
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
    pub fn baremetalsolution_projects_locations_instances_stop(
        &self,
        args: &BaremetalsolutionProjectsLocationsInstancesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_instances_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_instances_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations networks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Network result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_networks_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsNetworksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Network, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_networks_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_networks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations networks list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNetworksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_networks_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsNetworksListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNetworksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_networks_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_networks_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations networks list network usage.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNetworkUsageResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_networks_list_network_usage(
        &self,
        args: &BaremetalsolutionProjectsLocationsNetworksListNetworkUsageArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNetworkUsageResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_networks_list_network_usage_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_networks_list_network_usage_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations networks patch.
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
    pub fn baremetalsolution_projects_locations_networks_patch(
        &self,
        args: &BaremetalsolutionProjectsLocationsNetworksPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_networks_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_networks_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations networks rename.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Network result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_networks_rename(
        &self,
        args: &BaremetalsolutionProjectsLocationsNetworksRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Network, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_networks_rename_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_networks_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations nfs shares create.
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
    pub fn baremetalsolution_projects_locations_nfs_shares_create(
        &self,
        args: &BaremetalsolutionProjectsLocationsNfsSharesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_nfs_shares_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_nfs_shares_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations nfs shares delete.
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
    pub fn baremetalsolution_projects_locations_nfs_shares_delete(
        &self,
        args: &BaremetalsolutionProjectsLocationsNfsSharesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_nfs_shares_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_nfs_shares_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations nfs shares get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NfsShare result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_nfs_shares_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsNfsSharesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NfsShare, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_nfs_shares_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_nfs_shares_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations nfs shares list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNfsSharesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_nfs_shares_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsNfsSharesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNfsSharesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_nfs_shares_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_nfs_shares_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations nfs shares patch.
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
    pub fn baremetalsolution_projects_locations_nfs_shares_patch(
        &self,
        args: &BaremetalsolutionProjectsLocationsNfsSharesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_nfs_shares_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_nfs_shares_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations nfs shares rename.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the NfsShare result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_nfs_shares_rename(
        &self,
        args: &BaremetalsolutionProjectsLocationsNfsSharesRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<NfsShare, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_nfs_shares_rename_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_nfs_shares_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations operations get.
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
    pub fn baremetalsolution_projects_locations_operations_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations os images get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the OSImage result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_os_images_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsOsImagesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<OSImage, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_os_images_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_os_images_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations os images list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOSImagesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_os_images_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsOsImagesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOSImagesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_os_images_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_os_images_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations provisioning configs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProvisioningConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_provisioning_configs_create(
        &self,
        args: &BaremetalsolutionProjectsLocationsProvisioningConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProvisioningConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_provisioning_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.email,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_provisioning_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations provisioning configs get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProvisioningConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_provisioning_configs_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsProvisioningConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProvisioningConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_provisioning_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_provisioning_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations provisioning configs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProvisioningConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_provisioning_configs_patch(
        &self,
        args: &BaremetalsolutionProjectsLocationsProvisioningConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProvisioningConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_provisioning_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.email,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_provisioning_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations provisioning configs submit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubmitProvisioningConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_provisioning_configs_submit(
        &self,
        args: &BaremetalsolutionProjectsLocationsProvisioningConfigsSubmitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubmitProvisioningConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_provisioning_configs_submit_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_provisioning_configs_submit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations provisioning quotas list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProvisioningQuotasResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_provisioning_quotas_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsProvisioningQuotasListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProvisioningQuotasResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_provisioning_quotas_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_provisioning_quotas_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations ssh keys create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SSHKey result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_ssh_keys_create(
        &self,
        args: &BaremetalsolutionProjectsLocationsSshKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SSHKey, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_ssh_keys_create_builder(
            &self.http_client,
            &args.parent,
            &args.sshKeyId,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_ssh_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations ssh keys delete.
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
    pub fn baremetalsolution_projects_locations_ssh_keys_delete(
        &self,
        args: &BaremetalsolutionProjectsLocationsSshKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_ssh_keys_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_ssh_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations ssh keys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSSHKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_ssh_keys_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsSshKeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSSHKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_ssh_keys_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_ssh_keys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes evict.
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
    pub fn baremetalsolution_projects_locations_volumes_evict(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesEvictArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_evict_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_evict_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes get.
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
    pub fn baremetalsolution_projects_locations_volumes_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes list.
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
    pub fn baremetalsolution_projects_locations_volumes_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVolumesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes patch.
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
    pub fn baremetalsolution_projects_locations_volumes_patch(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes rename.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_volumes_rename(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesRenameArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Volume, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_rename_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_rename_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes resize.
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
    pub fn baremetalsolution_projects_locations_volumes_resize(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesResizeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_resize_builder(
            &self.http_client,
            &args.volume,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_resize_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes luns evict.
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
    pub fn baremetalsolution_projects_locations_volumes_luns_evict(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesLunsEvictArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_luns_evict_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_luns_evict_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes luns get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Lun result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_volumes_luns_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesLunsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Lun, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_luns_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_luns_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes luns list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLunsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_volumes_luns_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesLunsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLunsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_luns_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_luns_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes snapshots create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VolumeSnapshot result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn baremetalsolution_projects_locations_volumes_snapshots_create(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesSnapshotsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VolumeSnapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_snapshots_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_snapshots_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes snapshots delete.
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
    pub fn baremetalsolution_projects_locations_volumes_snapshots_delete(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesSnapshotsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_snapshots_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_snapshots_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes snapshots get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VolumeSnapshot result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_volumes_snapshots_get(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesSnapshotsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VolumeSnapshot, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_snapshots_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_snapshots_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes snapshots list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVolumeSnapshotsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn baremetalsolution_projects_locations_volumes_snapshots_list(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesSnapshotsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVolumeSnapshotsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_snapshots_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_snapshots_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Baremetalsolution projects locations volumes snapshots restore volume snapshot.
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
    pub fn baremetalsolution_projects_locations_volumes_snapshots_restore_volume_snapshot(
        &self,
        args: &BaremetalsolutionProjectsLocationsVolumesSnapshotsRestoreVolumeSnapshotArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = baremetalsolution_projects_locations_volumes_snapshots_restore_volume_snapshot_builder(
            &self.http_client,
            &args.volumeSnapshot,
        )
        .map_err(ProviderError::Api)?;

        let task = baremetalsolution_projects_locations_volumes_snapshots_restore_volume_snapshot_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
