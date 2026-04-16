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
    playdeveloperreporting_anomalies_list_builder, playdeveloperreporting_anomalies_list_task,
    playdeveloperreporting_apps_fetch_release_filter_options_builder, playdeveloperreporting_apps_fetch_release_filter_options_task,
    playdeveloperreporting_apps_search_builder, playdeveloperreporting_apps_search_task,
    playdeveloperreporting_vitals_anrrate_get_builder, playdeveloperreporting_vitals_anrrate_get_task,
    playdeveloperreporting_vitals_anrrate_query_builder, playdeveloperreporting_vitals_anrrate_query_task,
    playdeveloperreporting_vitals_crashrate_get_builder, playdeveloperreporting_vitals_crashrate_get_task,
    playdeveloperreporting_vitals_crashrate_query_builder, playdeveloperreporting_vitals_crashrate_query_task,
    playdeveloperreporting_vitals_errors_counts_get_builder, playdeveloperreporting_vitals_errors_counts_get_task,
    playdeveloperreporting_vitals_errors_counts_query_builder, playdeveloperreporting_vitals_errors_counts_query_task,
    playdeveloperreporting_vitals_errors_issues_search_builder, playdeveloperreporting_vitals_errors_issues_search_task,
    playdeveloperreporting_vitals_errors_reports_search_builder, playdeveloperreporting_vitals_errors_reports_search_task,
    playdeveloperreporting_vitals_excessivewakeuprate_get_builder, playdeveloperreporting_vitals_excessivewakeuprate_get_task,
    playdeveloperreporting_vitals_excessivewakeuprate_query_builder, playdeveloperreporting_vitals_excessivewakeuprate_query_task,
    playdeveloperreporting_vitals_lmkrate_get_builder, playdeveloperreporting_vitals_lmkrate_get_task,
    playdeveloperreporting_vitals_lmkrate_query_builder, playdeveloperreporting_vitals_lmkrate_query_task,
    playdeveloperreporting_vitals_slowrenderingrate_get_builder, playdeveloperreporting_vitals_slowrenderingrate_get_task,
    playdeveloperreporting_vitals_slowrenderingrate_query_builder, playdeveloperreporting_vitals_slowrenderingrate_query_task,
    playdeveloperreporting_vitals_slowstartrate_get_builder, playdeveloperreporting_vitals_slowstartrate_get_task,
    playdeveloperreporting_vitals_slowstartrate_query_builder, playdeveloperreporting_vitals_slowstartrate_query_task,
    playdeveloperreporting_vitals_stuckbackgroundwakelockrate_get_builder, playdeveloperreporting_vitals_stuckbackgroundwakelockrate_get_task,
    playdeveloperreporting_vitals_stuckbackgroundwakelockrate_query_builder, playdeveloperreporting_vitals_stuckbackgroundwakelockrate_query_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1AnrRateMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1CrashRateMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1ErrorCountMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1ExcessiveWakeupRateMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1ListAnomaliesResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1LmkRateMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryAnrRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryCrashRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryErrorCountMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryExcessiveWakeupRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryLmkRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QuerySlowRenderingRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QuerySlowStartRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1QueryStuckBackgroundWakelockRateMetricSetResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1ReleaseFilterOptions;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1SearchAccessibleAppsResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1SearchErrorIssuesResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1SearchErrorReportsResponse;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1SlowRenderingRateMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1SlowStartRateMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::GooglePlayDeveloperReportingV1beta1StuckBackgroundWakelockRateMetricSet;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingAnomaliesListArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingAppsFetchReleaseFilterOptionsArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingAppsSearchArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsAnrrateGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsAnrrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsCrashrateGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsCrashrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsErrorsCountsGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsErrorsCountsQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsErrorsIssuesSearchArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsErrorsReportsSearchArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsExcessivewakeuprateGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsExcessivewakeuprateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsLmkrateGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsLmkrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsSlowrenderingrateGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsSlowrenderingrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsSlowstartrateGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsSlowstartrateQueryArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsStuckbackgroundwakelockrateGetArgs;
