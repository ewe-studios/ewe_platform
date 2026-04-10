//! PlacesProvider - State-aware places API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       places API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::places::{
    places_places_autocomplete_builder, places_places_autocomplete_task,
    places_places_search_nearby_builder, places_places_search_nearby_task,
    places_places_search_text_builder, places_places_search_text_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::places::GoogleMapsPlacesV1AutocompletePlacesResponse;
use crate::providers::gcp::clients::places::GoogleMapsPlacesV1SearchNearbyResponse;
use crate::providers::gcp::clients::places::GoogleMapsPlacesV1SearchTextResponse;
use crate::providers::gcp::clients::places::PlacesPlacesAutocompleteArgs;
use crate::providers::gcp::clients::places::PlacesPlacesSearchNearbyArgs;
use crate::providers::gcp::clients::places::PlacesPlacesSearchTextArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PlacesProvider with automatic state tracking.
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
/// let provider = PlacesProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PlacesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PlacesProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PlacesProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Places places autocomplete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleMapsPlacesV1AutocompletePlacesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn places_places_autocomplete(
        &self,
        args: &PlacesPlacesAutocompleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleMapsPlacesV1AutocompletePlacesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = places_places_autocomplete_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = places_places_autocomplete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Places places search nearby.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleMapsPlacesV1SearchNearbyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn places_places_search_nearby(
        &self,
        args: &PlacesPlacesSearchNearbyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleMapsPlacesV1SearchNearbyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = places_places_search_nearby_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = places_places_search_nearby_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Places places search text.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleMapsPlacesV1SearchTextResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn places_places_search_text(
        &self,
        args: &PlacesPlacesSearchTextArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleMapsPlacesV1SearchTextResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = places_places_search_text_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = places_places_search_text_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
