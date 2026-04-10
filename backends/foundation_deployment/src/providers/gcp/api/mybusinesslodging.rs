//! MybusinesslodgingProvider - State-aware mybusinesslodging API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       mybusinesslodging API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::mybusinesslodging::{
    mybusinesslodging_locations_get_lodging_builder, mybusinesslodging_locations_get_lodging_task,
    mybusinesslodging_locations_update_lodging_builder, mybusinesslodging_locations_update_lodging_task,
    mybusinesslodging_locations_lodging_get_google_updated_builder, mybusinesslodging_locations_lodging_get_google_updated_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::mybusinesslodging::GetGoogleUpdatedLodgingResponse;
use crate::providers::gcp::clients::mybusinesslodging::Lodging;
use crate::providers::gcp::clients::mybusinesslodging::MybusinesslodgingLocationsGetLodgingArgs;
use crate::providers::gcp::clients::mybusinesslodging::MybusinesslodgingLocationsLodgingGetGoogleUpdatedArgs;
use crate::providers::gcp::clients::mybusinesslodging::MybusinesslodgingLocationsUpdateLodgingArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MybusinesslodgingProvider with automatic state tracking.
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
/// let provider = MybusinesslodgingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MybusinesslodgingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MybusinesslodgingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MybusinesslodgingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Mybusinesslodging locations get lodging.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Lodging result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinesslodging_locations_get_lodging(
        &self,
        args: &MybusinesslodgingLocationsGetLodgingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Lodging, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinesslodging_locations_get_lodging_builder(
            &self.http_client,
            &args.name,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinesslodging_locations_get_lodging_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinesslodging locations update lodging.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Lodging result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinesslodging_locations_update_lodging(
        &self,
        args: &MybusinesslodgingLocationsUpdateLodgingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Lodging, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinesslodging_locations_update_lodging_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinesslodging_locations_update_lodging_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinesslodging locations lodging get google updated.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetGoogleUpdatedLodgingResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinesslodging_locations_lodging_get_google_updated(
        &self,
        args: &MybusinesslodgingLocationsLodgingGetGoogleUpdatedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetGoogleUpdatedLodgingResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinesslodging_locations_lodging_get_google_updated_builder(
            &self.http_client,
            &args.name,
            &args.readMask,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinesslodging_locations_lodging_get_google_updated_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
