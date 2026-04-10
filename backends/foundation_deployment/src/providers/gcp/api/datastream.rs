//! DatastreamProvider - State-aware datastream API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       datastream API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::datastream::{
    datastream_projects_locations_connection_profiles_create_builder, datastream_projects_locations_connection_profiles_create_task,
    datastream_projects_locations_connection_profiles_delete_builder, datastream_projects_locations_connection_profiles_delete_task,
    datastream_projects_locations_connection_profiles_discover_builder, datastream_projects_locations_connection_profiles_discover_task,
    datastream_projects_locations_connection_profiles_patch_builder, datastream_projects_locations_connection_profiles_patch_task,
    datastream_projects_locations_operations_cancel_builder, datastream_projects_locations_operations_cancel_task,
    datastream_projects_locations_operations_delete_builder, datastream_projects_locations_operations_delete_task,
    datastream_projects_locations_private_connections_create_builder, datastream_projects_locations_private_connections_create_task,
    datastream_projects_locations_private_connections_delete_builder, datastream_projects_locations_private_connections_delete_task,
    datastream_projects_locations_private_connections_routes_create_builder, datastream_projects_locations_private_connections_routes_create_task,
    datastream_projects_locations_private_connections_routes_delete_builder, datastream_projects_locations_private_connections_routes_delete_task,
    datastream_projects_locations_streams_create_builder, datastream_projects_locations_streams_create_task,
    datastream_projects_locations_streams_delete_builder, datastream_projects_locations_streams_delete_task,
    datastream_projects_locations_streams_patch_builder, datastream_projects_locations_streams_patch_task,
    datastream_projects_locations_streams_run_builder, datastream_projects_locations_streams_run_task,
    datastream_projects_locations_streams_objects_lookup_builder, datastream_projects_locations_streams_objects_lookup_task,
    datastream_projects_locations_streams_objects_start_backfill_job_builder, datastream_projects_locations_streams_objects_start_backfill_job_task,
    datastream_projects_locations_streams_objects_stop_backfill_job_builder, datastream_projects_locations_streams_objects_stop_backfill_job_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datastream::DiscoverConnectionProfileResponse;
use crate::providers::gcp::clients::datastream::Empty;
use crate::providers::gcp::clients::datastream::Operation;
use crate::providers::gcp::clients::datastream::StartBackfillJobResponse;
use crate::providers::gcp::clients::datastream::StopBackfillJobResponse;
use crate::providers::gcp::clients::datastream::StreamObject;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesDiscoverArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesPatchArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsRoutesCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsRoutesDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsLookupArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsStartBackfillJobArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsStopBackfillJobArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsPatchArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsRunArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatastreamProvider with automatic state tracking.
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
/// let provider = DatastreamProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DatastreamProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DatastreamProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DatastreamProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Datastream projects locations connection profiles create.
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
    pub fn datastream_projects_locations_connection_profiles_create(
        &self,
        args: &DatastreamProjectsLocationsConnectionProfilesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_connection_profiles_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectionProfileId,
            &args.force,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_connection_profiles_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations connection profiles delete.
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
    pub fn datastream_projects_locations_connection_profiles_delete(
        &self,
        args: &DatastreamProjectsLocationsConnectionProfilesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_connection_profiles_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_connection_profiles_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations connection profiles discover.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DiscoverConnectionProfileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datastream_projects_locations_connection_profiles_discover(
        &self,
        args: &DatastreamProjectsLocationsConnectionProfilesDiscoverArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DiscoverConnectionProfileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_connection_profiles_discover_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_connection_profiles_discover_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations connection profiles patch.
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
    pub fn datastream_projects_locations_connection_profiles_patch(
        &self,
        args: &DatastreamProjectsLocationsConnectionProfilesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_connection_profiles_patch_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_connection_profiles_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations operations cancel.
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
    pub fn datastream_projects_locations_operations_cancel(
        &self,
        args: &DatastreamProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations operations delete.
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
    pub fn datastream_projects_locations_operations_delete(
        &self,
        args: &DatastreamProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations private connections create.
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
    pub fn datastream_projects_locations_private_connections_create(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.force,
            &args.privateConnectionId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations private connections delete.
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
    pub fn datastream_projects_locations_private_connections_delete(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations private connections routes create.
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
    pub fn datastream_projects_locations_private_connections_routes_create(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsRoutesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_routes_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.routeId,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_routes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations private connections routes delete.
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
    pub fn datastream_projects_locations_private_connections_routes_delete(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsRoutesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_routes_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_routes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams create.
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
    pub fn datastream_projects_locations_streams_create(
        &self,
        args: &DatastreamProjectsLocationsStreamsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_create_builder(
            &self.http_client,
            &args.parent,
            &args.force,
            &args.requestId,
            &args.streamId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams delete.
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
    pub fn datastream_projects_locations_streams_delete(
        &self,
        args: &DatastreamProjectsLocationsStreamsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams patch.
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
    pub fn datastream_projects_locations_streams_patch(
        &self,
        args: &DatastreamProjectsLocationsStreamsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_patch_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams run.
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
    pub fn datastream_projects_locations_streams_run(
        &self,
        args: &DatastreamProjectsLocationsStreamsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_run_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_run_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams objects lookup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StreamObject result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datastream_projects_locations_streams_objects_lookup(
        &self,
        args: &DatastreamProjectsLocationsStreamsObjectsLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StreamObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_objects_lookup_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_objects_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams objects start backfill job.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StartBackfillJobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datastream_projects_locations_streams_objects_start_backfill_job(
        &self,
        args: &DatastreamProjectsLocationsStreamsObjectsStartBackfillJobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StartBackfillJobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_objects_start_backfill_job_builder(
            &self.http_client,
            &args.object,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_objects_start_backfill_job_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams objects stop backfill job.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the StopBackfillJobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn datastream_projects_locations_streams_objects_stop_backfill_job(
        &self,
        args: &DatastreamProjectsLocationsStreamsObjectsStopBackfillJobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StopBackfillJobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_objects_stop_backfill_job_builder(
            &self.http_client,
            &args.object,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_objects_stop_backfill_job_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
