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
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PollenProvider with automatic state tracking.
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
/// let provider = PollenProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PollenProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PollenProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PollenProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PollenProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
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
            &args.location_latitude,
            &args.location_longitude,
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
