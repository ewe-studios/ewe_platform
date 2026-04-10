//! YoutubereportingProvider - State-aware youtubereporting API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       youtubereporting API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::youtubereporting::{
    youtubereporting_jobs_create_builder, youtubereporting_jobs_create_task,
    youtubereporting_jobs_delete_builder, youtubereporting_jobs_delete_task,
    youtubereporting_jobs_get_builder, youtubereporting_jobs_get_task,
    youtubereporting_jobs_list_builder, youtubereporting_jobs_list_task,
    youtubereporting_jobs_reports_get_builder, youtubereporting_jobs_reports_get_task,
    youtubereporting_jobs_reports_list_builder, youtubereporting_jobs_reports_list_task,
    youtubereporting_media_download_builder, youtubereporting_media_download_task,
    youtubereporting_report_types_list_builder, youtubereporting_report_types_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::youtubereporting::Empty;
use crate::providers::gcp::clients::youtubereporting::GdataMedia;
use crate::providers::gcp::clients::youtubereporting::Job;
use crate::providers::gcp::clients::youtubereporting::ListJobsResponse;
use crate::providers::gcp::clients::youtubereporting::ListReportTypesResponse;
use crate::providers::gcp::clients::youtubereporting::ListReportsResponse;
use crate::providers::gcp::clients::youtubereporting::Report;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingJobsCreateArgs;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingJobsDeleteArgs;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingJobsGetArgs;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingJobsListArgs;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingJobsReportsGetArgs;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingJobsReportsListArgs;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingMediaDownloadArgs;
use crate::providers::gcp::clients::youtubereporting::YoutubereportingReportTypesListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// YoutubereportingProvider with automatic state tracking.
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
/// let provider = YoutubereportingProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct YoutubereportingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> YoutubereportingProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new YoutubereportingProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Youtubereporting jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtubereporting_jobs_create(
        &self,
        args: &YoutubereportingJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_jobs_create_builder(
            &self.http_client,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtubereporting jobs delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Empty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn youtubereporting_jobs_delete(
        &self,
        args: &YoutubereportingJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_jobs_delete_builder(
            &self.http_client,
            &args.jobId,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtubereporting jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Job result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtubereporting_jobs_get(
        &self,
        args: &YoutubereportingJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Job, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_jobs_get_builder(
            &self.http_client,
            &args.jobId,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtubereporting jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtubereporting_jobs_list(
        &self,
        args: &YoutubereportingJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_jobs_list_builder(
            &self.http_client,
            &args.includeSystemManaged,
            &args.onBehalfOfContentOwner,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtubereporting jobs reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Report result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtubereporting_jobs_reports_get(
        &self,
        args: &YoutubereportingJobsReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Report, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_jobs_reports_get_builder(
            &self.http_client,
            &args.jobId,
            &args.reportId,
            &args.onBehalfOfContentOwner,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_jobs_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtubereporting jobs reports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtubereporting_jobs_reports_list(
        &self,
        args: &YoutubereportingJobsReportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_jobs_reports_list_builder(
            &self.http_client,
            &args.jobId,
            &args.createdAfter,
            &args.onBehalfOfContentOwner,
            &args.pageSize,
            &args.pageToken,
            &args.startTimeAtOrAfter,
            &args.startTimeBefore,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_jobs_reports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtubereporting media download.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GdataMedia result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtubereporting_media_download(
        &self,
        args: &YoutubereportingMediaDownloadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GdataMedia, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_media_download_builder(
            &self.http_client,
            &args.resourceName,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_media_download_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Youtubereporting report types list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListReportTypesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn youtubereporting_report_types_list(
        &self,
        args: &YoutubereportingReportTypesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListReportTypesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = youtubereporting_report_types_list_builder(
            &self.http_client,
            &args.includeSystemManaged,
            &args.onBehalfOfContentOwner,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = youtubereporting_report_types_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
