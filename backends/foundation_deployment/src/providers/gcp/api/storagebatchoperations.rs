//! StoragebatchoperationsProvider - State-aware storagebatchoperations API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       storagebatchoperations API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::storagebatchoperations::{
    storagebatchoperations_projects_locations_jobs_cancel_builder, storagebatchoperations_projects_locations_jobs_cancel_task,
    storagebatchoperations_projects_locations_jobs_create_builder, storagebatchoperations_projects_locations_jobs_create_task,
    storagebatchoperations_projects_locations_jobs_delete_builder, storagebatchoperations_projects_locations_jobs_delete_task,
    storagebatchoperations_projects_locations_operations_cancel_builder, storagebatchoperations_projects_locations_operations_cancel_task,
    storagebatchoperations_projects_locations_operations_delete_builder, storagebatchoperations_projects_locations_operations_delete_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::storagebatchoperations::CancelJobResponse;
use crate::providers::gcp::clients::storagebatchoperations::Empty;
use crate::providers::gcp::clients::storagebatchoperations::Operation;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsCancelArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsCreateArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsDeleteArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsOperationsDeleteArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// StoragebatchoperationsProvider with automatic state tracking.
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
/// let provider = StoragebatchoperationsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct StoragebatchoperationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> StoragebatchoperationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new StoragebatchoperationsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Storagebatchoperations projects locations jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelJobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storagebatchoperations_projects_locations_jobs_cancel(
        &self,
        args: &StoragebatchoperationsProjectsLocationsJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelJobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations jobs create.
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
    pub fn storagebatchoperations_projects_locations_jobs_create(
        &self,
        args: &StoragebatchoperationsProjectsLocationsJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_jobs_create_builder(
            &self.http_client,
            &args.parent,
            &args.jobId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations jobs delete.
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
    pub fn storagebatchoperations_projects_locations_jobs_delete(
        &self,
        args: &StoragebatchoperationsProjectsLocationsJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_jobs_delete_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations operations cancel.
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
    pub fn storagebatchoperations_projects_locations_operations_cancel(
        &self,
        args: &StoragebatchoperationsProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations operations delete.
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
    pub fn storagebatchoperations_projects_locations_operations_delete(
        &self,
        args: &StoragebatchoperationsProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
