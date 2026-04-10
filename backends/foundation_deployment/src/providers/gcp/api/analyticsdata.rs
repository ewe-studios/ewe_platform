//! AnalyticsdataProvider - State-aware analyticsdata API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       analyticsdata API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::analyticsdata::{
    analyticsdata_properties_batch_run_pivot_reports_builder, analyticsdata_properties_batch_run_pivot_reports_task,
    analyticsdata_properties_batch_run_reports_builder, analyticsdata_properties_batch_run_reports_task,
    analyticsdata_properties_check_compatibility_builder, analyticsdata_properties_check_compatibility_task,
    analyticsdata_properties_get_metadata_builder, analyticsdata_properties_get_metadata_task,
    analyticsdata_properties_run_pivot_report_builder, analyticsdata_properties_run_pivot_report_task,
    analyticsdata_properties_run_realtime_report_builder, analyticsdata_properties_run_realtime_report_task,
    analyticsdata_properties_run_report_builder, analyticsdata_properties_run_report_task,
    analyticsdata_properties_audience_exports_create_builder, analyticsdata_properties_audience_exports_create_task,
    analyticsdata_properties_audience_exports_get_builder, analyticsdata_properties_audience_exports_get_task,
    analyticsdata_properties_audience_exports_list_builder, analyticsdata_properties_audience_exports_list_task,
    analyticsdata_properties_audience_exports_query_builder, analyticsdata_properties_audience_exports_query_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::analyticsdata::AudienceExport;
use crate::providers::gcp::clients::analyticsdata::BatchRunPivotReportsResponse;
use crate::providers::gcp::clients::analyticsdata::BatchRunReportsResponse;
use crate::providers::gcp::clients::analyticsdata::CheckCompatibilityResponse;
use crate::providers::gcp::clients::analyticsdata::ListAudienceExportsResponse;
use crate::providers::gcp::clients::analyticsdata::Metadata;
use crate::providers::gcp::clients::analyticsdata::Operation;
use crate::providers::gcp::clients::analyticsdata::QueryAudienceExportResponse;
use crate::providers::gcp::clients::analyticsdata::RunPivotReportResponse;
use crate::providers::gcp::clients::analyticsdata::RunRealtimeReportResponse;
use crate::providers::gcp::clients::analyticsdata::RunReportResponse;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesAudienceExportsCreateArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesAudienceExportsGetArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesAudienceExportsListArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesAudienceExportsQueryArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesBatchRunPivotReportsArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesBatchRunReportsArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesCheckCompatibilityArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesGetMetadataArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesRunPivotReportArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesRunRealtimeReportArgs;
use crate::providers::gcp::clients::analyticsdata::AnalyticsdataPropertiesRunReportArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AnalyticsdataProvider with automatic state tracking.
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
/// let provider = AnalyticsdataProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AnalyticsdataProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AnalyticsdataProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AnalyticsdataProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Analyticsdata properties batch run pivot reports.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchRunPivotReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsdata_properties_batch_run_pivot_reports(
        &self,
        args: &AnalyticsdataPropertiesBatchRunPivotReportsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchRunPivotReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_batch_run_pivot_reports_builder(
            &self.http_client,
            &args.property,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_batch_run_pivot_reports_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties batch run reports.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BatchRunReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsdata_properties_batch_run_reports(
        &self,
        args: &AnalyticsdataPropertiesBatchRunReportsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BatchRunReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_batch_run_reports_builder(
            &self.http_client,
            &args.property,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_batch_run_reports_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties check compatibility.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckCompatibilityResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsdata_properties_check_compatibility(
        &self,
        args: &AnalyticsdataPropertiesCheckCompatibilityArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckCompatibilityResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_check_compatibility_builder(
            &self.http_client,
            &args.property,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_check_compatibility_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties get metadata.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Metadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsdata_properties_get_metadata(
        &self,
        args: &AnalyticsdataPropertiesGetMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Metadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_get_metadata_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_get_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties run pivot report.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunPivotReportResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsdata_properties_run_pivot_report(
        &self,
        args: &AnalyticsdataPropertiesRunPivotReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunPivotReportResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_run_pivot_report_builder(
            &self.http_client,
            &args.property,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_run_pivot_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties run realtime report.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunRealtimeReportResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsdata_properties_run_realtime_report(
        &self,
        args: &AnalyticsdataPropertiesRunRealtimeReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunRealtimeReportResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_run_realtime_report_builder(
            &self.http_client,
            &args.property,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_run_realtime_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties run report.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the RunReportResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsdata_properties_run_report(
        &self,
        args: &AnalyticsdataPropertiesRunReportArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<RunReportResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_run_report_builder(
            &self.http_client,
            &args.property,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_run_report_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties audience exports create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn analyticsdata_properties_audience_exports_create(
        &self,
        args: &AnalyticsdataPropertiesAudienceExportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_audience_exports_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_audience_exports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties audience exports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AudienceExport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsdata_properties_audience_exports_get(
        &self,
        args: &AnalyticsdataPropertiesAudienceExportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AudienceExport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_audience_exports_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_audience_exports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties audience exports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAudienceExportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsdata_properties_audience_exports_list(
        &self,
        args: &AnalyticsdataPropertiesAudienceExportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAudienceExportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_audience_exports_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_audience_exports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Analyticsdata properties audience exports query.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryAudienceExportResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn analyticsdata_properties_audience_exports_query(
        &self,
        args: &AnalyticsdataPropertiesAudienceExportsQueryArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryAudienceExportResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = analyticsdata_properties_audience_exports_query_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = analyticsdata_properties_audience_exports_query_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
