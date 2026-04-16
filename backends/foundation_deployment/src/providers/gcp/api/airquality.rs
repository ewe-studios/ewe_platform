//! AirqualityProvider - State-aware airquality API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       airquality API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::airquality::{
    airquality_current_conditions_lookup_builder, airquality_current_conditions_lookup_task,
    airquality_forecast_lookup_builder, airquality_forecast_lookup_task,
    airquality_history_lookup_builder, airquality_history_lookup_task,
    airquality_map_types_heatmap_tiles_lookup_heatmap_tile_builder, airquality_map_types_heatmap_tiles_lookup_heatmap_tile_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::airquality::HttpBody;
use crate::providers::gcp::clients::airquality::LookupCurrentConditionsResponse;
use crate::providers::gcp::clients::airquality::LookupForecastResponse;
use crate::providers::gcp::clients::airquality::LookupHistoryResponse;
use crate::providers::gcp::clients::airquality::AirqualityMapTypesHeatmapTilesLookupHeatmapTileArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AirqualityProvider with automatic state tracking.
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
/// let provider = AirqualityProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AirqualityProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AirqualityProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AirqualityProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AirqualityProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Airquality current conditions lookup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupCurrentConditionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn airquality_current_conditions_lookup(
        &self,
        args: &AirqualityCurrentConditionsLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupCurrentConditionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = airquality_current_conditions_lookup_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = airquality_current_conditions_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Airquality forecast lookup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupForecastResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn airquality_forecast_lookup(
        &self,
        args: &AirqualityForecastLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupForecastResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = airquality_forecast_lookup_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = airquality_forecast_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Airquality history lookup.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the LookupHistoryResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn airquality_history_lookup(
        &self,
        args: &AirqualityHistoryLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupHistoryResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = airquality_history_lookup_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = airquality_history_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Airquality map types heatmap tiles lookup heatmap tile.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the HttpBody result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn airquality_map_types_heatmap_tiles_lookup_heatmap_tile(
        &self,
        args: &AirqualityMapTypesHeatmapTilesLookupHeatmapTileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = airquality_map_types_heatmap_tiles_lookup_heatmap_tile_builder(
            &self.http_client,
            &args.mapType,
            &args.zoom,
            &args.x,
            &args.y,
        )
        .map_err(ProviderError::Api)?;

        let task = airquality_map_types_heatmap_tiles_lookup_heatmap_tile_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
