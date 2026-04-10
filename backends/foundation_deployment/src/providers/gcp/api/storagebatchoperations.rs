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
    storagebatchoperations_projects_locations_get_builder, storagebatchoperations_projects_locations_get_task,
    storagebatchoperations_projects_locations_list_builder, storagebatchoperations_projects_locations_list_task,
    storagebatchoperations_projects_locations_jobs_cancel_builder, storagebatchoperations_projects_locations_jobs_cancel_task,
    storagebatchoperations_projects_locations_jobs_create_builder, storagebatchoperations_projects_locations_jobs_create_task,
    storagebatchoperations_projects_locations_jobs_delete_builder, storagebatchoperations_projects_locations_jobs_delete_task,
    storagebatchoperations_projects_locations_jobs_get_builder, storagebatchoperations_projects_locations_jobs_get_task,
    storagebatchoperations_projects_locations_jobs_list_builder, storagebatchoperations_projects_locations_jobs_list_task,
    storagebatchoperations_projects_locations_jobs_bucket_operations_get_builder, storagebatchoperations_projects_locations_jobs_bucket_operations_get_task,
    storagebatchoperations_projects_locations_jobs_bucket_operations_list_builder, storagebatchoperations_projects_locations_jobs_bucket_operations_list_task,
    storagebatchoperations_projects_locations_operations_cancel_builder, storagebatchoperations_projects_locations_operations_cancel_task,
    storagebatchoperations_projects_locations_operations_delete_builder, storagebatchoperations_projects_locations_operations_delete_task,
    storagebatchoperations_projects_locations_operations_get_builder, storagebatchoperations_projects_locations_operations_get_task,
    storagebatchoperations_projects_locations_operations_list_builder, storagebatchoperations_projects_locations_operations_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::storagebatchoperations::BucketOperation;
use crate::providers::gcp::clients::storagebatchoperations::CancelJobResponse;
use crate::providers::gcp::clients::storagebatchoperations::Empty;
use crate::providers::gcp::clients::storagebatchoperations::Job;
use crate::providers::gcp::clients::storagebatchoperations::ListBucketOperationsResponse;
use crate::providers::gcp::clients::storagebatchoperations::ListJobsResponse;
use crate::providers::gcp::clients::storagebatchoperations::ListLocationsResponse;
use crate::providers::gcp::clients::storagebatchoperations::ListOperationsResponse;
use crate::providers::gcp::clients::storagebatchoperations::Location;
use crate::providers::gcp::clients::storagebatchoperations::Operation;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsGetArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsBucketOperationsGetArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsBucketOperationsListArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsCancelArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsCreateArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsDeleteArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsGetArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsJobsListArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsListArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::storagebatchoperations::StoragebatchoperationsProjectsLocationsOperationsListArgs;
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

    /// Storagebatchoperations projects locations get.
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
    pub fn storagebatchoperations_projects_locations_get(
        &self,
        args: &StoragebatchoperationsProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations list.
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
    pub fn storagebatchoperations_projects_locations_list(
        &self,
        args: &StoragebatchoperationsProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Storagebatchoperations projects locations jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagebatchoperations_projects_locations_jobs_get(
        &self,
        args: &StoragebatchoperationsProjectsLocationsJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_jobs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagebatchoperations_projects_locations_jobs_list(
        &self,
        args: &StoragebatchoperationsProjectsLocationsJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_jobs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations jobs bucket operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BucketOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagebatchoperations_projects_locations_jobs_bucket_operations_get(
        &self,
        args: &StoragebatchoperationsProjectsLocationsJobsBucketOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BucketOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_jobs_bucket_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_jobs_bucket_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations jobs bucket operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBucketOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagebatchoperations_projects_locations_jobs_bucket_operations_list(
        &self,
        args: &StoragebatchoperationsProjectsLocationsJobsBucketOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBucketOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_jobs_bucket_operations_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_jobs_bucket_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Storagebatchoperations projects locations operations get.
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
    pub fn storagebatchoperations_projects_locations_operations_get(
        &self,
        args: &StoragebatchoperationsProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagebatchoperations projects locations operations list.
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
    pub fn storagebatchoperations_projects_locations_operations_list(
        &self,
        args: &StoragebatchoperationsProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagebatchoperations_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = storagebatchoperations_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
