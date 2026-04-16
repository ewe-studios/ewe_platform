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
    datastream_projects_locations_fetch_static_ips_builder, datastream_projects_locations_fetch_static_ips_task,
    datastream_projects_locations_get_builder, datastream_projects_locations_get_task,
    datastream_projects_locations_list_builder, datastream_projects_locations_list_task,
    datastream_projects_locations_connection_profiles_create_builder, datastream_projects_locations_connection_profiles_create_task,
    datastream_projects_locations_connection_profiles_delete_builder, datastream_projects_locations_connection_profiles_delete_task,
    datastream_projects_locations_connection_profiles_discover_builder, datastream_projects_locations_connection_profiles_discover_task,
    datastream_projects_locations_connection_profiles_get_builder, datastream_projects_locations_connection_profiles_get_task,
    datastream_projects_locations_connection_profiles_list_builder, datastream_projects_locations_connection_profiles_list_task,
    datastream_projects_locations_connection_profiles_patch_builder, datastream_projects_locations_connection_profiles_patch_task,
    datastream_projects_locations_operations_cancel_builder, datastream_projects_locations_operations_cancel_task,
    datastream_projects_locations_operations_delete_builder, datastream_projects_locations_operations_delete_task,
    datastream_projects_locations_operations_get_builder, datastream_projects_locations_operations_get_task,
    datastream_projects_locations_operations_list_builder, datastream_projects_locations_operations_list_task,
    datastream_projects_locations_private_connections_create_builder, datastream_projects_locations_private_connections_create_task,
    datastream_projects_locations_private_connections_delete_builder, datastream_projects_locations_private_connections_delete_task,
    datastream_projects_locations_private_connections_get_builder, datastream_projects_locations_private_connections_get_task,
    datastream_projects_locations_private_connections_list_builder, datastream_projects_locations_private_connections_list_task,
    datastream_projects_locations_private_connections_routes_create_builder, datastream_projects_locations_private_connections_routes_create_task,
    datastream_projects_locations_private_connections_routes_delete_builder, datastream_projects_locations_private_connections_routes_delete_task,
    datastream_projects_locations_private_connections_routes_get_builder, datastream_projects_locations_private_connections_routes_get_task,
    datastream_projects_locations_private_connections_routes_list_builder, datastream_projects_locations_private_connections_routes_list_task,
    datastream_projects_locations_streams_create_builder, datastream_projects_locations_streams_create_task,
    datastream_projects_locations_streams_delete_builder, datastream_projects_locations_streams_delete_task,
    datastream_projects_locations_streams_get_builder, datastream_projects_locations_streams_get_task,
    datastream_projects_locations_streams_list_builder, datastream_projects_locations_streams_list_task,
    datastream_projects_locations_streams_patch_builder, datastream_projects_locations_streams_patch_task,
    datastream_projects_locations_streams_run_builder, datastream_projects_locations_streams_run_task,
    datastream_projects_locations_streams_objects_get_builder, datastream_projects_locations_streams_objects_get_task,
    datastream_projects_locations_streams_objects_list_builder, datastream_projects_locations_streams_objects_list_task,
    datastream_projects_locations_streams_objects_lookup_builder, datastream_projects_locations_streams_objects_lookup_task,
    datastream_projects_locations_streams_objects_start_backfill_job_builder, datastream_projects_locations_streams_objects_start_backfill_job_task,
    datastream_projects_locations_streams_objects_stop_backfill_job_builder, datastream_projects_locations_streams_objects_stop_backfill_job_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::datastream::ConnectionProfile;