use crate::providers::gcp::clients::playdeveloperreporting::PlaydeveloperreportingVitalsStuckbackgroundwakelockrateQueryArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PlaydeveloperreportingProvider with automatic state tracking.
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
/// let provider = PlaydeveloperreportingProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct PlaydeveloperreportingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> PlaydeveloperreportingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new PlaydeveloperreportingProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new PlaydeveloperreportingProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Playdeveloperreporting anomalies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1ListAnomaliesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_anomalies_list(
        &self,
        args: &PlaydeveloperreportingAnomaliesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1ListAnomaliesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_anomalies_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_anomalies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting apps fetch release filter options.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1ReleaseFilterOptions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_apps_fetch_release_filter_options(
        &self,
        args: &PlaydeveloperreportingAppsFetchReleaseFilterOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1ReleaseFilterOptions, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_apps_fetch_release_filter_options_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_apps_fetch_release_filter_options_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting apps search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1SearchAccessibleAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_apps_search(
        &self,
        args: &PlaydeveloperreportingAppsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1SearchAccessibleAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_apps_search_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_apps_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals anrrate get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1AnrRateMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_anrrate_get(
        &self,
        args: &PlaydeveloperreportingVitalsAnrrateGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1AnrRateMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_anrrate_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_anrrate_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals anrrate query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals crashrate get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1CrashRateMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_crashrate_get(
        &self,
        args: &PlaydeveloperreportingVitalsCrashrateGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1CrashRateMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_crashrate_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_crashrate_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals crashrate query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals errors counts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1ErrorCountMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_errors_counts_get(
        &self,
        args: &PlaydeveloperreportingVitalsErrorsCountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1ErrorCountMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_errors_counts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_errors_counts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals errors counts query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals errors issues search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1SearchErrorIssuesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_errors_issues_search(
        &self,
        args: &PlaydeveloperreportingVitalsErrorsIssuesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1SearchErrorIssuesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_errors_issues_search_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.interval_endTime_day,
            &args.interval_endTime_hours,
            &args.interval_endTime_minutes,
            &args.interval_endTime_month,
            &args.interval_endTime_nanos,
            &args.interval_endTime_seconds,
            &args.interval_endTime_timeZone_id,
            &args.interval_endTime_timeZone_version,
            &args.interval_endTime_utcOffset,
            &args.interval_endTime_year,
            &args.interval_startTime_day,
            &args.interval_startTime_hours,
            &args.interval_startTime_minutes,
            &args.interval_startTime_month,
            &args.interval_startTime_nanos,
            &args.interval_startTime_seconds,
            &args.interval_startTime_timeZone_id,
            &args.interval_startTime_timeZone_version,
            &args.interval_startTime_utcOffset,
            &args.interval_startTime_year,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
            &args.sampleErrorReportLimit,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_errors_issues_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals errors reports search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1SearchErrorReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_errors_reports_search(
        &self,
        args: &PlaydeveloperreportingVitalsErrorsReportsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1SearchErrorReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_errors_reports_search_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.interval_endTime_day,
            &args.interval_endTime_hours,
            &args.interval_endTime_minutes,
            &args.interval_endTime_month,
            &args.interval_endTime_nanos,
            &args.interval_endTime_seconds,
            &args.interval_endTime_timeZone_id,
            &args.interval_endTime_timeZone_version,
            &args.interval_endTime_utcOffset,
            &args.interval_endTime_year,
            &args.interval_startTime_day,
            &args.interval_startTime_hours,
            &args.interval_startTime_minutes,
            &args.interval_startTime_month,
            &args.interval_startTime_nanos,
            &args.interval_startTime_seconds,
            &args.interval_startTime_timeZone_id,
            &args.interval_startTime_timeZone_version,
            &args.interval_startTime_utcOffset,
            &args.interval_startTime_year,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_errors_reports_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals excessivewakeuprate get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1ExcessiveWakeupRateMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_excessivewakeuprate_get(
        &self,
        args: &PlaydeveloperreportingVitalsExcessivewakeuprateGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1ExcessiveWakeupRateMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_excessivewakeuprate_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_excessivewakeuprate_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals excessivewakeuprate query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals lmkrate get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1LmkRateMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_lmkrate_get(
        &self,
        args: &PlaydeveloperreportingVitalsLmkrateGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1LmkRateMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_lmkrate_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_lmkrate_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals lmkrate query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals slowrenderingrate get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1SlowRenderingRateMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_slowrenderingrate_get(
        &self,
        args: &PlaydeveloperreportingVitalsSlowrenderingrateGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1SlowRenderingRateMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_slowrenderingrate_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_slowrenderingrate_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals slowrenderingrate query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals slowstartrate get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1SlowStartRateMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_slowstartrate_get(
        &self,
        args: &PlaydeveloperreportingVitalsSlowstartrateGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1SlowStartRateMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_slowstartrate_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_slowstartrate_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals slowstartrate query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals stuckbackgroundwakelockrate get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GooglePlayDeveloperReportingV1beta1StuckBackgroundWakelockRateMetricSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn playdeveloperreporting_vitals_stuckbackgroundwakelockrate_get(
        &self,
        args: &PlaydeveloperreportingVitalsStuckbackgroundwakelockrateGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GooglePlayDeveloperReportingV1beta1StuckBackgroundWakelockRateMetricSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = playdeveloperreporting_vitals_stuckbackgroundwakelockrate_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = playdeveloperreporting_vitals_stuckbackgroundwakelockrate_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Playdeveloperreporting vitals stuckbackgroundwakelockrate query.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
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

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
