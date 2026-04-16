//! ApimProvider - State-aware apim API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       apim API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::apim::{
    apim_projects_locations_get_builder, apim_projects_locations_get_task,
    apim_projects_locations_get_entitlement_builder, apim_projects_locations_get_entitlement_task,
    apim_projects_locations_list_builder, apim_projects_locations_list_task,
    apim_projects_locations_list_api_observation_tags_builder, apim_projects_locations_list_api_observation_tags_task,
    apim_projects_locations_observation_jobs_create_builder, apim_projects_locations_observation_jobs_create_task,
    apim_projects_locations_observation_jobs_delete_builder, apim_projects_locations_observation_jobs_delete_task,
    apim_projects_locations_observation_jobs_disable_builder, apim_projects_locations_observation_jobs_disable_task,
    apim_projects_locations_observation_jobs_enable_builder, apim_projects_locations_observation_jobs_enable_task,
    apim_projects_locations_observation_jobs_get_builder, apim_projects_locations_observation_jobs_get_task,
    apim_projects_locations_observation_jobs_list_builder, apim_projects_locations_observation_jobs_list_task,
    apim_projects_locations_observation_jobs_api_observations_batch_edit_tags_builder, apim_projects_locations_observation_jobs_api_observations_batch_edit_tags_task,
    apim_projects_locations_observation_jobs_api_observations_get_builder, apim_projects_locations_observation_jobs_api_observations_get_task,
    apim_projects_locations_observation_jobs_api_observations_list_builder, apim_projects_locations_observation_jobs_api_observations_list_task,
    apim_projects_locations_observation_jobs_api_observations_api_operations_get_builder, apim_projects_locations_observation_jobs_api_observations_api_operations_get_task,
    apim_projects_locations_observation_jobs_api_observations_api_operations_list_builder, apim_projects_locations_observation_jobs_api_observations_api_operations_list_task,
    apim_projects_locations_observation_sources_create_builder, apim_projects_locations_observation_sources_create_task,
    apim_projects_locations_observation_sources_delete_builder, apim_projects_locations_observation_sources_delete_task,
    apim_projects_locations_observation_sources_get_builder, apim_projects_locations_observation_sources_get_task,
    apim_projects_locations_observation_sources_list_builder, apim_projects_locations_observation_sources_list_task,
    apim_projects_locations_operations_cancel_builder, apim_projects_locations_operations_cancel_task,
    apim_projects_locations_operations_delete_builder, apim_projects_locations_operations_delete_task,
    apim_projects_locations_operations_get_builder, apim_projects_locations_operations_get_task,
    apim_projects_locations_operations_list_builder, apim_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::apim::ApiObservation;
