//! TpuProvider - State-aware tpu API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       tpu API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::tpu::{
    tpu_projects_locations_generate_service_identity_builder, tpu_projects_locations_generate_service_identity_task,
    tpu_projects_locations_get_builder, tpu_projects_locations_get_task,
    tpu_projects_locations_list_builder, tpu_projects_locations_list_task,
    tpu_projects_locations_accelerator_types_get_builder, tpu_projects_locations_accelerator_types_get_task,
    tpu_projects_locations_accelerator_types_list_builder, tpu_projects_locations_accelerator_types_list_task,
    tpu_projects_locations_nodes_create_builder, tpu_projects_locations_nodes_create_task,
    tpu_projects_locations_nodes_delete_builder, tpu_projects_locations_nodes_delete_task,
    tpu_projects_locations_nodes_get_builder, tpu_projects_locations_nodes_get_task,
    tpu_projects_locations_nodes_get_guest_attributes_builder, tpu_projects_locations_nodes_get_guest_attributes_task,
    tpu_projects_locations_nodes_list_builder, tpu_projects_locations_nodes_list_task,
    tpu_projects_locations_nodes_patch_builder, tpu_projects_locations_nodes_patch_task,
    tpu_projects_locations_nodes_start_builder, tpu_projects_locations_nodes_start_task,
    tpu_projects_locations_nodes_stop_builder, tpu_projects_locations_nodes_stop_task,
    tpu_projects_locations_operations_cancel_builder, tpu_projects_locations_operations_cancel_task,
    tpu_projects_locations_operations_delete_builder, tpu_projects_locations_operations_delete_task,
    tpu_projects_locations_operations_get_builder, tpu_projects_locations_operations_get_task,
    tpu_projects_locations_operations_list_builder, tpu_projects_locations_operations_list_task,
    tpu_projects_locations_queued_resources_create_builder, tpu_projects_locations_queued_resources_create_task,
    tpu_projects_locations_queued_resources_delete_builder, tpu_projects_locations_queued_resources_delete_task,
    tpu_projects_locations_queued_resources_get_builder, tpu_projects_locations_queued_resources_get_task,
    tpu_projects_locations_queued_resources_list_builder, tpu_projects_locations_queued_resources_list_task,
    tpu_projects_locations_queued_resources_reset_builder, tpu_projects_locations_queued_resources_reset_task,
    tpu_projects_locations_runtime_versions_get_builder, tpu_projects_locations_runtime_versions_get_task,
    tpu_projects_locations_runtime_versions_list_builder, tpu_projects_locations_runtime_versions_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::tpu::AcceleratorType;
