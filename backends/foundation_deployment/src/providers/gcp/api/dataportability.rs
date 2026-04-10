//! DataportabilityProvider - State-aware dataportability API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dataportability API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dataportability::{
    dataportability_access_type_check_builder, dataportability_access_type_check_task,
    dataportability_archive_jobs_cancel_builder, dataportability_archive_jobs_cancel_task,
    dataportability_archive_jobs_get_portability_archive_state_builder, dataportability_archive_jobs_get_portability_archive_state_task,
    dataportability_archive_jobs_retry_builder, dataportability_archive_jobs_retry_task,
    dataportability_authorization_reset_builder, dataportability_authorization_reset_task,
    dataportability_portability_archive_initiate_builder, dataportability_portability_archive_initiate_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dataportability::CancelPortabilityArchiveResponse;
use crate::providers::gcp::clients::dataportability::CheckAccessTypeResponse;
use crate::providers::gcp::clients::dataportability::Empty;
use crate::providers::gcp::clients::dataportability::InitiatePortabilityArchiveResponse;
use crate::providers::gcp::clients::dataportability::PortabilityArchiveState;
use crate::providers::gcp::clients::dataportability::RetryPortabilityArchiveResponse;
use crate::providers::gcp::clients::dataportability::DataportabilityAccessTypeCheckArgs;
use crate::providers::gcp::clients::dataportability::DataportabilityArchiveJobsCancelArgs;
use crate::providers::gcp::clients::dataportability::DataportabilityArchiveJobsGetPortabilityArchiveStateArgs;
use crate::providers::gcp::clients::dataportability::DataportabilityArchiveJobsRetryArgs;
use crate::providers::gcp::clients::dataportability::DataportabilityAuthorizationResetArgs;
use crate::providers::gcp::clients::dataportability::DataportabilityPortabilityArchiveInitiateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DataportabilityProvider with automatic state tracking.
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
/// let provider = DataportabilityProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DataportabilityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DataportabilityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DataportabilityProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dataportability access type check.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckAccessTypeResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataportability_access_type_check(
        &self,
        args: &DataportabilityAccessTypeCheckArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckAccessTypeResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataportability_access_type_check_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dataportability_access_type_check_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataportability archive jobs cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelPortabilityArchiveResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataportability_archive_jobs_cancel(
        &self,
        args: &DataportabilityArchiveJobsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelPortabilityArchiveResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataportability_archive_jobs_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataportability_archive_jobs_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataportability archive jobs get portability archive state.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PortabilityArchiveState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn dataportability_archive_jobs_get_portability_archive_state(
        &self,
        args: &DataportabilityArchiveJobsGetPortabilityArchiveStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PortabilityArchiveState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataportability_archive_jobs_get_portability_archive_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataportability_archive_jobs_get_portability_archive_state_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataportability archive jobs retry.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RetryPortabilityArchiveResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataportability_archive_jobs_retry(
        &self,
        args: &DataportabilityArchiveJobsRetryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RetryPortabilityArchiveResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataportability_archive_jobs_retry_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = dataportability_archive_jobs_retry_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataportability authorization reset.
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
    pub fn dataportability_authorization_reset(
        &self,
        args: &DataportabilityAuthorizationResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataportability_authorization_reset_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dataportability_authorization_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dataportability portability archive initiate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the InitiatePortabilityArchiveResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dataportability_portability_archive_initiate(
        &self,
        args: &DataportabilityPortabilityArchiveInitiateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<InitiatePortabilityArchiveResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dataportability_portability_archive_initiate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = dataportability_portability_archive_initiate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