use crate::providers::gcp::clients::datastream::DiscoverConnectionProfileResponse;
use crate::providers::gcp::clients::datastream::Empty;
use crate::providers::gcp::clients::datastream::FetchStaticIpsResponse;
use crate::providers::gcp::clients::datastream::ListConnectionProfilesResponse;
use crate::providers::gcp::clients::datastream::ListLocationsResponse;
use crate::providers::gcp::clients::datastream::ListOperationsResponse;
use crate::providers::gcp::clients::datastream::ListPrivateConnectionsResponse;
use crate::providers::gcp::clients::datastream::ListRoutesResponse;
use crate::providers::gcp::clients::datastream::ListStreamObjectsResponse;
use crate::providers::gcp::clients::datastream::ListStreamsResponse;
use crate::providers::gcp::clients::datastream::Location;
use crate::providers::gcp::clients::datastream::Operation;
use crate::providers::gcp::clients::datastream::PrivateConnection;
use crate::providers::gcp::clients::datastream::Route;
use crate::providers::gcp::clients::datastream::StartBackfillJobResponse;
use crate::providers::gcp::clients::datastream::StopBackfillJobResponse;
use crate::providers::gcp::clients::datastream::Stream;
use crate::providers::gcp::clients::datastream::StreamObject;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesDiscoverArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesGetArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesListArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsConnectionProfilesPatchArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsFetchStaticIpsArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsGetArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsListArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsGetArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsListArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsRoutesCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsRoutesDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsRoutesGetArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsPrivateConnectionsRoutesListArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsCreateArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsDeleteArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsGetArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsListArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsGetArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsListArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsLookupArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsStartBackfillJobArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsObjectsStopBackfillJobArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsPatchArgs;
use crate::providers::gcp::clients::datastream::DatastreamProjectsLocationsStreamsRunArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DatastreamProvider with automatic state tracking.
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
/// let provider = DatastreamProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct DatastreamProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> DatastreamProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new DatastreamProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new DatastreamProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Datastream projects locations fetch static ips.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchStaticIpsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_fetch_static_ips(
        &self,
        args: &DatastreamProjectsLocationsFetchStaticIpsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchStaticIpsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_fetch_static_ips_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_fetch_static_ips_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations get.
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
    pub fn datastream_projects_locations_get(
        &self,
        args: &DatastreamProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations list.
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
    pub fn datastream_projects_locations_list(
        &self,
        args: &DatastreamProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datastream projects locations connection profiles get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectionProfile result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_connection_profiles_get(
        &self,
        args: &DatastreamProjectsLocationsConnectionProfilesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectionProfile, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_connection_profiles_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_connection_profiles_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations connection profiles list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectionProfilesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_connection_profiles_list(
        &self,
        args: &DatastreamProjectsLocationsConnectionProfilesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectionProfilesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_connection_profiles_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_connection_profiles_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datastream projects locations operations get.
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
    pub fn datastream_projects_locations_operations_get(
        &self,
        args: &DatastreamProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations operations list.
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
    pub fn datastream_projects_locations_operations_list(
        &self,
        args: &DatastreamProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datastream projects locations private connections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PrivateConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_private_connections_get(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PrivateConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations private connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPrivateConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_private_connections_list(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPrivateConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datastream projects locations private connections routes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Route result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_private_connections_routes_get(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsRoutesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Route, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_routes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_routes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations private connections routes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRoutesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_private_connections_routes_list(
        &self,
        args: &DatastreamProjectsLocationsPrivateConnectionsRoutesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRoutesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_private_connections_routes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_private_connections_routes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datastream projects locations streams get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Stream result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_streams_get(
        &self,
        args: &DatastreamProjectsLocationsStreamsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Stream, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStreamsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_streams_list(
        &self,
        args: &DatastreamProjectsLocationsStreamsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStreamsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Datastream projects locations streams objects get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_streams_objects_get(
        &self,
        args: &DatastreamProjectsLocationsStreamsObjectsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<StreamObject, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_objects_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_objects_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Datastream projects locations streams objects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListStreamObjectsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn datastream_projects_locations_streams_objects_list(
        &self,
        args: &DatastreamProjectsLocationsStreamsObjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListStreamObjectsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = datastream_projects_locations_streams_objects_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = datastream_projects_locations_streams_objects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
