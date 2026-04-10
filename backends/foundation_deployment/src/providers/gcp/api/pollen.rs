//! PollenProvider - State-aware pollen API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       pollen API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::pollen::{
    pollen_forecast_lookup_builder, pollen_forecast_lookup_task,
    pollen_map_types_heatmap_tiles_lookup_heatmap_tile_builder, pollen_map_types_heatmap_tiles_lookup_heatmap_tile_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::pollen::HttpBody;
use crate::providers::gcp::clients::pollen::LookupForecastResponse;
use crate::providers::gcp::clients::pollen::PollenForecastLookupArgs;
use crate::providers::gcp::clients::pollen::PollenMapTypesHeatmapTilesLookupHeatmapTileArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PollenProvider with automatic state tracking.
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
/// let provider = PollenProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PollenProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PollenProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PollenProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Pollen forecast lookup.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn pollen_forecast_lookup(
        &self,
        args: &PollenForecastLookupArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<LookupForecastResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pollen_forecast_lookup_builder(
            &self.http_client,
            &args.days,
            &args.languageCode,
            &args.location.latitude,
            &args.location.longitude,
            &args.pageSize,
            &args.pageToken,
            &args.plantsDescription,
        )
        .map_err(ProviderError::Api)?;

        let task = pollen_forecast_lookup_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Pollen map types heatmap tiles lookup heatmap tile.
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
    pub fn pollen_map_types_heatmap_tiles_lookup_heatmap_tile(
        &self,
        args: &PollenMapTypesHeatmapTilesLookupHeatmapTileArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = pollen_map_types_heatmap_tiles_lookup_heatmap_tile_builder(
            &self.http_client,
            &args.mapType,
            &args.zoom,
            &args.x,
            &args.y,
        )
        .map_err(ProviderError::Api)?;

        let task = pollen_map_types_heatmap_tiles_lookup_heatmap_tile_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
