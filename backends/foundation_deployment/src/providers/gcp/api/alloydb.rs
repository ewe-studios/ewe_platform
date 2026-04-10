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
    alloydb_projects_locations_backups_create_builder, alloydb_projects_locations_backups_create_task,
    alloydb_projects_locations_backups_delete_builder, alloydb_projects_locations_backups_delete_task,
    alloydb_projects_locations_backups_patch_builder, alloydb_projects_locations_backups_patch_task,
    alloydb_projects_locations_clusters_create_builder, alloydb_projects_locations_clusters_create_task,
    alloydb_projects_locations_clusters_createsecondary_builder, alloydb_projects_locations_clusters_createsecondary_task,
    alloydb_projects_locations_clusters_delete_builder, alloydb_projects_locations_clusters_delete_task,
    alloydb_projects_locations_clusters_export_builder, alloydb_projects_locations_clusters_export_task,
    alloydb_projects_locations_clusters_import_builder, alloydb_projects_locations_clusters_import_task,
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
    alloydb_projects_locations_clusters_instances_inject_fault_builder, alloydb_projects_locations_clusters_instances_inject_fault_task,
    alloydb_projects_locations_clusters_instances_patch_builder, alloydb_projects_locations_clusters_instances_patch_task,
    alloydb_projects_locations_clusters_instances_restart_builder, alloydb_projects_locations_clusters_instances_restart_task,
    alloydb_projects_locations_clusters_users_create_builder, alloydb_projects_locations_clusters_users_create_task,
    alloydb_projects_locations_clusters_users_delete_builder, alloydb_projects_locations_clusters_users_delete_task,
    alloydb_projects_locations_clusters_users_patch_builder, alloydb_projects_locations_clusters_users_patch_task,
    alloydb_projects_locations_operations_cancel_builder, alloydb_projects_locations_operations_cancel_task,
    alloydb_projects_locations_operations_delete_builder, alloydb_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::alloydb::Empty;
use crate::providers::gcp::clients::alloydb::Operation;
use crate::providers::gcp::clients::alloydb::User;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsBackupsPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersCreatesecondaryArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersExportArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersImportArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesCreatesecondaryArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesFailoverArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesInjectFaultArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersInstancesRestartArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersPromoteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersRestoreArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersRestoreFromCloudSQLArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersSwitchoverArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUpgradeArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersCreateArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersDeleteArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsClustersUsersPatchArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::alloydb::AlloydbProjectsLocationsOperationsDeleteArgs;
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

}