use crate::providers::gcp::clients::apim::ApiOperation;
use crate::providers::gcp::clients::apim::BatchEditTagsApiObservationsResponse;
use crate::providers::gcp::clients::apim::Empty;
use crate::providers::gcp::clients::apim::Entitlement;
use crate::providers::gcp::clients::apim::ListApiObservationTagsResponse;
use crate::providers::gcp::clients::apim::ListApiObservationsResponse;
use crate::providers::gcp::clients::apim::ListApiOperationsResponse;
use crate::providers::gcp::clients::apim::ListLocationsResponse;
use crate::providers::gcp::clients::apim::ListObservationJobsResponse;
use crate::providers::gcp::clients::apim::ListObservationSourcesResponse;
use crate::providers::gcp::clients::apim::ListOperationsResponse;
use crate::providers::gcp::clients::apim::Location;
use crate::providers::gcp::clients::apim::ObservationJob;
use crate::providers::gcp::clients::apim::ObservationSource;
use crate::providers::gcp::clients::apim::Operation;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsGetArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsGetEntitlementArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsListApiObservationTagsArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsListArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsApiObservationsApiOperationsGetArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsApiObservationsApiOperationsListArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsApiObservationsBatchEditTagsArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsApiObservationsGetArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsApiObservationsListArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsCreateArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsDeleteArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsDisableArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsEnableArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsGetArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationJobsListArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationSourcesCreateArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationSourcesDeleteArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationSourcesGetArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsObservationSourcesListArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::apim::ApimProjectsLocationsOperationsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ApimProvider with automatic state tracking.
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
/// let provider = ApimProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ApimProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ApimProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ApimProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ApimProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Apim projects locations get.
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
    pub fn apim_projects_locations_get(
        &self,
        args: &ApimProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations get entitlement.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entitlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_get_entitlement(
        &self,
        args: &ApimProjectsLocationsGetEntitlementArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entitlement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_get_entitlement_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_get_entitlement_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations list.
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
    pub fn apim_projects_locations_list(
        &self,
        args: &ApimProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations list api observation tags.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiObservationTagsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_list_api_observation_tags(
        &self,
        args: &ApimProjectsLocationsListApiObservationTagsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiObservationTagsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_list_api_observation_tags_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_list_api_observation_tags_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs create.
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
    pub fn apim_projects_locations_observation_jobs_create(
        &self,
        args: &ApimProjectsLocationsObservationJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.observationJobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs delete.
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
    pub fn apim_projects_locations_observation_jobs_delete(
        &self,
        args: &ApimProjectsLocationsObservationJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs disable.
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
    pub fn apim_projects_locations_observation_jobs_disable(
        &self,
        args: &ApimProjectsLocationsObservationJobsDisableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_disable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_disable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs enable.
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
    pub fn apim_projects_locations_observation_jobs_enable(
        &self,
        args: &ApimProjectsLocationsObservationJobsEnableArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_enable_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_enable_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObservationJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_jobs_get(
        &self,
        args: &ApimProjectsLocationsObservationJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObservationJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListObservationJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_jobs_list(
        &self,
        args: &ApimProjectsLocationsObservationJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListObservationJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs api observations batch edit tags.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchEditTagsApiObservationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn apim_projects_locations_observation_jobs_api_observations_batch_edit_tags(
        &self,
        args: &ApimProjectsLocationsObservationJobsApiObservationsBatchEditTagsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchEditTagsApiObservationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_api_observations_batch_edit_tags_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_api_observations_batch_edit_tags_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs api observations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiObservation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_jobs_api_observations_get(
        &self,
        args: &ApimProjectsLocationsObservationJobsApiObservationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiObservation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_api_observations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_api_observations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs api observations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiObservationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_jobs_api_observations_list(
        &self,
        args: &ApimProjectsLocationsObservationJobsApiObservationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiObservationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_api_observations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_api_observations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs api observations api operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ApiOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_jobs_api_observations_api_operations_get(
        &self,
        args: &ApimProjectsLocationsObservationJobsApiObservationsApiOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ApiOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_api_observations_api_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_api_observations_api_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation jobs api observations api operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListApiOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_jobs_api_observations_api_operations_list(
        &self,
        args: &ApimProjectsLocationsObservationJobsApiObservationsApiOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListApiOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_jobs_api_observations_api_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_jobs_api_observations_api_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation sources create.
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
    pub fn apim_projects_locations_observation_sources_create(
        &self,
        args: &ApimProjectsLocationsObservationSourcesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_sources_create_builder(
            &self.http_client,
            &args.parent,
            &args.observationSourceId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_sources_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation sources delete.
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
    pub fn apim_projects_locations_observation_sources_delete(
        &self,
        args: &ApimProjectsLocationsObservationSourcesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_sources_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_sources_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation sources get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ObservationSource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_sources_get(
        &self,
        args: &ApimProjectsLocationsObservationSourcesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ObservationSource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_sources_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_sources_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations observation sources list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListObservationSourcesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn apim_projects_locations_observation_sources_list(
        &self,
        args: &ApimProjectsLocationsObservationSourcesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListObservationSourcesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_observation_sources_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_observation_sources_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations operations cancel.
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
    pub fn apim_projects_locations_operations_cancel(
        &self,
        args: &ApimProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations operations delete.
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
    pub fn apim_projects_locations_operations_delete(
        &self,
        args: &ApimProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations operations get.
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
    pub fn apim_projects_locations_operations_get(
        &self,
        args: &ApimProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Apim projects locations operations list.
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
    pub fn apim_projects_locations_operations_list(
        &self,
        args: &ApimProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = apim_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = apim_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
