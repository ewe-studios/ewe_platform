//! AlloydbProvider - State-aware alloydb API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       alloydb API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::alloydb::{
    alloydb_projects_locations_get_builder, alloydb_projects_locations_get_task,
    alloydb_projects_locations_list_builder, alloydb_projects_locations_list_task,
    alloydb_projects_locations_backups_create_builder, alloydb_projects_locations_backups_create_task,
    alloydb_projects_locations_backups_delete_builder, alloydb_projects_locations_backups_delete_task,
    alloydb_projects_locations_backups_get_builder, alloydb_projects_locations_backups_get_task,
    alloydb_projects_locations_backups_list_builder, alloydb_projects_locations_backups_list_task,
    alloydb_projects_locations_backups_patch_builder, alloydb_projects_locations_backups_patch_task,
    alloydb_projects_locations_clusters_create_builder, alloydb_projects_locations_clusters_create_task,
    alloydb_projects_locations_clusters_createsecondary_builder, alloydb_projects_locations_clusters_createsecondary_task,
    alloydb_projects_locations_clusters_delete_builder, alloydb_projects_locations_clusters_delete_task,
    alloydb_projects_locations_clusters_export_builder, alloydb_projects_locations_clusters_export_task,
    alloydb_projects_locations_clusters_get_builder, alloydb_projects_locations_clusters_get_task,
    alloydb_projects_locations_clusters_import_builder, alloydb_projects_locations_clusters_import_task,
    alloydb_projects_locations_clusters_list_builder, alloydb_projects_locations_clusters_list_task,
    alloydb_projects_locations_clusters_patch_builder, alloydb_projects_locations_clusters_patch_task,
    alloydb_projects_locations_clusters_promote_builder, alloydb_projects_locations_clusters_promote_task,
    alloydb_projects_locations_clusters_restore_builder, alloydb_projects_locations_clusters_restore_task,
    alloydb_projects_locations_clusters_restore_from_cloud_s_q_l_builder, alloydb_projects_locations_clusters_restore_from_cloud_s_q_l_task,
    alloydb_projects_locations_clusters_switchover_builder, alloydb_projects_locations_clusters_switchover_task,
    alloydb_projects_locations_clusters_upgrade_builder, alloydb_projects_locations_clusters_upgrade_task,
    alloydb_projects_locations_clusters_instances_create_builder, alloydb_projects_locations_clusters_instances_create_task,
    alloydb_projects_locations_clusters_instances_createsecondary_builder, alloydb_projects_locations_clusters_instances_createsecondary_task,
    alloydb_projects_locations_clusters_instances_delete_builder, alloydb_projects_locations_clusters_instances_delete_task,
    alloydb_projects_locations_clusters_instances_failover_builder, alloydb_projects_locations_clusters_instances_failover_task,
    alloydb_projects_locations_clusters_instances_get_builder, alloydb_projects_locations_clusters_instances_get_task,
    alloydb_projects_locations_clusters_instances_get_connection_info_builder, alloydb_projects_locations_clusters_instances_get_connection_info_task,
    alloydb_projects_locations_clusters_instances_inject_fault_builder, alloydb_projects_locations_clusters_instances_inject_fault_task,
    alloydb_projects_locations_clusters_instances_list_builder, alloydb_projects_locations_clusters_instances_list_task,
    alloydb_projects_locations_clusters_instances_patch_builder, alloydb_projects_locations_clusters_instances_patch_task,
    alloydb_projects_locations_clusters_instances_restart_builder, alloydb_projects_locations_clusters_instances_restart_task,
    alloydb_projects_locations_clusters_users_create_builder, alloydb_projects_locations_clusters_users_create_task,
    alloydb_projects_locations_clusters_users_delete_builder, alloydb_projects_locations_clusters_users_delete_task,
    alloydb_projects_locations_clusters_users_get_builder, alloydb_projects_locations_clusters_users_get_task,
    alloydb_projects_locations_clusters_users_list_builder, alloydb_projects_locations_clusters_users_list_task,
    alloydb_projects_locations_clusters_users_patch_builder, alloydb_projects_locations_clusters_users_patch_task,
    alloydb_projects_locations_operations_cancel_builder, alloydb_projects_locations_operations_cancel_task,
    alloydb_projects_locations_operations_delete_builder, alloydb_projects_locations_operations_delete_task,
    alloydb_projects_locations_operations_get_builder, alloydb_projects_locations_operations_get_task,
    alloydb_projects_locations_operations_list_builder, alloydb_projects_locations_operations_list_task,
    alloydb_projects_locations_supported_database_flags_list_builder, alloydb_projects_locations_supported_database_flags_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::alloydb::Backup;
