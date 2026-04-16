//! VpcaccessProvider - State-aware vpcaccess API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       vpcaccess API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::vpcaccess::{
    vpcaccess_projects_locations_list_builder, vpcaccess_projects_locations_list_task,
    vpcaccess_projects_locations_connectors_create_builder, vpcaccess_projects_locations_connectors_create_task,
    vpcaccess_projects_locations_connectors_delete_builder, vpcaccess_projects_locations_connectors_delete_task,
    vpcaccess_projects_locations_connectors_get_builder, vpcaccess_projects_locations_connectors_get_task,
    vpcaccess_projects_locations_connectors_list_builder, vpcaccess_projects_locations_connectors_list_task,
    vpcaccess_projects_locations_connectors_patch_builder, vpcaccess_projects_locations_connectors_patch_task,
    vpcaccess_projects_locations_operations_get_builder, vpcaccess_projects_locations_operations_get_task,
    vpcaccess_projects_locations_operations_list_builder, vpcaccess_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::vpcaccess::Connector;
use crate::providers::gcp::clients::vpcaccess::ListConnectorsResponse;
use crate::providers::gcp::clients::vpcaccess::ListLocationsResponse;
use crate::providers::gcp::clients::vpcaccess::ListOperationsResponse;
use crate::providers::gcp::clients::vpcaccess::Operation;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsConnectorsCreateArgs;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsConnectorsDeleteArgs;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsConnectorsGetArgs;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsConnectorsListArgs;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsConnectorsPatchArgs;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsListArgs;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::vpcaccess::VpcaccessProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// VpcaccessProvider with automatic state tracking.
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
/// let provider = VpcaccessProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct VpcaccessProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> VpcaccessProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new VpcaccessProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new VpcaccessProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Vpcaccess projects locations list.
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
    pub fn vpcaccess_projects_locations_list(
        &self,
        args: &VpcaccessProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vpcaccess projects locations connectors create.
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
    pub fn vpcaccess_projects_locations_connectors_create(
        &self,
        args: &VpcaccessProjectsLocationsConnectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_connectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.connectorId,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_connectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vpcaccess projects locations connectors delete.
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
    pub fn vpcaccess_projects_locations_connectors_delete(
        &self,
        args: &VpcaccessProjectsLocationsConnectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_connectors_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_connectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vpcaccess projects locations connectors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Connector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vpcaccess_projects_locations_connectors_get(
        &self,
        args: &VpcaccessProjectsLocationsConnectorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Connector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_connectors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_connectors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vpcaccess projects locations connectors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn vpcaccess_projects_locations_connectors_list(
        &self,
        args: &VpcaccessProjectsLocationsConnectorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_connectors_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_connectors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vpcaccess projects locations connectors patch.
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
    pub fn vpcaccess_projects_locations_connectors_patch(
        &self,
        args: &VpcaccessProjectsLocationsConnectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_connectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_connectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vpcaccess projects locations operations get.
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
    pub fn vpcaccess_projects_locations_operations_get(
        &self,
        args: &VpcaccessProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Vpcaccess projects locations operations list.
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
    pub fn vpcaccess_projects_locations_operations_list(
        &self,
        args: &VpcaccessProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = vpcaccess_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = vpcaccess_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
