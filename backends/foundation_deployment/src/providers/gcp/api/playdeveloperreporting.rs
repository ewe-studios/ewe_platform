//! PlaydeveloperreportingProvider - State-aware playdeveloperreporting API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       playdeveloperreporting API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::playdeveloperreporting::{
    playdeveloperreporting_vitals_anrrate_query_builder, playdeveloperreporting_vitals_anrrate_query_task,
    playdeveloperreporting_vitals_crashrate_query_builder, playdeveloperreporting_vitals_crashrate_query_task,
    playdeveloperreporting_vitals_errors_counts_query_builder, playdeveloperreporting_vitals_errors_counts_query_task,
    playdeveloperreporting_vitals_excessivewakeuprate_query_builder, playdeveloperreporting_vitals_excessivewakeuprate_query_task,
    playdeveloperreporting_vitals_lmkrate_query_builder, playdeveloperreporting_vitals_lmkrate_query_task,
    playdeveloperreporting_vitals_slowrenderingrate_query_builder, playdeveloperreporting_vitals_slowrenderingrate_query_task,
    playdeveloperreporting_vitals_slowstartrate_query_builder, playdeveloperreporting_vitals_slowstartrate_query_task,
    playdeveloperreporting_vitals_stuckbackgroundwakelockrate_query_builder, playdeveloperreporting_vitals_stuckbackgroundwakelockrate_query_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryAnrRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryCrashRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryErrorCountMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryExcessiveWakeupRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryLmkRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QuerySlowRenderingRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QuerySlowStartRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryStuckBackgroundWakelockRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsAnrrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsCrashrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsErrorsCountsQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsExcessivewakeuprateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsLmkrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsSlowrenderingrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsSlowstartrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsStuckbackgroundwakelockrateQueryArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PlaydeveloperreportingProvider with automatic state tracking.
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
/// let provider = PlaydeveloperreportingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PlaydeveloperreportingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PlaydeveloperreportingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PlaydeveloperreportingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Playdeveloperreporting vitals anrrate query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QueryAnrRateMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_anrrate_query(
        &self,
        args: &PlaydeveloperreportingVitalsAnrrateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QueryAnrRateMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_anrrate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_anrrate_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals crashrate query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QueryCrashRateMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_crashrate_query(
        &self,
        args: &PlaydeveloperreportingVitalsCrashrateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QueryCrashRateMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_crashrate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_crashrate_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals errors counts query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QueryErrorCountMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_errors_counts_query(
        &self,
        args: &PlaydeveloperreportingVitalsErrorsCountsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QueryErrorCountMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_errors_counts_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_errors_counts_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals excessivewakeuprate query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QueryExcessiveWakeupRateMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_excessivewakeuprate_query(
        &self,
        args: &PlaydeveloperreportingVitalsExcessivewakeuprateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QueryExcessiveWakeupRateMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_excessivewakeuprate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_excessivewakeuprate_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals lmkrate query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QueryLmkRateMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_lmkrate_query(
        &self,
        args: &PlaydeveloperreportingVitalsLmkrateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QueryLmkRateMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_lmkrate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_lmkrate_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals slowrenderingrate query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QuerySlowRenderingRateMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_slowrenderingrate_query(
        &self,
        args: &PlaydeveloperreportingVitalsSlowrenderingrateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QuerySlowRenderingRateMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_slowrenderingrate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_slowrenderingrate_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals slowstartrate query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QuerySlowStartRateMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_slowstartrate_query(
        &self,
        args: &PlaydeveloperreportingVitalsSlowstartrateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QuerySlowStartRateMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_slowstartrate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_slowstartrate_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals stuckbackgroundwakelockrate query.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1QueryStuckBackgroundWakelockRateMetricSetResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn playdeveloperreporting_vitals_stuckbackgroundwakelockrate_query(
        &self,
        args: &PlaydeveloperreportingVitalsStuckbackgroundwakelockrateQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1QueryStuckBackgroundWakelockRateMetricSetResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_stuckbackgroundwakelockrate_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_stuckbackgroundwakelockrate_query_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