use crate::providers::gcp::clients::alloydb::Cluster;
use crate::providers::gcp::clients::alloydb::ConnectionInfo;
use crate::providers::gcp::clients::alloydb::Empty;
use crate::providers::gcp::clients::alloydb::GoogleCloudLocationListLocationsResponse;
use crate::providers::gcp::clients::alloydb::GoogleCloudLocationLocation;
use crate::providers::gcp::clients::alloydb::Instance;
use crate::providers::gcp::clients::alloydb::ListBackupsResponse;
use crate::providers::gcp::clients::alloydb::ListClustersResponse;
use crate::providers::gcp::clients::alloydb::ListInstancesResponse;
use crate::providers::gcp::clients::alloydb::ListOperationsResponse;
use crate::providers::gcp::clients::alloydb::ListSupportedDatabaseFlagsResponse;
use crate::providers::gcp::clients::alloydb::ListUsersResponse;
use crate::providers::gcp::clients::alloydb::Operation;
use crate::providers::gcp::clients::alloydb::User;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsGetArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsListArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersCreatesecondaryArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersExportArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersGetArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersImportArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesCreatesecondaryArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesFailoverArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesGetArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesGetConnectionInfoArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesInjectFaultArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesListArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesRestartArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersListArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersPromoteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersRestoreArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersRestoreFromCloudSQLArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersSwitchoverArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUpgradeArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersGetArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersListArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsGetArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsListArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsSupportedDatabaseFlagsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AlloydbProvider with automatic state tracking.
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
/// let provider = AlloydbProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AlloydbProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AlloydbProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AlloydbProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Alloydb projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudLocationLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alloydb_projects_locations_get(
        &self,
        args: &AlloydbProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudLocationLocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudLocationListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alloydb_projects_locations_list(
        &self,
        args: &AlloydbProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudLocationListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations backups create.
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
    pub fn alloydb_projects_locations_backups_create(
        &self,
        args: &AlloydbProjectsLocationsBackupsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_backups_create_builder(
            &self.http_client,
            &args.parent,
            &args.backupId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_backups_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations backups delete.
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
    pub fn alloydb_projects_locations_backups_delete(
        &self,
        args: &AlloydbProjectsLocationsBackupsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_backups_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_backups_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations backups get.
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
    pub fn alloydb_projects_locations_backups_get(
        &self,
        args: &AlloydbProjectsLocationsBackupsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Backup, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_backups_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_backups_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations backups list.
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
    pub fn alloydb_projects_locations_backups_list(
        &self,
        args: &AlloydbProjectsLocationsBackupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBackupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_backups_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_backups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations backups patch.
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
    pub fn alloydb_projects_locations_backups_patch(
        &self,
        args: &AlloydbProjectsLocationsBackupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_backups_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_backups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters create.
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
    pub fn alloydb_projects_locations_clusters_create(
        &self,
        args: &AlloydbProjectsLocationsClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters createsecondary.
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
    pub fn alloydb_projects_locations_clusters_createsecondary(
        &self,
        args: &AlloydbProjectsLocationsClustersCreatesecondaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_createsecondary_builder(
            &self.http_client,
            &args.parent,
            &args.clusterId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_createsecondary_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters delete.
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
    pub fn alloydb_projects_locations_clusters_delete(
        &self,
        args: &AlloydbProjectsLocationsClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters export.
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
    pub fn alloydb_projects_locations_clusters_export(
        &self,
        args: &AlloydbProjectsLocationsClustersExportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_export_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_export_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Cluster result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alloydb_projects_locations_clusters_get(
        &self,
        args: &AlloydbProjectsLocationsClustersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Cluster, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters import.
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
    pub fn alloydb_projects_locations_clusters_import(
        &self,
        args: &AlloydbProjectsLocationsClustersImportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_import_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_import_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListClustersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alloydb_projects_locations_clusters_list(
        &self,
        args: &AlloydbProjectsLocationsClustersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListClustersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters patch.
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
    pub fn alloydb_projects_locations_clusters_patch(
        &self,
        args: &AlloydbProjectsLocationsClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters promote.
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
    pub fn alloydb_projects_locations_clusters_promote(
        &self,
        args: &AlloydbProjectsLocationsClustersPromoteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_promote_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_promote_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters restore.
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
    pub fn alloydb_projects_locations_clusters_restore(
        &self,
        args: &AlloydbProjectsLocationsClustersRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_restore_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters restore from cloud s q l.
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
    pub fn alloydb_projects_locations_clusters_restore_from_cloud_s_q_l(
        &self,
        args: &AlloydbProjectsLocationsClustersRestoreFromCloudSQLArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_restore_from_cloud_s_q_l_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_restore_from_cloud_s_q_l_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters switchover.
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
    pub fn alloydb_projects_locations_clusters_switchover(
        &self,
        args: &AlloydbProjectsLocationsClustersSwitchoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_switchover_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_switchover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters upgrade.
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
    pub fn alloydb_projects_locations_clusters_upgrade(
        &self,
        args: &AlloydbProjectsLocationsClustersUpgradeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_upgrade_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_upgrade_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances create.
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
    pub fn alloydb_projects_locations_clusters_instances_create(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_create_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances createsecondary.
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
    pub fn alloydb_projects_locations_clusters_instances_createsecondary(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesCreatesecondaryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_createsecondary_builder(
            &self.http_client,
            &args.parent,
            &args.instanceId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_createsecondary_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances delete.
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
    pub fn alloydb_projects_locations_clusters_instances_delete(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances failover.
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
    pub fn alloydb_projects_locations_clusters_instances_failover(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesFailoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_failover_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_failover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances get.
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
    pub fn alloydb_projects_locations_clusters_instances_get(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Instance, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances get connection info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alloydb_projects_locations_clusters_instances_get_connection_info(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesGetConnectionInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_get_connection_info_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_get_connection_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances inject fault.
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
    pub fn alloydb_projects_locations_clusters_instances_inject_fault(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesInjectFaultArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_inject_fault_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_inject_fault_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances list.
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
    pub fn alloydb_projects_locations_clusters_instances_list(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListInstancesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances patch.
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
    pub fn alloydb_projects_locations_clusters_instances_patch(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters instances restart.
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
    pub fn alloydb_projects_locations_clusters_instances_restart(
        &self,
        args: &AlloydbProjectsLocationsClustersInstancesRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_instances_restart_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_instances_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters users create.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn alloydb_projects_locations_clusters_users_create(
        &self,
        args: &AlloydbProjectsLocationsClustersUsersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_users_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.userId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_users_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters users delete.
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
    pub fn alloydb_projects_locations_clusters_users_delete(
        &self,
        args: &AlloydbProjectsLocationsClustersUsersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_users_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_users_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters users get.
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
    pub fn alloydb_projects_locations_clusters_users_get(
        &self,
        args: &AlloydbProjectsLocationsClustersUsersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_users_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_users_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters users list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListUsersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alloydb_projects_locations_clusters_users_list(
        &self,
        args: &AlloydbProjectsLocationsClustersUsersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListUsersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_users_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_users_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations clusters users patch.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn alloydb_projects_locations_clusters_users_patch(
        &self,
        args: &AlloydbProjectsLocationsClustersUsersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<User, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_clusters_users_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_clusters_users_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations operations cancel.
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
    pub fn alloydb_projects_locations_operations_cancel(
        &self,
        args: &AlloydbProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations operations delete.
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
    pub fn alloydb_projects_locations_operations_delete(
        &self,
        args: &AlloydbProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations operations get.
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
    pub fn alloydb_projects_locations_operations_get(
        &self,
        args: &AlloydbProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations operations list.
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
    pub fn alloydb_projects_locations_operations_list(
        &self,
        args: &AlloydbProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Alloydb projects locations supported database flags list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSupportedDatabaseFlagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn alloydb_projects_locations_supported_database_flags_list(
        &self,
        args: &AlloydbProjectsLocationsSupportedDatabaseFlagsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSupportedDatabaseFlagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = alloydb_projects_locations_supported_database_flags_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.scope,
        )
        .map_err(ProviderError::Api)?;

        let task = alloydb_projects_locations_supported_database_flags_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
