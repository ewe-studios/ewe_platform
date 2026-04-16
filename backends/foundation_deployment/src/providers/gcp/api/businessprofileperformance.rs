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
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BusinessprofileperformanceProvider with automatic state tracking.
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
/// let provider = BusinessprofileperformanceProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct BusinessprofileperformanceProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> BusinessprofileperformanceProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new BusinessprofileperformanceProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new BusinessprofileperformanceProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
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
            &args.dailyRange_endDate_day,
            &args.dailyRange_endDate_month,
            &args.dailyRange_endDate_year,
            &args.dailyRange_startDate_day,
            &args.dailyRange_startDate_month,
            &args.dailyRange_startDate_year,
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
            &args.dailyRange_endDate_day,
            &args.dailyRange_endDate_month,
            &args.dailyRange_endDate_year,
            &args.dailyRange_startDate_day,
            &args.dailyRange_startDate_month,
            &args.dailyRange_startDate_year,
            &args.dailySubEntityType_dayOfWeek,
            &args.dailySubEntityType_timeOfDay_hours,
            &args.dailySubEntityType_timeOfDay_minutes,
            &args.dailySubEntityType_timeOfDay_nanos,
            &args.dailySubEntityType_timeOfDay_seconds,
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
            &args.monthlyRange_endMonth_day,
            &args.monthlyRange_endMonth_month,
            &args.monthlyRange_endMonth_year,
            &args.monthlyRange_startMonth_day,
            &args.monthlyRange_startMonth_month,
            &args.monthlyRange_startMonth_year,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = businessprofileperformance_locations_searchkeywords_impressions_monthly_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
