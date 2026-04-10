//! OracledatabaseProvider - State-aware oracledatabase API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       oracledatabase API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::oracledatabase::{
    oracledatabase_projects_locations_autonomous_databases_create_builder, oracledatabase_projects_locations_autonomous_databases_create_task,
    oracledatabase_projects_locations_autonomous_databases_delete_builder, oracledatabase_projects_locations_autonomous_databases_delete_task,
    oracledatabase_projects_locations_autonomous_databases_failover_builder, oracledatabase_projects_locations_autonomous_databases_failover_task,
    oracledatabase_projects_locations_autonomous_databases_generate_wallet_builder, oracledatabase_projects_locations_autonomous_databases_generate_wallet_task,
    oracledatabase_projects_locations_autonomous_databases_patch_builder, oracledatabase_projects_locations_autonomous_databases_patch_task,
    oracledatabase_projects_locations_autonomous_databases_restart_builder, oracledatabase_projects_locations_autonomous_databases_restart_task,
    oracledatabase_projects_locations_autonomous_databases_restore_builder, oracledatabase_projects_locations_autonomous_databases_restore_task,
    oracledatabase_projects_locations_autonomous_databases_start_builder, oracledatabase_projects_locations_autonomous_databases_start_task,
    oracledatabase_projects_locations_autonomous_databases_stop_builder, oracledatabase_projects_locations_autonomous_databases_stop_task,
    oracledatabase_projects_locations_autonomous_databases_switchover_builder, oracledatabase_projects_locations_autonomous_databases_switchover_task,
    oracledatabase_projects_locations_cloud_exadata_infrastructures_create_builder, oracledatabase_projects_locations_cloud_exadata_infrastructures_create_task,
    oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_builder, oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_task,
    oracledatabase_projects_locations_cloud_vm_clusters_create_builder, oracledatabase_projects_locations_cloud_vm_clusters_create_task,
    oracledatabase_projects_locations_cloud_vm_clusters_delete_builder, oracledatabase_projects_locations_cloud_vm_clusters_delete_task,
    oracledatabase_projects_locations_db_systems_create_builder, oracledatabase_projects_locations_db_systems_create_task,
    oracledatabase_projects_locations_db_systems_delete_builder, oracledatabase_projects_locations_db_systems_delete_task,
    oracledatabase_projects_locations_exadb_vm_clusters_create_builder, oracledatabase_projects_locations_exadb_vm_clusters_create_task,
    oracledatabase_projects_locations_exadb_vm_clusters_delete_builder, oracledatabase_projects_locations_exadb_vm_clusters_delete_task,
    oracledatabase_projects_locations_exadb_vm_clusters_patch_builder, oracledatabase_projects_locations_exadb_vm_clusters_patch_task,
    oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_builder, oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_task,
    oracledatabase_projects_locations_exascale_db_storage_vaults_create_builder, oracledatabase_projects_locations_exascale_db_storage_vaults_create_task,
    oracledatabase_projects_locations_exascale_db_storage_vaults_delete_builder, oracledatabase_projects_locations_exascale_db_storage_vaults_delete_task,
    oracledatabase_projects_locations_odb_networks_create_builder, oracledatabase_projects_locations_odb_networks_create_task,
    oracledatabase_projects_locations_odb_networks_delete_builder, oracledatabase_projects_locations_odb_networks_delete_task,
    oracledatabase_projects_locations_odb_networks_odb_subnets_create_builder, oracledatabase_projects_locations_odb_networks_odb_subnets_create_task,
    oracledatabase_projects_locations_odb_networks_odb_subnets_delete_builder, oracledatabase_projects_locations_odb_networks_odb_subnets_delete_task,
    oracledatabase_projects_locations_operations_cancel_builder, oracledatabase_projects_locations_operations_cancel_task,
    oracledatabase_projects_locations_operations_delete_builder, oracledatabase_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::oracledatabase::Empty;
