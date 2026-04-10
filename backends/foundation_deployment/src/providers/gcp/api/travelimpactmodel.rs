//! TravelimpactmodelProvider - State-aware travelimpactmodel API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       travelimpactmodel API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::travelimpactmodel::{
    travelimpactmodel_flights_compute_flight_emissions_builder, travelimpactmodel_flights_compute_flight_emissions_task,
    travelimpactmodel_flights_compute_scope3_flight_emissions_builder, travelimpactmodel_flights_compute_scope3_flight_emissions_task,
    travelimpactmodel_flights_compute_typical_flight_emissions_builder, travelimpactmodel_flights_compute_typical_flight_emissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::travelimpactmodel::ComputeFlightEmissionsResponse;
use crate::providers::gcp::clients::travelimpactmodel::ComputeScope3FlightEmissionsResponse;
use crate::providers::gcp::clients::travelimpactmodel::ComputeTypicalFlightEmissionsResponse;
use crate::providers::gcp::clients::travelimpactmodel::TravelimpactmodelFlightsComputeFlightEmissionsArgs;
use crate::providers::gcp::clients::travelimpactmodel::TravelimpactmodelFlightsComputeScope3FlightEmissionsArgs;
use crate::providers::gcp::clients::travelimpactmodel::TravelimpactmodelFlightsComputeTypicalFlightEmissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// TravelimpactmodelProvider with automatic state tracking.
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
/// let provider = TravelimpactmodelProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct TravelimpactmodelProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> TravelimpactmodelProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new TravelimpactmodelProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Travelimpactmodel flights compute flight emissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeFlightEmissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn travelimpactmodel_flights_compute_flight_emissions(
        &self,
        args: &TravelimpactmodelFlightsComputeFlightEmissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeFlightEmissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = travelimpactmodel_flights_compute_flight_emissions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = travelimpactmodel_flights_compute_flight_emissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Travelimpactmodel flights compute scope3 flight emissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeScope3FlightEmissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn travelimpactmodel_flights_compute_scope3_flight_emissions(
        &self,
        args: &TravelimpactmodelFlightsComputeScope3FlightEmissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeScope3FlightEmissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = travelimpactmodel_flights_compute_scope3_flight_emissions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = travelimpactmodel_flights_compute_scope3_flight_emissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Travelimpactmodel flights compute typical flight emissions.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ComputeTypicalFlightEmissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn travelimpactmodel_flights_compute_typical_flight_emissions(
        &self,
        args: &TravelimpactmodelFlightsComputeTypicalFlightEmissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ComputeTypicalFlightEmissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = travelimpactmodel_flights_compute_typical_flight_emissions_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = travelimpactmodel_flights_compute_typical_flight_emissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
