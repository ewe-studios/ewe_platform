//! BusinessprofileperformanceProvider - State-aware businessprofileperformance API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       businessprofileperformance API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::businessprofileperformance::{
    businessprofileperformance_locations_fetch_multi_daily_metrics_time_series_builder, businessprofileperformance_locations_fetch_multi_daily_metrics_time_series_task,
    businessprofileperformance_locations_get_daily_metrics_time_series_builder, businessprofileperformance_locations_get_daily_metrics_time_series_task,
    businessprofileperformance_locations_searchkeywords_impressions_monthly_list_builder, businessprofileperformance_locations_searchkeywords_impressions_monthly_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::businessprofileperformance::FetchMultiDailyMetricsTimeSeriesResponse;
use crate::providers::gcp::clients::businessprofileperformance::GetDailyMetricsTimeSeriesResponse;
use crate::providers::gcp::clients::businessprofileperformance::ListSearchKeywordImpressionsMonthlyResponse;
use crate::providers::gcp::clients::businessprofileperformance::BusinessprofileperformanceLocationsFetchMultiDailyMetricsTimeSeriesArgs;
use crate::providers::gcp::clients::businessprofileperformance::BusinessprofileperformanceLocationsGetDailyMetricsTimeSeriesArgs;
use crate::providers::gcp::clients::businessprofileperformance::BusinessprofileperformanceLocationsSearchkeywordsImpressionsMonthlyListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BusinessprofileperformanceProvider with automatic state tracking.
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
/// let provider = BusinessprofileperformanceProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BusinessprofileperformanceProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BusinessprofileperformanceProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BusinessprofileperformanceProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Businessprofileperformance locations fetch multi daily metrics time series.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchMultiDailyMetricsTimeSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn businessprofileperformance_locations_fetch_multi_daily_metrics_time_series(
        &self,
        args: &BusinessprofileperformanceLocationsFetchMultiDailyMetricsTimeSeriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchMultiDailyMetricsTimeSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = businessprofileperformance_locations_fetch_multi_daily_metrics_time_series_builder(
            &self.http_client,
            &args.location,
            &args.dailyMetrics,
            &args.dailyRange.endDate.day,
            &args.dailyRange.endDate.month,
            &args.dailyRange.endDate.year,
            &args.dailyRange.startDate.day,
            &args.dailyRange.startDate.month,
            &args.dailyRange.startDate.year,
        )
        .map_err(ProviderError::Api)?;

        let task = businessprofileperformance_locations_fetch_multi_daily_metrics_time_series_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Businessprofileperformance locations get daily metrics time series.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetDailyMetricsTimeSeriesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn businessprofileperformance_locations_get_daily_metrics_time_series(
        &self,
        args: &BusinessprofileperformanceLocationsGetDailyMetricsTimeSeriesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetDailyMetricsTimeSeriesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = businessprofileperformance_locations_get_daily_metrics_time_series_builder(
            &self.http_client,
            &args.name,
            &args.dailyMetric,
            &args.dailyRange.endDate.day,
            &args.dailyRange.endDate.month,
            &args.dailyRange.endDate.year,
            &args.dailyRange.startDate.day,
            &args.dailyRange.startDate.month,
            &args.dailyRange.startDate.year,
            &args.dailySubEntityType.dayOfWeek,
            &args.dailySubEntityType.timeOfDay.hours,
            &args.dailySubEntityType.timeOfDay.minutes,
            &args.dailySubEntityType.timeOfDay.nanos,
            &args.dailySubEntityType.timeOfDay.seconds,
        )
        .map_err(ProviderError::Api)?;

        let task = businessprofileperformance_locations_get_daily_metrics_time_series_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Businessprofileperformance locations searchkeywords impressions monthly list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSearchKeywordImpressionsMonthlyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn businessprofileperformance_locations_searchkeywords_impressions_monthly_list(
        &self,
        args: &BusinessprofileperformanceLocationsSearchkeywordsImpressionsMonthlyListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSearchKeywordImpressionsMonthlyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = businessprofileperformance_locations_searchkeywords_impressions_monthly_list_builder(
            &self.http_client,
            &args.parent,
            &args.monthlyRange.endMonth.day,
            &args.monthlyRange.endMonth.month,
            &args.monthlyRange.endMonth.year,
            &args.monthlyRange.startMonth.day,
            &args.monthlyRange.startMonth.month,
            &args.monthlyRange.startMonth.year,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = businessprofileperformance_locations_searchkeywords_impressions_monthly_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
