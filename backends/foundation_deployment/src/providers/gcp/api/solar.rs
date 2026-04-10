//! SolarProvider - State-aware solar API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       solar API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::solar::{
    solar_building_insights_find_closest_builder, solar_building_insights_find_closest_task,
    solar_data_layers_get_builder, solar_data_layers_get_task,
    solar_geo_tiff_get_builder, solar_geo_tiff_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::solar::BuildingInsights;
use crate::providers::gcp::clients::solar::DataLayers;
use crate::providers::gcp::clients::solar::HttpBody;
use crate::providers::gcp::clients::solar::SolarBuildingInsightsFindClosestArgs;
use crate::providers::gcp::clients::solar::SolarDataLayersGetArgs;
use crate::providers::gcp::clients::solar::SolarGeoTiffGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SolarProvider with automatic state tracking.
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
/// let provider = SolarProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SolarProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SolarProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SolarProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Solar building insights find closest.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BuildingInsights result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn solar_building_insights_find_closest(
        &self,
        args: &SolarBuildingInsightsFindClosestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BuildingInsights, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = solar_building_insights_find_closest_builder(
            &self.http_client,
            &args.exactQualityRequired,
            &args.experiments,
            &args.location.latitude,
            &args.location.longitude,
            &args.requiredQuality,
        )
        .map_err(ProviderError::Api)?;

        let task = solar_building_insights_find_closest_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Solar data layers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DataLayers result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn solar_data_layers_get(
        &self,
        args: &SolarDataLayersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DataLayers, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = solar_data_layers_get_builder(
            &self.http_client,
            &args.exactQualityRequired,
            &args.experiments,
            &args.location.latitude,
            &args.location.longitude,
            &args.pixelSizeMeters,
            &args.radiusMeters,
            &args.requiredQuality,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = solar_data_layers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Solar geo tiff get.
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
    pub fn solar_geo_tiff_get(
        &self,
        args: &SolarGeoTiffGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<HttpBody, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = solar_geo_tiff_get_builder(
            &self.http_client,
            &args.id,
        )
        .map_err(ProviderError::Api)?;

        let task = solar_geo_tiff_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
