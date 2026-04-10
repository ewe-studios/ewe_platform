//! DoubleclicksearchProvider - State-aware doubleclicksearch API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       doubleclicksearch API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::doubleclicksearch::{
    doubleclicksearch_conversion_insert_builder, doubleclicksearch_conversion_insert_task,
    doubleclicksearch_conversion_update_builder, doubleclicksearch_conversion_update_task,
    doubleclicksearch_conversion_update_availability_builder, doubleclicksearch_conversion_update_availability_task,
    doubleclicksearch_reports_generate_builder, doubleclicksearch_reports_generate_task,
    doubleclicksearch_reports_request_builder, doubleclicksearch_reports_request_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::doubleclicksearch::ConversionList;
use crate::providers::gcp::clients::doubleclicksearch::Report;
use crate::providers::gcp::clients::doubleclicksearch::UpdateAvailabilityResponse;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchConversionInsertArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchConversionUpdateArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchConversionUpdateAvailabilityArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchReportsGenerateArgs;
use crate::providers::gcp::clients::doubleclicksearch::DoubleclicksearchReportsRequestArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DoubleclicksearchProvider with automatic state tracking.
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
/// let provider = DoubleclicksearchProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DoubleclicksearchProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DoubleclicksearchProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DoubleclicksearchProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Doubleclicksearch conversion insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclicksearch_conversion_insert(
        &self,
        args: &DoubleclicksearchConversionInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_insert_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch conversion update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConversionList result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclicksearch_conversion_update(
        &self,
        args: &DoubleclicksearchConversionUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConversionList, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_update_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch conversion update availability.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UpdateAvailabilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclicksearch_conversion_update_availability(
        &self,
        args: &DoubleclicksearchConversionUpdateAvailabilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UpdateAvailabilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_conversion_update_availability_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_conversion_update_availability_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch reports generate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclicksearch_reports_generate(
        &self,
        args: &DoubleclicksearchReportsGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_reports_generate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_reports_generate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Doubleclicksearch reports request.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn doubleclicksearch_reports_request(
        &self,
        args: &DoubleclicksearchReportsRequestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = doubleclicksearch_reports_request_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = doubleclicksearch_reports_request_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