use crate::providers::gcp::clients::oracledatabase::GenerateAutonomousDatabaseWalletResponse;
use crate::providers::gcp::clients::oracledatabase::Operation;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesFailoverArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesGenerateWalletArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesPatchArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesRestartArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesRestoreArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesStartArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesStopArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsAutonomousDatabasesSwitchoverArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudExadataInfrastructuresCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudExadataInfrastructuresDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudVmClustersCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsCloudVmClustersDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemsCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsDbSystemsDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersPatchArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExadbVmClustersRemoveVirtualMachineArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExascaleDbStorageVaultsCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsExascaleDbStorageVaultsDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsCreateArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsDeleteArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::oracledatabase::OracledatabaseProjectsLocationsOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// OracledatabaseProvider with automatic state tracking.
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
/// let provider = OracledatabaseProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct OracledatabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> OracledatabaseProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new OracledatabaseProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Oracledatabase projects locations autonomous databases create.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_create(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_create_builder(
            &self.http_client,
            &args.parent,
            &args.autonomousDatabaseId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases delete.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_delete(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases failover.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_failover(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesFailoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_failover_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_failover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases generate wallet.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateAutonomousDatabaseWalletResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn oracledatabase_projects_locations_autonomous_databases_generate_wallet(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesGenerateWalletArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateAutonomousDatabaseWalletResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_generate_wallet_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_generate_wallet_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases patch.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_patch(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases restart.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_restart(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesRestartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_restart_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_restart_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases restore.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_restore(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesRestoreArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_restore_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_restore_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases start.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_start(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases stop.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_stop(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations autonomous databases switchover.
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
    pub fn oracledatabase_projects_locations_autonomous_databases_switchover(
        &self,
        args: &OracledatabaseProjectsLocationsAutonomousDatabasesSwitchoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_autonomous_databases_switchover_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_autonomous_databases_switchover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud exadata infrastructures create.
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
    pub fn oracledatabase_projects_locations_cloud_exadata_infrastructures_create(
        &self,
        args: &OracledatabaseProjectsLocationsCloudExadataInfrastructuresCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_exadata_infrastructures_create_builder(
            &self.http_client,
            &args.parent,
            &args.cloudExadataInfrastructureId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_exadata_infrastructures_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud exadata infrastructures delete.
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
    pub fn oracledatabase_projects_locations_cloud_exadata_infrastructures_delete(
        &self,
        args: &OracledatabaseProjectsLocationsCloudExadataInfrastructuresDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_exadata_infrastructures_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud vm clusters create.
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
    pub fn oracledatabase_projects_locations_cloud_vm_clusters_create(
        &self,
        args: &OracledatabaseProjectsLocationsCloudVmClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_vm_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.cloudVmClusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_vm_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations cloud vm clusters delete.
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
    pub fn oracledatabase_projects_locations_cloud_vm_clusters_delete(
        &self,
        args: &OracledatabaseProjectsLocationsCloudVmClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_cloud_vm_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_cloud_vm_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db systems create.
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
    pub fn oracledatabase_projects_locations_db_systems_create(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_systems_create_builder(
            &self.http_client,
            &args.parent,
            &args.dbSystemId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_systems_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations db systems delete.
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
    pub fn oracledatabase_projects_locations_db_systems_delete(
        &self,
        args: &OracledatabaseProjectsLocationsDbSystemsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_db_systems_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_db_systems_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters create.
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
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_create(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_create_builder(
            &self.http_client,
            &args.parent,
            &args.exadbVmClusterId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters delete.
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
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_delete(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters patch.
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
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_patch(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exadb vm clusters remove virtual machine.
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
    pub fn oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine(
        &self,
        args: &OracledatabaseProjectsLocationsExadbVmClustersRemoveVirtualMachineArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exadb_vm_clusters_remove_virtual_machine_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exascale db storage vaults create.
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
    pub fn oracledatabase_projects_locations_exascale_db_storage_vaults_create(
        &self,
        args: &OracledatabaseProjectsLocationsExascaleDbStorageVaultsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exascale_db_storage_vaults_create_builder(
            &self.http_client,
            &args.parent,
            &args.exascaleDbStorageVaultId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exascale_db_storage_vaults_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations exascale db storage vaults delete.
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
    pub fn oracledatabase_projects_locations_exascale_db_storage_vaults_delete(
        &self,
        args: &OracledatabaseProjectsLocationsExascaleDbStorageVaultsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_exascale_db_storage_vaults_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_exascale_db_storage_vaults_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks create.
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
    pub fn oracledatabase_projects_locations_odb_networks_create(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_create_builder(
            &self.http_client,
            &args.parent,
            &args.odbNetworkId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks delete.
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
    pub fn oracledatabase_projects_locations_odb_networks_delete(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks odb subnets create.
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
    pub fn oracledatabase_projects_locations_odb_networks_odb_subnets_create(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_odb_subnets_create_builder(
            &self.http_client,
            &args.parent,
            &args.odbSubnetId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_odb_subnets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations odb networks odb subnets delete.
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
    pub fn oracledatabase_projects_locations_odb_networks_odb_subnets_delete(
        &self,
        args: &OracledatabaseProjectsLocationsOdbNetworksOdbSubnetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_odb_networks_odb_subnets_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_odb_networks_odb_subnets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations operations cancel.
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
    pub fn oracledatabase_projects_locations_operations_cancel(
        &self,
        args: &OracledatabaseProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Oracledatabase projects locations operations delete.
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
    pub fn oracledatabase_projects_locations_operations_delete(
        &self,
        args: &OracledatabaseProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = oracledatabase_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = oracledatabase_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