use crate::providers::gcp::clients::tpu::Empty;
use crate::providers::gcp::clients::tpu::GenerateServiceIdentityResponse;
use crate::providers::gcp::clients::tpu::GetGuestAttributesResponse;
use crate::providers::gcp::clients::tpu::ListAcceleratorTypesResponse;
use crate::providers::gcp::clients::tpu::ListLocationsResponse;
use crate::providers::gcp::clients::tpu::ListNodesResponse;
use crate::providers::gcp::clients::tpu::ListOperationsResponse;
use crate::providers::gcp::clients::tpu::ListQueuedResourcesResponse;
use crate::providers::gcp::clients::tpu::ListRuntimeVersionsResponse;
use crate::providers::gcp::clients::tpu::Location;
use crate::providers::gcp::clients::tpu::Node;
use crate::providers::gcp::clients::tpu::Operation;
use crate::providers::gcp::clients::tpu::QueuedResource;
use crate::providers::gcp::clients::tpu::RuntimeVersion;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsAcceleratorTypesGetArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsAcceleratorTypesListArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsGenerateServiceIdentityArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsGetArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsListArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesCreateArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesDeleteArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesGetArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesGetGuestAttributesArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesListArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesPatchArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesStartArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsNodesStopArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsQueuedResourcesCreateArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsQueuedResourcesDeleteArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsQueuedResourcesGetArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsQueuedResourcesListArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsQueuedResourcesResetArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsRuntimeVersionsGetArgs;
use crate::providers::gcp::clients::tpu::TpuProjectsLocationsRuntimeVersionsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// TpuProvider with automatic state tracking.
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
/// let provider = TpuProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct TpuProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> TpuProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new TpuProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new TpuProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Tpu projects locations generate service identity.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateServiceIdentityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn tpu_projects_locations_generate_service_identity(
        &self,
        args: &TpuProjectsLocationsGenerateServiceIdentityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateServiceIdentityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_generate_service_identity_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_generate_service_identity_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations get.
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
    pub fn tpu_projects_locations_get(
        &self,
        args: &TpuProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations list.
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
    pub fn tpu_projects_locations_list(
        &self,
        args: &TpuProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations accelerator types get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AcceleratorType result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_accelerator_types_get(
        &self,
        args: &TpuProjectsLocationsAcceleratorTypesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AcceleratorType, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_accelerator_types_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_accelerator_types_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations accelerator types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAcceleratorTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_accelerator_types_list(
        &self,
        args: &TpuProjectsLocationsAcceleratorTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAcceleratorTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_accelerator_types_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_accelerator_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes create.
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
    pub fn tpu_projects_locations_nodes_create(
        &self,
        args: &TpuProjectsLocationsNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_create_builder(
            &self.http_client,
            &args.parent,
            &args.nodeId,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes delete.
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
    pub fn tpu_projects_locations_nodes_delete(
        &self,
        args: &TpuProjectsLocationsNodesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Node result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_nodes_get(
        &self,
        args: &TpuProjectsLocationsNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Node, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes get guest attributes.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetGuestAttributesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_nodes_get_guest_attributes(
        &self,
        args: &TpuProjectsLocationsNodesGetGuestAttributesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetGuestAttributesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_get_guest_attributes_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_get_guest_attributes_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_nodes_list(
        &self,
        args: &TpuProjectsLocationsNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes patch.
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
    pub fn tpu_projects_locations_nodes_patch(
        &self,
        args: &TpuProjectsLocationsNodesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes start.
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
    pub fn tpu_projects_locations_nodes_start(
        &self,
        args: &TpuProjectsLocationsNodesStartArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_start_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_start_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations nodes stop.
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
    pub fn tpu_projects_locations_nodes_stop(
        &self,
        args: &TpuProjectsLocationsNodesStopArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_nodes_stop_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_nodes_stop_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations operations cancel.
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
    pub fn tpu_projects_locations_operations_cancel(
        &self,
        args: &TpuProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations operations delete.
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
    pub fn tpu_projects_locations_operations_delete(
        &self,
        args: &TpuProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations operations get.
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
    pub fn tpu_projects_locations_operations_get(
        &self,
        args: &TpuProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations operations list.
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
    pub fn tpu_projects_locations_operations_list(
        &self,
        args: &TpuProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations queued resources create.
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
    pub fn tpu_projects_locations_queued_resources_create(
        &self,
        args: &TpuProjectsLocationsQueuedResourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_queued_resources_create_builder(
            &self.http_client,
            &args.parent,
            &args.queuedResourceId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_queued_resources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations queued resources delete.
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
    pub fn tpu_projects_locations_queued_resources_delete(
        &self,
        args: &TpuProjectsLocationsQueuedResourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_queued_resources_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_queued_resources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations queued resources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueuedResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_queued_resources_get(
        &self,
        args: &TpuProjectsLocationsQueuedResourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueuedResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_queued_resources_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_queued_resources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations queued resources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListQueuedResourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_queued_resources_list(
        &self,
        args: &TpuProjectsLocationsQueuedResourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListQueuedResourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_queued_resources_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_queued_resources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations queued resources reset.
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
    pub fn tpu_projects_locations_queued_resources_reset(
        &self,
        args: &TpuProjectsLocationsQueuedResourcesResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_queued_resources_reset_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_queued_resources_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations runtime versions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RuntimeVersion result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_runtime_versions_get(
        &self,
        args: &TpuProjectsLocationsRuntimeVersionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RuntimeVersion, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_runtime_versions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_runtime_versions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Tpu projects locations runtime versions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListRuntimeVersionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn tpu_projects_locations_runtime_versions_list(
        &self,
        args: &TpuProjectsLocationsRuntimeVersionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListRuntimeVersionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = tpu_projects_locations_runtime_versions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = tpu_projects_locations_runtime_versions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
