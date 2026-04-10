//! DatalineageProvider - State-aware datalineage API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       datalineage API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::datalineage::{
    datalineage_folders_locations_config_patch_builder, datalineage_folders_locations_config_patch_task,
    datalineage_organizations_locations_config_patch_builder, datalineage_organizations_locations_config_patch_task,
    datalineage_projects_locations_batch_search_link_processes_builder, datalineage_projects_locations_batch_search_link_processes_task,
    datalineage_projects_locations_process_open_lineage_run_event_builder, datalineage_projects_locations_process_open_lineage_run_event_task,
    datalineage_projects_locations_search_links_builder, datalineage_projects_locations_search_links_task,
    datalineage_projects_locations_config_patch_builder, datalineage_projects_locations_config_patch_task,
    datalineage_projects_locations_operations_cancel_builder, datalineage_projects_locations_operations_cancel_task,
    datalineage_projects_locations_operations_delete_builder, datalineage_projects_locations_operations_delete_task,
    datalineage_projects_locations_processes_create_builder, datalineage_projects_locations_processes_create_task,
    datalineage_projects_locations_processes_delete_builder, datalineage_projects_locations_processes_delete_task,
    datalineage_projects_locations_processes_patch_builder, datalineage_projects_locations_processes_patch_task,
    datalineage_projects_locations_processes_runs_create_builder, datalineage_projects_locations_processes_runs_create_task,
    datalineage_projects_locations_processes_runs_delete_builder, datalineage_projects_locations_processes_runs_delete_task,
    datalineage_projects_locations_processes_runs_patch_builder, datalineage_projects_locations_processes_runs_patch_task,
    datalineage_projects_locations_processes_runs_lineage_events_create_builder, datalineage_projects_locations_processes_runs_lineage_events_create_task,
    datalineage_projects_locations_processes_runs_lineage_events_delete_builder, datalineage_projects_locations_processes_runs_lineage_events_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datalineage::GoogleCloudDatacatalogLineageConfigmanagementV1Config;
use crate::providers::gcp::clients::datalineage::GoogleCloudDatacatalogLineageV1BatchSearchLinkProcessesResponse;
use crate::providers::gcp::clients::datalineage::GoogleCloudDatacatalogLineageV1LineageEvent;
use crate::providers::gcp::clients::datalineage::GoogleCloudDatacatalogLineageV1Process;
use crate::providers::gcp::clients::datalineage::GoogleCloudDatacatalogLineageV1ProcessOpenLineageRunEventResponse;
use crate::providers::gcp::clients::datalineage::GoogleCloudDatacatalogLineageV1Run;
use crate::providers::gcp::clients::datalineage::GoogleCloudDatacatalogLineageV1SearchLinksResponse;
use crate::providers::gcp::clients::datalineage::GoogleLongrunningOperation;
use crate::providers::gcp::clients::datalineage::GoogleProtobufEmpty;
use crate::providers::gcp::clients::datalineage::DatalineageFoldersLocationsConfigPatchArgs;
use crate::providers::gcp::clients::datalineage::DatalineageOrganizationsLocationsConfigPatchArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsBatchSearchLinkProcessesArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsConfigPatchArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessOpenLineageRunEventArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesCreateArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesDeleteArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesPatchArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesRunsCreateArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesRunsDeleteArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesRunsLineageEventsCreateArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesRunsLineageEventsDeleteArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsProcessesRunsPatchArgs;
use crate::providers::gcp::clients::datalineage::DatalineageProjectsLocationsSearchLinksArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatalineageProvider with automatic state tracking.
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
/// let provider = DatalineageProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DatalineageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DatalineageProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DatalineageProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Datalineage folders locations config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageConfigmanagementV1Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_folders_locations_config_patch(
        &self,
        args: &DatalineageFoldersLocationsConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageConfigmanagementV1Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_folders_locations_config_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_folders_locations_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage organizations locations config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageConfigmanagementV1Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_organizations_locations_config_patch(
        &self,
        args: &DatalineageOrganizationsLocationsConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageConfigmanagementV1Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_organizations_locations_config_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_organizations_locations_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations batch search link processes.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1BatchSearchLinkProcessesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_batch_search_link_processes(
        &self,
        args: &DatalineageProjectsLocationsBatchSearchLinkProcessesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1BatchSearchLinkProcessesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_batch_search_link_processes_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_batch_search_link_processes_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations process open lineage run event.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1ProcessOpenLineageRunEventResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_process_open_lineage_run_event(
        &self,
        args: &DatalineageProjectsLocationsProcessOpenLineageRunEventArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1ProcessOpenLineageRunEventResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_process_open_lineage_run_event_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_process_open_lineage_run_event_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations search links.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1SearchLinksResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_search_links(
        &self,
        args: &DatalineageProjectsLocationsSearchLinksArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1SearchLinksResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_search_links_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_search_links_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations config patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageConfigmanagementV1Config result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_config_patch(
        &self,
        args: &DatalineageProjectsLocationsConfigPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageConfigmanagementV1Config, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_config_patch_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_config_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_operations_cancel(
        &self,
        args: &DatalineageProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations operations delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_operations_delete(
        &self,
        args: &DatalineageProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1Process result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_processes_create(
        &self,
        args: &DatalineageProjectsLocationsProcessesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1Process, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes delete.
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
    pub fn datalineage_projects_locations_processes_delete(
        &self,
        args: &DatalineageProjectsLocationsProcessesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1Process result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_processes_patch(
        &self,
        args: &DatalineageProjectsLocationsProcessesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1Process, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes runs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1Run result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_processes_runs_create(
        &self,
        args: &DatalineageProjectsLocationsProcessesRunsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1Run, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_runs_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_runs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes runs delete.
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
    pub fn datalineage_projects_locations_processes_runs_delete(
        &self,
        args: &DatalineageProjectsLocationsProcessesRunsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_runs_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_runs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes runs patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1Run result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_processes_runs_patch(
        &self,
        args: &DatalineageProjectsLocationsProcessesRunsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1Run, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_runs_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_runs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes runs lineage events create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudDatacatalogLineageV1LineageEvent result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_processes_runs_lineage_events_create(
        &self,
        args: &DatalineageProjectsLocationsProcessesRunsLineageEventsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudDatacatalogLineageV1LineageEvent, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_runs_lineage_events_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_runs_lineage_events_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datalineage projects locations processes runs lineage events delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datalineage_projects_locations_processes_runs_lineage_events_delete(
        &self,
        args: &DatalineageProjectsLocationsProcessesRunsLineageEventsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datalineage_projects_locations_processes_runs_lineage_events_delete_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
        )
        .map_err(ProviderError::Api)?;

        let task = datalineage_projects_locations_processes_runs_lineage_events_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
